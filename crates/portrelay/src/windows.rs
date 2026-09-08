//! Windows user-side integration. Privileged USB operations live in the separate GPL service.
use crate::{
    backend::{HelperReply, HelperRequest},
    protocol::{Device, read_frame, write_frame},
};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{
    ffi::c_void,
    os::windows::{fs::MetadataExt, io::AsRawHandle},
    path::{Path, PathBuf},
    ptr,
    time::Duration,
};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, LocalFree},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            GetNamedSecurityInfoW, SE_FILE_OBJECT, SetNamedSecurityInfoW,
        },
        DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetTokenInformation,
        OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_INFORMATION_CLASS,
        TOKEN_OWNER, TOKEN_QUERY, TOKEN_USER, TokenOwner, TokenUser,
    },
    System::{
        Pipes::GetNamedPipeServerProcessId,
        Services::{
            CloseServiceHandle, OpenSCManagerW, OpenServiceW, QUERY_SERVICE_CONFIGW,
            QueryServiceConfigW, QueryServiceStatusEx, SC_HANDLE, SC_MANAGER_CONNECT,
            SC_STATUS_PROCESS_INFO, SERVICE_QUERY_CONFIG, SERVICE_QUERY_STATUS, SERVICE_RUNNING,
            SERVICE_STATUS_PROCESS, SERVICE_WIN32_OWN_PROCESS,
        },
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct ServiceHandle(SC_HANDLE);
impl Drop for ServiceHandle {
    fn drop(&mut self) {
        unsafe {
            CloseServiceHandle(self.0);
        }
    }
}

// Normal users cannot open a LocalSystem process token. Ask the Service Control
// Manager instead, and bind its authenticated service identity to this pipe's
// actual server PID. Merely trusting the pipe name or executable path is unsafe.
fn verify_service(pid: u32) -> Result<()> {
    unsafe {
        let manager = ServiceHandle(OpenSCManagerW(ptr::null(), ptr::null(), SC_MANAGER_CONNECT));
        if manager.0.is_null() {
            bail!("Cannot contact the Windows service manager");
        }
        let service = ServiceHandle(OpenServiceW(
            manager.0,
            wide("PortRelayHelper").as_ptr(),
            SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG,
        ));
        if service.0.is_null() {
            bail!("The PortRelay USB service is not installed");
        }
        let mut status: SERVICE_STATUS_PROCESS = std::mem::zeroed();
        let mut size = 0;
        if QueryServiceStatusEx(
            service.0,
            SC_STATUS_PROCESS_INFO,
            (&mut status as *mut SERVICE_STATUS_PROCESS).cast(),
            std::mem::size_of_val(&status) as u32,
            &mut size,
        ) == 0
            || status.dwProcessId != pid
            || pid == 0
            || status.dwCurrentState != SERVICE_RUNNING
            || status.dwServiceType != SERVICE_WIN32_OWN_PROCESS
        {
            bail!("The USB pipe does not belong to the running PortRelay service");
        }
        QueryServiceConfigW(service.0, ptr::null_mut(), 0, &mut size);
        if size == 0 || size > 65536 {
            bail!("Invalid USB service configuration");
        }
        let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
        if QueryServiceConfigW(service.0, buffer.as_mut_ptr().cast(), size, &mut size) == 0 {
            bail!("Cannot read the USB service configuration");
        }
        let config = &*buffer.as_ptr().cast::<QUERY_SERVICE_CONFIGW>();
        let expected = wide("LocalSystem");
        if config.lpServiceStartName.is_null()
            || !expected
                .iter()
                .enumerate()
                .all(|(i, c)| *config.lpServiceStartName.add(i) == *c)
        {
            bail!("The USB service must belong to Windows LocalSystem");
        }
    }
    Ok(())
}
fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
unsafe fn sid_text(sid: *mut c_void) -> Result<String> {
    let mut string = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut string) } == 0 {
        bail!("Cannot read the Windows account identity");
    }
    let mut len = 0;
    unsafe {
        while *string.add(len) != 0 {
            len += 1;
        }
    }
    let result = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(string, len) });
    unsafe {
        LocalFree(string.cast());
    }
    Ok(result)
}
fn process_sid(process: HANDLE) -> Result<String> {
    process_identity(process, TokenUser)
}
fn process_identity(process: HANDLE, class: TOKEN_INFORMATION_CLASS) -> Result<String> {
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            bail!("Cannot verify the USB service identity");
        }
        let token = Handle(token);
        let mut length = 0;
        GetTokenInformation(token.0, class, ptr::null_mut(), 0, &mut length);
        if length == 0 || length > 16384 {
            bail!("Invalid Windows account token");
        }
        let mut storage = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
        if GetTokenInformation(
            token.0,
            class,
            storage.as_mut_ptr().cast(),
            length,
            &mut length,
        ) == 0
        {
            bail!("Cannot read the Windows account token");
        }
        sid_text(if class == TokenOwner {
            (*(storage.as_ptr().cast::<TOKEN_OWNER>())).Owner
        } else {
            (*(storage.as_ptr().cast::<TOKEN_USER>())).User.Sid
        })
    }
}
pub fn current_sid() -> Result<String> {
    process_sid(unsafe { GetCurrentProcess() })
}

