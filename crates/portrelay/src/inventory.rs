use crate::protocol::{Device, valid_bus_id};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub fn inventory() -> Vec<Device> {
    scan(Path::new("/sys/bus/usb/devices"))
}

fn attr(path: &Path, key: &str) -> String {
    fs::read_to_string(path.join(key))
        .unwrap_or_default()
        .trim()
        .to_owned()
}
fn class(path: &Path, key: &str) -> u8 {
    u8::from_str_radix(&attr(path, key), 16).unwrap_or(0)
}
pub fn scan(root: &Path) -> Vec<Device> {
    let Ok(entries) = fs::read_dir(root) else {
        return vec![];
    };
    let mut devices = Vec::new();
    for entry in entries.flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        if !valid_bus_id(&id) {
            continue;
        }
        let p = entry.path();
        let vendor = attr(&p, "idVendor");
        let product = attr(&p, "idProduct");
        if vendor.is_empty() || product.is_empty() {
            continue;
        }
        let mut classes = vec![class(&p, "bDeviceClass")];
        let mut drivers = Vec::new();
        let mut network = false;
        for interface in fs::read_dir(&p).into_iter().flatten().flatten() {
            let q = interface.path();
            if q.join("bInterfaceClass").exists() {
                classes.push(class(&q, "bInterfaceClass"));
                network |= q.join("net").exists();
                if let Ok(driver) = fs::read_link(q.join("driver")) {
                    drivers.push(
                        driver
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
        let canonical = fs::canonicalize(&p).unwrap_or(p.clone());
        let bluetooth = classes.contains(&0xe0) || drivers.iter().any(|d| d == "btusb");
        let blocked = if canonical.to_string_lossy().contains("vhci_hcd") {
            Some("Imported devices cannot be shared again")
        } else if classes.contains(&9) {
            Some("USB hubs stay on this computer")
        } else if classes.contains(&3) {
            Some("Input devices stay on this computer in this alpha")
        } else if classes.contains(&8) {
            Some("Storage sharing is disabled until recovery is validated")
        } else if network
            || drivers.iter().any(|d| {
                [
                    "cdc_ether",
                    "cdc_ncm",
                    "rndis_host",
                    "r8152",
                    "asix",
                    "ax88179_178a",
                ]
                .contains(&d.as_str())
            })
        {
            Some("Network adapters stay on this computer")
        } else if !classes.iter().any(|c| [2, 7, 10, 0xe0, 0xff].contains(c)) {
            Some("This device class is not enabled in this alpha")
        } else {
            None
        };
        let speed = match attr(&p, "speed").as_str() {
            "1.5" => 1,
            "12" => 2,
            "480" => 3,
            "5000" => 5,
            "10000" | "20000" => 6,
            _ => 0,
        };
        let bus: u32 = attr(&p, "busnum").parse().unwrap_or(0);
        let dev: u32 = attr(&p, "devnum").parse().unwrap_or(0);
        let mut h = Sha256::new();
        for value in [
            canonical.to_string_lossy().into_owned(),
            attr(&p, "serial"),
            vendor.clone(),
            product.clone(),
            attr(&p, "busnum"),
            attr(&p, "devnum"),
            fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap_or_default(),
        ] {
            h.update(value.as_bytes());
            h.update([0]);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(m) = fs::metadata(&p) {
                h.update(m.ino().to_be_bytes());
            }
        }
        let name = attr(&p, "product");
        devices.push(Device {
            id,
            generation: hex::encode(h.finalize()),
            name: if name.is_empty() {
                format!("USB {vendor}:{product}")
            } else {
                name.chars().take(120).collect()
            },
            vendor,
            product,
            kind: if bluetooth { "bluetooth" } else { "usb" }.into(),
            speed,
            devid: (bus << 16) | dev,
            blocked: blocked.map(str::to_owned),
        });
    }
    devices.sort_by(|a, b| a.id.cmp(&b.id));
    devices.truncate(128);
    devices
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_never_exports_storage_hubs_or_input() {
        let dir = tempfile::tempdir().unwrap();
        for (id, cls) in [("1-1", "08"), ("1-2", "03"), ("1-3", "09"), ("1-4", "ff")] {
            let p = dir.path().join(id);
            fs::create_dir(&p).unwrap();
            for (k, v) in [
                ("idVendor", "1234"),
                ("idProduct", "5678"),
                ("bDeviceClass", cls),
                ("speed", "480"),
                ("busnum", "1"),
                ("devnum", "2"),
            ] {
                fs::write(p.join(k), v).unwrap();
            }
        }
        let d = scan(dir.path());
        assert_eq!(d.len(), 4);
        assert!(d[..3].iter().all(|x| x.blocked.is_some()));
        assert!(d[3].blocked.is_none());
    }
}
