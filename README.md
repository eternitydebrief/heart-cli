# heart-cli

two small rust tui toys:

- `heartfetch` — animated 3d heart system fetch
- `sandboxheart` — 3d heart physics sandbox

## install

```sh
./install.sh
```

handles everything: detects the distro, installs deps via the system package manager, builds both crates, and installs the binaries to `/usr/local/bin`.

custom prefix (no root if writable):

```sh
PREFIX=$HOME/.local ./install.sh
```

skip dep install (deps already present):

```sh
./install.sh --no-deps
```

only install deps:

```sh
./install.sh --deps-only
```

## dependencies

| distro                          | packages                                       |
| ------------------------------- | ---------------------------------------------- |
| debian, ubuntu, mint, pop, etc. | `cargo rustc build-essential pkg-config`       |
| fedora, rhel                    | `cargo rust gcc pkgconf-pkg-config`            |
| arch, manjaro                   | `rust base-devel pkgconf`                      |
| opensuse                        | `cargo rust gcc pkg-config`                    |
| alpine                          | `cargo rust build-base pkgconf`                |
| void                            | `cargo rust base-devel pkg-config`             |
| macos (brew)                    | `rust pkg-config`                              |

the script auto-detects apt, dnf, yum, pacman, zypper, apk, xbps, and brew. if none are found it prints what to install manually.

## run

```sh
heartfetch
sandboxheart
```

## uninstall

```sh
sudo rm /usr/local/bin/heartfetch /usr/local/bin/sandboxheart
```

## nixos

skip `install.sh`. build per-crate with `cargo build --release` inside a shell that has rust, or wire each `Cargo.toml` into a flake.
