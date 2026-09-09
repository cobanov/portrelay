//! Optional Debian/Ubuntu desktop integration. No device streaming code lives here.
#[cfg(not(any(windows, target_os = "macos")))]
use anyhow::Context;
use anyhow::{Result, bail};
use serde::Serialize;
use std::sync::Arc;
#[cfg(not(windows))]
use std::{path::Path, time::Duration};
#[cfg(not(windows))]
use tokio::process::Command;
use tokio::sync::Mutex;

#[cfg(not(windows))]
const SETUP_PROGRAM: &str = "/usr/lib/portrelay/setup-usb";
#[derive(Clone, Default, Serialize)]
pub struct Progress {
    pub running: bool,
    pub error: Option<String>,
}
#[derive(Default)]
pub struct Setup {
    progress: Arc<Mutex<Progress>>,
}
impl Setup {
    pub fn available() -> bool {
        #[cfg(windows)]
        return crate::windows::installed();
        #[cfg(not(windows))]
        {
            cfg!(target_os = "linux") && Path::new(SETUP_PROGRAM).is_file()
        }
    }
    pub async fn snapshot(&self) -> Progress {
        self.progress.lock().await.clone()
    }
    pub async fn start(&self) -> Result<()> {
        if !Self::available() {
            bail!(
                "Install the PortRelay package for this system to enable USB sharing from this window."
            );
        }
        let mut progress = self.progress.lock().await;
        if progress.running {
            bail!("USB setup is already running.");
        }
        *progress = Progress {
            running: true,
            error: None,
        };
        let state = self.progress.clone();
        tokio::spawn(async move {
            #[cfg(windows)]
            let error = crate::windows::request_setup()
                .await
                .err()
                .map(|e| e.to_string());
            #[cfg(not(windows))]
            let error = {
                let result = tokio::time::timeout(
                    Duration::from_secs(300),
                    Command::new("/usr/bin/pkexec")
                        .arg("--disable-internal-agent")
                        .arg(SETUP_PROGRAM)
                        .current_dir("/")
                        .kill_on_drop(true)
                        .output(),
                )
                .await;
                match result {
                Ok(Ok(output)) if output.status.success() => None,
                Ok(Ok(output)) if matches!(output.status.code(), Some(126 | 127)) => Some("System permission was cancelled or unavailable. Try again from your Linux desktop.".into()),
                Ok(Ok(output)) => Some(String::from_utf8_lossy(&output.stderr).trim().chars().take(1200).collect()),
                Ok(Err(_)) => Some("Could not open the system permission window. Reinstall the PortRelay package.".into()),
                Err(_) => Some("USB setup is taking longer than expected. Wait for system package downloads to finish, then retry.".into()),
            }
            };
            *state.lock().await = Progress {
                running: false,
                error,
            };
        });
        Ok(())
    }
}

pub async fn start_desktop_service() -> Result<()> {
    #[cfg(windows)]
    return crate::windows::start_desktop().await;
    #[cfg(target_os = "macos")]
    return crate::macos::start_desktop().await;
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        if !cfg!(target_os = "linux")
            || !Path::new("/usr/lib/systemd/user/portrelay.service").is_file()
        {
            bail!(
                "The desktop launcher requires the Ubuntu or Debian package. Other builds can use portrelay run."
            );
        }
        // Keep the desktop's authentication/display environment available to pkexec.
        let keys: Vec<_> = [
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "XAUTHORITY",
            "DBUS_SESSION_BUS_ADDRESS",
        ]
        .into_iter()
        .filter(|key| std::env::var_os(key).is_some())
        .collect();
        if !keys.is_empty() {
            user_systemctl(&[&["import-environment"][..], &keys].concat()).await?;
        }
        user_systemctl(&["daemon-reload"]).await?;
        user_systemctl(&["enable", "--now", "portrelay.service"]).await
    }
}
#[cfg(not(any(windows, target_os = "macos")))]
async fn user_systemctl(args: &[&str]) -> Result<()> {
    let output = tokio::time::timeout(
        Duration::from_secs(20),
        Command::new("/usr/bin/systemctl")
            .arg("--user")
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .context("PortRelay's background service did not respond. Try opening it again.")??;
    if !output.status.success() {
        bail!(
            "Could not start PortRelay's background service. Open the app from a Linux desktop session, or use the headless setup guide."
        );
    }
    Ok(())
}
