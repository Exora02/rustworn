# devenv.sh — Reproducible RUSTWORN Dev Environment

A single `devenv.nix` file replaces the platform-specific setup (`apt install`, `rpm-ostree install`, Visual Studio Build Tools, Homebrew). One config, every machine.

## Why

You have three machines with three different package managers:
- VPS (Ubuntu) — apt
- Bazzite (Fedora Atomic) — rpm-ostree / distrobox
- Windows (WSL2) — apt inside WSL

devenv.sh + Nix gives all three the same reproducible environment. Plus it's git-versioned so any contributor (or future you on a new laptop) gets the exact same setup.

## TL;DR — quick start

```bash
git clone git@github.com:Exora02/slop_rustworn.git
cd rustworn

# Install Nix + devenv if you don't have them yet (one time, per machine).
# See "Install devenv" below for the platform-specific recipe.

# Trust this project, then dev tools activate automatically on cd:
devenv allow
cargo run
```

That's it. No `direnv allow`, no `.envrc` to remember. The `devenv hook bash` line in your shell config handles the rest.

## Install devenv (one time per machine)

### Bazzite / Fedora Atomic

Bazzite and other Fedora Atomic distros have a read-only rootfs, so the standard Nix installer can't create `/nix`. **Use a distrobox.** We provide a one-shot script that does everything:

```bash
git clone git@github.com:Exora02/slop_rustworn.git
cd rustworn
chmod +x scripts/setup-nuked-dev.sh
./scripts/setup-nuked-dev.sh
distrobox enter nuked-dev
cd ~/projects/rustworn    # or wherever you cloned it
devenv allow              # one time per machine
cargo run
```

The script:
- Creates a `nuked-dev` distrobox (Fedora 39) if missing
- Installs Nix (via the official installer, `--no-daemon` mode for distrobox's lack of systemd)
- Installs devenv via `nix profile install nixpkgs#devenv`
- Adds `eval "$(devenv hook bash)"` to your `~/.bashrc` inside the box
- Idempotent — safe to re-run

**Why not system-wide on Bazzite?** The Determinate Nix installer fails on read-only rootfs:
```
Error saving receipt: RecordingReceipt("/nix", Os { code: 30, kind: ReadOnlyFilesystem })
Failed to execute command "systemctl" "start" "nix.mount"
```
The distrobox gives Nix a writable rootfs inside a Fedora container.

### Ubuntu / Debian (workstation or server)

```bash
curl -fsSL https://install.determinate.systems/nix | sh -s -- install
. /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
nix profile install nixpkgs#devenv

# Add the devenv hook to your shell:
echo 'eval "$(devenv hook bash)"' >> ~/.bashrc

# Trust the project (one time):
cd /path/to/rustworn
devenv allow
cargo run
```

### Windows (via WSL2)

Inside your WSL2 distro (Ubuntu 22.04 or 24.04):

```bash
curl -fsSL https://install.determinate.systems/nix | sh -s -- install
. /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
nix profile install nixpkgs#devenv
echo 'eval "$(devenv hook bash)"' >> ~/.bashrc

cd /path/to/rustworn
devenv allow
cargo run
```

> ⚠️ Do NOT use WSL1. Nix needs a real Linux kernel.

### macOS

```bash
curl -fsSL https://install.determinate.systems/nix | sh
nix profile install nixpkgs#devenv
echo 'eval "$(devenv hook bash)"' >> ~/.zshrc   # or ~/.bashrc for bash

cd /path/to/rustworn
devenv allow
cargo run
```

### Non-Atomic Fedora / RHEL

```bash
curl -fsSL https://install.determinate.systems/nix | sh -s -- install
. /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
nix profile install nixpkgs#devenv
echo 'eval "$(devenv hook bash)"' >> ~/.bashrc

cd /path/to/rustworn
devenv allow
cargo run
```

## How the auto-activation works

The `devenv hook bash` line in your shell config installs a PROMPT_COMMAND (bash) or chpwd_functions (zsh) hook that:

1. Walks up from `$PWD` looking for a `devenv.yaml` file.
2. Checks the trust database to verify the project was allowed (`devenv allow`).
3. If trusted, runs `devenv shell` in a subshell for that project.

When you `cd` back out, the subshell exits and your normal environment returns. No `.envrc`, no `direnv` dependency.

> If the project hasn't been trusted yet, devenv prints:
> ```
> devenv: /home/user/myproject is not allowed. Run 'devenv allow' to trust this directory.
> ```

To revoke trust later: `devenv revoke`.

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

## What if I already have direnv set up?

direnv still works with devenv (via the `devenv direnvrc` helper for v1.4+), but the recommended 2.0+ approach is to use `devenv hook bash` and skip direnv entirely. This project uses the new approach.

If you have direnv set up globally for other projects, leave it alone — the two systems don't conflict. Just use `devenv allow` to trust this project.

## Why not flake.nix directly?

devenv.sh is a higher-level wrapper around Nix flakes. You get:
- A nicer DX (native auto-activation via `devenv hook`, named scripts)
- Sensible defaults for Rust dev
- Cross-platform abstractions (`lib.optionals stdenv.isLinux`)
- Built-in services module (postgres, redis, etc.) for later phases

If you ever outgrow devenv, you can `devenv flake` it into a pure `flake.nix` without losing anything.

## Pitfalls

- **First shell activation is slow** (5-15 min) because Nix downloads/builds all packages. After that, incremental activations are seconds.
- **Disk usage** — Nix keeps every package version. Expect 5-20GB in `/nix/store`. Run `nix-collect-garbage -d` occasionally.
- **Macros and pre-commit hooks** — the uncommented `git-hooks` block installs pre-commit hooks. Enable it when you're ready to enforce format on every commit.
- **`RUSTFLAGS="-C target-cpu=native"`** is set in `devenv.nix`. This optimizes for the current CPU but produces non-portable binaries. Fine for dev, remove for `cargo build --release` if you ship binaries.
- **Re-entry protection** — the hook won't nest. While inside a `devenv shell`, navigating into a subdirectory of the same project keeps the current shell. Only navigating outside the project triggers deactivation.
