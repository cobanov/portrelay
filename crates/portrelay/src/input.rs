//! Explicit keyboard/pointer control, independent of whole USB device sharing.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

pub const SOCKET: &str = "/run/portrelay-input/helper.sock";
pub const DEADLINE: Duration = Duration::from_secs(2);
pub const MAX_EVENTS: usize = 64;

pub fn socket_path() -> PathBuf {
    // Only the local process environment can select a test/helper path.
    std::env::var_os("PORTRELAY_INPUT_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| SOCKET.into())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Move { x: i32, y: i32 },
    Wheel { x: i32, y: i32 },
    Button { button: u8, down: bool },
    Key { code: u16, value: u8 },
    Release,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub sequence: u64,
    pub events: Vec<Event>,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Probe,
    Open,
}
#[derive(Serialize, Deserialize)]
pub struct Response {
    pub error: Option<String>,
    pub sequence: u64,
}
pub fn valid_key(code: u16) -> bool {
    // No power, sleep, wake or SysRq injection. Physical key positions, not text.
    matches!(code, 1..=83 | 86..=98 | 100 | 102..=111 | 117 | 119 | 125..=127)
}
impl Batch {
    pub fn validate(&self, expected: u64) -> Result<()> {
        if self.sequence != expected || self.sequence == u64::MAX || self.events.len() > MAX_EVENTS
        {
            bail!("Invalid input sequence or batch size");
        }
        for event in &self.events {
            match event {
                Event::Move { x, y } if x.unsigned_abs() <= 32767 && y.unsigned_abs() <= 32767 => {}
                Event::Wheel { x, y } if x.unsigned_abs() <= 16 && y.unsigned_abs() <= 16 => {}
                Event::Button { button, .. } if *button <= 4 => {}
                Event::Key { code, value } if valid_key(*code) && *value <= 2 => {}
                Event::Release => {}
                _ => bail!("Unsupported input event"),
            }
        }
        Ok(())
    }
}
pub async fn health() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let _ = open(Command::Probe).await?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    bail!("Receiving keyboard and mouse control currently requires Linux")
}
#[cfg(target_os = "linux")]
pub async fn open(command: Command) -> Result<tokio::net::UnixStream> {
    use crate::protocol::{read_frame, write_frame};
    use anyhow::Context;
    let mut socket = tokio::time::timeout(DEADLINE, tokio::net::UnixStream::connect(socket_path()))
        .await
        .context("Linux input support did not respond")?
        .context("Enable receiving control in PortRelay on the Linux desktop first")?;
    write_frame(&mut socket, &command).await?;
    let response: Response = tokio::time::timeout(DEADLINE, read_frame(&mut socket)).await??;
    if let Some(error) = response.error {
        bail!("{error}");
    }
    Ok(socket)
}
#[cfg(target_os = "linux")]
#[path = "input_linux.rs"]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::serve;
#[cfg(not(target_os = "linux"))]
pub async fn serve(_uid: u32, _runtime: &std::path::Path) -> Result<()> {
    bail!("The input helper requires Linux")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_bounds_reject_replay_privileged_keys_and_unbounded_motion() {
        let valid = Batch {
            sequence: 1,
            events: vec![
                Event::Move { x: 20, y: -3 },
                Event::Key { code: 30, value: 1 },
                Event::Button {
                    button: 0,
                    down: true,
                },
            ],
        };
        valid.validate(1).unwrap();
        assert!(valid.validate(2).is_err());
        for event in [
            Event::Key {
                code: 116,
                value: 1,
            },
            Event::Key { code: 99, value: 1 },
            Event::Key { code: 30, value: 3 },
            Event::Move { x: i32::MIN, y: 0 },
            Event::Wheel { x: 0, y: 17 },
            Event::Button {
                button: 5,
                down: true,
            },
        ] {
            assert!(
                Batch {
                    sequence: 1,
                    events: vec![event]
                }
                .validate(1)
                .is_err()
            );
        }
        assert!(
            Batch {
                sequence: 1,
                events: vec![Event::Release; 65]
            }
            .validate(1)
            .is_err()
        );
        assert!(
            serde_json::from_str::<Event>(r#"{"type":"key","code":30,"value":1,"text":"secret"}"#)
                .is_err()
        );
    }
}
