use crate::{
    backend,
    protocol::*,
    storage::{self, Config, Grant, Peer},
};
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use iroh::{
    Endpoint, EndpointAddr, RelayMode,
    endpoint::{Connection, QuicTransportConfig, presets},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{Mutex, Semaphore},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub device: String,
    pub peer: String,
    pub direction: String,
    pub state: String,
    pub error: Option<String>,
    pub metadata: Option<Device>,
}
struct Session {
    info: SessionInfo,
    cancel: CancellationToken,
    exported_device: Option<Device>,
}
struct PendingInvite {
    secret: String,
    expires: u64,
}
struct Inner {
    config: Config,
    invitations: Vec<PendingInvite>,
    pair_attempts: VecDeque<std::time::Instant>,
    sessions: BTreeMap<String, Session>,
    history: VecDeque<SessionInfo>,
}
pub struct Agent {
    pub endpoint: Endpoint,
    pub cancel: CancellationToken,
    pub helper: PathBuf,
    pub token: String,
    pub network_mode: String,
    dir: PathBuf,
    inner: Mutex<Inner>,
    limits: Arc<Semaphore>,
    setup: crate::setup::Setup,
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Action {
    SetupUsb,
    BluetoothSettings,
    Rename {
        name: String,
    },
    Invite,
    Pair {
        invitation: String,
    },
    Approve {
        peer: String,
    },
    Revoke {
        peer: String,
    },
    Share {
        device: String,
        peer: String,
        #[serde(default)]
        acknowledge_bluetooth: bool,
        #[serde(default)]
        acknowledge_risks: Vec<String>,
    },
    Unshare {
        device: String,
    },
    Remote {
        peer: String,
    },
    Connect {
        peer: String,
        device: String,
        generation: String,
    },
    Disconnect {
        session: String,
    },
}
impl Agent {
    pub async fn start(
        dir: PathBuf,
        name: Option<String>,
        helper: PathBuf,
        bind: String,
        relay: Option<String>,
        relay_only: bool,
    ) -> Result<Arc<Self>> {
        let config = Config::load(&dir, name)?;
        let transport = QuicTransportConfig::builder()
            .max_concurrent_bidi_streams(1u32.into())
            .max_concurrent_uni_streams(0u32.into())
            .keep_alive_interval(Duration::from_secs(5))
            .max_idle_timeout(Some(Duration::from_secs(20).try_into()?))
            .build();
        let relay_mode = match &relay {
            Some(url) => RelayMode::custom([url.parse()?]),
            None => RelayMode::Disabled,
        };
        if relay_only && relay.is_none() {
            bail!("Relay-only mode requires a relay URL");
        }
        let builder = Endpoint::builder(presets::Minimal)
            .secret_key(config.key()?)
            .alpns(vec![ALPN.to_vec()])
            .relay_mode(relay_mode)
            .transport_config(transport)
            .max_tls_tickets(0)
            .clear_ip_transports();
        let builder = if relay_only {
            builder
        } else {
            builder.bind_addr(bind)?
        };
        let endpoint = builder.bind().await?;
        let agent = Arc::new(Self {
            endpoint,
            cancel: CancellationToken::new(),
            helper,
            token: storage::random_secret(),
            network_mode: if relay_only {
                "Relay only · encrypted"
            } else if relay.is_some() {
                "Custom relay enabled"
            } else {
                "Direct connections · no public services"
            }
            .into(),
            dir,
            inner: Mutex::new(Inner {
                config,
                invitations: vec![],
                pair_attempts: VecDeque::new(),
                sessions: BTreeMap::new(),
                history: VecDeque::new(),
            }),
            limits: Arc::new(Semaphore::new(32)),
            setup: crate::setup::Setup::default(),
        });
        let a = agent.clone();
        tokio::spawn(async move {
            a.accept().await;
        });
        Ok(agent)
    }
    fn persist(&self, inner: &Inner) -> Result<()> {
        storage::save(&self.dir.join("config.json"), &inner.config)
    }
    pub fn address(&self) -> EndpointAddr {
        self.endpoint.addr()
    }
    fn devices(inner: &Inner, devices: Vec<Device>) -> Vec<Device> {
        devices
            .into_iter()
            .map(|device| {
                inner
                    .sessions
                    .values()
                    .filter(|s| s.info.direction == "outgoing")
                    .filter_map(|s| s.exported_device.as_ref())
                    .find(|d| d.id == device.id && d.generation == device.generation)
                    .cloned()
                    .unwrap_or(device)
            })
            .collect()
    }
    pub async fn snapshot(&self) -> serde_json::Value {
        let health = backend::health(&self.helper).await;
        let setup = self.setup.snapshot().await;
        let devices = backend::devices(&self.helper).await;
        let inner = self.inner.lock().await;
        serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"id":self.endpoint.id().to_string(),"name":inner.config.name,"network":self.network_mode,"address":self.address(),"platform":std::env::consts::OS,"setup_available":crate::setup::Setup::available(),"setup":setup,"helper_ready":health.is_ok(),"helper_error":health.err().map(|e|e.to_string()),"devices":Self::devices(&inner, devices),"peers":inner.config.peers,"grants":inner.config.grants,"sessions":inner.sessions.values().map(|s|s.info.clone()).collect::<Vec<_>>(),"history":inner.history})
    }
    pub async fn action(self: &Arc<Self>, action: Action) -> Result<serde_json::Value> {
        match action {
            Action::SetupUsb => {
                self.setup.start().await?;
                Ok(
                    serde_json::json!({"message":"Approve the system permission window to enable USB sharing."}),
                )
            }
            Action::Rename { name } => {
                let name = name.trim();
                if name.is_empty() || name.len() > 80 {
                    bail!("Choose a shorter computer name.");
                }
                let mut inner = self.inner.lock().await;
                inner.config.name = name.to_owned();
                self.persist(&inner)?;
                Ok(
                    serde_json::json!({"message":"Computer name saved. New invitations will use this name."}),
                )
            }
            Action::Invite => {
                if self.address().addrs.is_empty() {
                    bail!(
                        "The network is still starting. Try creating the invitation again in a moment."
                    );
                }
                let mut inner = self.inner.lock().await;
                inner.invitations.retain(|x| x.expires > storage::now());
                if inner.invitations.len() >= 8 {
                    bail!("Eight invitations are already active; wait for expiry");
                }
                let secret = storage::random_secret();
                let expires = storage::now() + 600;
                let ticket = Invitation {
                    version: VERSION,
                    address: self.address(),
                    secret: secret.clone(),
                    name: inner.config.name.clone(),
                    expires,
                };
                inner.invitations.push(PendingInvite { secret, expires });
                Ok(
                    serde_json::json!({"invitation":format!("portrelay1:{}",URL_SAFE_NO_PAD.encode(serde_json::to_vec(&ticket)?)),"expires":expires}),
                )
            }
            Action::Pair { invitation } => {
                if invitation.len() > 8192 {
                    bail!("Invitation is too long");
                }
                let raw = invitation
                    .trim()
                    .strip_prefix("portrelay1:")
                    .context("Paste a PortRelay invitation")?;
                let invite: Invitation = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(raw)?)?;
                if invite.version != VERSION
                    || invite.expires <= storage::now()
                    || invite.secret.len() != 64
                {
                    bail!("Invitation has expired or uses an unsupported version");
                }
                if invite.address.id == self.endpoint.id() {
                    bail!("This invitation belongs to this computer");
                }
                let name = self.inner.lock().await.config.name.clone();
                let reply = self
                    .request(
                        invite.address.clone(),
                        Request::Pair {
                            secret: invite.secret,
                            name,
                            address: self.address(),
                        },
                    )
                    .await?;
                match reply {
                    Reply::Pending { name } => {
                        let mut inner = self.inner.lock().await;
                        if inner.config.peers.len() >= 64
                            && !inner
                                .config
                                .peers
                                .contains_key(&invite.address.id.to_string())
                        {
                            bail!("Peer limit reached");
                        }
                        inner.config.peers.insert(
                            invite.address.id.to_string(),
                            Peer {
                                name,
                                address: invite.address,
                                approved: true,
                            },
                        );
                        self.persist(&inner)?;
                        Ok(
                            serde_json::json!({"message":"Pairing requested. Approve this computer on the owner’s screen."}),
                        )
                    }
                    Reply::Error { message } => bail!("{message}"),
                    _ => bail!("Unexpected pairing response"),
                }
            }
            Action::Approve { peer } => {
                let mut inner = self.inner.lock().await;
                inner
                    .config
                    .peers
                    .get_mut(&peer)
                    .context("Unknown computer")?
                    .approved = true;
                self.persist(&inner)?;
                Ok(serde_json::json!({"message":"Computer approved"}))
            }
            Action::Revoke { peer } => {
                let mut inner = self.inner.lock().await;
                inner.config.peers.remove(&peer);
                for g in inner.config.grants.values_mut() {
                    g.peers.retain(|x| x != &peer);
                }
                for s in inner.sessions.values().filter(|s| s.info.peer == peer) {
                    s.cancel.cancel();
                }
                self.persist(&inner)?;
                Ok(serde_json::json!({"message":"Trust revoked; active sessions are closing"}))
            }
            Action::BluetoothSettings => {
                crate::settings::open_bluetooth()?;
                Ok(
                    serde_json::json!({"message":"Bluetooth settings opened. Select the borrowed adapter and pair a nearby device."}),
                )
            }
            Action::Share {
                device,
                peer,
                acknowledge_bluetooth,
                acknowledge_risks,
            } => {
                if !backend::available(&self.helper) {
                    bail!("Enable USB support before sharing");
                }
                let d = backend::devices(&self.helper)
                    .await
                    .into_iter()
                    .find(|d| d.id == device)
                    .context("Device was unplugged")?;
                if let Some(reason) = &d.blocked {
                    bail!("{reason}");
                }
                if d.kind == "bluetooth"
                    && !acknowledge_bluetooth
                    && !acknowledge_risks.iter().any(|r| r == "bluetooth")
                {
                    bail!(
                        "Confirm the Bluetooth adapter handoff warning before sharing this device"
                    );
                }
                for risk in &d.risks {
                    if !acknowledge_risks.contains(risk)
                        && !(risk == "bluetooth" && acknowledge_bluetooth)
                    {
                        bail!("Confirm the {risk} handoff warning before sharing this device");
                    }
                }
                let mut inner = self.inner.lock().await;
                if !inner.config.peers.get(&peer).is_some_and(|p| p.approved) {
                    bail!("Approve the computer first");
                }
                let grant = inner.config.grants.entry(device).or_insert(Grant {
                    generation: d.generation.clone(),
                    peers: vec![],
                });
                if grant.generation != d.generation {
                    grant.generation = d.generation;
                    grant.peers.clear();
                }
                if !grant.peers.contains(&peer) {
                    grant.peers.push(peer);
                }
                self.persist(&inner)?;
                Ok(serde_json::json!({"message":"Device shared with the selected computer"}))
            }
            Action::Unshare { device } => {
                let mut inner = self.inner.lock().await;
                inner.config.grants.remove(&device);
                for s in inner
                    .sessions
                    .values()
                    .filter(|s| s.info.direction == "outgoing" && s.info.device == device)
                {
                    s.cancel.cancel();
                }
                self.persist(&inner)?;
                Ok(serde_json::json!({"message":"Sharing stopped; any active loan is closing"}))
            }
            Action::Remote { peer } => {
                let address = self.peer(&peer).await?;
                match self.request(address, Request::List).await? {
                    Reply::Devices { devices } => Ok(serde_json::json!({"devices":devices})),
                    Reply::Error { message } => bail!("{message}"),
                    _ => bail!("Unexpected device response"),
                }
            }
            Action::Connect {
                peer,
                device,
                generation,
            } => self.connect_device(peer, device, generation).await,
            Action::Disconnect { session } => {
                let inner = self.inner.lock().await;
                inner
                    .sessions
                    .get(&session)
                    .context("Session already ended")?
                    .cancel
                    .cancel();
                Ok(serde_json::json!({"message":"Disconnecting and restoring the device"}))
            }
        }
    }
    async fn peer(&self, id: &str) -> Result<EndpointAddr> {
        let inner = self.inner.lock().await;
        let peer = inner
            .config
            .peers
            .get(id)
            .filter(|p| p.approved)
            .context("Computer is not trusted")?;
        Ok(peer.address.clone())
    }
    async fn request(&self, address: EndpointAddr, request: Request) -> Result<Reply> {
        let conn = tokio::time::timeout(
            Duration::from_secs(15),
            self.endpoint.connect(address, ALPN),
        )
        .await
        .context("Computer is offline or the network blocks the connection")??;
        let result = tokio::time::timeout(Duration::from_secs(15), async {
            let (mut send, mut recv) = conn.open_bi().await?;
            write_frame(
                &mut send,
                &Envelope {
                    version: VERSION,
                    request,
                },
            )
            .await?;
            let reply = read_frame(&mut recv).await?;
            Ok::<_, anyhow::Error>(reply)
        })
        .await;
        conn.close(0u32.into(), b"request complete");
        result?
    }
    async fn accept(self: Arc<Self>) {
        loop {
            let incoming =
                tokio::select! {_=self.cancel.cancelled()=>break,inc=self.endpoint.accept()=>inc};
            let Some(incoming) = incoming else {
                break;
            };
            let Ok(permit) = self.limits.clone().try_acquire_owned() else {
                incoming.refuse();
                continue;
            };
            let agent = self.clone();
            tokio::spawn(async move {
                let _permit = permit;
                let Ok(Ok(conn)) = tokio::time::timeout(Duration::from_secs(10), incoming).await
                else {
                    return;
                };
                if agent.serve_connection(&conn).await.is_err() {
                    conn.close(1u32.into(), b"request rejected");
                }
            });
        }
    }
    async fn serve_connection(&self, conn: &Connection) -> Result<()> {
        let (mut send, mut recv) =
            tokio::time::timeout(Duration::from_secs(8), conn.accept_bi()).await??;
        let envelope: Envelope =
            tokio::time::timeout(Duration::from_secs(8), read_frame(&mut recv)).await??;
        let remote = conn.remote_id().to_string();
        if envelope.version != VERSION {
            write_frame(
                &mut send,
                &Reply::Error {
                    message: "Unsupported protocol version".into(),
                },
            )
            .await?;
            return Ok(());
        }
        if let Request::Open { device, generation } = &envelope.request {
            let preparation = self.reserve_export(&remote, device, generation).await;
            match preparation {
                Err(e) => {
                    write_frame(
                        &mut send,
                        &Reply::Error {
                            message: e.to_string(),
                        },
                    )
                    .await?
                }
                Ok((session, cancel)) => {
                    let mut ready = false;
                    let result=async {
                        #[cfg(any(unix, windows))] {
                            let (mut ipc,reply)=backend::open(&self.helper,&backend::HelperRequest::Export{device:device.clone(),generation:generation.clone()}).await?;
                            if cancel.is_cancelled() {bail!("Permission was revoked");}
                            let device=reply.device.context("Helper did not return device metadata")?;
                            if let Some(s)=self.inner.lock().await.sessions.get_mut(&session){s.exported_device=Some(device.clone());s.info.metadata=Some(device.clone());}
                            write_frame(&mut send,&Reply::Ready{device}).await?;
                            ready=true;
                            self.mark_connected(&session).await;
                            tokio::select!{_=cancel.cancelled()=>Ok(()),_=self.cancel.cancelled()=>Ok(()),r=bridge(&mut ipc,&mut recv,&mut send)=>r}
                        }
                        #[cfg(not(any(unix, windows)))] {bail!("Device sharing is not implemented on this platform");}
                    }.await;
                    if !ready && let Err(e) = &result {
                        let _ = write_frame(
                            &mut send,
                            &Reply::Error {
                                message: e.to_string(),
                            },
                        )
                        .await;
                        let _ = send.finish();
                        let _ = tokio::time::timeout(Duration::from_secs(2), conn.closed()).await;
                    }
                    let mut error = result.err().and_then(|e| {
                        if ready {
                            session_error(e)
                        } else {
                            Some(e.to_string())
                        }
                    });
                    conn.close(
                        if error.is_some() { 1u32 } else { 0u32 }.into(),
                        b"device session ended",
                    );
                    if let Some(s) = self.inner.lock().await.sessions.get_mut(&session) {
                        s.info.state = "restoring".into();
                    }
                    #[cfg(any(unix, windows))]
                    if ready
                        && let Err(e) = backend::open(
                            &self.helper,
                            &backend::HelperRequest::WaitExport {
                                device: device.clone(),
                            },
                        )
                        .await
                    {
                        error = Some(format!("Device restoration: {e}"));
                    }
                    self.finish(&session, error).await;
                    return Ok(());
                }
            }
        } else {
            let reply = self
                .control(&remote, envelope.request)
                .await
                .unwrap_or_else(|e| Reply::Error {
                    message: e.to_string(),
                });
            write_frame(&mut send, &reply).await?;
        }
        send.finish()?;
        let _ = tokio::time::timeout(Duration::from_secs(2), conn.closed()).await;
        conn.close(0u32.into(), b"request complete");
        Ok(())
    }
    async fn control(&self, remote: &str, request: Request) -> Result<Reply> {
        let devices = if matches!(request, Request::List) {
            if !self
                .inner
                .lock()
                .await
                .config
                .peers
                .get(remote)
                .is_some_and(|p| p.approved)
            {
                bail!("Waiting for approval on the device owner’s computer");
            }
            backend::devices(&self.helper).await
        } else {
            vec![]
        };
        let mut inner = self.inner.lock().await;
        if let Request::Pair {
            secret,
            name,
            address,
        } = request
        {
            inner
                .pair_attempts
                .retain(|t| t.elapsed() < Duration::from_secs(60));
            if inner.pair_attempts.len() >= 30 {
                bail!("Too many pairing attempts; try again in one minute");
            }
            inner.pair_attempts.push_back(std::time::Instant::now());
            if address.id.to_string() != remote
                || serde_json::to_vec(&address)?.len() > 4096
                || name.trim().is_empty()
                || name.len() > 80
                || secret.len() != 64
            {
                bail!("Invalid pairing request");
            }
            inner.invitations.retain(|x| x.expires > storage::now());
            let index = inner
                .invitations
                .iter()
                .position(|x| bool::from(x.secret.as_bytes().ct_eq(secret.as_bytes())))
                .context("Invitation expired or already used")?;
            if inner.config.peers.len() >= 64 && !inner.config.peers.contains_key(remote) {
                bail!("Peer limit reached");
            }
            inner.invitations.remove(index);
            // Re-pairing never silently restores revoked sharing grants.
            inner.config.peers.insert(
                remote.into(),
                Peer {
                    name,
                    address,
                    approved: false,
                },
            );
            for g in inner.config.grants.values_mut() {
                g.peers.retain(|p| p != remote);
            }
            for s in inner.sessions.values().filter(|s| s.info.peer == remote) {
                s.cancel.cancel();
            }
            self.persist(&inner)?;
            return Ok(Reply::Pending {
                name: inner.config.name.clone(),
            });
        }
        if !inner.config.peers.get(remote).is_some_and(|p| p.approved) {
            bail!("Waiting for approval on the device owner’s computer");
        }
        match request {
            Request::List => {
                let devices = Self::devices(&inner, devices)
                    .into_iter()
                    .filter(|d| {
                        d.blocked.is_none()
                            && inner.config.grants.get(&d.id).is_some_and(|g| {
                                g.generation == d.generation && g.peers.iter().any(|p| p == remote)
                            })
                    })
                    .map(|device| {
                        let busy = inner
                            .sessions
                            .values()
                            .any(|s| s.info.direction == "outgoing" && s.info.device == device.id);
                        RemoteDevice { device, busy }
                    })
                    .collect();
                Ok(Reply::Devices { devices })
            }
            _ => bail!("Invalid control operation"),
        }
    }
    async fn reserve_export(
        &self,
        peer: &str,
        device: &str,
        generation: &str,
    ) -> Result<(String, CancellationToken)> {
        let mut inner = self.inner.lock().await;
        if !inner.config.peers.get(peer).is_some_and(|p| p.approved) {
            bail!("Computer is not authorized");
        }
        if !inner
            .config
            .grants
            .get(device)
            .is_some_and(|g| g.generation == generation && g.peers.iter().any(|p| p == peer))
        {
            bail!("Device is not shared with this computer");
        }
        Self::reserve(&mut inner, peer, device, "outgoing")
    }
    fn reserve(
        inner: &mut Inner,
        peer: &str,
        device: &str,
        direction: &str,
    ) -> Result<(String, CancellationToken)> {
        if inner.sessions.len() >= 16 {
            bail!("Device session limit reached");
        }
        if inner.sessions.values().any(|s| {
            s.info.device == device
                && s.info.direction == direction
                && (direction == "outgoing" || s.info.peer == peer)
        }) {
            bail!("Device is already in use");
        }
        let id = storage::random_secret()[..24].to_owned();
        let cancel = CancellationToken::new();
        inner.sessions.insert(
            id.clone(),
            Session {
                info: SessionInfo {
                    id: id.clone(),
                    device: device.into(),
                    peer: peer.into(),
                    direction: direction.into(),
                    state: "connecting".into(),
                    error: None,
                    metadata: None,
                },
                cancel: cancel.clone(),
                exported_device: None,
            },
        );
        Ok((id, cancel))
    }
    async fn mark_connected(&self, id: &str) {
        if let Some(s) = self.inner.lock().await.sessions.get_mut(id) {
            s.info.state = "connected".into();
        }
    }
    async fn finish(&self, id: &str, error: Option<String>) {
        let mut inner = self.inner.lock().await;
        if let Some(mut s) = inner.sessions.remove(id) {
            s.info.state = if error.is_some() { "error" } else { "ended" }.into();
            s.info.error = error;
            inner.history.push_front(s.info);
            inner.history.truncate(20);
        }
    }
    async fn connect_device(
        self: &Arc<Self>,
        peer: String,
        device: String,
        generation: String,
    ) -> Result<serde_json::Value> {
        if !backend::available(&self.helper) {
            bail!(
                "Enable USB support to connect real devices. This platform may only manage peers."
            );
        }
        let (address, session, cancel) = {
            let mut inner = self.inner.lock().await;
            let address = inner
                .config
                .peers
                .get(&peer)
                .filter(|p| p.approved)
                .context("Computer is not trusted")?
                .address
                .clone();
            let (session, cancel) = Self::reserve(&mut inner, &peer, &device, "incoming")?;
            (address, session, cancel)
        };
        let result=async {
            let conn=tokio::time::timeout(Duration::from_secs(15),self.endpoint.connect(address,ALPN)).await??;
            let(mut send,mut recv)=conn.open_bi().await?;
            write_frame(&mut send,&Envelope{version:VERSION,request:Request::Open{device,generation}}).await?;
            let reply:Reply=tokio::time::timeout(Duration::from_secs(25),read_frame(&mut recv)).await??;
            let metadata=match reply {Reply::Ready{device}=>device,Reply::Error{message}=>bail!("{message}"),_=>bail!("Unexpected device response")};
            #[cfg(any(unix, windows))] {
                if cancel.is_cancelled(){bail!("Connection cancelled");}
                let(mut ipc,local)=backend::open(&self.helper,&backend::HelperRequest::Import{device:metadata.clone()}).await?;
                if let Some(s)=self.inner.lock().await.sessions.get_mut(&session){s.info.metadata=Some(metadata);}
                if cancel.is_cancelled(){bail!("Connection cancelled");}
                self.mark_connected(&session).await;
                let agent=self.clone();let task_session=session.clone();
                tokio::spawn(async move {
                    let result=tokio::select!{_=cancel.cancelled()=>Ok(()),_=agent.cancel.cancelled()=>Ok(()),r=bridge(&mut ipc,&mut recv,&mut send)=>r};
                    drop(ipc);
                    let mut error=result.err().and_then(session_error);
                    conn.close(if error.is_some(){1u32}else{0u32}.into(),b"device session ended");
                    if let Some(s)=agent.inner.lock().await.sessions.get_mut(&task_session){s.info.state="detaching".into();}
                    if let Some(port)=local.port
                        && let Err(e)=backend::open(&agent.helper,&backend::HelperRequest::WaitImport{port}).await {error=Some(format!("Device detachment: {e}"));}
                    agent.finish(&task_session,error).await;
                });
                Ok(serde_json::json!({"session":session,"port":local.port,"message":"USB transport attached. The operating system is enumerating the device."}))
            }
            #[cfg(not(any(unix, windows)))] {let _=metadata;bail!("Device attachment is not implemented on this platform");}
        }.await;
        if let Err(e) = &result {
            self.finish(&session, Some(e.to_string())).await;
        }
        result
    }
    pub async fn shutdown(&self) {
        self.cancel.cancel();
        self.endpoint.close().await;
        for _ in 0..500 {
            if self.inner.lock().await.sessions.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
fn session_error(error: anyhow::Error) -> Option<String> {
    // QUIC surfaces an intentional peer disconnect as an I/O "connection lost"
    // error. Preserve real network/helper failures, but recognize our explicit
    // successful device-session close, including through io::Error wrappers.
    let normal = error.chain().any(|cause| matches!(
        cause.downcast_ref::<iroh::endpoint::ConnectionError>(),
        Some(iroh::endpoint::ConnectionError::ApplicationClosed(close))
            if close.error_code == 0u32.into() && close.reason.as_ref() == b"device session ended"
    ));
    if normal {
        None
    } else {
        Some(error.to_string())
    }
}

async fn bridge<S: AsyncRead + AsyncWrite + Unpin, R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    socket: &mut S,
    recv: &mut R,
    send: &mut W,
) -> Result<()> {
    let (mut read, mut write) = tokio::io::split(socket);
    // USB/IP has no useful half-closed session. Either EOF tears down both directions.
    tokio::select! {r=tokio::io::copy(recv,&mut write)=>{r?;},r=tokio::io::copy(&mut read,send)=>{r?;}}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intentional_peer_close_is_distinct_from_transport_and_helper_failure() {
        use iroh::endpoint::{ApplicationClose, ConnectionError, ReadError};
        for (code, reason, normal) in [
            (0u32, "device session ended", true),
            (1, "device session ended", false),
            (0, "request complete", false),
        ] {
            let transport =
                ReadError::ConnectionLost(ConnectionError::ApplicationClosed(ApplicationClose {
                    error_code: code.into(),
                    reason: reason.as_bytes().to_vec().into(),
                }));
            let io: std::io::Error = transport.into();
            assert_eq!(session_error(io.into()).is_none(), normal);
        }
        let timeout: std::io::Error = ReadError::ConnectionLost(ConnectionError::TimedOut).into();
        assert!(session_error(timeout.into()).is_some());
        assert!(
            session_error(
                std::io::Error::new(std::io::ErrorKind::BrokenPipe, "helper stopped").into()
            )
            .is_some()
        );
    }
    async fn make(dir: &std::path::Path, name: &str) -> Arc<Agent> {
        let path = storage::state_dir(Some(dir.into())).unwrap();
        Agent::start(
            path,
            Some(name.into()),
            dir.join("helper.sock"),
            "127.0.0.1:0".into(),
            None,
            false,
        )
        .await
        .unwrap()
    }
    async fn pair(owner: &Arc<Agent>, client: &Arc<Agent>) -> String {
        let value = owner.action(Action::Invite).await.unwrap();
        let ticket = value["invitation"].as_str().unwrap().to_owned();
        client
            .action(Action::Pair {
                invitation: ticket.clone(),
            })
            .await
            .unwrap();
        ticket
    }
    #[tokio::test]
    async fn computer_name_survives_restart_and_invalid_names_preserve_it() {
        let directory = tempfile::tempdir().unwrap();
        let agent = make(directory.path(), "Initial name").await;
        agent
            .action(Action::Rename {
                name: "  Studio PC  ".into(),
            })
            .await
            .unwrap();
        for name in ["   ".to_string(), "x".repeat(81)] {
            assert!(agent.action(Action::Rename { name }).await.is_err());
        }
        let identity = agent.endpoint.id();
        assert_eq!(agent.snapshot().await["name"], "Studio PC");
        agent.shutdown().await;
        drop(agent);
        let restarted = Agent::start(
            directory.path().into(),
            None,
            directory.path().join("helper.sock"),
            "127.0.0.1:0".into(),
            None,
            false,
        )
        .await
        .unwrap();
        assert_eq!(restarted.snapshot().await["name"], "Studio PC");
        assert_eq!(restarted.endpoint.id(), identity);
        restarted.shutdown().await;
    }
    #[tokio::test]
    async fn real_quic_pairing_requires_approval_and_single_use_invites() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let owner = make(a.path(), "Owner").await;
        let client = make(b.path(), "Client").await;
        assert!(matches!(
            client
                .request(owner.address(), Request::List)
                .await
                .unwrap(),
            Reply::Error { .. }
        ));
        let ticket = pair(&owner, &client).await;
        assert!(
            client
                .action(Action::Pair { invitation: ticket })
                .await
                .is_err()
        );
        assert!(
            client
                .action(Action::Remote {
                    peer: owner.endpoint.id().to_string()
                })
                .await
                .is_err()
        );
        owner
            .action(Action::Approve {
                peer: client.endpoint.id().to_string(),
            })
            .await
            .unwrap();
        assert_eq!(
            client
                .action(Action::Remote {
                    peer: owner.endpoint.id().to_string()
                })
                .await
                .unwrap()["devices"],
            serde_json::json!([])
        );
        owner
            .action(Action::Revoke {
                peer: client.endpoint.id().to_string(),
            })
            .await
            .unwrap();
        assert!(
            client
                .action(Action::Remote {
                    peer: owner.endpoint.id().to_string()
                })
                .await
                .is_err()
        );
        client.shutdown().await;
        owner.shutdown().await;
    }
    #[tokio::test]
    async fn grants_are_exclusive_generation_bound_and_revocable() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let owner = make(a.path(), "Owner").await;
        let client = make(b.path(), "Client").await;
        pair(&owner, &client).await;
        let id = client.endpoint.id().to_string();
        owner
            .action(Action::Approve { peer: id.clone() })
            .await
            .unwrap();
        owner.inner.lock().await.config.grants.insert(
            "1-1".into(),
            Grant {
                generation: "first".into(),
                peers: vec![id.clone()],
            },
        );
        assert!(
            owner
                .reserve_export(&id, "1-1", "replacement")
                .await
                .is_err()
        );
        assert!(
            owner
                .reserve_export("stranger", "1-1", "first")
                .await
                .is_err()
        );
        let (session, cancel) = owner.reserve_export(&id, "1-1", "first").await.unwrap();
        assert!(owner.reserve_export(&id, "1-1", "first").await.is_err());
        owner
            .action(Action::Revoke { peer: id.clone() })
            .await
            .unwrap();
        assert!(cancel.is_cancelled());
        assert!(owner.reserve_export(&id, "1-1", "first").await.is_err());
        owner.finish(&session, None).await;
        client.shutdown().await;
        owner.shutdown().await;
    }
    #[tokio::test]
    async fn expired_invitation_cannot_pair() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let owner = make(a.path(), "Owner").await;
        let client = make(b.path(), "Client").await;
        let secret = storage::random_secret();
        owner.inner.lock().await.invitations.push(PendingInvite {
            secret: secret.clone(),
            expires: storage::now() - 1,
        });
        assert!(
            owner
                .control(
                    &client.endpoint.id().to_string(),
                    Request::Pair {
                        secret,
                        name: "Client".into(),
                        address: client.address()
                    }
                )
                .await
                .is_err()
        );
        assert!(owner.inner.lock().await.config.peers.is_empty());
        client.shutdown().await;
        owner.shutdown().await;
    }
    #[tokio::test]
    async fn local_api_requires_token_and_rejects_foreign_origin() {
        let a = tempfile::tempdir().unwrap();
        let agent = make(a.path(), "Owner").await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server =
            tokio::spawn(axum::serve(listener, crate::api::router(agent.clone())).into_future());
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let url = format!("http://{address}/api/state");
        assert_eq!(http.get(&url).send().await.unwrap().status(), 401);
        assert_eq!(
            http.get(&url)
                .bearer_auth(&agent.token)
                .header("Origin", "https://evil.example")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        let response = http
            .get(&url)
            .bearer_auth(&agent.token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body = response.text().await.unwrap();
        assert!(!body.contains("secret_key"));
        assert!(!body.contains(&agent.token));
        server.abort();
        agent.shutdown().await;
    }
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn encrypted_device_stream_and_disconnect_release_both_leases() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::UnixListener,
        };
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let owner = make(a.path(), "Owner").await;
        let client = make(b.path(), "Client").await;
        pair(&owner, &client).await;
        let peer = client.endpoint.id().to_string();
        owner
            .action(Action::Approve { peer: peer.clone() })
            .await
            .unwrap();
        owner.inner.lock().await.config.grants.insert(
            "1-1".into(),
            Grant {
                generation: "test".into(),
                peers: vec![peer],
            },
        );
        let export = UnixListener::bind(&owner.helper).unwrap();
        let import = UnixListener::bind(&client.helper).unwrap();
        let descriptor = Device {
            id: "1-1".into(),
            generation: "test".into(),
            name: "Test fixture (not hardware)".into(),
            vendor: "1234".into(),
            product: "5678".into(),
            kind: "usb".into(),
            speed: 3,
            devid: 65537,
            blocked: None,
            risks: vec![],
            parent_hub: None,
        };
        let export_task = tokio::spawn(async move {
            let (mut s, _) = export.accept().await.unwrap();
            let _: backend::HelperRequest = read_frame(&mut s).await.unwrap();
            write_frame(
                &mut s,
                &backend::HelperReply {
                    error: None,
                    device: Some(descriptor),
                    port: None,
                },
            )
            .await
            .unwrap();
            let (mut r, mut w) = s.split();
            tokio::io::copy(&mut r, &mut w).await.unwrap();
            drop(s);
            let (mut wait, _) = export.accept().await.unwrap();
            assert!(matches!(
                read_frame::<_, backend::HelperRequest>(&mut wait)
                    .await
                    .unwrap(),
                backend::HelperRequest::WaitExport { .. }
            ));
            write_frame(
                &mut wait,
                &backend::HelperReply {
                    error: None,
                    device: None,
                    port: None,
                },
            )
            .await
            .unwrap();
        });
        let (tx, rx) = tokio::sync::oneshot::channel();
        let import_task = tokio::spawn(async move {
            let (mut s, _) = import.accept().await.unwrap();
            let _: backend::HelperRequest = read_frame(&mut s).await.unwrap();
            write_frame(
                &mut s,
                &backend::HelperReply {
                    error: None,
                    device: None,
                    port: Some(0),
                },
            )
            .await
            .unwrap();
            let bytes = vec![0x5a; 131072];
            s.write_all(&bytes).await.unwrap();
            let mut received = vec![0; bytes.len()];
            s.read_exact(&mut received).await.unwrap();
            tx.send(received == bytes).unwrap();
            let mut tail = [0; 1];
            assert_eq!(s.read(&mut tail).await.unwrap(), 0);
            drop(s);
            let (mut wait, _) = import.accept().await.unwrap();
            assert!(matches!(
                read_frame::<_, backend::HelperRequest>(&mut wait)
                    .await
                    .unwrap(),
                backend::HelperRequest::WaitImport { port: 0 }
            ));
            write_frame(
                &mut wait,
                &backend::HelperReply {
                    error: None,
                    device: None,
                    port: None,
                },
            )
            .await
            .unwrap();
        });
        let result = client
            .action(Action::Connect {
                peer: owner.endpoint.id().to_string(),
                device: "1-1".into(),
                generation: "test".into(),
            })
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(10), rx)
                .await
                .unwrap()
                .unwrap()
        );
        client
            .action(Action::Disconnect {
                session: result["session"].as_str().unwrap().into(),
            })
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            while !owner.inner.lock().await.sessions.is_empty()
                || !client.inner.lock().await.sessions.is_empty()
            {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        import_task.await.unwrap();
        export_task.await.unwrap();
        assert!(
            owner
                .inner
                .lock()
                .await
                .history
                .iter()
                .all(|s| s.error.is_none())
        );
        assert!(
            client
                .inner
                .lock()
                .await
                .history
                .iter()
                .all(|s| s.error.is_none())
        );
        client.shutdown().await;
        owner.shutdown().await;
    }
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn backend_failure_is_reported_and_releases_reservation() {
        use tokio::net::UnixListener;
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let owner = make(a.path(), "Owner").await;
        let client = make(b.path(), "Client").await;
        pair(&owner, &client).await;
        let id = client.endpoint.id().to_string();
        owner
            .action(Action::Approve { peer: id.clone() })
            .await
            .unwrap();
        owner.inner.lock().await.config.grants.insert(
            "1-1".into(),
            Grant {
                generation: "test".into(),
                peers: vec![id],
            },
        );
        let export = UnixListener::bind(&owner.helper).unwrap();
        let _import = UnixListener::bind(&client.helper).unwrap();
        let helper = tokio::spawn(async move {
            let (mut stream, _) = export.accept().await.unwrap();
            let _: backend::HelperRequest = read_frame(&mut stream).await.unwrap();
            write_frame(
                &mut stream,
                &backend::HelperReply {
                    error: Some("Intentional backend failure".into()),
                    device: None,
                    port: None,
                },
            )
            .await
            .unwrap();
        });
        let error = client
            .action(Action::Connect {
                peer: owner.endpoint.id().to_string(),
                device: "1-1".into(),
                generation: "test".into(),
            })
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("Intentional backend failure"),
            "{error}"
        );
        tokio::time::timeout(Duration::from_secs(4), async {
            while !owner.inner.lock().await.sessions.is_empty() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(client.inner.lock().await.sessions.is_empty());
        helper.await.unwrap();
        client.shutdown().await;
        owner.shutdown().await;
    }
}
