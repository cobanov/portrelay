#!/usr/bin/env python3
"""CLI acceptance: actual agents for pairing, explicit HTTP fixtures for device UI.
No physical USB attachment or compatibility is claimed by these tests.
"""
import copy
import http.server
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time

binary = Path(sys.argv[1]).resolve()
if os.name == 'nt' and not binary.exists():
    binary = binary.with_suffix('.exe')
assert binary.is_file(), binary


def cli(directory, *args, input='', ok=True):
    result = subprocess.run([str(binary), '--data-dir', str(directory), *args],
                            input=input, text=True, capture_output=True, timeout=75)
    output = result.stdout + result.stderr
    assert (result.returncode == 0) == ok, (args, result.returncode, output)
    return output


def wait_state(directory):
    for _ in range(150):
        if (directory / 'api.json').exists():
            result = subprocess.run([str(binary), '--data-dir', str(directory), 'status'],
                                    text=True, capture_output=True, timeout=5)
            if result.returncode == 0:
                return json.loads(result.stdout)
        time.sleep(.1)
    raise AssertionError('Agent did not become ready')


def actual_pairing(root):
    processes = []
    try:
        dirs = [root / 'owner', root / 'receiver']
        for directory, name in zip(dirs, ['CLI Owner', 'CLI Receiver']):
            directory.mkdir()
            processes.append(subprocess.Popen([str(binary), '--data-dir', str(directory),
                'run', '--name', name, '--bind', '127.0.0.1:0', '--no-open',
                '--helper', str(root / 'missing-helper')], stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL))
            wait_state(directory)
        owner, receiver = dirs
        ticket = cli(owner, 'invite', '--plain').strip()
        assert ticket.startswith('portrelay1:')
        assert 'message' in json.loads(cli(receiver, 'pair', input=ticket + '\n'))
        cli(receiver, 'pair', input=ticket + '\n', ok=False)  # single use
        assert 'waiting for your approval' in cli(owner, 'computers')
        cli(receiver, 'remote', 'CLI Owner', ok=False)
        cli(owner, 'approve', 'CLI Receiver')
        assert 'No devices shared' in cli(receiver, 'remote', 'CLI Owner')
        cli(owner, 'rename', 'Headless Pi')
        assert json.loads(cli(owner, 'status'))['name'] == 'Headless Pi'
        cli(owner, 'revoke', 'CLI Receiver')
        assert not json.loads(cli(owner, 'status'))['peers']
        cli(receiver, 'remote', 'CLI Owner', ok=False)
        cli(owner, 'menu', ok=False)  # pipes must never choose or loop
        print('PASS: real agents: invite, single use, pending approval, approve, remote, rename, revoke')
    finally:
        for process in processes:
            process.terminate()
        for process in processes:
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


peer = 'a' * 64
session_id = 'b' * 24
device = dict(id='1-2', generation='selected-generation', name='Fixture USB disk',
              vendor='1234', product='5678', kind='storage', speed=3, devid=65538,
              blocked=None, risks=['storage'], parent_hub='1-1')
state = dict(name='Fixture Pi', version='test', helper_ready=True, helper_error=None,
             devices=[device], peers={peer: dict(name='Laptop', approved=True, address={'id': peer, 'addrs': []})},
             grants={'1-2': dict(generation=device['generation'], peers=[peer])},
             sessions=[dict(id=session_id, device='1-2', peer=peer, direction='outgoing',
                            state='connected', error=None, metadata=device)], history=[])
actions = []
busy = False
remote_error = False


class Fixture(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def reply(self, value, code=200):
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps(value).encode())

    def authorized(self):
        assert self.headers.get('Authorization') == 'Bearer cli-fixture-token'

    def do_GET(self):
        self.authorized()
        assert self.path == '/api/state'
        self.reply(state)

    def do_POST(self):
        self.authorized()
        assert self.path == '/api/action'
        action = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        actions.append(action)
        if action['op'] == 'remote':
            if remote_error:
                return self.reply({'error': 'offline\x1b[2J\u202e'}, 400)
            return self.reply({'devices': [dict(device, busy=busy)]})
        self.reply({'session': session_id, 'message': 'Fixture action accepted'})


def expect_no_write(directory, args, text):
    before = len(actions)
    assert text in cli(directory, *args, ok=False)
    assert not any(a['op'] != 'remote' for a in actions[before:]), actions[before:]


