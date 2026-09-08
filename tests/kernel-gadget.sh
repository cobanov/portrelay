#!/bin/sh
# Linux-only, explicit virtual USB fixture. Never binds a physical device.
set -eu
[ "$(id -u)" -eq 0 ] || { echo 'Run with sudo.' >&2; exit 1; }
gadget=/sys/kernel/config/usb_gadget/portrelay_test
case "${1:-}" in
  start)
    [ ! -d "$gadget" ] || { echo 'Fixture already exists; refusing to overwrite it.' >&2; exit 1; }
    modprobe libcomposite
    modprobe dummy_hcd
    test_udc=dummy_udc.0
    [ -d "/sys/class/udc/$test_udc" ] || { echo 'Use an isolated test VM with the dummy_hcd kernel module.' >&2; exit 1; }
    mkdir "$gadget"
    printf '0x1d6b' > "$gadget/idVendor"
    printf '0x0104' > "$gadget/idProduct"
    printf '0x0200' > "$gadget/bcdUSB"
    mkdir "$gadget/strings/0x409"
    printf 'PORTRELAY-TEST-ONLY' > "$gadget/strings/0x409/serialnumber"
    printf 'PortRelay tests' > "$gadget/strings/0x409/manufacturer"
    printf 'Virtual USB serial fixture' > "$gadget/strings/0x409/product"
    mkdir "$gadget/configs/c.1"
    mkdir "$gadget/functions/acm.usb0"
    ln -s "$gadget/functions/acm.usb0" "$gadget/configs/c.1/acm.usb0"
    printf '%s' "$test_udc" > "$gadget/UDC"
    echo 'Kernel serial gadget ready. It uses the normal USB/IP device backend.'
    ;;
  stop)
    [ -d "$gadget" ] || exit 0
    printf '\n' > "$gadget/UDC"
    rm "$gadget/configs/c.1/acm.usb0"
    rmdir "$gadget/configs/c.1" "$gadget/functions/acm.usb0" "$gadget/strings/0x409" "$gadget"
    echo 'PortRelay test gadget removed. Shared kernel modules were left loaded.'
    ;;
  *) echo 'Usage: kernel-gadget.sh start|stop' >&2; exit 1 ;;
esac
