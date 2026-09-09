use super::*;
use crate::protocol::{read_frame, write_frame};
use anyhow::Context;
use evdev::{
    AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode, uinput::VirtualDevice,
};
use fs2::FileExt;
use std::{
    collections::BTreeSet,
    fs,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
    sync::Arc,
};
use tokio::{
    net::{UnixListener, UnixStream},
    sync::Semaphore,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

struct Devices {
    keyboard: VirtualDevice,
    pointer: VirtualDevice,
    pressed: BTreeSet<u16>,
}
impl Devices {
    fn create() -> Result<Self> {
        let keys: AttributeSet<KeyCode> = (1..=127)
            .filter(|c| valid_key(*c))
            .map(KeyCode::new)
            .collect();
        let buttons: AttributeSet<KeyCode> = (272..=276).map(KeyCode::new).collect();
        let axes: AttributeSet<RelativeAxisCode> = [
            RelativeAxisCode::REL_X,
            RelativeAxisCode::REL_Y,
            RelativeAxisCode::REL_WHEEL,
            RelativeAxisCode::REL_HWHEEL,
        ]
        .into_iter()
        .collect();
        let keyboard = VirtualDevice::builder()?
            .name("PortRelay keyboard")
            .with_keys(&keys)?
            .build()?;
        let pointer = VirtualDevice::builder()?
            .name("PortRelay pointer")
            .with_keys(&buttons)?
            .with_relative_axes(&axes)?
            .build()?;
        Ok(Self {
            keyboard,
            pointer,
            pressed: BTreeSet::new(),
        })
    }
    fn key(&mut self, code: u16, value: u8) -> Result<()> {
        let emit = match value {
            0 => self.pressed.remove(&code),
            1 => self.pressed.insert(code),
            2 => self.pressed.contains(&code),
            _ => false,
        };
        if emit {
            let device = if code >= 272 {
                &mut self.pointer
            } else {
                &mut self.keyboard
            };
            device.emit(&[InputEvent::new(EventType::KEY.0, code, value.into())])?;
        }
        Ok(())
    }
    fn release(&mut self) -> Result<()> {
        let mut failed = None;
        for code in self.pressed.clone() {
            if let Err(error) = self.key(code, 0) {
                failed = Some(error);
            }
        }
        if let Some(error) = failed {
            return Err(error);
        }
        Ok(())
    }
    fn apply(&mut self, batch: &Batch) -> Result<()> {
        for event in &batch.events {
            match *event {
                Event::Move { x, y } => self.pointer.emit(&[
                    InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, x),
                    InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_Y.0, y),
                ])?,
                Event::Wheel { x, y } => self.pointer.emit(&[
                    InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_HWHEEL.0, x),
                    InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_WHEEL.0, y),
                ])?,
                Event::Key { code, value } => self.key(code, value)?,
                Event::Button { button, down } => self.key(
                    [272, 274, 273, 275, 276][usize::from(button)],
                    u8::from(down),
                )?,
                Event::Release => self.release()?,
            }
        }
        Ok(())
    }
}
impl Drop for Devices {
    fn drop(&mut self) {
        let _ = self.release();
    }
}
async fn active_owner(uid: u32) -> Result<()> {
    async fn loginctl(args: &[&str]) -> Result<String> {
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            tokio::process::Command::new("/usr/bin/loginctl")
                .args(args)
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .kill_on_drop(true)
                .output(),
        )
        .await??;
        if !result.status.success() {
            bail!("Cannot verify the active Linux desktop");
        }
        Ok(String::from_utf8(result.stdout)?)
    }
    let session = loginctl(&["show-seat", "seat0", "-p", "ActiveSession", "--value"]).await?;
    let session = session.trim();
    if session.is_empty()
        || session.len() > 64
        || !session.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        bail!("Sign in to the Linux desktop first");
    }
    let info = loginctl(&[
        "show-session",
        session,
        "-p",
        "User",
        "-p",
        "Active",
        "-p",
        "Remote",
        "-p",
        "LockedHint",
    ])
    .await?;
    if !info.lines().any(|l| l == format!("User={uid}"))
        || !info.lines().any(|l| l == "Active=yes")
        || !info.lines().any(|l| l == "Remote=no")
        || !info.lines().any(|l| l == "LockedHint=no")
    {
        bail!("Sign in and unlock this user's Linux desktop before receiving control");
    }
    Ok(())
}
async fn client(
    mut socket: UnixStream,
    uid: u32,
    lease: Arc<Semaphore>,
    cancel: CancellationToken,
) -> Result<()> {
    if socket.peer_cred()?.uid() != uid {
        bail!("Wrong local user");
    }
    let command: Command = tokio::time::timeout(DEADLINE, read_frame(&mut socket)).await??;
    let prepared = async {
        active_owner(uid).await?;
        fs::OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .context("Linux uinput is unavailable")?;
        if matches!(command, Command::Probe) {
            return Ok(None);
        }
        let permit = lease
            .try_acquire_owned()
            .context("Keyboard and mouse are already controlled")?;
        Ok::<_, anyhow::Error>(Some((permit, Devices::create()?)))
    }
    .await;
    match prepared {
        Err(error) => {
            write_frame(
                &mut socket,
                &Response {
                    error: Some(error.to_string()),
                    sequence: 0,
                },
            )
            .await?;
        }
        Ok(None) => {
            write_frame(
                &mut socket,
                &Response {
                    error: None,
                    sequence: 0,
                },
            )
            .await?;
        }
        Ok(Some((_lease, mut devices))) => {
            write_frame(
                &mut socket,
                &Response {
                    error: None,
                    sequence: 0,
                },
            )
            .await?;
            let mut sequence = 1;
            let mut owner_check = tokio::time::interval(Duration::from_secs(1));
            let mut window = std::time::Instant::now();
            let mut count = 0;
            loop {
                // Keep read_frame alive while checking desktop ownership; never cancel
                // a partially consumed length-prefixed frame and start reading anew.
                let batch = {
                    let read = tokio::time::timeout(DEADLINE, read_frame::<_, Batch>(&mut socket));
                    tokio::pin!(read);
                    loop {
                        tokio::select! {
                            _=cancel.cancelled()=>return Ok(()),
                            _=owner_check.tick()=>active_owner(uid).await?,
                            result=&mut read=>break result??,
                        }
                    }
                };
                batch.validate(sequence)?;
                if window.elapsed() >= Duration::from_secs(1) {
                    window = std::time::Instant::now();
                    count = 0;
                }
                count += 1;
                if count > 250 {
                    bail!("Input rate limit exceeded");
                }
                devices.apply(&batch)?;
                tokio::time::timeout(
                    DEADLINE,
                    write_frame(
                        &mut socket,
                        &Response {
                            error: None,
                            sequence,
                        },
                    ),
                )
                .await??;
                sequence += 1;
            }
        }
    }
    Ok(())
}
pub async fn serve(uid: u32, runtime: &Path) -> Result<()> {
    use nix::unistd::{Gid, Uid, chown};
    if !Uid::effective().is_root() || uid == 0 {
        bail!("Run the input helper as root for a regular user");
    }
    fs::create_dir_all(runtime)?;
    let meta = fs::symlink_metadata(runtime)?;
    if !meta.is_dir() || meta.uid() != 0 {
        bail!("Input runtime must be a root-owned directory");
    }
    fs::set_permissions(runtime, fs::Permissions::from_mode(0o711))?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(runtime.join("helper.lock"))?;
    lock.try_lock_exclusive()
        .context("Input helper is already running")?;
    let path = runtime.join("helper.sock");
    if path.exists() {
        fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    chown(&path, Some(Uid::from_raw(uid)), Some(Gid::from_raw(0)))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    let cancel = CancellationToken::new();
    let lease = Arc::new(Semaphore::new(1));
    let limits = Arc::new(Semaphore::new(16));
    let mut tasks = JoinSet::new();
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    loop {
        tokio::select! {
            _=tokio::signal::ctrl_c()=>break,
            _=term.recv()=>break,
            Some(_)=tasks.join_next()=>{},
            connection=listener.accept()=>{
                let (socket,_)=connection?;
                let Ok(permit)=limits.clone().try_acquire_owned() else {continue;};
                let lease=lease.clone();let cancel=cancel.clone();
                tasks.spawn(async move{let _permit=permit;let _=client(socket,uid,lease,cancel).await;});
            }
        }
    }
    cancel.cancel();
    while tasks.join_next().await.is_some() {}
    fs::remove_file(path)?;
    Ok(())
}
