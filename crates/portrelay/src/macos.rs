//! Unprivileged, per-device IOKit worker. Its only transport is inherited pipes.
use crate::{
    backend::{HelperReply, HelperRequest},
    protocol::{Device, read_frame, valid_bus_id},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    net::UnixStream,
    process::{Child, Command},
    sync::watch,
};

pub const IMPORT_REASON: &str = "Receiving USB devices on a Mac needs Apple's USB host-controller entitlement and a native import backend. This preview can only share eligible devices with Linux or Windows.";

pub fn worker_path() -> PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .with_file_name("portrelay-macos-usb")
}
fn worker(path: &Path, args: &[&str]) -> Result<Child> {
    Command::new(path)
        .args(args)
        .env("USBIPD_LOG_LEVEL", "critical")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .context("Could not start the Mac USB worker. Build or reinstall the Mac app.")
}
async fn query<T: DeserializeOwned>(path: &Path, argument: &str) -> Result<T> {
    let mut child = worker(path, &[argument])?;
    drop(child.stdin.take());
    tokio::time::timeout(Duration::from_secs(4), async {
        let value = read_frame(child.stdout.as_mut().context("Worker output missing")?).await?;
        if !child.wait().await?.success() {
            bail!("The Mac USB worker could not read device state");
        }
        Ok(value)
    })
    .await
    .context("The Mac USB worker did not respond")?
}
pub async fn health(path: &Path) -> Result<()> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Probe {
        protocol_version: u32,
        usb_export: bool,
        usb_import: bool,
    }
    let probe: Probe = query(path, "probe").await?;
    if probe.protocol_version != 1 || !probe.usb_export || probe.usb_import {
        bail!("Incompatible Mac USB worker; reinstall the matching app");
    }
    Ok(())
}
pub async fn devices(path: &Path) -> Result<Vec<Device>> {
    query(path, "inventory").await
}

