use anyhow::{Context, Result, bail};
use iroh::{EndpointAddr, SecretKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Peer {
    pub name: String,
    pub address: EndpointAddr,
    pub approved: bool,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub online: Option<bool>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Grant {
    pub generation: String,
    pub peers: Vec<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub account: Option<crate::account::Account>,
    pub secret_key: String,
    pub name: String,
    #[serde(default)]
    pub peers: BTreeMap<String, Peer>,
    #[serde(default)]
    pub grants: BTreeMap<String, Grant>,
    /// Separate permission to control this computer. Pairing alone never grants it.
    #[serde(default)]
    pub input_controllers: Vec<String>,
}
#[derive(Serialize, Deserialize)]
pub struct ApiAccess {
    pub port: u16,
    pub token: String,
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn random_secret() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}
pub fn state_dir(override_dir: Option<PathBuf>) -> Result<PathBuf> {
    let dir = override_dir.unwrap_or(
        directories::ProjectDirs::from("dev", "cobanov", "portrelay")
            .context("Cannot locate user data directory")?
            .data_local_dir()
            .to_path_buf(),
    );
    fs::create_dir_all(&dir)?;
    #[cfg(windows)]
    crate::windows::protect_state(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let m = fs::symlink_metadata(&dir)?;
        if m.file_type().is_symlink() || m.uid() != nix::unistd::getuid().as_raw() {
            bail!("State directory must belong to the current user and not be a symlink");
        }
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}
pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", random_secret()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> Result<()> {
        let mut file = options.open(&temporary)?;
        file.write_all(&serde_json::to_vec_pretty(value)?)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
impl Config {
    pub fn load(dir: &Path, name: Option<String>) -> Result<Self> {
        let path = dir.join("config.json");
        let mut config = if path.exists() {
            serde_json::from_slice(&fs::read(&path)?)?
        } else {
            Self {
                account: None,
                secret_key: random_secret(),
                name: name.clone().unwrap_or_else(|| {
                    fs::read_to_string("/proc/sys/kernel/hostname")
                        .ok()
                        .or_else(|| std::env::var("COMPUTERNAME").ok())
                        .map(|s| s.trim().chars().take(80).collect())
                        .filter(|s: &String| !s.is_empty())
                        .unwrap_or_else(|| "My computer".into())
                }),
                peers: BTreeMap::new(),
                grants: BTreeMap::new(),
                input_controllers: vec![],
            }
        };
        if let Some(name) = name {
            if name.trim().is_empty() || name.len() > 80 {
                bail!("Computer name must contain 1–80 bytes");
            }
            config.name = name;
        }
        config.key()?;
        save(&path, &config)?;
        Ok(config)
    }
    pub fn key(&self) -> Result<SecretKey> {
        let bytes: [u8; 32] = hex::decode(&self.secret_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid identity key"))?;
        Ok(SecretKey::from_bytes(&bytes))
    }
}
