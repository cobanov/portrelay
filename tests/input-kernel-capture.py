#!/usr/bin/env python3
"""Capture ONLY PortRelay's virtual input, grabbing it away from the desktop.

Requires an isolated helper named portrelay-input-test.service. On timeout or
failure stop that test helper BEFORE releasing grabs. Never reads physical input.
"""
import errno
import fcntl
import json
import os
from pathlib import Path
import select
import struct
import subprocess
import sys
import time

assert os.geteuid() == 0
output = Path(sys.argv[1])
assert str(output).startswith('/tmp/portrelay-input-')
assert subprocess.check_output(['systemctl','show','portrelay-input-test','-p','ActiveState','--value'],text=True).strip() == 'active'
opened = {}; events = []; ready = False; removed = set()
deadline = time.monotonic() + 90
event_format = struct.Struct('@llHHi')
try:
    while time.monotonic() < deadline:
        for entry in Path('/sys/class/input').glob('event*'):
            if '/devices/virtual/input/' not in str(entry.resolve()): continue
            name = (entry/'device/name').read_text().strip()
            if name not in ('PortRelay keyboard','PortRelay pointer') or name in opened: continue
            fd = os.open('/dev/input/'+entry.name, os.O_RDONLY | os.O_NONBLOCK)
            fcntl.ioctl(fd, 0x40044590, 1)  # EVIOCGRAB, just this new virtual device.
            opened[name] = fd
        if len(opened) == 2 and not ready:
            output.with_suffix('.ready').write_text('PortRelay virtual keyboard and pointer exclusively grabbed\n')
            ready = True
        live = [fd for fd in opened.values() if fd not in removed]
        for fd in select.select(live, [], [], .02)[0]:
            try:
                raw = os.read(fd, event_format.size * 256)
                for offset in range(0,len(raw),event_format.size):
                    _,_,kind,code,value = event_format.unpack_from(raw,offset)
                    if kind and len(events) < 2048: events.append([kind,code,value])
            except OSError as error:
                if error.errno == errno.ENODEV: removed.add(fd)
                elif error.errno != errno.EAGAIN: raise
        if ready and len(removed) == 2:
            output.write_text(json.dumps({'events':events,'devices_removed':True})+'\n')
            break
    else: raise RuntimeError('Capture expired')
finally:
    if len(removed) != 2:
        subprocess.run(['systemctl','stop','portrelay-input-test'],check=True)
    for fd in opened.values(): os.close(fd)
