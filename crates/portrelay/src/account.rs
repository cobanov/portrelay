//! Open account registry client. Only identity and connection metadata leave this agent.
use anyhow::{Context, Result, bail};
use iroh::{EndpointAddr, SecretKey};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::time::Duration;

pub const DEFAULT_SERVER: &str = "https://portrelay-account.cobanov.dev";
pub const TRUST_SECONDS: u64 = 90;
#[derive(Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: String,
    pub provider: String,
    pub name: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub server: String,
    pub token: String,
    pub identity: Option<Identity>,
    pub authorization_url: String,
    pub expires: u64,
}
#[derive(Default)]
pub struct Runtime {
    pub last_sync: u64,
    pub last_check: Option<std::time::Instant>,
    pub error: Option<String>,
}
impl Runtime {
    pub fn fresh(&self) -> bool {
        self.last_check
            .is_some_and(|time| time.elapsed() < Duration::from_secs(TRUST_SECONDS))
    }
}
#[derive(Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub address: EndpointAddr,
    pub online: bool,
}
#[derive(Deserialize)]
pub struct Sync {
    pub account: Identity,
    pub devices: Vec<Device>,
}
#[derive(Deserialize)]
pub struct Login {
    pub status: String,
    pub account: Option<Identity>,
}
#[derive(Debug)]
pub struct Unauthorized;
impl std::fmt::Display for Unauthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "This computer was signed out or removed. Sign in again.")
    }
}
impl std::error::Error for Unauthorized {}

pub fn server(value: &str) -> Result<String> {
    let url = reqwest::Url::parse(value)?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        bail!("Account server must be an HTTPS origin (HTTP is allowed only on loopback)");
    }
    Ok(url.origin().ascii_serialization())
}
pub struct Client {
    http: reqwest::Client,
    origin: String,
}
impl Client {
    pub fn new(origin: &str) -> Result<Self> {
        Ok(Self {
            origin: server(origin)?,
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(4))
                .timeout(Duration::from_secs(8))
                .build()?,
        })
    }
    async fn request<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
        value: Option<serde_json::Value>,
    ) -> Result<T> {
        let mut req = self.http.request(method, format!("{}{path}", self.origin));
        if let Some(token) = token {
            req = req.bearer_auth(token);
        }
        if let Some(value) = value {
            req = req.json(&value);
        }
        let mut response = req
            .send()
            .await
            .context("Cannot reach the account service")?;
        let status = response.status();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > 262144 {
                bail!("Account response is too large");
            }
            bytes.extend_from_slice(&chunk);
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Unauthorized.into());
        }
        if !status.is_success() {
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
            bail!(
                "{}",
                value["error"]
                    .as_str()
                    .unwrap_or("Account service request failed")
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(200)
                    .collect::<String>()
            );
        }
        serde_json::from_slice(&bytes).context("Invalid account service response")
    }
    pub async fn enroll(
        &self,
        key: SecretKey,
        name: &str,
        address: EndpointAddr,
    ) -> Result<Account> {
        #[derive(Deserialize)]
        struct Challenge {
            id: String,
            nonce: String,
        }
        #[derive(Deserialize)]
        struct Registered {
            authorization_url: String,
            expires: u64,
        }
        let challenge: Challenge = self
            .request(reqwest::Method::POST, "/api/challenge", None, None)
            .await?;
        let token = crate::storage::random_secret();
        let payload = serde_json::to_string(
            &serde_json::json!({"version":1,"purpose":"portrelay-account-enrollment","origin":self.origin,"device_id":key.public().to_string(),"token_hash":hex::encode(Sha256::digest(token.as_bytes())),"challenge":challenge.id,"nonce":challenge.nonce,"name":name,"platform":std::env::consts::OS,"address":address}),
        )?;
        let signature = hex::encode(key.sign(payload.as_bytes()).to_bytes());
        let registered: Registered = self
            .request(
                reqwest::Method::POST,
                "/api/register",
                None,
                Some(serde_json::json!({"payload":payload,"signature":signature})),
            )
            .await?;
        let url = reqwest::Url::parse(&registered.authorization_url)?;
        if url.origin().ascii_serialization() != self.origin || url.path() != "/login" {
            bail!("Unexpected sign-in URL from account service");
        }
        Ok(Account {
            server: self.origin.clone(),
            token,
            identity: None,
            authorization_url: registered.authorization_url,
            expires: registered.expires,
        })
    }
    pub async fn login(&self, token: &str) -> Result<Login> {
        self.request(reqwest::Method::GET, "/api/login", Some(token), None)
            .await
    }
    pub async fn sync(&self, token: &str, name: &str, address: EndpointAddr) -> Result<Sync> {
        let result:Sync=self.request(reqwest::Method::POST,"/api/sync",Some(token),Some(serde_json::json!({"name":name,"platform":std::env::consts::OS,"address":address}))).await?;
        if result.devices.len() > 64
            || result.account.provider != "github"
            || result.account.id.is_empty()
        {
            bail!("Invalid account registry");
        }
        for device in &result.devices {
            if device.id != device.address.id.to_string()
                || device.name.trim().is_empty()
                || device.name.len() > 80
                || device.name.chars().any(char::is_control)
                || !["linux", "macos", "windows"].contains(&device.platform.as_str())
                || device.address.addrs.len() > 16
            {
                bail!("Invalid registered computer");
            }
        }
        Ok(result)
    }
    pub async fn cancel(&self, token: &str) -> Result<()> {
        let _: serde_json::Value = self
            .request(reqwest::Method::DELETE, "/api/login", Some(token), None)
            .await?;
        Ok(())
    }
    pub async fn remove(&self, token: &str, id: &str) -> Result<()> {
        if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            bail!("Invalid computer identity");
        }
        let _: serde_json::Value = self
            .request(
                reqwest::Method::DELETE,
                &format!("/api/devices/{id}"),
                Some(token),
                None,
            )
            .await?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_registry_origins() {
        assert_eq!(server(DEFAULT_SERVER).unwrap(), DEFAULT_SERVER);
        assert!(server("http://127.0.0.1:8787").is_ok());
        for value in [
            "http://example.com",
            "https://user:pass@example.com",
            "https://example.com/path",
            "https://example.com?token=secret",
            "file:///tmp/test",
        ] {
            assert!(server(value).is_err(), "{value}");
        }
    }
}
