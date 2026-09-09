use super::*;
use crate::input::{self, Batch, DEADLINE, Response};
use tokio::sync::{mpsc, oneshot};

pub(super) type QueuedInput = (Batch, oneshot::Sender<Result<(), String>>);

impl Agent {
    pub(super) async fn allow_input(
        &self,
        peer: String,
        allowed: bool,
    ) -> Result<serde_json::Value> {
        if allowed {
            input::health()
                .await
                .context("Enable keyboard and mouse support on this Linux desktop first")?;
        }
        let mut inner = self.inner.lock().await;
        if !inner.config.peers.get(&peer).is_some_and(|p| p.approved) {
            bail!("Approve the computer first");
        }
        inner.config.input_controllers.retain(|p| p != &peer);
        if allowed {
            inner.config.input_controllers.push(peer.clone());
        } else {
            for s in inner
                .sessions
                .values()
                .filter(|s| s.info.peer == peer && s.info.direction == "input-receiving")
            {
                s.cancel.cancel();
            }
        }
        self.persist(&inner)?;
        Ok(
            serde_json::json!({"message":if allowed {"Keyboard and mouse control allowed"} else {"Keyboard and mouse control stopped and permission removed"}}),
        )
    }

    pub(super) fn reserve_input(
        inner: &mut Inner,
        peer: &str,
        receiving: bool,
    ) -> Result<(String, CancellationToken)> {
        if !inner.config.peers.get(peer).is_some_and(|p| p.approved) {
            bail!("Approve the computer first");
        }
        if receiving && !inner.config.input_controllers.iter().any(|p| p == peer) {
            bail!(
                "Allow keyboard and mouse control for this computer on the receiving Linux desktop"
            );
        }
        if inner
            .sessions
            .values()
            .any(|s| s.info.direction.starts_with("input-"))
        {
            bail!("Keyboard and mouse control is already in use");
        }
        Self::reserve(
            inner,
            peer,
            "keyboard-mouse",
            if receiving {
                "input-receiving"
            } else {
                "input-sending"
            },
        )
    }

