use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use fs2::FileExt;
use portrelay::{
    agent::{Action, Agent},
    api, backend,
    storage::{self, ApiAccess},
};
use std::{fs, io::Read, path::PathBuf, process::Command};

#[derive(Parser)]
#[command(
    version,
    about = "Share USB devices through authenticated, encrypted peer connections"
)]
struct Cli {
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Subcommand)]
enum Commands {
    /// Start the application and open its local control window.
    Run {
        #[arg(long)]
        name: Option<String>,
        #[arg(long, default_value = "0.0.0.0:24816")]
        bind: String,
        #[arg(long, default_value_t = 0)]
        port: u16,
        #[arg(long)]
        relay: Option<String>,
        /// Use only the configured relay (for networks that block direct UDP).
        #[arg(long, requires = "relay")]
        relay_only: bool,
        #[arg(long,default_value=backend::SOCKET)]
        helper: PathBuf,
        #[arg(long)]
        no_open: bool,
    },
    /// Open the running application's control window.
    Open,
    /// Show live device, peer, and connection state.
    Status,
    /// Exit successfully only when the agent and USB helper are ready.
    Check,
    /// Create a single-use invitation valid for ten minutes.
    Invite,
    /// Read an invitation from stdin and request pairing.
    Pair,
    /// Read a typed local API action as JSON from stdin.
    Api,
    /// Run the Linux privileged device helper (normally installed as a service).
    Helper {
        #[arg(long)]
        uid: u32,
        #[arg(long, default_value = "/run/portrelay")]
        runtime_dir: PathBuf,
    },
}
fn open_browser(url: &str) -> Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    Command::new(program)
        .arg(url)
        .spawn()
        .context("Could not open the browser; install xdg-utils or use the local CLI")?;
    Ok(())
}
fn access(dir: &std::path::Path) -> Result<ApiAccess> {
    Ok(serde_json::from_slice(
        &fs::read(dir.join("api.json"))
            .context("PortRelay is not running. Start it with portrelay run.")?,
    )?)
}
fn open_ui(dir: &std::path::Path) -> Result<()> {
    let access = access(dir)?;
    open_browser(&format!(
        "http://127.0.0.1:{}/#{}",
        access.port, access.token
    ))
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Commands::Run {
        name: None,
        bind: "0.0.0.0:24816".into(),
        port: 0,
        relay: None,
        relay_only: false,
        helper: backend::SOCKET.into(),
        no_open: false,
    });
    if let Commands::Helper { uid, runtime_dir } = &command {
        return backend::serve(*uid, runtime_dir).await;
    }
    let dir = storage::state_dir(cli.data_dir)?;
    match command {
        Commands::Run {
            name,
            bind,
            port,
            relay,
            relay_only,
            helper,
            no_open,
        } => {
            #[cfg(unix)]
            if nix::unistd::geteuid().is_root() {
                bail!("Run the application as your normal user. Only the helper runs as root.");
            }
            let lock = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(dir.join("agent.lock"))?;
            if lock.try_lock_exclusive().is_err() {
                if !no_open {
                    return open_ui(&dir);
                }
                bail!("PortRelay is already running in this data directory");
            }
            let agent = Agent::start(dir.clone(), name, helper, bind, relay, relay_only).await?;
            let listener =
                tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
            let port = listener.local_addr()?.port();
            storage::save(
                &dir.join("api.json"),
                &ApiAccess {
                    port,
                    token: agent.token.clone(),
                },
            )?;
            println!(
                "PortRelay {} is running at http://127.0.0.1:{port}. Use 'portrelay open' to open the authenticated window.",
                env!("CARGO_PKG_VERSION")
            );
            if !no_open && let Err(e) = open_ui(&dir) {
                eprintln!("{e}");
            }
            let cancel = agent.cancel.clone();
            let server = tokio::spawn(
                axum::serve(listener, api::router(agent.clone()))
                    .with_graceful_shutdown(async move {
                        cancel.cancelled().await;
                    })
                    .into_future(),
            );
            #[cfg(unix)]
            {
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
                tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
            }
            #[cfg(not(unix))]
            tokio::signal::ctrl_c().await?;
            agent.shutdown().await;
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;
            fs::remove_file(dir.join("api.json"))?;
        }
        Commands::Open => open_ui(&dir)?,
        other => {
            let checking = matches!(&other, Commands::Check);
            let access = access(&dir)?;
            let client = reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(60))
                .build()?;
            let base = format!("http://127.0.0.1:{}", access.port);
            let request = match other {
                Commands::Status | Commands::Check => client.get(format!("{base}/api/state")),
                _ => {
                    let action = match other {
                        Commands::Invite => {
                            serde_json::to_value(serde_json::json!({"op":"invite"}))?
                        }
                        Commands::Pair => {
                            let mut input = String::new();
                            std::io::stdin().take(8193).read_to_string(&mut input)?;
                            serde_json::json!({"op":"pair","invitation":input.trim()})
                        }
                        Commands::Api => {
                            let mut input = String::new();
                            std::io::stdin().take(65537).read_to_string(&mut input)?;
                            if input.len() > 65536 {
                                bail!("Action too large");
                            }
                            let value: serde_json::Value = serde_json::from_str(&input)?;
                            let _: Action = serde_json::from_value(value.clone())?;
                            value
                        }
                        _ => unreachable!(),
                    };
                    client.post(format!("{base}/api/action")).json(&action)
                }
            };
            let response = request.bearer_auth(access.token).send().await?;
            let status = response.status();
            let value: serde_json::Value = response.json().await?;
            if !status.is_success() {
                bail!(
                    "{}",
                    value
                        .get("error")
                        .and_then(|x| x.as_str())
                        .unwrap_or("Local API failed")
                );
            }
            if checking {
                if value.get("helper_ready").and_then(|v| v.as_bool()) != Some(true) {
                    bail!(
                        "{}",
                        value
                            .get("helper_error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("USB helper is not ready")
                    );
                }
                println!("PortRelay agent and USB helper ready");
            } else {
                println!("{}", serde_json::to_string_pretty(&value)?);
            }
        }
    }
    Ok(())
}
