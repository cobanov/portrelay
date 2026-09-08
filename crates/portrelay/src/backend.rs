use crate::protocol::Device;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[cfg(unix)]
use tokio::net::UnixStream;

#[cfg(not(windows))]
pub const SOCKET: &str = "/run/portrelay/helper.sock";
#[cfg(windows)]
pub const SOCKET: &str = r"\\.\pipe\PortRelay.Helper.v1";
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum HelperRequest {
    Health,
    Inventory,
    WaitExport { device: String },
    WaitImport { port: u32 },
    Export { device: String, generation: String },
    Import { device: Device },
}
#[derive(Serialize, Deserialize)]
pub struct HelperReply {
    pub error: Option<String>,
    pub device: Option<Device>,
    pub port: Option<u32>,
}
pub fn available(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let _ = path;
        crate::windows::installed()
    }
    #[cfg(not(windows))]
    {
        cfg!(target_os = "linux") && path.exists()
    }
}
pub async fn devices(path: &Path) -> Vec<Device> {
    #[cfg(windows)]
    {
        crate::windows::devices(path).await.unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        crate::inventory::inventory()
    }
}
pub async fn health(path: &Path) -> Result<()> {
    if !available(path) {
        bail!("Device helper is not installed or not running");
    }
    #[cfg(any(unix, windows))]
    {
        tokio::time::timeout(
            std::time::Duration::from_secs(4),
            open(path, &HelperRequest::Health),
        )
        .await??;
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        bail!("USB support currently requires Linux");
    }
}
#[cfg(windows)]
pub use crate::windows::open;
#[cfg(unix)]
pub async fn open(path: &Path, request: &HelperRequest) -> Result<(UnixStream, HelperReply)> {
    let mut stream = UnixStream::connect(path).await.map_err(|e| {
        anyhow::anyhow!("Device helper is unavailable: {e}. Install the Linux helper first.")
    })?;
    crate::protocol::write_frame(&mut stream, request).await?;
    let reply: HelperReply = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        crate::protocol::read_frame(&mut stream),
    )
    .await??;
    if let Some(error) = &reply.error {
        bail!("{error}");
    }
    Ok((stream, reply))
}
#[cfg(not(target_os = "linux"))]
pub async fn serve(_uid: u32, _runtime: &Path) -> Result<()> {
    bail!("The USB helper is implemented for Linux only");
}
#[cfg(target_os = "linux")]
pub use linux::serve;

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use crate::{
        inventory,
        protocol::{read_frame, valid_bus_id, write_frame},
        storage,
    };
    use anyhow::Context;
    use fs2::FileExt;
    use std::{
        fs,
        net::SocketAddr,
        os::{
            fd::AsRawFd,
            unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        },
        path::PathBuf,
        sync::Arc,
        time::Duration,
    };
    use tokio::{
        io::{AsyncRead, AsyncWrite},
        net::{TcpListener, TcpStream, UnixListener},
        process::Command,
        sync::{Mutex, Semaphore},
        task::JoinSet,
    };
    use tokio_util::sync::CancellationToken;
    const VHCI: &str = "/sys/devices/platform/vhci_hcd.0";
    fn usbip() -> Result<&'static str> {
        ["/usr/bin/usbip", "/usr/sbin/usbip"]
            .into_iter()
            .find(|p| Path::new(p).is_file())
            .context("Install the distribution USB/IP tools first")
    }
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "kind")]
    enum Recovery {
        Export { device: String, generation: String },
        Import { port: u32, devid: u32 },
    }
    #[derive(Debug)]
    struct RecoveryFailure(String);
    impl std::fmt::Display for RecoveryFailure {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.0.fmt(f)
        }
    }
    impl std::error::Error for RecoveryFailure {}
    async fn command(program: &str, args: &[&str]) -> Result<()> {
        let child = Command::new(program)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(Duration::from_secs(12), child)
            .await
            .context("Device command timed out")??;
        if !output.status.success() {
            bail!(
                "{program} failed: {}",
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(400)
                    .collect::<String>()
            );
        }
        Ok(())
    }
    fn write_sys(path: impl AsRef<Path>, value: String) -> Result<()> {
        fs::write(path, value.as_bytes()).context("Kernel device operation failed")
    }
    pub(super) fn ports(text: &str) -> Vec<(String, u32, u32, u32)> {
        text.lines()
            .skip(1)
            .filter_map(|line| {
                let p: Vec<_> = line.split_whitespace().collect();
                if p.len() < 5 || !["hs", "ss"].contains(&p[0]) {
                    return None;
                }
                Some((
                    p[0].to_string(),
                    p[1].parse().ok()?,
                    p[2].parse().ok()?,
                    u32::from_str_radix(p[4], 16).ok()?,
                ))
            })
            .collect()
    }
    fn all_ports() -> Result<Vec<(String, u32, u32, u32)>> {
        let mut result = vec![];
        for e in fs::read_dir(VHCI)?.flatten() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name == "status"
                || name
                    .strip_prefix("status.")
                    .is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit()))
            {
                result.extend(ports(&fs::read_to_string(e.path())?));
            }
        }
        Ok(result)
    }
    async fn cleanup(record: &Recovery, path: &Path) -> Result<()> {
        match record {
            Recovery::Export { device, generation } => {
                if !valid_bus_id(device) {
                    bail!("Invalid recovery device");
                }
                if let Some(current) = inventory::inventory().into_iter().find(|d| &d.id == device)
                    && &current.generation == generation
                {
                    let device_path = PathBuf::from(format!("/sys/bus/usb/devices/{device}"));
                    let driver = fs::read_link(device_path.join("driver")).ok();
                    if driver
                        .as_ref()
                        .and_then(|x| x.file_name())
                        .is_some_and(|x| x == "usbip-host")
                    {
                        // Let the kernel's EOF handler stop URBs and its worker threads before
                        // unbinding the driver. Racing unbind against that work can strand it.
                        let mut stopped = false;
                        for _ in 0..250 {
                            if fs::read_to_string(device_path.join("usbip_status"))
                                .unwrap_or_default()
                                .trim()
                                == "1"
                            {
                                stopped = true;
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(20)).await;
                        }
                        if !stopped {
                            bail!(
                                "USB kernel session did not finish shutting down; recovery is still required"
                            );
                        }
                        command(usbip()?, &["unbind", "--busid", device]).await?;
                    }
                    if device_path.exists() && !device_path.join("driver").exists() {
                        // A previous helper may have stopped between unbind and driver probing.
                        let matcher = Path::new("/sys/bus/usb/drivers/usbip-host/match_busid");
                        if fs::read_to_string(matcher)?
                            .split_whitespace()
                            .any(|id| id == device)
                        {
                            write_sys(matcher, format!("del {device}"))?;
                        }
                        write_sys("/sys/bus/usb/drivers_probe", device.clone())?;
                    }
                    if device_path.exists()
                        && !fs::read_link(device_path.join("driver"))
                            .ok()
                            .and_then(|p| p.file_name().map(|s| s.to_owned()))
                            .is_some_and(|name| name == "usb")
                    {
                        bail!("The original USB driver has not been restored");
                    }
                }
            }
            Recovery::Import { port, devid } => {
                if *port > 255 {
                    bail!("Invalid recovery port");
                }
                if let Some((_, _, status, current)) =
                    all_ports()?.into_iter().find(|(_, p, _, _)| p == port)
                {
                    if status != 4 && current == *devid {
                        let result = write_sys(Path::new(VHCI).join("detach"), port.to_string());
                        // EOF can finish detachment between the status read and write.
                        // Treat an already-empty port as success; never detach a new owner.
                        if let Err(error) = result
                            && !all_ports()?.iter().any(|(_, p, s, _)| p == port && *s == 4)
                        {
                            return Err(error);
                        }
                    } else if status != 4 {
                        bail!("Port changed owner; manual recovery is required");
                    }
                }
            }
        }
        fs::remove_file(path)?;
        Ok(())
    }
    // The listener exists only while constructing a pair, never while USB data flows.
    // Verify both endpoints before closing it; a local connection race cannot steal USB data.
    async fn tcp_pair() -> Result<(TcpStream, TcpStream)> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let client = TcpStream::connect(listener.local_addr()?).await?;
        let expected: SocketAddr = client.local_addr()?;
        let (server, peer) =
            tokio::time::timeout(Duration::from_secs(2), listener.accept()).await??;
        if peer != expected || server.local_addr()? != client.peer_addr()? {
            bail!("Local socket pair was interrupted");
        }
        drop(listener);
        client.set_nodelay(true)?;
        server.set_nodelay(true)?;
        Ok((client, server))
    }
    async fn prepare(
        request: HelperRequest,
        runtime: &Path,
    ) -> Result<(TcpStream, HelperReply, Recovery, PathBuf)> {
        match request {
            HelperRequest::Inventory
            | HelperRequest::Health
            | HelperRequest::WaitExport { .. }
            | HelperRequest::WaitImport { .. } => bail!("Not a device operation"),
            HelperRequest::Export { device, generation } => {
                if !valid_bus_id(&device) {
                    bail!("Invalid USB device identifier");
                }
                let d = inventory::inventory()
                    .into_iter()
                    .find(|d| d.id == device)
                    .context("Device was unplugged")?;
                if d.generation != generation {
                    bail!("Device changed; share it again");
                }
                if let Some(reason) = &d.blocked {
                    bail!("{reason}");
                }
                if d.speed == 0 {
                    bail!("Unsupported USB speed");
                }
                if !fs::read_link(format!("/sys/bus/usb/devices/{device}/driver"))
                    .ok()
                    .and_then(|p| p.file_name().map(|s| s.to_owned()))
                    .is_some_and(|name| name == "usb")
                {
                    bail!(
                        "Device is already managed by USB/IP or needs its local driver restored first"
                    );
                }
                let record = Recovery::Export {
                    device: device.clone(),
                    generation,
                };
                let path = runtime.join(format!("export-{device}.json"));
                if path.exists() {
                    bail!("Device is busy or needs recovery; restart the helper");
                }
                storage::save(&path, &record)?;
                let result = async {
                    command(usbip()?, &["bind", "--busid", &device]).await?;
                    let (user, kernel) = tcp_pair().await?;
                    let sock_path = format!("/sys/bus/usb/devices/{device}/usbip_sockfd");
                    write_sys(sock_path, kernel.as_raw_fd().to_string())?;
                    drop(kernel);
                    Ok::<_, anyhow::Error>(user)
                }
                .await;
                match result {
                    Ok(user) => Ok((
                        user,
                        HelperReply {
                            error: None,
                            device: Some(d),
                            port: None,
                        },
                        record,
                        path,
                    )),
                    Err(e) => {
                        if let Err(clean) = cleanup(&record, &path).await {
                            return Err(
                                RecoveryFailure(format!("{e}; recovery failed: {clean}")).into()
                            );
                        }
                        Err(e)
                    }
                }
            }
            HelperRequest::Import { device } => {
                if ![1, 2, 3, 5, 6].contains(&device.speed)
                    || device.devid >> 16 == 0
                    || device.devid & 0xffff == 0
                {
                    bail!("Invalid remote USB metadata");
                }
                let hub = if device.speed >= 5 { "ss" } else { "hs" };
                let port = all_ports()?
                    .into_iter()
                    .find(|(h, p, s, _)| {
                        h == hub && *s == 4 && !runtime.join(format!("import-{p}.json")).exists()
                    })
                    .context("No free virtual USB ports")?
                    .1;
                let (user, kernel) = tcp_pair().await?;
                let record = Recovery::Import {
                    port,
                    devid: device.devid,
                };
                let path = runtime.join(format!("import-{port}.json"));
                storage::save(&path, &record)?;
                if let Err(e) = write_sys(
                    Path::new(VHCI).join("attach"),
                    format!(
                        "{port} {} {} {}",
                        kernel.as_raw_fd(),
                        device.devid,
                        device.speed
                    ),
                ) {
                    drop(kernel);
                    drop(user);
                    if let Err(clean) = cleanup(&record, &path).await {
                        return Err(
                            RecoveryFailure(format!("{e}; recovery failed: {clean}")).into()
                        );
                    }
                    return Err(e);
                }
                drop(kernel);
                Ok((
                    user,
                    HelperReply {
                        error: None,
                        device: Some(device),
                        port: Some(port),
                    },
                    record,
                    path,
                ))
            }
        }
    }
    async fn bridge<A: AsyncRead + AsyncWrite + Unpin, B: AsyncRead + AsyncWrite + Unpin>(
        a: &mut A,
        b: &mut B,
    ) -> Result<()> {
        let (mut ar, mut aw) = tokio::io::split(a);
        let (mut br, mut bw) = tokio::io::split(b);
        tokio::select! {r=tokio::io::copy(&mut ar,&mut bw)=>{r?;},r=tokio::io::copy(&mut br,&mut aw)=>{r?;}}
        Ok(())
    }
    pub async fn serve(uid: u32, runtime: &Path) -> Result<()> {
        if !nix::unistd::geteuid().is_root() {
            bail!("Run the device helper with sudo or the installed system service");
        }
        fs::create_dir_all(runtime)?;
        let meta = fs::symlink_metadata(runtime)?;
        if meta.file_type().is_symlink() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
            bail!("Helper runtime must be root-owned and not writable by other users");
        }
        fs::set_permissions(runtime, fs::Permissions::from_mode(0o711))?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(runtime.join("helper.lock"))?;
        lock.try_lock_exclusive()
            .context("Another device helper is running")?;
        for module in ["usbip-host", "vhci-hcd"] {
            command("/usr/sbin/modprobe", &[module]).await?;
        }
        for e in fs::read_dir(runtime)?.flatten() {
            if e.path().extension().is_some_and(|x| x == "json") {
                let record: Recovery = serde_json::from_slice(&fs::read(e.path())?)?;
                cleanup(&record, &e.path())
                    .await
                    .context("Unfinished device recovery; sharing is disabled")?;
            }
        }
        let socket = runtime.join("helper.sock");
        if socket.exists() {
            fs::remove_file(&socket)?;
        }
        let listener = UnixListener::bind(&socket)?;
        nix::unistd::chown(&socket, Some(nix::unistd::Uid::from_raw(uid)), None)?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
        let cancel = CancellationToken::new();
        let cancellation = cancel.clone();
        tokio::spawn(async move {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("signal handler");
            tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
            cancellation.cancel();
        });
        let runtime = runtime.to_path_buf();
        let preparation = Arc::new(Mutex::new(()));
        let recovery_errors = Arc::new(Mutex::new(Vec::<String>::new()));
        // Leave room for health and cleanup acknowledgements while all 16
        // device sessions are active.
        let limits = Arc::new(Semaphore::new(64));
        let mut tasks = JoinSet::new();
        eprintln!("Device helper ready for UID {uid}");
        loop {
            tokio::select! {
                _=cancel.cancelled()=>break,
                Some(r)=tasks.join_next()=>{if let Err(e)=r {eprintln!("Helper task failed: {e}");}},
                accepted=listener.accept()=>{
                    let (mut ipc,_)=accepted?;
                    if ipc.peer_cred()?.uid()!=uid {continue;}
                    let Ok(permit)=limits.clone().try_acquire_owned() else {continue;};
                    let runtime=runtime.clone();let preparation=preparation.clone();let cancel=cancel.clone();let recovery_errors=recovery_errors.clone();
                    tasks.spawn(async move {
                        let _permit=permit;
                        let result=tokio::time::timeout(Duration::from_secs(5),read_frame::<_,HelperRequest>(&mut ipc)).await;
                        let request=match result {Ok(Ok(r))=>r,_=>return};
                        let errors=recovery_errors.lock().await;
                        if matches!(request,HelperRequest::Health) || !errors.is_empty() {
                            let error=if errors.is_empty(){None}else{Some(format!("Device recovery needs attention: {}",errors.join("; ")))};
                            let _=write_frame(&mut ipc,&HelperReply{error,device:None,port:None}).await;
                            return;
                        }
                        drop(errors);
                        let wait_path=match &request {
                            HelperRequest::WaitExport{device} if valid_bus_id(device)=>Some(runtime.join(format!("export-{device}.json"))),
                            HelperRequest::WaitImport{port} if *port<=255=>Some(runtime.join(format!("import-{port}.json"))),
                            _=>None,
                        };
                        if let Some(path)=wait_path {
                            let result=tokio::time::timeout(Duration::from_secs(18),async {
                                loop {
                                    let errors=recovery_errors.lock().await;
                                    if !errors.is_empty(){bail!("Device recovery needs attention: {}",errors.join("; "));}
                                    if !path.exists(){return Ok::<_,anyhow::Error>(());}
                                    drop(errors);
                                    tokio::time::sleep(Duration::from_millis(25)).await;
                                }
                            }).await;
                            let error=match result {Ok(Ok(()))=>None,Ok(Err(e))=>Some(e.to_string()),Err(_)=>Some("Device cleanup is still pending; inspect the helper".into())};
                            let _=write_frame(&mut ipc,&HelperReply{error,device:None,port:None}).await;
                            return;
                        }
                        let guard=preparation.lock().await;
                        if cancel.is_cancelled(){return;}
                        let mut extra=[0u8;1];
                        match ipc.try_read(&mut extra) {
                            Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{},
                            _=>return,
                        }
                        let result=prepare(request,&runtime).await;
                        drop(guard);
                        match result {
                            Ok((mut tcp,reply,record,path))=>{
                                if write_frame(&mut ipc,&reply).await.is_ok() {
                                    tokio::select!{_=cancel.cancelled()=>{},_=bridge(&mut tcp,&mut ipc)=>{}}
                                }
                                drop(tcp);drop(ipc);
                                let _guard=preparation.lock().await;
                                if let Err(e)=cleanup(&record,&path).await {
                                    let message=format!("{e:#}. Restart the helper to retry.");
                                    eprintln!("Device recovery failed: {message}");
                                    recovery_errors.lock().await.push(message);
                                }
                            },
                            Err(e)=>{
                                if e.downcast_ref::<RecoveryFailure>().is_some() {
                                    recovery_errors.lock().await.push(e.to_string());
                                }
                                let _=write_frame(&mut ipc,&HelperReply{error:Some(e.to_string()),device:None,port:None}).await;
                            }
                        }
                    });
                }
            }
        }
        while tasks.join_next().await.is_some() {}
        fs::remove_file(socket)?;
        Ok(())
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn parses_only_kernel_status_rows() {
            let p = ports(
                "hub port sta spd dev socket bus\nhs 0000 004 000 00000000 0 0-0\nss 0008 006 005 00010002 2 2-1\n",
            );
            assert_eq!(p.len(), 2);
            assert_eq!(p[1], ("ss".into(), 8, 6, 0x10002));
        }
        #[tokio::test]
        async fn private_tcp_pair_transfers_without_listener() {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut a, mut b) = tcp_pair().await.unwrap();
            a.write_all(b"usb").await.unwrap();
            let mut data = [0; 3];
            b.read_exact(&mut data).await.unwrap();
            assert_eq!(&data, b"usb");
        }
    }
}
