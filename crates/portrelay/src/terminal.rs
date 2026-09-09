//! Human terminal controls. Every device operation goes through the authenticated local API.
use crate::{
    agent::SessionInfo,
    protocol::{Device, RemoteDevice},
    storage::{ApiAccess, Grant, Peer},
};
use anyhow::{Context, Result, bail};
use clap::{Subcommand, ValueEnum};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, IsTerminal, Read, Write},
};

#[derive(Subcommand)]
pub enum Commands {
    /// Choose computers and devices in an interactive menu (also works over SSH).
    Menu,
    /// List USB devices attached to this computer.
    Devices,
    /// List paired computers and pending approvals.
    Computers,
    /// Approve a pairing request. Select by name or computer ID.
    Approve { computer: Option<String> },
    /// Allow one computer to borrow a local device. Does not connect it yet.
    Share {
        device: Option<String>,
        #[arg(long)]
        to: Option<String>,
        /// Explicitly accept each applicable handoff warning in scripts.
        #[arg(long, value_enum, value_delimiter = ',')]
        acknowledge: Vec<Risk>,
    },
    /// List devices shared with you by an approved computer.
    Remote { computer: Option<String> },
    /// Borrow a shared device. Select any omitted choices interactively.
    Connect {
        computer: Option<String>,
        #[arg(long)]
        device: Option<String>,
    },
    /// Show active connections and the most recent completed connections.
    Sessions,
    /// Return a borrowed device or end an outgoing connection.
    Disconnect {
        session: Option<String>,
        /// Confirm all affected disks have been ejected on the receiving computer.
        #[arg(long)]
        ejected: bool,
    },
    /// Stop sharing a local device with all computers and close its connections.
    Unshare {
        device: Option<String>,
        #[arg(long)]
        ejected: bool,
    },
    /// Remove a computer's approval and grants; close its connections.
    Revoke {
        computer: Option<String>,
        #[arg(long)]
        ejected: bool,
    },
    /// Set this computer's name for new invitations.
    Rename { name: String },
}
#[derive(Clone, Debug, ValueEnum)]
pub enum Risk {
    Input,
    Storage,
    Network,
    Bluetooth,
}
impl Risk {
    fn name(&self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Storage => "storage",
            Self::Network => "network",
            Self::Bluetooth => "bluetooth",
        }
    }
}
#[derive(Deserialize)]
struct State {
    name: String,
    version: String,
    helper_ready: bool,
    helper_error: Option<String>,
    devices: Vec<Device>,
    peers: BTreeMap<String, Peer>,
    grants: BTreeMap<String, Grant>,
    sessions: Vec<SessionInfo>,
    history: Vec<SessionInfo>,
}
pub struct Client {
    http: reqwest::Client,
    access: ApiAccess,
}
impl Client {
    pub fn new(access: ApiAccess) -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(60))
                .build()?,
            access,
        })
    }
    async fn request<T: DeserializeOwned>(&self, action: Option<Value>) -> Result<T> {
        let base = format!("http://127.0.0.1:{}", self.access.port);
        let request = match action {
            Some(action) => self.http.post(format!("{base}/api/action")).json(&action),
            None => self.http.get(format!("{base}/api/state")),
        };
        let response = request
            .bearer_auth(&self.access.token)
            .send()
            .await
            .context("Cannot reach the local agent. Run portrelay check or restart PortRelay")?;
        let status = response.status();
        let value: Value = response
            .json()
            .await
            .context("Invalid response from the local agent")?;
        if !status.is_success() {
            bail!(
                "{}",
                clean(value["error"].as_str().unwrap_or("Local API failed"))
            );
        }
        Ok(serde_json::from_value(value)?)
    }
    async fn state(&self) -> Result<State> {
        self.request(None).await
    }
    async fn action(&self, value: Value) -> Result<Value> {
        self.request(Some(value)).await
    }
    pub async fn invite(&self) -> Result<String> {
        let value = self.action(json!({"op":"invite"})).await?;
        let ticket = value["invitation"].as_str().context("Missing invitation")?;
        // Invitation output is intentionally machine-copyable; never print terminal controls.
        if !ticket.starts_with("portrelay1:")
            || !ticket
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_:".contains(&b))
        {
            bail!("Invalid invitation from the local agent");
        }
        Ok(ticket.into())
    }
    pub async fn pair(&self, invitation: String) -> Result<Value> {
        self.action(json!({"op":"pair","invitation":invitation}))
            .await
    }

    async fn remote(&self, peer: &str) -> Result<Vec<RemoteDevice>> {
        #[derive(Deserialize)]
        struct Reply {
            devices: Vec<RemoteDevice>,
        }
        let reply: Reply = self
            .request(Some(json!({"op":"remote","peer":peer})))
            .await?;
        Ok(reply.devices)
    }
}

