#!/bin/sh
set -eu
binary=${1:-target/release/portrelay}
distro=${2:-debian}
arch=${3:-$(dpkg --print-architecture)}
case "$arch" in amd64|arm64|armhf) ;; *) echo 'Choose amd64, arm64, or armhf' >&2; exit 1 ;; esac
case "$distro" in debian) usb_dependencies=usbip ;; ubuntu) usb_dependencies=linux-tools-common ;; *) echo 'Choose debian or ubuntu' >&2; exit 1 ;; esac
[ "$distro:$arch" != ubuntu:armhf ] || { echo 'Ubuntu armhf is not a release target.' >&2; exit 1; }
release_version=$(python3 scripts/check-linux-binary.py "$binary" "$arch")
version=$(printf '%s' "$release_version" | sed 's/-/~/')
name=portrelay-$release_version-$distro-$arch
mkdir -p target/deb
stage=$(mktemp -d "target/deb/$name.XXXXXX")
mkdir -p "$stage/DEBIAN" "$stage/usr/lib/portrelay" "$stage/usr/bin" "$stage/usr/lib/systemd/system" "$stage/usr/lib/systemd/user" "$stage/usr/share/applications" "$stage/usr/share/icons/hicolor/scalable/apps" "$stage/usr/share/polkit-1/actions" "$stage/usr/share/doc/portrelay"
install -m755 "$binary" "$stage/usr/lib/portrelay/portrelay"
ln -s ../lib/portrelay/portrelay "$stage/usr/bin/portrelay"
install -m755 packaging/deb/setup-usb "$stage/usr/lib/portrelay/setup-usb"
install -m755 packaging/deb/setup-input "$stage/usr/lib/portrelay/setup-input"
cp packaging/deb/portrelay-input.service "$stage/usr/lib/systemd/system/"
install -m755 packaging/deb/setup-headless "$stage/usr/lib/portrelay/setup-headless"
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
Architecture: $arch
Maintainer: Mert Cobanov <mertcobanov@gmail.com>
Section: net
Priority: optional
Depends: libc6 (>= 2.36), systemd, libpam-systemd, dbus-user-session, pkexec, polkitd, xdg-utils, kmod, util-linux, $usb_dependencies
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
