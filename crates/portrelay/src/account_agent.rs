//! Account membership manages peer trust; input and USB grants stay separate.
use super::*;
use crate::account;

impl Agent {
    pub(super) async fn account_login(&self) -> Result<serde_json::Value> {
        let _operation = self.account_operation.lock().await;
        let (key, name) = {
            let inner = self.inner.lock().await;
            if let Some(account) = &inner.config.account {
                if account.identity.is_some() {
                    bail!("This computer is already signed in");
                }
                if account.expires > storage::now() {
                    return Ok(
                        serde_json::json!({"authorization_url":account.authorization_url,"message":"Finish sign-in in your browser"}),
                    );
                }
            }
            (inner.config.key()?, inner.config.name.clone())
        };
        let server = std::env::var("PORTRELAY_ACCOUNT_SERVER")
            .unwrap_or_else(|_| account::DEFAULT_SERVER.into());
        let pending = account::Client::new(&server)?
            .enroll(key, &name, self.address())
            .await?;
        let reply = serde_json::json!({"authorization_url":pending.authorization_url,"message":"Finish sign-in in your browser"});
        let mut inner = self.inner.lock().await;
        inner.config.account = Some(pending);
        inner.account_runtime = account::Runtime::default();
        self.persist(&inner)?;
        Ok(reply)
    }
    fn remove_account_peer(inner: &mut Inner, id: &str) {
        inner.config.peers.remove(id);
        inner.config.input_controllers.retain(|p| p != id);
        for grant in inner.config.grants.values_mut() {
            grant.peers.retain(|p| p != id);
        }
        for session in inner.sessions.values().filter(|s| s.info.peer == id) {
            session.cancel.cancel();
        }
    }
    fn clear_account(inner: &mut Inner) {
        let peers: Vec<_> = inner
            .config
            .peers
            .iter()
            .filter(|(_, p)| p.account_id.is_some())
            .map(|(id, _)| id.clone())
            .collect();
        for id in peers {
            Self::remove_account_peer(inner, &id);
        }
        inner.config.account = None;
        inner.account_runtime = account::Runtime::default();
    }
    pub(super) async fn account_logout(&self) -> Result<serde_json::Value> {
        let _operation = self.account_operation.lock().await;
        let account = self.inner.lock().await.config.account.clone();
        let mut error = None;
        if let Some(account) = account {
            let client = account::Client::new(&account.server)?;
            error = if account.identity.is_some() {
                client
                    .remove(&account.token, &self.endpoint.id().to_string())
                    .await
                    .err()
            } else {
                client.cancel(&account.token).await.err()
            };
        }
        let mut inner = self.inner.lock().await;
        Self::clear_account(&mut inner);
        self.persist(&inner)?;
        Ok(
            serde_json::json!({"message":if error.is_some(){"Signed out locally. The service could not confirm removal; remove this computer from another signed-in computer."}else{"Signed out. Account connections and permissions were removed."}}),
        )
    }
    pub(super) async fn revoke_peer(&self, id: &str) -> Result<serde_json::Value> {
        let _operation = self.account_operation.lock().await;
        let account = {
            let inner = self.inner.lock().await;
            if inner
                .config
                .peers
                .get(id)
                .is_some_and(|p| p.account_id.is_some())
            {
                Some(
                    inner
                        .config
                        .account
                        .clone()
                        .context("Sign in before removing an account computer")?,
                )
            } else {
                None
            }
        };
        if let Some(account) = account {
            account::Client::new(&account.server)?
                .remove(&account.token, id)
                .await?;
        }
        let mut inner = self.inner.lock().await;
        Self::remove_account_peer(&mut inner, id);
        self.persist(&inner)?;
        Ok(serde_json::json!({"message":"Computer removed; active connections are closing"}))
    }
    pub(super) fn account_snapshot(inner: &Inner) -> serde_json::Value {
        let runtime = &inner.account_runtime;
        match &inner.config.account {
            Some(account) => {
                serde_json::json!({"status":if account.identity.is_none(){"pending"}else if !runtime.fresh(){"offline"}else{"connected"},"identity":account.identity,"server":account.server,"authorization_url":if account.identity.is_none(){Some(&account.authorization_url)}else{None},"expires":account.expires,"last_sync":runtime.last_sync,"error":runtime.error})
            }
            None => serde_json::json!({"status":"signed_out","error":runtime.error}),
        }
    }
    async fn sync_account(&self) -> Result<()> {
        let _operation = self.account_operation.lock().await;
        let (account, name) = {
            let inner = self.inner.lock().await;
            (inner.config.account.clone(), inner.config.name.clone())
        };
        let Some(mut account) = account else {
            return Ok(());
        };
        let client = account::Client::new(&account.server)?;
        if account.identity.is_none() {
            let login = client.login(&account.token).await?;
            if login.status == "pending" {
                return Ok(());
            }
            if login.status != "connected" {
                bail!("Unexpected sign-in status");
            }
            account.identity = Some(login.account.context("Account identity is missing")?);
        }
        let reply = client.sync(&account.token, &name, self.address()).await?;
        if account
            .identity
            .as_ref()
            .is_some_and(|a| a.id != reply.account.id)
        {
            bail!("Account identity changed unexpectedly");
        }
        account.identity = Some(reply.account.clone());
        let mut inner = self.inner.lock().await;
        let removed: Vec<_> = inner
            .config
            .peers
            .iter()
            .filter(|(id, p)| p.account_id.is_some() && !reply.devices.iter().any(|d| &d.id == *id))
            .map(|(id, _)| id.clone())
            .collect();
        for id in removed {
            Self::remove_account_peer(&mut inner, &id);
        }
        for device in reply.devices {
            if device.address.id == self.endpoint.id() {
                continue;
            }
            inner.config.peers.insert(
                device.id,
                Peer {
                    name: device.name,
                    address: device.address,
                    approved: true,
                    account_id: Some(reply.account.id.clone()),
                    online: Some(device.online),
                },
            );
        }
        inner.config.account = Some(account);
        inner.account_runtime.last_sync = storage::now();
        inner.account_runtime.last_check = Some(std::time::Instant::now());
        inner.account_runtime.error = None;
        self.persist(&inner)
    }
    pub(super) async fn accounts(self: Arc<Self>) {
        let mut timer = tokio::time::interval(Duration::from_secs(5));
        let mut cycle = 0u8;
        loop {
            tokio::select! {_=self.cancel.cancelled()=>break,_=timer.tick()=>{}}
            let previous = {
                let inner = self.inner.lock().await;
                if inner
                    .config
                    .account
                    .as_ref()
                    .is_some_and(|a| a.identity.is_some())
                    && !cycle.is_multiple_of(3)
                {
                    cycle = cycle.wrapping_add(1);
                    continue;
                }
                inner.config.account.as_ref().map(|a| a.token.clone())
            };
            cycle = cycle.wrapping_add(1);
            if let Err(error) = self.sync_account().await {
                let _operation = self.account_operation.lock().await;
                let mut inner = self.inner.lock().await;
                if inner.config.account.as_ref().map(|a| a.token.clone()) != previous {
                    continue;
                }
                if error.is::<account::Unauthorized>() {
                    Self::clear_account(&mut inner);
                }
                inner.account_runtime.error = Some(error.to_string());
                let _ = self.persist(&inner);
            }
        }
    }
    pub(super) async fn account_watchdog(self: Arc<Self>) {
        let mut timer = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {_=self.cancel.cancelled()=>break,_=timer.tick()=>{}}
            let mut inner = self.inner.lock().await;
            if !inner.account_runtime.fresh() {
                let mut expired = vec![];
                for (id, peer) in &mut inner.config.peers {
                    if peer.account_id.is_some() && peer.approved {
                        peer.approved = false;
                        expired.push(id.clone());
                    }
                }
                for session in inner.sessions.values() {
                    if expired.contains(&session.info.peer) {
                        session.cancel.cancel();
                    }
                }
            }
        }
    }
}