    pub(super) async fn start_input(self: &Arc<Self>, peer: String) -> Result<serde_json::Value> {
        let (tx, mut rx) = mpsc::channel::<QueuedInput>(1);
        let (address, session, cancel) = {
            let mut inner = self.inner.lock().await;
            let (session, cancel) = Self::reserve_input(&mut inner, &peer, false)?;
            inner.sessions.get_mut(&session).unwrap().input_tx = Some(tx);
            (inner.config.peers[&peer].address.clone(), session, cancel)
        };
        let (ready_tx, ready_rx) = oneshot::channel();
        let agent = self.clone();
        let id = session.clone();
        // The task owns cleanup even if the local HTTP caller disappears during setup.
        tokio::spawn(async move {
            let mut ready_tx = Some(ready_tx);
            let mut connection = None;
            let result = tokio::select! {
                _ = cancel.cancelled() => Ok(()),
                _ = agent.cancel.cancelled() => Ok(()),
                result = async {
                    let conn = tokio::time::timeout(Duration::from_secs(15), agent.endpoint.connect(address, ALPN)).await??;
                    connection = Some(conn.clone());
                    let (mut send, mut recv) = tokio::time::timeout(DEADLINE, conn.open_bi()).await??;
                    let reply: Reply = tokio::time::timeout(Duration::from_secs(6), async {
                        write_frame(&mut send, &Envelope { version: VERSION, request: Request::InputOpen }).await?;
                        read_frame(&mut recv).await
                    }).await??;
                    match reply {
                        Reply::InputReady => {},
                        Reply::Error { message } => bail!("{message}"),
                        _ => bail!("Update PortRelay on the other computer to use keyboard and mouse control"),
                    }
                    agent.mark_connected(&id).await;
                    if ready_tx.take().unwrap().send(Ok(())).is_err() { return Ok(()); }
                    loop {
                        let Some((batch, ack)) = tokio::time::timeout(DEADLINE, rx.recv()).await.context("Control window stopped responding; keys released")? else { return Ok(()); };
                        let result: Result<()> = tokio::time::timeout(DEADLINE, async {
                            write_frame(&mut send, &batch).await?;
                            let reply: Response = read_frame(&mut recv).await?;
                            if let Some(error) = reply.error { bail!("{error}"); }
                            if reply.sequence != batch.sequence { bail!("Input acknowledgement out of sequence"); }
                            Ok(())
                        }).await.context("Input connection timed out; keys released")?;
                        let _ = ack.send(result.as_ref().map(|_| ()).map_err(|e| e.to_string()));
                        result?;
                    }
                } => result,
            };
            if let Some(tx) = ready_tx {
                let _ = tx.send(Err(result
                    .as_ref()
                    .err()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "Control cancelled".into())));
            }
            if let Some(conn) = connection {
                conn.close(0u32.into(), b"input session ended");
            }
            agent
                .finish(&id, result.err().and_then(session_error))
                .await;
        });
        ready_rx
            .await
            .context("Control connection ended")?
            .map_err(anyhow::Error::msg)?;
        Ok(serde_json::json!({"session":session}))
    }

    pub(super) async fn send_input(
        &self,
        session: String,
        batch: Batch,
    ) -> Result<serde_json::Value> {
        let (ack_tx, ack_rx) = oneshot::channel();
        let cancel = {
            let mut inner = self.inner.lock().await;
            let s = inner
                .sessions
                .get_mut(&session)
                .filter(|s| s.info.direction == "input-sending" && s.info.state == "connected")
                .context("Control session has ended")?;
            if let Err(error) = batch.validate(s.input_sequence) {
                s.cancel.cancel();
                return Err(error);
            }
            if s.input_tx
                .as_ref()
                .context("Control session unavailable")?
                .try_send((batch, ack_tx))
                .is_err()
            {
                s.cancel.cancel();
                bail!("Input queue is full; control stopped to release keys");
            }
            s.input_sequence += 1;
            s.cancel.clone()
        };
        let result = tokio::time::timeout(DEADLINE + Duration::from_millis(500), ack_rx).await;
        match result {
            Ok(Ok(Ok(()))) => Ok(serde_json::json!({"ok":true})),
            other => {
                cancel.cancel();
                let error = match other {
                    Ok(Ok(Err(e))) => e,
                    _ => "Input connection ended; keys released".into(),
                };
                bail!("{error}");
            }
        }
    }

    pub(super) async fn receive_input<S, R>(
        &self,
        conn: &Connection,
        remote: String,
        mut send: S,
        mut recv: R,
    ) -> Result<()>
    where
        S: AsyncWrite + Unpin,
        R: AsyncRead + Unpin,
    {
        let reserved = Self::reserve_input(&mut *self.inner.lock().await, &remote, true);
        let (id, cancel) = match reserved {
            Ok(value) => value,
            Err(error) => {
                write_frame(
                    &mut send,
                    &Reply::Error {
                        message: error.to_string(),
                    },
                )
                .await?;
                tokio::io::AsyncWriteExt::shutdown(&mut send).await?;
                let _ = tokio::time::timeout(Duration::from_secs(2), conn.closed()).await;
                return Ok(());
            }
        };
        let mut ready = false;
        let result: Result<()> = tokio::select! {
            _ = cancel.cancelled() => Ok(()),
            _ = self.cancel.cancelled() => Ok(()),
            result = async {
                #[cfg(target_os="linux")]
                {
                    let mut ipc = input::open(input::Command::Open).await?;
                    write_frame(&mut send, &Reply::InputReady).await?;
                    ready = true;
                    self.mark_connected(&id).await;
                    let mut sequence = 1;
                    loop {
                        let batch: Batch = tokio::time::timeout(DEADLINE, read_frame(&mut recv)).await.context("Controller disconnected; keys released")??;
                        batch.validate(sequence)?;
                        let reply: Response = tokio::time::timeout(DEADLINE, async {
                            write_frame(&mut ipc, &batch).await?;
                            read_frame(&mut ipc).await
                        }).await??;
                        if let Some(error) = reply.error { bail!("{error}"); }
                        if reply.sequence != sequence { bail!("Input helper acknowledgement out of sequence"); }
                        tokio::time::timeout(DEADLINE, write_frame(&mut send, &reply)).await??;
                        sequence += 1;
                    }
                }
                #[cfg(not(target_os="linux"))]
                { let _ = (&mut recv, &mut ready); bail!("Receiving keyboard and mouse control currently requires Linux"); }
            } => result,
        };
        if !ready {
            let message = result
                .as_ref()
                .err()
                .map(ToString::to_string)
                .unwrap_or_else(|| "Control cancelled".into());
            let _ =
                tokio::time::timeout(DEADLINE, write_frame(&mut send, &Reply::Error { message }))
                    .await;
            // Give the request rejection time to arrive before closing QUIC.
            let _ = tokio::time::timeout(Duration::from_millis(500), conn.closed()).await;
        }
        conn.close(0u32.into(), b"input session ended");
        self.finish(&id, result.err().and_then(session_error)).await;
        Ok(())
    }
}