type Completion = Option<std::result::Result<(), String>>;
struct Lease {
    completion: watch::Receiver<Completion>,
}
type Leases = HashMap<String, Arc<Lease>>;
fn leases() -> &'static Mutex<Leases> {
    static LEASES: OnceLock<Mutex<Leases>> = OnceLock::new();
    LEASES.get_or_init(Mutex::default)
}
struct Reservation {
    device: String,
    lease: Arc<Lease>,
    armed: bool,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        if self.armed {
            let mut map = leases().lock().unwrap();
            if map
                .get(&self.device)
                .is_some_and(|l| Arc::ptr_eq(l, &self.lease))
            {
                map.remove(&self.device);
            }
        }
    }
}
fn empty_reply() -> Result<(UnixStream, HelperReply)> {
    let (stream, other) = UnixStream::pair()?;
    drop(other);
    Ok((
        stream,
        HelperReply {
            error: None,
            device: None,
            port: None,
        },
    ))
}
pub async fn open(path: &Path, request: &HelperRequest) -> Result<(UnixStream, HelperReply)> {
    match request {
        HelperRequest::Health => {
            health(path).await?;
            empty_reply()
        }
        HelperRequest::Import { .. } | HelperRequest::WaitImport { .. } => bail!(IMPORT_REASON),
        HelperRequest::Inventory => bail!("Use the Mac inventory query"),
        HelperRequest::WaitExport { device } => {
            let lease = leases()
                .lock()
                .unwrap()
                .get(device)
                .cloned()
                .context("No Mac export lease to release")?;
            let mut completion = lease.completion.clone();
            let result = tokio::time::timeout(Duration::from_secs(8), async {
                loop {
                    if let Some(result) = completion.borrow().clone() {
                        return Ok::<_, anyhow::Error>(result);
                    }
                    completion
                        .changed()
                        .await
                        .context("Mac worker cleanup monitor stopped")?;
                }
            })
            .await
            .context("Mac USB worker has not exited; cleanup is still pending")??;
            let _remove = Reservation {
                device: device.clone(),
                lease,
                armed: true,
            };
            result.map_err(anyhow::Error::msg)?;
            empty_reply()
        }
        HelperRequest::Export { device, generation } => {
            if !valid_bus_id(device)
                || generation.len() != 64
                || !generation.bytes().all(|b| b.is_ascii_hexdigit())
            {
                bail!("Invalid Mac device identity");
            }
            let (completed, completion) = watch::channel(None);
            let lease = Arc::new(Lease { completion });
            {
                let mut map = leases().lock().unwrap();
                if map.contains_key(device) {
                    bail!("Device is busy or still being released");
                }
                if map.len() >= 32 {
                    bail!("Too many Mac device leases");
                }
                map.insert(device.clone(), lease.clone());
            }
            // Cancellation while opening must roll back both the process and reservation.
            let mut reservation = Reservation {
                device: device.clone(),
                lease,
                armed: true,
            };
            let mut child = worker(path, &["export", device, generation])?;
            let mut input = child.stdin.take().context("Worker input missing")?;
            let mut output = child.stdout.take().context("Worker output missing")?;
            let reply: HelperReply =
                tokio::time::timeout(Duration::from_secs(20), read_frame(&mut output))
                    .await
                    .context("Mac USB device did not open in time")??;
            if let Some(error) = &reply.error {
                bail!("{error}");
            }
            let actual = reply
                .device
                .as_ref()
                .context("Mac worker returned no device")?;
            if actual.id != *device
                || actual.generation != *generation
                || actual.blocked.is_some()
                || reply.port.is_some()
            {
                bail!("Mac worker opened an unexpected device");
            }
            let (agent, ipc) = UnixStream::pair()?;
            reservation.armed = false;
            tokio::spawn(async move {
                let (mut read, mut write) = ipc.into_split();
                let outcome: Result<()> = async {
                    let parent_closed = tokio::select! {
                        result = tokio::io::copy(&mut read, &mut input) => { result?; true }
                        result = tokio::io::copy(&mut output, &mut write) => { result?; false }
                    };
                    // EOF stops the worker even if requests are pending. A parent SIGKILL
                    // also closes the same pipe without needing a recovery daemon.
                    let _ = input.shutdown().await;
                    drop(input);
                    let status = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
                    match status {
                        Ok(Ok(status)) if status.success() && parent_closed => Ok(()),
                        Ok(Ok(status)) => bail!("Mac USB worker stopped unexpectedly ({status}); device handles were released"),
                        Ok(Err(error)) => Err(error.into()),
                        Err(_) => { child.kill().await?; bail!("Mac USB worker had to be stopped; device handles were released"); }
                    }
                }.await;
                // On a pipe failure, kill and reap before publishing cleanup completion.
                let outcome = if let Err(error) = outcome {
                    match child.kill().await {
                        Ok(()) => Err(error),
                        Err(cleanup) => {
                            Err(anyhow::anyhow!("{error}; worker cleanup failed: {cleanup}"))
                        }
                    }
                } else {
                    outcome
                };
                drop(write);
                let _ = completed.send(Some(outcome.map_err(|e| e.to_string())));
            });
            Ok((agent, reply))
        }
    }
}

