#!/usr/bin/env python3
"""Real Windows/Linux USB tests on explicitly prepared, disposable machines.

Requires PortRelay agents, helpers, and virtual serial fixtures. Does not install
drivers or select physical USB devices. See docs/validation.md for the record.
Invitations and API credentials are never written to the test log.
"""
import argparse
import base64
import importlib.util
import json
from pathlib import Path
import shlex
import subprocess
import time

spec = importlib.util.spec_from_file_location("linux_smoke", Path(__file__).with_name("two-host-smoke.py"))
linux = importlib.util.module_from_spec(spec)
spec.loader.exec_module(linux)


class Machine:
    def __init__(self, host, options, windows_user=None):
        self.host = host
        self.windows = windows_user is not None
        self.ssh = ["ssh", *options, "-o", "BatchMode=yes", "-o", "ConnectTimeout=8"]
        if windows_user:
            self.ssh += ["-l", windows_user]
        self.ssh += [host]

    def run(self, command, data=None, timeout=70):
        result = subprocess.run([*self.ssh, command], input=data, text=True, capture_output=True, timeout=timeout)
        if result.returncode:
            raise RuntimeError(f"{self.host}: {result.stderr.strip()} {result.stdout.strip()}")
        return result.stdout.strip()

    def ps(self, code):
        encoded = base64.b64encode(("$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';" + code + ";exit 0").encode("utf-16le")).decode()
        return self.run("powershell -NoProfile -NonInteractive -EncodedCommand " + encoded)

    def api(self, action=None):
        exe = "& 'C:\\Program Files\\PortRelay\\portrelay.exe'" if self.windows else "/usr/local/bin/portrelay"
        return json.loads(self.run(exe + (" status" if action is None else " api"), None if action is None else json.dumps(action)))

    def tty(self):
        if self.windows:
            return self.ps("$ports=[IO.Ports.SerialPort]::GetPortNames(); Get-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Enum\\USB\\VID_1D6B&PID_0104*\\*\\Device Parameters' -Name PortName -ErrorAction SilentlyContinue | Where-Object {$_.PortName -in $ports} | Select-Object -ExpandProperty PortName")
        code = linux.FIND_TTY.replace("ttyACM*", "ttyUSB*").replace("'1d6b'", "'0403'").replace("'0104'", "'6001'")
        paths = json.loads(self.run("python3 -c " + shlex.quote(code)))
        return paths[0] if paths else None

    def transfer(self, port):
        if not self.windows:
            return json.loads(self.run("sudo -n python3 -c " + shlex.quote(linux.TRANSFER), json.dumps(port)))
        assert port.startswith("COM") and port[3:].isdigit()
        code = r'''
$serial=[IO.Ports.SerialPort]::new('PORT',115200,[IO.Ports.Parity]::None,8,[IO.Ports.StopBits]::One)
$serial.ReadTimeout=15000; $serial.WriteTimeout=15000; $serial.DtrEnable=$true
$payload=[byte[]]::new(65536); for($i=0;$i -lt $payload.Length;$i++){$payload[$i]=$i % 256}
$received=[byte[]]::new($payload.Length)
try {
 $serial.Open()
 for($offset=0;$offset -lt $payload.Length;$offset+=512){
  $serial.Write($payload,$offset,512)
  $count=0; while($count -lt 512){$count+=$serial.Read($received,$offset+$count,512-$count)}
 }
 for($i=0;$i -lt $payload.Length;$i++){if($payload[$i] -ne $received[$i]){throw "Payload mismatch at $i"}}
 $hash=[BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash($received)).Replace('-','').ToLowerInvariant()
 @{bytes=$received.Length;sha256=$hash;matched=$true}|ConvertTo-Json -Compress
} finally {$serial.Dispose()}
'''.replace("PORT", port)
        return json.loads(self.ps(code))


def require_failure(fn, description):
    try:
        fn()
    except RuntimeError:
        return
    raise RuntimeError(description)


def connect(owner, client, device):
    return client.api({"op": "connect", "peer": owner.api()["id"], "device": device["id"], "generation": device["generation"]})