pub fn protect_state(dir: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(dir)?;
    if metadata.file_attributes() & 0x400 != 0 {
        bail!("The state directory must not be a Windows reparse point");
    }
    let sid = current_sid()?;
    let path = wide(dir);
    unsafe {
        let mut owner = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        if GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        ) != 0
        {
            bail!("Cannot verify the state directory owner");
        }
        let owner_text = sid_text(owner);
        LocalFree(descriptor);
        let owner_text = owner_text?;
        // Elevated tokens create directories owned by Administrators.
        // Accept only this account or the current token's default owner.
        if owner_text != sid && owner_text != process_identity(GetCurrentProcess(), TokenOwner)? {
            bail!("The state directory must belong to the current Windows user");
        }
        let text = wide(format!(
            "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;{sid})"
        ));
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            bail!("Cannot protect the state directory");
        }
        let mut dacl = ptr::null_mut();
        let mut present = 0;
        let mut defaulted = 0;
        let read = GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted);
        let status = if read != 0 && present != 0 {
            SetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                dacl,
                ptr::null_mut(),
            )
        } else {
            1
        };
        LocalFree(descriptor);
        if status != 0 {
            bail!("Cannot restrict the state directory to this Windows user");
        }
    }
    Ok(())
}

pub fn install_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(exe
        .parent()
        .context("Cannot locate the PortRelay installation")?
        .to_path_buf())
}
pub fn installed() -> bool {
    install_dir().is_ok_and(|p| p.join("device-service/usbipd.exe").is_file())
}

async fn pipe(path: &Path) -> Result<NamedPipeClient> {
    let stream = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match ClientOptions::new().open(path) {
                Ok(stream) => return Ok(stream),
                Err(e) if e.raw_os_error() == Some(231) => {
                    tokio::time::sleep(Duration::from_millis(30)).await
                }
                Err(e) => return Err(e),
            }
        }
    })
    .await
    .context("The Windows USB service is busy")??;
    unsafe {
        let mut pid = 0;
        if GetNamedPipeServerProcessId(stream.as_raw_handle(), &mut pid) == 0 {
            bail!("Cannot verify the Windows USB service");
        }
        verify_service(pid)?;
    }
    Ok(stream)
}
pub async fn open(path: &Path, request: &HelperRequest) -> Result<(NamedPipeClient, HelperReply)> {
    let mut stream = pipe(path)
        .await
        .context("USB service is unavailable. Enable USB support or restart Windows.")?;
    write_frame(&mut stream, request).await?;
    let reply: HelperReply =
        tokio::time::timeout(Duration::from_secs(20), read_frame(&mut stream)).await??;
    if let Some(error) = &reply.error {
        bail!("{error}");
    }
    Ok((stream, reply))
}
pub async fn devices(path: &Path) -> Result<Vec<Device>> {
    #[derive(Deserialize)]
    struct Inventory {
        devices: Option<Vec<Device>>,
        error: Option<String>,
    }
    let mut stream = pipe(path).await?;
    write_frame(&mut stream, &HelperRequest::Inventory).await?;
    let reply: Inventory =
        tokio::time::timeout(Duration::from_secs(5), read_frame(&mut stream)).await??;
    if let Some(error) = reply.error {
        bail!("{error}");
    }
    reply.devices.context("Device inventory is unavailable")
}

