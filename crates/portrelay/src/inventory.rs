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
        let mut network_active = false;
        let mut bluetooth = class(&p, "bDeviceClass") == 0xe0
            && class(&p, "bDeviceSubClass") == 1
            && class(&p, "bDeviceProtocol") == 1;
        for interface in fs::read_dir(&p).into_iter().flatten().flatten() {
            let q = interface.path();
            if q.join("bInterfaceClass").exists() {
                classes.push(class(&q, "bInterfaceClass"));
                network |= q.join("net").exists();
                for net in fs::read_dir(q.join("net")).into_iter().flatten().flatten() {
                    // IFF_UP: require the owner to disable the interface first, even
                    // when it has no carrier. This also protects non-default routes.
                    let flags = attr(&net.path(), "flags");
                    network_active |= u32::from_str_radix(flags.trim_start_matches("0x"), 16)
                        .map_or(true, |flags| flags & 1 != 0);
                }
                bluetooth |= class(&q, "bInterfaceClass") == 0xe0
                    && class(&q, "bInterfaceSubClass") == 1
                    && class(&q, "bInterfaceProtocol") == 1;
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
        bluetooth |= drivers.iter().any(|d| d == "btusb");
        network |= drivers.iter().any(|d| {
            [
                "cdc_ether",
                "cdc_ncm",
                "rndis_host",
                "r8152",
                "asix",
                "ax88179_178a",
            ]
            .contains(&d.as_str())
        });
        // RNDIS uses wireless-controller class e0 too, but is not Bluetooth.
        network |= class(&p, "bDeviceClass") == 0xe0 && class(&p, "bDeviceSubClass") == 4;
        let storage = classes.contains(&8);
        let input = classes.contains(&3);
        let hub = classes.contains(&9);
        let risks: Vec<String> = [
            (storage, "storage"),
            (input, "input"),
            (network, "network"),
            (bluetooth, "bluetooth"),
        ]
        .into_iter()
        .filter(|(present, _)| *present)
        .map(|(_, risk)| risk.to_owned())
        .collect();
        let blocked = if canonical.to_string_lossy().contains("vhci_hcd") {
            Some("Imported devices cannot be shared again".into())
        } else if hub {
            Some("Select the devices connected to this hub".into())
        } else if network_active {
            Some("Disable this network adapter in system settings before sharing it".into())
        } else if storage {
            storage_blocked(&p)
        } else if !classes
            .iter()
            .any(|c| [2, 3, 7, 8, 10, 0xe0, 0xff].contains(c))
        {
            Some("This device class is not enabled in this alpha".into())
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
            parent_hub: id.rsplit_once('.').map(|(parent, _)| parent.to_owned()),
            id,
            generation: hex::encode(h.finalize()),
            name: if name.is_empty() {
                format!("USB {vendor}:{product}")
            } else {
                name.chars().take(120).collect()
            },
            vendor,
            product,
            kind: if hub {
                "hub"
            } else if bluetooth {
                "bluetooth"
            } else if storage {
                "storage"
            } else if input {
                "input"
            } else if network {
                "network"
            } else {
                "usb"
            }
            .into(),
            speed,
            devid: (bus << 16) | dev,
            blocked,
            risks,
        });
    }
    devices.sort_by(|a, b| a.id.cmp(&b.id));
    devices.truncate(128);
    devices
}