def clean(owner, client):
    linux.wait_for(lambda: not owner.api()["sessions"] and not client.api()["sessions"], 40)
    linux.wait_for(lambda: not client.tty(), 30)
    for machine in [owner, client]:
        state = machine.api()
        assert state["helper_ready"], state.get("helper_error")
        assert not any(s["error"] for s in state["history"]), state["history"]


def exercise(owner, client, address, fixture, cycles):
    initial = owner.api()
    assert not initial["sessions"] and not client.api()["sessions"], "Disconnect existing test sessions first"
    if client.api()["id"] in initial["peers"]:
        owner.api({"op": "revoke", "peer": client.api()["id"]})
    device = next(d for d in initial["devices"] if (d["vendor"], d["product"]) == fixture)
    assert device["blocked"] is None, device["blocked"]
    ticket = owner.api({"op": "invite"})["invitation"]
    encoded = ticket.split(":", 1)[1]
    invitation = json.loads(base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4)))
    if address:
        invitation["address"]["addrs"] = [{"Ip": address}]
    ticket = "portrelay1:" + base64.urlsafe_b64encode(json.dumps(invitation).encode()).decode().rstrip("=")
    client.api({"op": "pair", "invitation": ticket})
    owner_id, client_id = initial["id"], client.api()["id"]
    require_failure(lambda: client.api({"op": "remote", "peer": owner_id}), "Unapproved peer could list devices")
    owner.api({"op": "approve", "peer": client_id})
    assert client.api({"op": "remote", "peer": owner_id})["devices"] == []
    owner.api({"op": "share", "peer": client_id, "device": device["id"]})
    connection = connect(owner, client, device)
    echo = None
    try:
        port = linux.wait_for(client.tty, 40)
        require_failure(lambda: connect(owner, client, device), "A second session could claim the same device")
        if not owner.windows:
            echo = subprocess.Popen([*owner.ssh, "sudo -n python3 -c " + shlex.quote(linux.ECHO)], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            time.sleep(.5)
        result = client.transfer(port)
        print(json.dumps({"check": "kernel_usb_roundtrip", "owner": owner.host, "client": client.host, **result}), flush=True)
    finally:
        if echo:
            echo.terminate()
            try:
                echo.wait(timeout=3)
            except subprocess.TimeoutExpired:
                echo.kill()
        client.api({"op": "disconnect", "session": connection["session"]})
    clean(owner, client)
    print(json.dumps({"check": "disconnect_cleanup", "passed": True}), flush=True)
    for _ in range(cycles):
        connection = connect(owner, client, device)
        linux.wait_for(client.tty, 40)
        client.api({"op": "disconnect", "session": connection["session"]})
        clean(owner, client)
    print(json.dumps({"check": "connection_cycles", "cycles": cycles, "passed": True}), flush=True)
    connect(owner, client, device)
    linux.wait_for(client.tty, 40)
    owner.api({"op": "revoke", "peer": client_id})
    clean(owner, client)
    print(json.dumps({"check": "revocation_cleanup", "passed": True}), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--confirm-disposable", action="store_true", required=True, help="Confirm both machines and serial devices are disposable fixtures")
    parser.add_argument("--ssh-config", required=True)
    parser.add_argument("--windows", required=True)
    parser.add_argument("--windows-user", required=True)
    parser.add_argument("--linux", required=True)
    parser.add_argument("--direction", choices=["export", "import"], required=True, help="Windows role")
    parser.add_argument("--owner-address", help="Explicit UDP mapping only for an isolated NAT test fixture")
    parser.add_argument("--cycles", type=int, default=3)
    args = parser.parse_args()
    options = ["-F", args.ssh_config, "-o", "ControlMaster=auto", "-o", "ControlPersist=60", "-o", "ControlPath=/tmp/portrelay-windows-smoke-%C"]
    windows = Machine(args.windows, options, args.windows_user)
    other = Machine(args.linux, options)
    owner, client = (windows, other) if args.direction == "export" else (other, windows)
    fixture = ("0403", "6001") if args.direction == "export" else ("1d6b", "0104")
    exercise(owner, client, args.owner_address, fixture, args.cycles)


if __name__ == "__main__":
    main()
