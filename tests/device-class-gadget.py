#!/usr/bin/env python3
"""Create only named virtual USB fixtures inside an explicitly disposable Linux VM."""
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path('/sys/kernel/config/usb_gadget/portrelay_class_test')
IMAGE = pathlib.Path('/var/lib/portrelay-qa/disk.img')

def write(path, value):
    path.write_bytes(value if isinstance(value, bytes) else value.encode())

def main():
    if os.geteuid() != 0 or os.environ.get('PORTRELAY_DISPOSABLE_VM') != '1':
        raise SystemExit('Requires root and PORTRELAY_DISPOSABLE_VM=1 in a disposable VM. Never run on a workstation.')
    if sys.argv[1:] == ['stop']:
        if not ROOT.exists(): return
        write(ROOT/'UDC', '\n')
        for link in (ROOT/'configs/c.1').iterdir():
            if link.is_symlink(): link.unlink()
        (ROOT/'configs/c.1').rmdir()
        for function in (ROOT/'functions').iterdir(): function.rmdir()
        (ROOT/'strings/0x409').rmdir(); ROOT.rmdir()
        return
    if len(sys.argv) != 2 or sys.argv[1] not in ('storage', 'input', 'network'):
        raise SystemExit('Usage: device-class-gadget.py storage|input|network|stop')
    kind = sys.argv[1]
    if ROOT.exists(): raise SystemExit('Fixture already exists; stop it first.')
    subprocess.run(['modprobe', 'libcomposite'], check=True)
    subprocess.run(['modprobe', 'dummy_hcd'], check=True)
    if not pathlib.Path('/sys/class/udc/dummy_udc.0').exists(): raise SystemExit('No dummy USB controller')
    ROOT.mkdir(); write(ROOT/'idVendor', '0x1d6b'); write(ROOT/'idProduct', {'storage':'0x0105','input':'0x0106','network':'0x0107'}[kind])
    write(ROOT/'bcdUSB', '0x0200')
    (ROOT/'strings/0x409').mkdir()
    for key, value in [('serialnumber','PORTRELAY-CLASS-TEST-ONLY'),('manufacturer','PortRelay tests'),('product','Virtual '+kind+' fixture')]: write(ROOT/'strings/0x409'/key, value)
    (ROOT/'configs/c.1').mkdir()
    functions = []
    if kind == 'storage':
        if not IMAGE.exists():
            IMAGE.parent.mkdir(parents=True, exist_ok=True)
            with IMAGE.open('xb') as file: file.truncate(32*1024*1024)
            subprocess.run(['mkfs.ext4', '-q', '-F', str(IMAGE)], check=True)
        function = ROOT/'functions/mass_storage.usb0'; function.mkdir()
        write(function/'lun.0/removable', '1'); write(function/'lun.0/file', str(IMAGE)); functions.append(function)
    elif kind == 'input':
        # Boot keyboard and relative mouse. Events are delivered only inside the VM.
        for name, protocol, length, desc in [
            ('keyboard', '1', '8', '05010906a101050719e029e71500250175019508810295017508810195057501050819012905910295017503910195067508150025650507190029658100c0'),
            ('mouse', '2', '3', '05010902a1010901a100050919012903150025019503750181029501750581010501093009311581257f750895028106c0c0'),
        ]:
            function = ROOT/('functions/hid.'+name); function.mkdir()
            write(function/'protocol', protocol); write(function/'subclass', '1'); write(function/'report_length', length)
            write(function/'report_desc', bytes.fromhex(desc)); functions.append(function)
    else:
        function = ROOT/'functions/ecm.usb0'; function.mkdir()
        write(function/'host_addr','02:00:00:00:04:01'); write(function/'dev_addr','02:00:00:00:04:02'); functions.append(function)
    for function in functions: (ROOT/'configs/c.1'/function.name).symlink_to(function)
    write(ROOT/'UDC', 'dummy_udc.0')
    print('Virtual '+kind+' fixture ready')

if __name__ == '__main__': main()
