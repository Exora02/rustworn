# devenv.sh — Reproducible RUSTWORN Dev Environment

A single `devenv.nix` file replaces the platform-specific setup (`apt install`, `rpm-ostree install`, Visual Studio Build Tools, Homebrew). One config, every machine.

## Why

You have three machines with three different package managers:
- VPS (Ubuntu) — apt
- Bazzite (Fedora Atomic) — rpm-ostree / distrobox
- Windows (WSL2) — apt inside WSL

devenv.sh + Nix gives all three the same reproducible environment. Plus it's git-versioned so any contributor (or future you on a new laptop) gets the exact same setup.

## Install devenv (one time per machine)

Pick your platform:

### Bazzite / Fedora / RHEL

Bazzite and other Fedora Atomic distros have a read-only rootfs, so the standard Nix installer can't create `/nix`. **Use a distrobox:**

```bash
# Create a Fedora container for Nix work
distrobox create --image fedora:39 --name nix-dev
distrobox enter nix-dev

# Inside the container (writable rootfs — Nix installs cleanly):
sudo dnf install -y nix
. /etc/profile.d/nix.sh

# Install devenv
nix profile install nixpkgs#devenv

# Now do dev work — your home folder is auto-bind-mounted
cd ~/projects/rustworn
direnv allow
cargo run
```

For direnv auto-activation inside the distrobox:
```bash
sudo dnf install -y direnv
echo 'eval "$(direnv hook bash)"' >> ~/.bashrc
```

On non-Atomic Fedora / RHEL (workstation, server) you can install Nix system-wide:
```bash
curl -fsSL https://install.determinate.systems/nix | sh -s -- install
nix profile install nixpkgs#devenv
sudo dnf install -y direnv
echo 'eval "$(direnv hook bash)"' >> ~/.bashrc
```

**Why not system-wide on Bazzite?** The Determinate installer fails with:
```
Error saving receipt: RecordingReceipt("/nix", Os { code: 30, kind: ReadOnlyFilesystem, ... })
Failed to execute command "systemctl" "start" "nix.mount"
```
That's `/` being read-only. The distrobox approach gives Nix a writable container rootfs.

### Ubuntu / Debian

```bash
curl -fsSL https://install.determinate.systems/nix | sh -s -- install
. /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
nix profile install nixpkgs#devenv
sudo apt install -y direnv
echo 'eval "$(direnv hook bash)"' >> ~/.bashrc
```

### Windows (via WSL2)

Inside your WSL2 distro (Ubuntu 22.04 or 24.04):

```bash
curl -fsSL https://install.determinate.systems/nix | sh -s -- install
. /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
nix profile install nixpkgs#devenv
sudo apt install -y direnv
echo 'eval "$(direnv hook bash)"' >> ~/.bashrc
```

Note: do NOT use WSL1. Nix needs a real Linux kernel.

### macOS

```bash
curl -fsSL https://install.determinate.systems/nix | sh
nix profile install nixpkgs#devenv
brew install direnv
echo 'eval "$(direnv hook zsh)"' >> ~/.zshrc
```

## Activate the env

In the project directory:

```bash
# First time: trust the .envrc
direnv allow

# Or manual activation
devenv shell
```

You'll see the ASCII banner. Now `cargo`, `rustc`, `mold`, `pkg-config`, and the Bevy system libs are all on PATH, isolated from your system.

## What you get

| Tool | Source | Notes |
|---|---|---|
| `rustc` / `cargo` | devenv's `languages.rust` | stable channel, rustfmt + clippy + rust-src |
| `mold` | pkgs.mold | linked via `.cargo/config.toml` rustflags |
| `wayland` / `xkbcommon` | pkgs | Bevy input on Linux |
| `vulkan-loader` + headers | pkgs | wgpu / naga shader compilation |
| `alsa-lib` | pkgs | audio |
| `libudev` | pkgs | device hotplug |
| `shaderc` | pkgs | SPIR-V compilation |
| `pkg-config` | pkgs | build script helper |
| `direnv` | system | auto-activation on `cd` |

The Linux section conditionally adds the right set on Fedora/Ubuntu/etc. macOS gets the IOKit/Cocoa frameworks automatically.

## Scripts

devenv exposes the `scripts` block as commands. From inside the shell:

```bash
check    # cargo check
build    # cargo build
run      # cargo run
fmt      # cargo fmt --all
clippy   # cargo clippy --all-targets -- -W clippy::all
bench    # cargo bench
```

Or run them as `devenv shell check` from outside the shell.

## What about the VPS?

The VPS already has a working Rust + mold setup (apt-installed) and is used as a **headless build server** — we only need `cargo check` and `cargo build` to work there, never `cargo run`. Installing devenv on the VPS would be redundant.

You can if you want — `devenv shell cargo check` works fine — but it's not necessary. The VPS uses the system Rust, which the project compiles against without issues.

## Why not flake.nix directly?

devenv.sh is a higher-level wrapper around Nix flakes. You get:
- A nicer DX (auto-activation via direnv, named scripts)
- Sensible defaults for Rust dev
- Cross-platform abstractions (lib.optionals stdenv.isLinux)
- Built-in services module (postgres, redis, etc.) for later phases

If you ever outgrow devenv, you can `devenv flake` it into a pure `flake.nix` without losing anything.

## Pitfalls

- **First shell activation is slow** (5-15 min) because Nix downloads/builds all packages. After that, incremental activations are seconds.
- **Disk usage** — Nix keeps every package version. Expect 5-20GB in `/nix/store`. Run `nix-collect-garbage -d` occasionally.
- **Macros and pre-commit hooks** — the uncommented `git-hooks` block installs pre-commit hooks. Enable it when you're ready to enforce format on every commit.
- **`RUSTFLAGS="-C target-cpu=native"`** is set in devenv.nix. This optimizes for the current CPU but produces non-portable binaries. Fine for dev, remove for `cargo build --release` if you ship binaries.