def terminal_menu(directory):
    if os.name == 'nt':
        print('SKIP: Unix PTY menu; Windows direct CLI tested')
        return
    import pty
    import select
    master, slave = pty.openpty()
    process = subprocess.Popen([str(binary), '--data-dir', str(directory), 'menu'],
                               stdin=slave, stdout=slave, stderr=slave, close_fds=True)
    os.close(slave)
    pending = b''

    def until(text):
        nonlocal pending
        needle = text.encode()
        deadline = time.monotonic() + 15
        while needle not in pending:
            assert time.monotonic() < deadline, (text, pending.decode(errors='replace'))
            ready, _, _ = select.select([master], [], [], .2)
            if ready:
                pending += os.read(master, 65536)
        pending = pending.split(needle, 1)[1]

    def send(text):
        os.write(master, (text + '\n').encode())

    try:
        until('Choose: ')
        send('99')
        until('Enter a number')
        until('Choose: ')
        before = len(actions)
        send('5')
        until('Number (0 cancels): ')
        send('0')
        until('Cancelled')
        until('Choose: ')
        assert len(actions) == before
        send('5')
        until('Number (0 cancels): ')
        send('1')
        until('Number (0 cancels): ')
        send('1')
        until('Type yes (Enter cancels): ')
        send('')
        until('Cancelled; device was not shared')
        until('Choose: ')
        assert len(actions) == before
        send('5')
        until('Number (0 cancels): ')
        send('1')
        until('Number (0 cancels): ')
        send('1')
        until('Type yes (Enter cancels): ')
        send('yes')
        until('Shared Fixture USB disk with Laptop')
        until('Choose: ')
        assert actions[-1] == dict(op='share', device='1-2', generation=device['generation'],
                                   peer=peer, acknowledge_risks=['storage'])
        send('8')
        until('Number (0 cancels): ')
        send('1')
        until('Type yes (Enter cancels): ')
        send('yes')
        until('Disconnect requested')
        until('Choose: ')
        assert actions[-1] == dict(op='disconnect', session=session_id)
        os.write(master, b'\x04')  # EOF must leave the menu, not loop.
        assert process.wait(timeout=10) == 0
        print('PASS: PTY menu: invalid selection, cancel, risk refusal, explicit share, eject, EOF')
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)


def device_controls(root):
    global busy, remote_error
    directory = root / 'fixture'
    directory.mkdir()
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    (directory / 'api.json').write_text(json.dumps(dict(port=server.server_port, token='cli-fixture-token')))
    try:
        assert 'Shared with: Laptop' in cli(directory, 'devices')
        assert session_id in cli(directory, 'sessions')
        expect_no_write(directory, ['share', '1-2', '--to', 'Laptop'], '--acknowledge storage')
        expect_no_write(directory, ['share', '1-2', '--to', 'Laptop', '--acknowledge', 'input'], '--acknowledge storage')
        cli(directory, 'share', '1-2', '--to', peer[:8], '--acknowledge', 'storage')
        assert actions[-1]['generation'] == device['generation']
        assert actions[-1]['acknowledge_risks'] == ['storage']
        device['blocked'] = 'Disk is mounted'
        expect_no_write(directory, ['share', '1-2', '--to', 'Laptop', '--acknowledge', 'storage'], 'Disk is mounted')
        device['blocked'] = None
        expect_no_write(directory, ['share'], 'Specify local device')
        second = 'a' * 8 + 'c' * 56
        state['peers'][second] = copy.deepcopy(state['peers'][peer])
        expect_no_write(directory, ['approve', 'Laptop'], 'more than one')
        expect_no_write(directory, ['approve', peer[:8]], 'more than one')
        del state['peers'][second]
        cli(directory, 'connect', 'Laptop', '--device', '1-2')
        assert actions[-1] == dict(op='connect', peer=peer, device='1-2', generation=device['generation'])
        busy = True
        expect_no_write(directory, ['connect', 'Laptop', '--device', '1-2'], 'already in use')
        busy = False
        remote_error = True
        output = cli(directory, 'remote', 'Laptop', ok=False)
        assert '\x1b' not in output and '\u202e' not in output
        remote_error = False
        for args in [['disconnect', session_id], ['unshare', '1-2'], ['revoke', 'Laptop']]:
            expect_no_write(directory, args, '--ejected')
            cli(directory, *args, '--ejected')
        terminal_menu(directory)
        print('PASS: HTTP fixtures: selections, generation, consent, blocked/busy devices, eject, sanitization')
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


with tempfile.TemporaryDirectory(prefix='portrelay-cli-') as tmp:
    root = Path(tmp)
    actual_pairing(root)
    device_controls(root)
