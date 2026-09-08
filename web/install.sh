#!/bin/sh
# PortRelay's release bootstrapper. MIT licensed; source and notices:
# https://github.com/cobanov/portrelay
# Usage: curl -fsSL https://portrelay.cobanov.dev/install.sh | sh
set -eu

main() {
    download_only=false
    case "${1:-}" in
        '') ;;
        --download-only) download_only=true ;;
        --help) echo 'Usage: sh install.sh [--download-only]'; return ;;
        *) echo 'Unknown option. Use --help.' >&2; return 1 ;;
    esac
    [ "$#" -le 1 ] || { echo 'Too many arguments.' >&2; return 1; }

    [ "$(uname -s)" = Linux ] || {
        echo 'This installer requires Ubuntu 24.04 or Debian 13. macOS USB is not available yet.' >&2
        return 1
    }
    for tool in curl dpkg sha256sum apt-get; do
        command -v "$tool" >/dev/null 2>&1 || {
            echo "Missing $tool. Install it with your distribution package manager and retry." >&2
            return 1
        }
    done
    [ "$(dpkg --print-architecture)" = amd64 ] || {
        echo 'This alpha requires Intel/AMD 64-bit Linux. ARM packages are not available yet.' >&2
        return 1
    }
    [ -r /etc/os-release ] || { echo 'Cannot identify this Linux distribution.' >&2; return 1; }
    . /etc/os-release
    case "${ID:-}:${VERSION_ID:-}" in
        ubuntu:24.04)
            distro=ubuntu
            checksum=24aaad100688797dca8061b9bca6d6e2eee0406f6d7bec983260e52fab17d58a
            ;;
        debian:13)
            distro=debian
            checksum=8952546b26d77e4f9fe6b5cd4d240f1017e72422b8821b34a5cf90d85ba902cb
            ;;
        *) echo 'This alpha supports Ubuntu 24.04 and Debian 13 only.' >&2; return 1 ;;
    esac

    if [ "$download_only" = false ]; then
        [ -d /run/systemd/system ] || { echo 'PortRelay requires a systemd-based system.' >&2; return 1; }
        if [ "$(id -u)" != 0 ]; then
            command -v sudo >/dev/null 2>&1 || { echo 'Install sudo or run this script as root.' >&2; return 1; }
        fi
    fi

    version=0.1.0-alpha.4
    package=portrelay-$version-$distro-amd64.deb
    # Pin the package hash with its version. A replaced release asset fails closed.
    temp_dir=$(mktemp -d /tmp/portrelay.XXXXXXXXXX)
    trap 'rm -rf "$temp_dir"' EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    echo "Downloading PortRelay $version for $distro..."
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
        apt-get install -y "$temp_dir/$package"
    else
        sudo apt-get update
        sudo apt-get install -y "$temp_dir/$package"
    fi
    echo 'Installed! Open PortRelay from your applications, choose Enable USB sharing, then Add computer.'
}

# Define the entire installer before running it, including when piped into sh.
main "$@"
