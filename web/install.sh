#!/bin/sh
# PortRelay's release bootstrapper. MIT licensed; source and notices:
# https://github.com/cobanov/portrelay
# Usage: curl -fsSL https://portrelay.cobanov.dev/install.sh | sh
set -eu

main() {
    download_only=false
    headless=false
    setup_user=${SUDO_USER:-}
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --download-only) download_only=true ;;
            --headless) headless=true ;;
            --user)
                [ "$#" -ge 2 ] || { echo '--user needs a username.' >&2; return 1; }
                shift; setup_user=$1 ;;
            --help) echo 'Usage: sh install.sh [--download-only] [--headless [--user USERNAME]]'; return ;;
            *) echo 'Unknown option. Use --help.' >&2; return 1 ;;
        esac
        shift
    done

    [ "$(uname -s)" = Linux ] || {
        echo 'This installer requires Ubuntu 24.04, Debian 12/13, or Raspberry Pi OS Bookworm/Trixie. macOS USB is not available yet.' >&2
        return 1
    }
    for tool in curl dpkg sha256sum apt-get; do
        command -v "$tool" >/dev/null 2>&1 || {
            echo "Missing $tool. Install it with your distribution package manager and retry." >&2
            return 1
        }
    done
    arch=$(dpkg --print-architecture)
    case "$arch" in
        amd64|arm64) ;;
        armhf)
            # A 32-bit userspace may run under a 64-bit Pi kernel. Do not select
            # arm64 from uname alone; original Pi 1 / Zero ARMv6 cannot run ARMv7.
            case "$(uname -m)" in armv7l|armv8l|aarch64) ;;
                *) echo '32-bit PortRelay requires ARMv7 or newer. Pi 1 and the original Pi Zero are not supported.' >&2; return 1 ;;
            esac ;;
        *) echo "No PortRelay package is available for $arch." >&2; return 1 ;;
    esac
    [ -r /etc/os-release ] || { echo 'Cannot identify this Linux distribution.' >&2; return 1; }
    . /etc/os-release
    case "${ID:-}:${VERSION_ID:-}" in
        ubuntu:24.04) distro=ubuntu ;;
        debian:12|debian:13|raspbian:12|raspbian:13) distro=debian ;;
        *) echo 'Use Ubuntu 24.04, Debian 12/13, or Raspberry Pi OS Bookworm/Trixie.' >&2; return 1 ;;
    esac
    case "$distro:$arch" in
        ubuntu:amd64) checksum=852361005787bddf61162615c5665b8d2eaa82b44e29b76839afb021df11d856 ;;
        ubuntu:arm64) checksum=89c91a8d88c6ee487b85539b4be819c6dcccf07110cd7d01b010f68f173394d2 ;;
        debian:amd64) checksum=5f045d28ff44544fb08a8531285a3a2ae8226bc896353aac4ad58571ad99af15 ;;
        debian:arm64) checksum=a1a20b31a1bcafd70e4a5b81c2b9721a3059559a8c4ebf1d4c3110da6bb67490 ;;
        debian:armhf) checksum=71160e1069b1e155bf66cbcee54425eae9be87e2a356c67940b48f4ae39cdeec ;;
        *) echo "No PortRelay package is available for $ID $arch." >&2; return 1 ;;
    esac

    if [ "$download_only" = false ]; then
        [ -d /run/systemd/system ] || { echo 'PortRelay requires a systemd-based system.' >&2; return 1; }
        if [ "$(id -u)" != 0 ]; then
            command -v sudo >/dev/null 2>&1 || { echo 'Install sudo or run this script as root.' >&2; return 1; }
            if [ "$headless" = true ]; then setup_user=$(id -un); fi
        fi
        if [ "$headless" = true ]; then
            case "$setup_user" in ''|*[!a-zA-Z0-9_-]*|-*) echo 'Headless setup needs a regular account. As root, add --user USERNAME.' >&2; return 1 ;; esac
            setup_uid=$(id -u "$setup_user") || return 1
            [ "$setup_uid" -ne 0 ] || { echo 'Headless setup cannot run the application as root. Add --user USERNAME.' >&2; return 1; }
        fi
    fi

    version=0.1.0-alpha.6
    package=portrelay-$version-$distro-$arch.deb
    # Pin the package hash with its version. A replaced release asset fails closed.
    temp_dir=$(mktemp -d /tmp/portrelay.XXXXXXXXXX)
    trap 'rm -rf "$temp_dir"' EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    echo "Downloading PortRelay $version for $ID ($arch)..."
    curl --fail --show-error --location --proto '=https' --proto-redir '=https' \
        --tlsv1.2 --connect-timeout 15 --max-time 600 --retry 2 \
        "https://github.com/cobanov/portrelay/releases/download/v$version/$package" \
        --output "$temp_dir/$package"
    printf '%s  %s\n' "$checksum" "$temp_dir/$package" | sha256sum --check --status || {
        echo 'Package checksum mismatch. Nothing was installed. Please retry or report this on GitHub.' >&2
        return 1
    }
    echo 'Package checksum verified.'
    [ "$download_only" = false ] || return 0

    echo 'Installing the experimental alpha. You may be asked for your administrator password.'
    if [ "$(id -u)" = 0 ]; then
        apt-get update
        apt-get install --no-install-recommends -y "$temp_dir/$package"
    else
        sudo apt-get update
        sudo apt-get install --no-install-recommends -y "$temp_dir/$package"
    fi
    if [ "$headless" = true ]; then
        if [ "$(id -u)" = 0 ]; then
            /usr/lib/portrelay/setup-headless "$setup_user"
        else
            sudo /usr/lib/portrelay/setup-headless "$setup_user"
        fi
    else
        echo 'Installed! Open PortRelay from your applications, choose Enable USB sharing, then Add computer.'
        echo 'No desktop? Run: sudo /usr/lib/portrelay/setup-headless YOUR_USERNAME'
    fi
}

# Define the entire installer before running it, including when piped into sh.
main "$@"
