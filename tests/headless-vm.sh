#!/bin/bash
# Boot a disposable Debian VM; hosted-runner kernels may omit USB/IP entirely.
set -euo pipefail
[[ ${GITHUB_ACTIONS:-} == true ]] || exit 1
package=$(realpath "$1")
arch=$2
qa_dir=$(mktemp -d)
vm_pid=''
cleanup() {
    if [[ -n $vm_pid ]]; then kill "$vm_pid" 2>/dev/null || true; wait "$vm_pid" || true; fi
    rm -rf "$qa_dir"
}
trap cleanup EXIT
sudo apt-get update
case "$arch" in
    arm64)
        sudo apt-get install -y qemu-system-arm qemu-efi-aarch64 cloud-image-utils
        machine=(qemu-system-aarch64 -machine virt -cpu max -bios /usr/share/qemu-efi-aarch64/QEMU_EFI.fd) ;;
    amd64)
        sudo apt-get install -y qemu-system-x86 cloud-image-utils
        machine=(qemu-system-x86_64 -machine q35 -cpu max) ;;
    *) exit 1 ;;
esac
image=debian-13-generic-$arch.qcow2
base=https://cloud.debian.org/images/cloud/trixie/latest
curl -fL --retry 3 "$base/$image" -o "$qa_dir/$image"
curl -fL --retry 3 "$base/SHA512SUMS" -o "$qa_dir/SHA512SUMS"
(cd "$qa_dir"; awk -v name="$image" '$2 == name {print}' SHA512SUMS > selected.sha512; test -s selected.sha512; sha512sum -c selected.sha512)
qemu-img resize "$qa_dir/$image" 8G
ssh-keygen -q -t ed25519 -N '' -f "$qa_dir/key"
cat > "$qa_dir/user-data" <<CLOUD
#cloud-config
users:
  - name: portrelay-qa
    shell: /bin/bash
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - $(cat "$qa_dir/key.pub")
CLOUD
printf 'instance-id: portrelay-headless-qa\nlocal-hostname: portrelay-qa\n' > "$qa_dir/meta-data"
cloud-localds "$qa_dir/seed.img" "$qa_dir/user-data" "$qa_dir/meta-data"
accel=tcg
if [[ -r /dev/kvm && -w /dev/kvm ]]; then accel=kvm; machine+=( -cpu host ); fi
"${machine[@]}" -accel "$accel" -m 2048 -smp 2 -nographic \
    -drive "file=$qa_dir/$image,if=virtio,format=qcow2" \
    -drive "file=$qa_dir/seed.img,if=virtio,format=raw" \
    -nic user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:2222-:22 > "$qa_dir/console.log" 2>&1 &
vm_pid=$!
ssh_args=(-i "$qa_dir/key" -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile="$qa_dir/known_hosts" -o ConnectTimeout=3)
ready=false
for ((attempt=0; attempt<120; attempt++)); do
    if ssh "${ssh_args[@]}" -p 2222 portrelay-qa@127.0.0.1 true 2>/dev/null; then ready=true; break; fi
    kill -0 "$vm_pid" || { cat "$qa_dir/console.log"; exit 1; }
    sleep 3
done
[[ $ready == true ]] || { tail -100 "$qa_dir/console.log"; exit 1; }
ssh "${ssh_args[@]}" -p 2222 portrelay-qa@127.0.0.1 'sudo cloud-init status --wait; uname -a'
scp "${ssh_args[@]}" -P 2222 "$package" portrelay-qa@127.0.0.1:/tmp/portrelay.deb
scp "${ssh_args[@]}" -P 2222 tests/headless-install.sh portrelay-qa@127.0.0.1:/tmp/headless-install.sh
ssh "${ssh_args[@]}" -p 2222 portrelay-qa@127.0.0.1 'sudo env GITHUB_ACTIONS=true sh /tmp/headless-install.sh /tmp/portrelay.deb'
