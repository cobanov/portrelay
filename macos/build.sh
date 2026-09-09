#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
[ "$(uname -s)" = Darwin ] || { echo 'Build the Mac app on macOS.' >&2; exit 1; }
if [ -n "${PORTRELAY_NOTARY_PROFILE:-}" ] && [ -z "${PORTRELAY_SIGN_IDENTITY:-}" ]; then
  echo 'Notarization requires a Developer ID signing identity.' >&2; exit 1
fi
python3 macos/prepare.py
swift build --package-path macos -c release --product portrelay-macos-usb
cargo build --locked --release --bin portrelay --bin portrelay-desktop
python3 - <<'PY'
from pathlib import Path
import json,plistlib,shutil,subprocess,tomllib
root=Path.cwd(); version=tomllib.loads((root/'Cargo.toml').read_text())['workspace']['package']['version']
app=root/'target/macos/package/PortRelay.app'; contents=app/'Contents'
if app.exists(): shutil.rmtree(app)
(contents/'MacOS').mkdir(parents=True); (contents/'Resources').mkdir()
for name in ('portrelay','portrelay-desktop'):
 shutil.copy2(root/'target/release'/name,contents/'MacOS'/name)
binpath=subprocess.check_output(['swift','build','--package-path','macos','-c','release','--show-bin-path'],text=True).strip()
shutil.copy2(Path(binpath)/'portrelay-macos-usb',contents/'MacOS/portrelay-macos-usb')
info={'CFBundleName':'PortRelay','CFBundleDisplayName':'PortRelay','CFBundleIdentifier':'dev.cobanov.portrelay',
'CFBundleExecutable':'portrelay-desktop','CFBundlePackageType':'APPL','CFBundleShortVersionString':version.split('-')[0],
'CFBundleVersion':'2','LSMinimumSystemVersion':'14.0','NSHighResolutionCapable':True,
'LSApplicationCategoryType':'public.app-category.utilities',
'NSLocalNetworkUsageDescription':'PortRelay connects to computers you pair with to share selected USB devices.',
'NSHumanReadableCopyright':'PortRelay contributors. MIT license.'}
(contents/'Info.plist').write_bytes(plistlib.dumps(info))
for source,name in [('LICENSE','LICENSE.txt'),('macos/UPSTREAM-LICENSE','usbipd-mac-LICENSE.txt'),('docs/macos-alpha.md','Mac-preview-guide.md')]:
 shutil.copy2(root/source,contents/'Resources'/name)
revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
dirty=bool(subprocess.check_output(['git','status','--porcelain','--untracked-files=normal'],text=True).strip())
(contents/'Resources/build.json').write_text(json.dumps({'version':version,'commit':revision,'uncommitted_changes':dirty,'role':'macos-input-and-export-preview','usb_import':False,'input_send':True,'input_receive':False},indent=2)+'\n')
print(app)
PY
app=target/macos/package/PortRelay.app
python3 macos/sign.py
codesign --verify --deep --strict --verbose=2 "$app"
archive="target/macos/PortRelay-macos-$(uname -m)-preview.zip"
ditto -c -k --keepParent "$app" "$archive"
if [ -n "${PORTRELAY_NOTARY_PROFILE:-}" ]; then
  xcrun notarytool submit "$archive" --keychain-profile "$PORTRELAY_NOTARY_PROFILE" --wait
  xcrun stapler staple "$app"
  xcrun stapler validate "$app"
  spctl --assess --type execute --verbose=2 "$app"
  ditto -c -k --keepParent "$app" "$archive"
else
  echo 'Development preview only: this app has NOT been notarized for public installation.'
fi
shasum -a 256 "$archive" > "$archive.sha256"
echo "$archive"
