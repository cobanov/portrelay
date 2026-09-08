use anyhow::{Result, bail};

/// Fixed system-settings entry points, run only as the interactive owner.
pub fn open_bluetooth() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        for (program, args) in [
            ("/usr/bin/gnome-control-center", vec!["bluetooth"]),
            ("/usr/bin/blueman-manager", vec![]),
            ("/usr/bin/systemsettings", vec!["kcm_bluetooth"]),
            ("/usr/bin/systemsettings5", vec!["kcm_bluetooth"]),
        ] {
            if std::path::Path::new(program).is_file() {
                std::process::Command::new(program).args(args).spawn()?;
                return Ok(());
            }
        }
    }
    #[cfg(windows)]
    {
        crate::windows::open_bluetooth_settings()?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    {
        bail!(
            "Open your operating system's Bluetooth settings and select the borrowed adapter. On Linux, install your desktop's Bluetooth settings application if needed."
        )
    }
}
