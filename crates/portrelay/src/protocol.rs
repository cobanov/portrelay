use anyhow::{Result, bail};
use iroh::EndpointAddr;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const ALPN: &[u8] = b"portrelay/1";
pub const MAX_FRAME: usize = 64 * 1024;
pub const VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub generation: String,
    pub name: String,
    pub vendor: String,
    pub product: String,
    pub kind: String,
    pub speed: u32,
    pub devid: u32,
    pub blocked: Option<String>,
    /// Every applicable handoff warning, including composite interfaces.
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub parent_hub: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub version: u16,
    pub request: Request,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Pair {
        secret: String,
        name: String,
        address: EndpointAddr,
    },
    List,
    InputStatus,
    InputOpen,
    Open {
        device: String,
        generation: String,
    },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Reply {
    Pending {
        name: String,
    },
    Devices {
        devices: Vec<RemoteDevice>,
    },
    Ready {
        device: Device,
    },
    Error {
        message: String,
    },
    InputStatus {
        ready: bool,
        allowed: bool,
        busy: bool,
    },
    InputReady,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDevice {
    #[serde(flatten)]
    pub device: Device,
    pub busy: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Invitation {
    pub version: u16,
    pub address: EndpointAddr,
    pub secret: String,
    pub name: String,
    pub expires: u64,
}

pub async fn read_frame<R: AsyncRead + Unpin, T: DeserializeOwned>(r: &mut R) -> Result<T> {
    let length = r.read_u32().await? as usize;
    if length == 0 || length > MAX_FRAME {
        bail!("Invalid protocol frame length");
    }
    let mut bytes = vec![0; length];
    r.read_exact(&mut bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}
pub async fn write_frame<W: AsyncWrite + Unpin, T: Serialize>(w: &mut W, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_FRAME {
        bail!("Protocol frame is too large");
    }
    w.write_u32(bytes.len() as u32).await?;
    w.write_all(&bytes).await?;
    w.flush().await?;
    Ok(())
}
pub fn valid_bus_id(id: &str) -> bool {
    if id.len() > 31 {
        return false;
    }
    let Some((bus, ports)) = id.split_once('-') else {
        return false;
    };
    !bus.is_empty()
        && bus.bytes().all(|b| b.is_ascii_digit())
        && ports
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn rejects_oversized_and_truncated_frames() {
        let bytes = (MAX_FRAME as u32 + 1).to_be_bytes();
        assert!(
            read_frame::<_, Envelope>(&mut bytes.as_slice())
                .await
                .is_err()
        );
        assert!(
            read_frame::<_, Envelope>(&mut b"\0\0\0\x10{}".as_slice())
                .await
                .is_err()
        );
    }
    #[test]
    fn bus_ids_cannot_escape_sysfs() {
        for id in ["1-2", "12-2.10"] {
            assert!(valid_bus_id(id));
        }
        for id in [
            "../unbind",
            "1-2/driver",
            "1-",
            "1-2\n",
            "usb1",
            "-2",
            "1-.2",
        ] {
            assert!(!valid_bus_id(id));
        }
    }
}