/// Walk only real descendants, never symlinks back into the global device tree.
fn block_nodes(
    path: &Path,
    depth: usize,
    result: &mut Vec<std::path::PathBuf>,
) -> std::io::Result<()> {
    if depth > 16 {
        return Err(std::io::Error::other("Device tree is too deep"));
    }
    if path.join("dev").exists() && (path.join("partition").exists() || path.join("size").exists())
    {
        result.push(path.to_owned());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            block_nodes(&entry.path(), depth + 1, result)?;
        }
    }
    Ok(())
}
fn check_storage(path: &Path, mount_tables: &[String], swaps: &str) -> Result<(), String> {
    let mut nodes = Vec::new();
    block_nodes(path, 0, &mut nodes).map_err(|_| "Disk usage could not be verified")?;
    if nodes.is_empty() {
        return Err("Wait for the disk to appear, then unmount its volumes before sharing".into());
    }
    for node in nodes {
        let dev = attr(&node, "dev");
        if dev.is_empty() {
            return Err("Disk identity could not be verified".into());
        }
        if mount_tables.iter().any(|table| {
            table
                .lines()
                .any(|line| line.split_whitespace().nth(2) == Some(dev.as_str()))
        }) {
            return Err("Unmount every volume on this disk before sharing it".into());
        }
        let holders =
            fs::read_dir(node.join("holders")).map_err(|_| "Disk usage could not be verified")?;
        if holders.count() != 0 {
            return Err(
                "This disk is used by a storage pool, encrypted volume or another block device"
                    .into(),
            );
        }
        #[cfg(target_os = "linux")]
        for swap in swaps
            .lines()
            .skip(1)
            .filter_map(|l| l.split_whitespace().next())
        {
            use std::os::unix::fs::{FileTypeExt, MetadataExt};
            let meta = fs::metadata(swap).map_err(|_| "Swap usage could not be verified")?;
            // A swap file is protected by its mount; a swap partition by rdev.
            if meta.file_type().is_block_device() {
                let id = format!(
                    "{}:{}",
                    nix::sys::stat::major(meta.rdev()),
                    nix::sys::stat::minor(meta.rdev())
                );
                if id == dev {
                    return Err("Disable swap on this disk before sharing it".into());
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = swaps;
    }
    Ok(())
}
fn storage_blocked(path: &Path) -> Option<String> {
    let tables = ["/proc/self/mountinfo", "/proc/1/mountinfo"]
        .iter()
        .map(fs::read_to_string)
        .collect::<Result<Vec<_>, _>>();
    let swaps = fs::read_to_string("/proc/swaps");
    match (tables, swaps) {
        (Ok(tables), Ok(swaps)) => check_storage(path, &tables, &swaps).err(),
        _ => Some("Disk usage could not be verified; check the USB service permissions".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn device(root: &Path, id: &str, cls: &str) -> std::path::PathBuf {
        let p = root.join(id);
        fs::create_dir_all(&p).unwrap();
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
        p
    }
    #[test]
    fn input_needs_consent_hubs_are_groups_and_rndis_is_not_bluetooth() {
        let dir = tempfile::tempdir().unwrap();
        device(dir.path(), "1-1", "03");
        device(dir.path(), "1-2", "09");
        let bt = device(dir.path(), "1-3", "e0");
        fs::write(bt.join("bDeviceSubClass"), "01").unwrap();
        fs::write(bt.join("bDeviceProtocol"), "01").unwrap();
        let net = device(dir.path(), "1-4", "e0");
        fs::write(net.join("bDeviceSubClass"), "04").unwrap();
        let devices = scan(dir.path());
        assert!(devices[0].blocked.is_none());
        assert_eq!(devices[0].risks, ["input"]);
        assert_eq!(devices[1].kind, "hub");
        assert!(devices[1].blocked.is_some());
        assert_eq!(devices[2].risks, ["bluetooth"]);
        assert_eq!(devices[3].kind, "network");
    }
    #[test]
    fn composite_input_network_checks_admin_state() {
        let dir = tempfile::tempdir().unwrap();
        let p = device(dir.path(), "1-1", "03");
        let interface = p.join("interface0");
        fs::create_dir_all(interface.join("net/usb0")).unwrap();
        fs::write(interface.join("bInterfaceClass"), "02").unwrap();
        fs::write(interface.join("net/usb0/flags"), "0x1003").unwrap();
        let d = scan(dir.path()).remove(0);
        assert!(d.blocked.unwrap().contains("Disable"));
        assert_eq!(d.risks, ["input", "network"]);
        fs::write(interface.join("net/usb0/flags"), "0x1002").unwrap();
        assert!(scan(dir.path())[0].blocked.is_none());
    }
    #[test]
    fn storage_checks_all_partitions_and_stacked_users() {
        let dir = tempfile::tempdir().unwrap();
        let disk = dir.path().join("block/sda");
        let part = disk.join("sda1");
        fs::create_dir_all(part.join("holders")).unwrap();
        fs::create_dir(disk.join("holders")).unwrap();
        for (p, dev) in [(&disk, "8:0"), (&part, "8:1")] {
            fs::write(p.join("dev"), dev).unwrap();
            fs::write(p.join("size"), "100").unwrap();
        }
        assert!(check_storage(dir.path(), &[], "Filename Type").is_ok());
        assert!(
            check_storage(
                dir.path(),
                &["31 22 8:1 / /data rw - ext4 /dev/sda1 rw".into()],
                "Filename Type"
            )
            .unwrap_err()
            .contains("Unmount")
        );
        fs::write(part.join("holders/dm-0"), "").unwrap();
        assert!(
            check_storage(dir.path(), &[], "Filename Type")
                .unwrap_err()
                .contains("storage pool")
        );
        assert!(check_storage(tempfile::tempdir().unwrap().path(), &[], "").is_err());
    }
}