/// Treat remote names, USB descriptors and errors as plain text, never terminal commands.
fn clean(text: &str) -> String {
    text.chars().take(2048).map(|c| {
        if c.is_control() || matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}') { ' ' } else { c }
    }).collect()
}
fn interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}
fn read_line(reader: &mut impl BufRead, limit: usize) -> Result<Option<String>> {
    let mut bytes = Vec::new();
    let mut total = 0;
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            break;
        }
        let newline = buffer.iter().position(|b| *b == b'\n');
        let count = newline.map_or(buffer.len(), |i| i + 1);
        total += count;
        let keep = count.min((limit + 2).saturating_sub(bytes.len()));
        bytes.extend_from_slice(&buffer[..keep]);
        reader.consume(count);
        if newline.is_some() {
            break;
        }
    }
    if total == 0 {
        return Ok(None);
    }
    if total > limit + 2 {
        bail!("Input is too long (maximum {limit} bytes)");
    }
    let value = String::from_utf8(bytes).context("Input must be UTF-8")?;
    let value = value.trim_end_matches(['\r', '\n']);
    if value.len() > limit {
        bail!("Input is too long (maximum {limit} bytes)");
    }
    Ok(Some(value.to_owned()))
}

fn prompt(label: &str, limit: usize) -> Result<Option<String>> {
    if !interactive() {
        bail!(
            "This choice needs a terminal. Supply the arguments shown by --help, or run portrelay menu in a terminal"
        );
    }
    print!("{label}");
    io::stdout().flush()?;
    read_line(&mut io::stdin().lock(), limit)
}
pub fn read_invitation() -> Result<String> {
    let value = if interactive() {
        prompt("Paste invitation and press Enter (blank cancels): ", 8192)?.unwrap_or_default()
    } else {
        let mut value = String::new();
        io::stdin().take(8193).read_to_string(&mut value)?;
        if value.len() > 8192 {
            bail!("Invitation is too long");
        }
        value
    };
    let value = value.trim();
    if value.is_empty() {
        bail!("Pairing cancelled; no invitation supplied");
    }
    Ok(value.into())
}
struct Choice {
    id: String,
    name: String,
    detail: String,
}
fn resolve(choices: &[Choice], query: &str, prefix: bool) -> Result<usize> {
    if let Some(index) = choices.iter().position(|c| c.id == query) {
        return Ok(index);
    }
    let matches: Vec<_> = choices
        .iter()
        .enumerate()
        .filter(|(_, c)| c.name == query || (prefix && query.len() >= 8 && c.id.starts_with(query)))
        .map(|(i, _)| i)
        .collect();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => bail!(
            "No match for '{}'. List the choices and use an exact ID or name",
            clean(query)
        ),
        _ => bail!(
            "'{}' matches more than one item. Use the full ID",
            clean(query)
        ),
    }
}
fn number(value: &str, count: usize) -> Result<Option<usize>> {
    let value = value.trim();
    if value.is_empty() || value == "0" || value.eq_ignore_ascii_case("q") {
        return Ok(None);
    }
    let index: usize = value
        .parse()
        .context("Enter a listed number, or 0 to cancel")?;
    if index == 0 || index > count {
        bail!("Enter a number from 1 to {count}, or 0 to cancel");
    }
    Ok(Some(index - 1))
}
fn select(label: &str, choices: &[Choice], query: Option<&str>, prefix: bool) -> Result<usize> {
    if choices.is_empty() {
        bail!("No {label} available. Refresh after pairing, approval or sharing");
    }
    if let Some(query) = query {
        return resolve(choices, query, prefix);
    }
    if !interactive() {
        bail!("Specify {label}; run the command with --help or use portrelay menu in a terminal");
    }
    println!("\nChoose {label}:");
    for (i, choice) in choices.iter().enumerate() {
        println!(
            "  {}. {} [{}] {}",
            i + 1,
            clean(&choice.name),
            clean(&choice.id),
            clean(&choice.detail)
        );
    }
    loop {
        let Some(input) = prompt("Number (0 cancels): ", 32)? else {
            bail!("Cancelled");
        };
        match number(&input, choices.len()) {
            Ok(Some(index)) => return Ok(index),
            Ok(None) => bail!("Cancelled"),
            Err(e) => println!("{e}"),
        }
    }
}
fn peer_choices(state: &State, approved_only: bool) -> Vec<Choice> {
    state
        .peers
        .iter()
        .filter(|(_, peer)| !approved_only || peer.approved)
        .map(|(id, peer)| Choice {
            id: id.clone(),
            name: peer.name.clone(),
            detail: if peer.approved {
                "approved here"
            } else {
                "waiting for your approval"
            }
            .into(),
        })
        .collect()
}
fn device_choices(devices: &[Device]) -> Vec<Choice> {
    devices
        .iter()
        .map(|d| Choice {
            id: d.id.clone(),
            name: d.name.clone(),
            detail: d
                .blocked
                .as_ref()
                .map(|reason| format!("blocked: {reason}"))
                .unwrap_or_else(|| d.kind.clone()),
        })
        .collect()
}
fn peer_name(state: &State, id: &str) -> String {
    state
        .peers
        .get(id)
        .map(|p| clean(&p.name))
        .unwrap_or_else(|| clean(id))
}
fn risks(device: &Device) -> Vec<String> {
    let mut risks = device.risks.clone();
    if matches!(
        device.kind.as_str(),
        "input" | "storage" | "network" | "bluetooth"
    ) && !risks.contains(&device.kind)
    {
        risks.push(device.kind.clone());
    }
    risks.sort();
    risks.dedup();
    risks
}
fn warning(risk: &str) -> &'static str {
    match risk {
        "input" => {
            "This computer loses this keyboard or mouse while borrowed. Keep another way to control it."
        }
        "storage" => {
            "Unmount every volume first (take the disk offline on Windows). Eject it on the receiver before disconnecting. Connection loss can lose unsaved data."
        }
        "network" => {
            "Disable this network adapter first. Its connection is unavailable here while borrowed. Keep another way to reach this computer."
        }
        "bluetooth" => {
            "Use a dedicated USB Bluetooth adapter. Local Bluetooth connections stop while borrowed. Keep peripherals near this computer and pair them on the receiver."
        }
        _ => {
            "This device requires an additional handoff warning. Review it in the application before continuing."
        }
    }
}
fn consent(device: &Device, accepted: &[Risk]) -> Result<Vec<String>> {
    if let Some(reason) = &device.blocked {
        bail!("{}", clean(reason));
    }
    let risks = risks(device);
    let missing: Vec<_> = risks
        .iter()
        .filter(|risk| !accepted.iter().any(|r| r.name() == risk.as_str()))
        .collect();
    if missing.is_empty() {
        return Ok(risks);
    }
    println!("\nBefore sharing {}:", clean(&device.name));
    for risk in &missing {
        println!("  {}: {}", clean(risk), warning(risk));
    }
    if !interactive() {
        bail!(
            "Sharing requires explicit acknowledgement. After preparing the device, add --acknowledge {}",
            missing
                .iter()
                .map(|r| r.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    if !matches!(
        prompt(
            "Device prepared and warnings accepted? Type yes (Enter cancels): ",
            16
        )?
        .as_deref(),
        Some("yes")
    ) {
        bail!("Cancelled; device was not shared");
    }
    Ok(risks)
}
fn confirm_return<'a>(
    sessions: impl Iterator<Item = &'a SessionInfo>,
    ejected: bool,
) -> Result<()> {
    let disks: Vec<_> = sessions
        .filter(|s| {
            s.metadata
                .as_ref()
                .is_some_and(|d| risks(d).iter().any(|r| r == "storage"))
        })
        .collect();
    if ejected || disks.is_empty() {
        return Ok(());
    }
    println!(
        "Eject or unmount all volumes on the RECEIVING computer before ending these connections:"
    );
    for s in disks {
        println!(
            "  {} [{}]",
            clean(
                s.metadata
                    .as_ref()
                    .map(|d| d.name.as_str())
                    .unwrap_or(&s.device)
            ),
            clean(&s.id)
        );
    }
    if !interactive() {
        bail!("After ejecting the disks, repeat with --ejected");
    }
    if !matches!(
        prompt(
            "All affected disks safely ejected? Type yes (Enter cancels): ",
            16
        )?
        .as_deref(),
        Some("yes")
    ) {
        bail!("Cancelled; connections were not closed");
    }
    Ok(())
}
fn show_devices(state: &State) {
    if state.devices.is_empty() {
        println!("No local USB devices found.");
    }
    for d in &state.devices {
        println!("{}  {} ({})", clean(&d.id), clean(&d.name), clean(&d.kind));
        if let Some(reason) = &d.blocked {
            println!("  Blocked: {}", clean(reason));
        }
        if let Some(hub) = &d.parent_hub {
            println!(
                "  Hub: {} (select this child device to share it)",
                clean(hub)
            );
        }
        let allowed: Vec<_> = state
            .grants
            .get(&d.id)
            .filter(|g| g.generation == d.generation)
            .map(|g| g.peers.iter().map(|id| peer_name(state, id)).collect())
            .unwrap_or_default();
        println!(
            "  {}",
            if allowed.is_empty() {
                "Private".into()
            } else {
                format!("Shared with: {}", allowed.join(", "))
            }
        );
    }
    if !state.helper_ready {
        println!(
            "USB helper: {}",
            clean(state.helper_error.as_deref().unwrap_or("not ready"))
        );
    }
}
fn show_sessions(state: &State) {
    if state.sessions.is_empty() {
        println!("No active connections.");
    }
    for s in &state.sessions {
        println!(
            "{}  {} {} {}: {}",
            clean(&s.id),
            clean(&s.device),
            if s.direction == "incoming" {
                "from"
            } else {
                "to"
            },
            peer_name(state, &s.peer),
            clean(&s.state)
        );
        if let Some(error) = &s.error {
            println!("  {}", clean(error));
        }
    }
    for s in state.history.iter().take(3) {
        println!(
            "Recent: {} {}: {}",
            clean(&s.device),
            clean(&s.state),
            clean(s.error.as_deref().unwrap_or("connection ended"))
        );
    }
}
async fn execute(client: &Client, command: Commands) -> Result<()> {
    let state = client.state().await?;
    match command {
        Commands::Menu => unreachable!(),
        Commands::Devices => show_devices(&state),
        Commands::Computers => {
            if state.peers.is_empty() {
                println!(
                    "No paired computers. Create an invitation with portrelay invite --plain, or use portrelay menu."
                );
            }
            for c in peer_choices(&state, false) {
                println!("{}  {} ({})", clean(&c.id), clean(&c.name), c.detail);
            }
            println!(
                "Approval here does not confirm approval or availability on the other computer."
            );
        }
        Commands::Approve { computer } => {
            let choices = peer_choices(&state, false);
            let index = select("computer to approve", &choices, computer.as_deref(), true)?;
            client
                .action(json!({"op":"approve","peer":choices[index].id}))
                .await?;
            println!("Computer approved. Choose Share local device to give it access.");
        }
        Commands::Share {
            device,
            to,
            acknowledge,
        } => {
            let choices = device_choices(&state.devices);
            let index = select("local device", &choices, device.as_deref(), false)?;
            let device = &state.devices[index];
            if let Some(reason) = &device.blocked {
                bail!("{}", clean(reason));
            }
            let peers = peer_choices(&state, true);
            let peer = select("approved computer", &peers, to.as_deref(), true)?;
            let acknowledged = consent(device, &acknowledge)?;
            client.action(json!({"op":"share","device":device.id,"generation":device.generation,"peer":peers[peer].id,"acknowledge_risks":acknowledged})).await?;
            println!(
                "Shared {} with {}. On that computer, choose Connect remote device.",
                clean(&device.name),
                clean(&peers[peer].name)
            );
        }
        Commands::Remote { computer } => {
            let peers = peer_choices(&state, true);
            let index = select("approved computer", &peers, computer.as_deref(), true)?;
            let devices = client.remote(&peers[index].id).await?;
            if devices.is_empty() {
                println!(
                    "No devices shared with you. The owner must approve this computer and share a device."
                );
            }
            for d in devices {
                println!(
                    "{}  {} ({})",
                    clean(&d.device.id),
                    clean(&d.device.name),
                    if d.busy { "busy" } else { "available" }
                );
            }
        }
        Commands::Connect { computer, device } => {
            let peers = peer_choices(&state, true);
            let peer = select("approved computer", &peers, computer.as_deref(), true)?;
            let devices = client.remote(&peers[peer].id).await?;
            let choices: Vec<_> = devices
                .iter()
                .map(|d| Choice {
                    id: d.device.id.clone(),
                    name: d.device.name.clone(),
                    detail: if d.busy { "busy" } else { "available" }.into(),
                })
                .collect();
            let index = select(
                "shared device (the owner must share it first)",
                &choices,
                device.as_deref(),
                false,
            )?;
            let remote = &devices[index];
            if remote.busy {
                bail!("Device is already in use. Wait for it to be returned");
            }
            let d = &remote.device;
            if let Some(reason) = &d.blocked {
                bail!("{}", clean(reason));
            }
            let value = client.action(json!({"op":"connect","peer":peers[peer].id,"device":d.id,"generation":d.generation})).await?;
            println!(
                "Connection requested for {}. Use portrelay sessions to check attachment or errors.",
                clean(&d.name)
            );
            if let Some(id) = value["session"].as_str() {
                println!("Session: {}", clean(id));
            }
            if risks(d).iter().any(|r| r == "storage") {
                println!("Eject this disk before disconnecting it.");
            }
            if risks(d).iter().any(|r| r == "bluetooth") {
                println!(
                    "Once attached, open Bluetooth settings here to pair. Keep peripherals near the original computer."
                );
            }
        }
        Commands::Sessions => show_sessions(&state),
        Commands::Disconnect { session, ejected } => {
            let choices: Vec<_> = state
                .sessions
                .iter()
                .map(|s| Choice {
                    id: s.id.clone(),
                    name: format!("{} ({})", s.device, peer_name(&state, &s.peer)),
                    detail: format!("{} {}", s.direction, s.state),
                })
                .collect();
            let index = select("active connection", &choices, session.as_deref(), true)?;
            let session = &state.sessions[index];
            confirm_return(std::iter::once(session), ejected)?;
            client
                .action(json!({"op":"disconnect","session":session.id}))
                .await?;
            println!("Disconnect requested. Use portrelay sessions to check restoration.");
        }
        Commands::Unshare { device, ejected } => {
            // Include unplugged devices so stale grants can be removed too.
            let choices: Vec<_> = state
                .grants
                .iter()
                .filter(|(_, g)| !g.peers.is_empty())
                .map(|(id, _)| Choice {
                    id: id.clone(),
                    name: state
                        .devices
                        .iter()
                        .find(|d| &d.id == id)
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| "Unplugged device".into()),
                    detail: "remove access for all computers".into(),
                })
                .collect();
            let index = select("device to stop sharing", &choices, device.as_deref(), false)?;
            let id = &choices[index].id;
            confirm_return(
                state
                    .sessions
                    .iter()
                    .filter(|s| s.direction == "outgoing" && &s.device == id),
                ejected,
            )?;
            client.action(json!({"op":"unshare","device":id})).await?;
            println!("Sharing stopped for all computers. Active connections are closing.");
        }
        Commands::Revoke { computer, ejected } => {
            let choices = peer_choices(&state, false);
            let index = select("computer to remove", &choices, computer.as_deref(), true)?;
            let peer = &choices[index].id;
            confirm_return(state.sessions.iter().filter(|s| &s.peer == peer), ejected)?;
            client.action(json!({"op":"revoke","peer":peer})).await?;
            println!("Computer removed. Its grants were removed and connections are closing.");
        }
        Commands::Rename { name } => {
            client.action(json!({"op":"rename","name":name})).await?;
            println!("Computer name saved. New invitations use this name.");
        }
    }
    Ok(())
}
async fn menu(client: &Client) -> Result<()> {
    if !interactive() {
        bail!(
            "Run portrelay menu in an interactive terminal (SSH works). For scripts, use portrelay --help"
        );
    }
    loop {
        let state = client.state().await?;
        println!(
            "\nPortRelay {} | {} | USB {}",
            clean(&state.version),
            clean(&state.name),
            if state.helper_ready {
                "ready"
            } else {
                "not ready"
            }
        );
        println!(
            "{} computers, {} local devices, {} connections",
            state.peers.len(),
            state.devices.len(),
            state.sessions.len()
        );
        println!(
            "1. Local devices\n2. Create invitation\n3. Add computer (paste invitation)\n4. Approve computer\n5. Share local device\n6. Connect remote device\n7. Connections\n8. Disconnect device\n9. Stop sharing device\n10. Computers\n11. Remove computer\n0. Exit"
        );
        let Some(input) = prompt("Choose: ", 32)? else {
            break;
        };
        let choice = match number(&input, 11) {
            Ok(Some(n)) => n + 1,
            Ok(None) => break,
            Err(e) => {
                println!("{e}");
                continue;
            }
        };
        let result = match choice {
            2 => client.invite().await.map(|ticket| println!("\n{ticket}\n\nCopy this entire invitation to the other computer. It expires in 10 minutes and can be used once. Then approve that computer here.")),
            3 => match read_invitation() { Ok(ticket) => client.pair(ticket).await.map(|_| println!("Pairing requested. Approve this computer on the other computer using portrelay menu.")), Err(e) => Err(e) },
            n => execute(client, match n {
                1 => Commands::Devices,
                4 => Commands::Approve { computer: None },
                5 => Commands::Share { device: None, to: None, acknowledge: vec![] },
                6 => Commands::Connect { computer: None, device: None },
                7 => Commands::Sessions,
                8 => Commands::Disconnect { session: None, ejected: false },
                9 => Commands::Unshare { device: None, ejected: false },
                10 => Commands::Computers,
                11 => Commands::Revoke { computer: None, ejected: false },
                _ => unreachable!(),
            }).await,
        };
        if let Err(e) = result {
            println!("{}", clean(&format!("{e:#}")));
        }
    }
    Ok(())
}
pub async fn run(access: ApiAccess, command: Commands) -> Result<()> {
    let client = Client::new(access)?;
    let result = if matches!(command, Commands::Menu) {
        menu(&client).await
    } else {
        execute(&client, command).await
    };
    result.map_err(|e| anyhow::anyhow!(clean(&format!("{e:#}"))))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_never_guesses_ambiguous_names_or_short_prefixes() {
        let choices = vec![
            Choice {
                id: "aaaaaaaa11111111".into(),
                name: "Pi".into(),
                detail: "".into(),
            },
            Choice {
                id: "aaaaaaaa22222222".into(),
                name: "Pi".into(),
                detail: "".into(),
            },
        ];
        assert!(resolve(&choices, "Pi", true).is_err());
        assert!(resolve(&choices, "aaaaaaaa", true).is_err());
        assert!(resolve(&choices, "aaaa", true).is_err());
        assert_eq!(resolve(&choices, "aaaaaaaa2", true).unwrap(), 1);
        assert_eq!(resolve(&choices, "aaaaaaaa11111111", false).unwrap(), 0);
        assert!(resolve(&choices, "1", true).is_err());
    }
    #[test]
    fn cancel_eof_and_invalid_choices_never_select_a_device() {
        for input in ["0", "", "q", "Q", " "] {
            assert_eq!(number(input, 3).unwrap(), None);
        }
        for input in ["4", "-1", "yes", "9999999999999999999999999"] {
            assert!(number(input, 3).is_err());
        }
        assert_eq!(number(" 2 ", 3).unwrap(), Some(1));
        assert_eq!(read_line(&mut io::Cursor::new(b""), 4).unwrap(), None);
        assert_eq!(
            read_line(&mut io::Cursor::new(b"abcd\r\n"), 4).unwrap(),
            Some("abcd".into())
        );
        assert!(read_line(&mut io::Cursor::new(b"abcde\n"), 4).is_err());
    }
    #[test]
    fn remote_descriptors_cannot_control_the_terminal() {
        let text = clean("disk\x1b[2J\r\n\x07\u{009b}\u{202e}name");
        assert!(!text.chars().any(char::is_control));
        assert!(!text.contains('\u{202e}'));
        assert_eq!(clean(&"x".repeat(3000)).len(), 2048);
    }
}
