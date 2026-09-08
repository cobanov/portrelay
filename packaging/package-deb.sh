#!/bin/sh
set -eu
binary=${1:-target/release/portrelay}
distro=${2:-debian}
case "$distro" in debian) usb_dependencies=usbip ;; ubuntu) usb_dependencies=linux-tools-common ;; *) echo 'Choose debian or ubuntu' >&2; exit 1 ;; esac
version=0.1.0~alpha.3
name=portrelay-0.1.0-alpha.3-$distro-amd64
mkdir -p target/deb
stage=$(mktemp -d "target/deb/$name.XXXXXX")
mkdir -p "$stage/DEBIAN" "$stage/usr/lib/portrelay" "$stage/usr/bin" "$stage/usr/lib/systemd/system" "$stage/usr/lib/systemd/user" "$stage/usr/share/applications" "$stage/usr/share/icons/hicolor/scalable/apps" "$stage/usr/share/polkit-1/actions" "$stage/usr/share/doc/portrelay"
install -m755 "$binary" "$stage/usr/lib/portrelay/portrelay"
ln -s ../lib/portrelay/portrelay "$stage/usr/bin/portrelay"
install -m755 packaging/deb/setup-usb "$stage/usr/lib/portrelay/setup-usb"
cp packaging/deb/portrelay-helper.service "$stage/usr/lib/systemd/system/"
cp packaging/deb/portrelay.service "$stage/usr/lib/systemd/user/"
cp packaging/deb/dev.cobanov.portrelay.setup.policy "$stage/usr/share/polkit-1/actions/"
cp web/favicon.svg "$stage/usr/share/icons/hicolor/scalable/apps/portrelay.svg"
cp LICENSE "$stage/usr/share/doc/portrelay/copyright"
cp target/packages/THIRD_PARTY_NOTICES.txt target/packages/SBOM.cdx.json Cargo.lock "$stage/usr/share/doc/portrelay/"
cp docs/linux-alpha.md docs/validation.md "$stage/usr/share/doc/portrelay/"
for script in preinst postinst prerm postrm; do install -m755 "packaging/deb/$script" "$stage/DEBIAN/$script"; done
cat > "$stage/DEBIAN/control" <<CONTROL
Package: portrelay
Version: $version
Architecture: amd64
Maintainer: Mert Cobanov <mertcobanov@gmail.com>
Section: net
Priority: optional
Depends: libc6 (>= 2.36), systemd, dbus-user-session, pkexec, polkitd, xdg-utils, kmod, util-linux, $usb_dependencies
Homepage: https://portrelay.cobanov.dev
Description: Encrypted USB sharing between your Linux computers
 Pair computers, choose a device, and connect through a local control window.
 Experimental alpha; physical USB and Bluetooth compatibility is unvalidated.
CONTROL
cat > "$stage/usr/share/applications/portrelay.desktop" <<'DESKTOP'
[Desktop Entry]
Name=PortRelay
Comment=Share devices between your computers
Exec=/usr/bin/portrelay desktop
Icon=portrelay
Terminal=false
Type=Application
Categories=Network;Utility;
StartupNotify=false
DESKTOP
mkdir -p target/packages
dpkg-deb --root-owner-group --build "$stage" "target/packages/$name.deb"
(cd target/packages && shasum -a 256 "$name.deb" > "$name.deb.sha256")