fn system_program(name: &str) -> Result<PathBuf> {
    let mut buffer = [0u16; 32768];
    let length = unsafe {
        windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
    } as usize;
    if length == 0 || length >= buffer.len() {
        bail!("Cannot locate Windows system tools");
    }
    Ok(PathBuf::from(String::from_utf16_lossy(&buffer[..length])).join(name))
}
pub async fn setup_elevated(owner: &str) -> Result<()> {
    if !owner.starts_with("S-1-5-21-")
        || !owner
            .bytes()
            .all(|c| c.is_ascii_digit() || c == b'S' || c == b'-')
    {
        bail!("Choose a normal Windows account for PortRelay");
    }
    let script = install_dir()?.join("setup-windows.ps1");
    let output =
        tokio::process::Command::new(system_program(r"WindowsPowerShell\v1.0\powershell.exe")?)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .arg("-OwnerSid")
            .arg(owner)
            .output()
            .await?;
    if !output.status.success() {
        bail!(
            "{}",
            String::from_utf8_lossy(&output.stderr)
                .trim()
                .chars()
                .take(1000)
                .collect::<String>()
        );
    }
    Ok(())
}
pub async fn request_setup() -> Result<()> {
    tokio::task::spawn_blocking(setup_permission).await?
}
fn setup_permission() -> Result<()> {
    use windows_sys::Win32::{
        Foundation::WAIT_TIMEOUT,
        System::Threading::{GetExitCodeProcess, WaitForSingleObject},
        UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
    };
    let exe = wide(std::env::current_exe()?);
    let parameters = wide(format!("windows-setup --owner-sid {}", current_sid()?));
    let verb = wide("runas");
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = exe.as_ptr();
    info.lpParameters = parameters.as_ptr();
    info.nShow = 0;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        bail!("Windows permission was cancelled or unavailable. Try enabling USB support again.");
    }
    let process = Handle(info.hProcess);
    if process.0.is_null() {
        bail!("Windows did not return the setup process");
    }
    while unsafe { WaitForSingleObject(process.0, 1000) } == WAIT_TIMEOUT {}
    let mut code = 1;
    if unsafe { GetExitCodeProcess(process.0, &mut code) } == 0 || code != 0 {
        bail!(
            "USB setup could not finish. Check C:\\ProgramData\\PortRelay\\setup.log, then retry. An existing usbipd-win or VirtualBox installation may need attention."
        );
    }
    Ok(())
}
pub async fn start_desktop() -> Result<()> {
    use std::os::windows::process::CommandExt;
    let dir = crate::storage::state_dir(None)?;
    let exe = std::env::current_exe()?;
    let output = tokio::process::Command::new(system_program("reg.exe")?)
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "PortRelay",
            "/t",
            "REG_SZ",
            "/d",
        ])
        .arg(format!("\"{}\" desktop --no-open", exe.display()))
        .arg("/f")
        .creation_flags(0x08000000)
        .output()
        .await?;
    if !output.status.success() {
        bail!("Could not enable PortRelay at Windows sign-in");
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("agent.log"))?;
    std::process::Command::new(exe)
        .args(["run", "--no-open"])
        .creation_flags(0x08000000)
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;
    Ok(())
}
