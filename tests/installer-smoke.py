#!/usr/bin/env python3
"""Exercise package setup in a fresh disposable Linux VM, without a browser.

Requires pexpect, an installed .deb, an active PortRelay user service, and a
password-enabled sudo-group test account. Never use a production workstation.
The password is sent only to the OS authentication agent; it is never logged.
"""
import argparse, getpass, json, pathlib, subprocess, time
import pexpect

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--confirm-disposable', action='store_true', required=True)
parser.add_argument('--password-file', type=pathlib.Path, required=True)
args = parser.parse_args()

def state(): return json.loads(subprocess.check_output(['portrelay', 'status']))

def setup(cancel):
    pid = int(subprocess.check_output(['systemctl','--user','show','portrelay.service','-p','MainPID','--value']))
    start = pathlib.Path(f'/proc/{pid}/stat').read_text().split(') ',1)[1].split()[19]
    auth = pexpect.spawn('/usr/bin/pkttyagent', ['--process', f'{pid},{start}'], encoding='utf-8', echo=False, timeout=40)
    try:
        time.sleep(1)
        assert auth.isalive(), 'Authentication agent failed to register'
        action = subprocess.run(['portrelay','api'], input='{"op":"setup_usb"}', text=True, capture_output=True, check=True)
        auth.expect('Password:')
        assert getpass.getuser() in auth.before, 'Unexpected administrator identity'
        if cancel:
            auth.sendcontrol('c')
        else:
            auth.sendline(args.password_file.read_text().strip())
            auth.expect('AUTHENTICATION COMPLETE', timeout=60)
        for _ in range(330):
            current = state()
            if not current['setup']['running']:
                if cancel:
                    assert current['setup']['error'] and not current['helper_ready'], current['setup']
                    print('OS permission cancellation: PASS')
                else:
                    assert not current['setup']['error'] and current['helper_ready'], current['setup']
                    print('API setup + real polkit password authentication + helper ready: PASS')
                return
            time.sleep(1)
        raise AssertionError('Setup did not finish')
    finally: auth.close(force=True)
assert not state()['helper_ready'], 'Use a fresh installation before testing cancellation'
setup(True)
setup(False)
subprocess.run(['portrelay','check'], check=True, stdout=subprocess.DEVNULL)
print('No physical devices attached; setup path only.')