pub async fn start_desktop() -> Result<()> {
    let exe = std::env::current_exe()?;
    if !exe.with_file_name("portrelay-macos-usb").is_file() {
        bail!("Open the complete PortRelay.app, or build the Mac USB worker first.");
    }
    let home = std::env::var_os("HOME").context("Home directory is unavailable")?;
    let home = PathBuf::from(home);
    if exe
        .ancestors()
        .any(|path| path.extension().is_some_and(|ext| ext == "app"))
        && !exe.starts_with("/Applications")
        && !exe.starts_with(home.join("Applications"))
    {
        bail!(
            "Move PortRelay.app to Applications before opening it. This keeps the background app available after the download is moved or removed."
        );
    }
    let directory = home.join("Library/LaunchAgents");
    std::fs::create_dir_all(&directory)?;
    let plist = directory.join("dev.cobanov.portrelay.plist");
    let contents = launch_agent(&exe)?;
    let domain = format!("gui/{}", nix::unistd::getuid());
    let service = format!("{domain}/dev.cobanov.portrelay");
    if std::fs::read_to_string(&plist).ok().as_deref() != Some(&contents) {
        let _ = launchctl(&["bootout", &service]).await;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let temporary = plist.with_extension(format!("{}.tmp", crate::storage::random_secret()));
        let result = (|| -> Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(contents.as_bytes())?;
            file.sync_all()?;
            std::fs::rename(&temporary, &plist)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result?;
    }
    if !launchctl(&["print", &service]).await?.success()
        && !launchctl(&[
            "bootstrap",
            &domain,
            plist.to_str().context("Invalid app path")?,
        ])
        .await?
        .success()
    {
        bail!(
            "Could not start PortRelay at login. Open the app from your Mac desktop, or use portrelay run from a terminal."
        );
    }
    if !launchctl(&["kickstart", &service]).await?.success() {
        bail!("PortRelay could not start. Check Login Items in System Settings.");
    }
    Ok(())
}
async fn launchctl(args: &[&str]) -> Result<std::process::ExitStatus> {
    Ok(tokio::time::timeout(
        Duration::from_secs(10),
        Command::new("/bin/launchctl")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .status(),
    )
    .await??)
}
fn launch_agent(exe: &Path) -> Result<String> {
    let executable = exe.to_str().context("App path is not valid UTF-8")?;
    let escaped = executable
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;");
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>dev.cobanov.portrelay</string>
<key>ProgramArguments</key><array><string>{escaped}</string><string>run</string><string>--no-open</string></array>
<key>RunAtLoad</key><true/>
<key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
<key>ThrottleInterval</key><integer>10</integer>
</dict></plist>
"#
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    fn fixture(dir: &Path) -> PathBuf {
        let file = dir.join("worker");
        std::fs::write(&file, r#"#!/usr/bin/env python3
import sys,struct,json,os

def frame(value):
 b=json.dumps(value).encode();sys.stdout.buffer.write(struct.pack('!I',len(b))+b);sys.stdout.buffer.flush()
if sys.argv[1]=='probe':
 frame({'protocolVersion':1,'usbExport':True,'usbImport':False})
elif sys.argv[1]=='inventory': frame([])
else:
 device,generation=sys.argv[2:]
 if generation[0]=='b':
  frame({'error':'Device changed; select it again'});sys.exit(1)
 frame({'device':{'id':device,'generation':generation,'name':'Process fixture, not hardware','vendor':'1234','product':'5678','kind':'usb','speed':3,'devid':65537}})
 if generation[0]=='c': sys.exit(7)
 while True:
  data=os.read(0,65536)
  if not data: break
  sys.stdout.buffer.write(data);sys.stdout.buffer.flush()
"#).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
        file
    }
    fn export(id: &str, generation: char) -> HelperRequest {
        HelperRequest::Export {
            device: id.into(),
            generation: generation.to_string().repeat(64),
        }
    }
    #[tokio::test]
    async fn worker_stream_is_exclusive_and_released_after_eof() {
        let dir = tempfile::tempdir().unwrap();
        let worker = fixture(dir.path());
        health(&worker).await.unwrap();
        assert!(devices(&worker).await.unwrap().is_empty());
        let (mut stream, _) = open(&worker, &export("241-1", 'a')).await.unwrap();
        assert!(open(&worker, &export("241-1", 'a')).await.is_err());
        stream.write_all(b"fixture device bytes").await.unwrap();
        let mut bytes = [0; 20];
        stream.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"fixture device bytes");
        drop(stream);
        open(
            &worker,
            &HelperRequest::WaitExport {
                device: "241-1".into(),
            },
        )
        .await
        .unwrap();
        let (stream, _) = open(&worker, &export("241-1", 'a')).await.unwrap();
        drop(stream);
        open(
            &worker,
            &HelperRequest::WaitExport {
                device: "241-1".into(),
            },
        )
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn rejected_open_does_not_leak_a_lease_and_crash_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let worker = fixture(dir.path());
        assert!(open(&worker, &export("242-1", 'b')).await.is_err());
        let (mut stream, _) = open(&worker, &export("242-1", 'c')).await.unwrap();
        assert_eq!(stream.read(&mut [0; 1]).await.unwrap(), 0);
        drop(stream);
        let result = open(
            &worker,
            &HelperRequest::WaitExport {
                device: "242-1".into(),
            },
        )
        .await;
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("stopped unexpectedly")
        );
        assert!(!leases().lock().unwrap().contains_key("242-1"));
    }
    #[test]
    fn launch_agent_paths_are_data_not_xml_or_shell() {
        let plist = launch_agent(Path::new(
            "/Applications/A & B <test>/PortRelay.app/Contents/MacOS/portrelay",
        ))
        .unwrap();
        assert!(plist.contains("A &amp; B &lt;test&gt;"));
        assert!(!plist.contains("/bin/sh"));
        assert!(crate::backend::import_unavailable_reason().is_some());
    }
}
