#!/bin/sh
set -eu
binary=${1:-target/release/portrelay}
arch=${2:-$(uname -m)}
case "$arch" in x86_64|aarch64|armv7) ;; *) echo 'Unknown architecture' >&2; exit 1;; esac
[ -x "$binary" ] || { echo 'Build the release binary first.' >&2; exit 1; }
version=$(python3 scripts/check-linux-binary.py "$binary" "$arch")
name=portrelay-$version-linux-$arch
stage=target/packages/$name
mkdir -p "$stage/docs"
python3 scripts/dependency-notices.py
python3 scripts/sbom.py
cp target/packages/THIRD_PARTY_NOTICES.txt target/packages/SBOM.cdx.json Cargo.lock "$stage/"
cp "$binary" "$stage/portrelay"
cp packaging/install-linux.sh packaging/uninstall-linux.sh "$stage/"
cp LICENSE README.md SECURITY.md CONTRIBUTING.md "$stage/"
cp docs/*.md "$stage/docs/"
python3 - "$name" <<'PY'
import pathlib, sys, tarfile
name=sys.argv[1]
def portable(info):
    info.uid=info.gid=0
    info.uname=info.gname='root'
    info.pax_headers={}
    return info
with tarfile.open(pathlib.Path('target/packages')/(name+'.tar.gz'),'w:gz',format=tarfile.USTAR_FORMAT) as archive:
    archive.add(pathlib.Path('target/packages')/name,arcname=name,filter=portable)
PY
(cd target/packages && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
echo "target/packages/$name.tar.gz"
