#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let result = (|| -> std::io::Result<()> {
            let exe = std::env::current_exe()?.with_file_name("portrelay.exe");
            let output = std::process::Command::new(exe)
                .arg("desktop")
                .creation_flags(0x08000000)
                .output()?;
            if !output.status.success() {
                return Err(std::io::Error::other(
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                ));
            }
            Ok(())
        })();
        if let Err(error) = result {
            let text: Vec<u16> = error.to_string().encode_utf16().chain(Some(0)).collect();
            let title: Vec<u16> = "PortRelay".encode_utf16().chain(Some(0)).collect();
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                    std::ptr::null_mut(),
                    text.as_ptr(),
                    title.as_ptr(),
                    0x10,
                );
            }
        }
    }
}
