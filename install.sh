#!/bin/sh
# install deps (if needed), build, and install heartfetch + sandboxheart.
# usage:
#   ./install.sh              # auto: detect distro, install deps, build, install to /usr/local/bin
#   PREFIX=$HOME/.local ./install.sh
#   ./install.sh --no-deps    # skip distro dep install (assume cargo + cc available)
#   ./install.sh --deps-only  # only install distro deps, no build/install

set -eu

PREFIX="${PREFIX:-/usr/local}"
BINDIR="$PREFIX/bin"
ROOT="$(cd "$(dirname "$0")" && pwd)"
CRATES="heartfetch sandboxheart"

NO_DEPS=0
DEPS_ONLY=0
for arg in "$@"; do
    case "$arg" in
        --no-deps)   NO_DEPS=1 ;;
        --deps-only) DEPS_ONLY=1 ;;
        -h|--help)
            sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "unknown arg: $arg" >&2; exit 2 ;;
    esac
done

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    if command -v sudo >/dev/null 2>&1; then
        SUDO="sudo"
    fi
fi

detect_pm() {
    if   command -v apt-get >/dev/null 2>&1; then echo apt
    elif command -v dnf     >/dev/null 2>&1; then echo dnf
    elif command -v yum     >/dev/null 2>&1; then echo yum
    elif command -v pacman  >/dev/null 2>&1; then echo pacman
    elif command -v zypper  >/dev/null 2>&1; then echo zypper
    elif command -v apk     >/dev/null 2>&1; then echo apk
    elif command -v xbps-install >/dev/null 2>&1; then echo xbps
    elif command -v brew    >/dev/null 2>&1; then echo brew
    else echo unknown
    fi
}

install_deps() {
    pm="$(detect_pm)"
    echo ">> detected package manager: $pm"
    case "$pm" in
        apt)
            $SUDO apt-get update
            $SUDO apt-get install -y cargo rustc build-essential pkg-config
            ;;
        dnf)
            $SUDO dnf install -y cargo rust gcc pkgconf-pkg-config
            ;;
        yum)
            $SUDO yum install -y cargo rust gcc pkgconfig
            ;;
        pacman)
            $SUDO pacman -S --needed --noconfirm rust base-devel pkgconf
            ;;
        zypper)
            $SUDO zypper install -y cargo rust gcc pkg-config
            ;;
        apk)
            $SUDO apk add --no-cache cargo rust build-base pkgconf
            ;;
        xbps)
            $SUDO xbps-install -Sy cargo rust base-devel pkg-config
            ;;
        brew)
            brew install rust pkg-config
            ;;
        unknown)
            echo "could not detect a supported package manager." >&2
            echo "install manually: rust toolchain (cargo + rustc), a c compiler (gcc/clang), pkg-config." >&2
            echo "then re-run with --no-deps." >&2
            exit 1
            ;;
    esac
}

if [ "$NO_DEPS" -eq 0 ]; then
    install_deps
else
    echo ">> skipping dep install (--no-deps)"
fi

if [ "$DEPS_ONLY" -eq 1 ]; then
    echo "deps installed. exiting (--deps-only)."
    exit 0
fi

command -v cargo >/dev/null 2>&1 || {
    echo "cargo still not available after dep install. aborting." >&2
    exit 1
}

if [ ! -d "$BINDIR" ] || [ ! -w "$BINDIR" ]; then
    if [ "$(id -u)" -ne 0 ] && [ -z "$SUDO" ]; then
        echo "no write access to $BINDIR and sudo not available." >&2
        echo "re-run as root or set PREFIX=\$HOME/.local" >&2
        exit 1
    fi
fi

for c in $CRATES; do
    echo ">> building $c"
    (cd "$ROOT/$c" && cargo build --release)
done

$SUDO mkdir -p "$BINDIR"
for c in $CRATES; do
    echo ">> installing $c -> $BINDIR/$c"
    $SUDO install -m 0755 "$ROOT/$c/target/release/$c" "$BINDIR/$c"
done

echo "done. installed to $BINDIR"
