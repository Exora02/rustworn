# Dev Environment Setup

How to get RUSTWORN building and running on Windows, Linux, and the VPS.

## TL;DR

| Platform | Goal | Command |
|---|---|---|
| **VPS (headless)** | type-check / link | `cargo check` or `cargo build` |
| **Windows (dev)** | full game run | `cargo run` from a local checkout |
| **Linux desktop** | full game run | `cargo run` |

The VPS is the build server. Your local machine is the runtime. Code lives on the VPS, syncs to local via Forgejo.

---

## 1. VPS (Ubuntu 24.04) — already done

The VPS is set up with everything needed to compile Bevy 0.16:

```bash
# System packages
build-essential, pkg-config, libudev-dev, libasound2-dev,
libwayland-dev, libvulkan-dev, libxkbcommon-dev, mold

# Rust
stable toolchain (1.85+), rustfmt, clippy
```

`.cargo/config.toml` wires mold into the linker. `rust-toolchain.toml` pins the Rust version. `cd`-ing into the project auto-installs the right toolchain on first build.

### Verify

```bash
cd /root/projects/rustworn
cargo check
```

Should finish in ~30s after the initial dep download (cold build is ~2-3min).

### Limitations

- **No display.** `cargo run` will fail with a winit error. Always use `cargo check` or `cargo build` for VPS work.
- **No GPU.** Bevy will compile the wgpu / Vulkan backend but you can't actually render anything. That's fine for type-checking and ECS work.

---

## 2. Windows local dev

This is where you actually play the game.

### One-time setup

1. **Install Rust** via [rustup](https://rustup.rs/). Accept the MSVC default toolchain.
2. **Install Visual Studio Build Tools** with the "Desktop development with C++" workload (needed for linking).
3. **Install VS Code** + the `rust-analyzer` extension (NOT the deprecated `Rust` extension).
4. **Optional but recommended** — install `bevy_fmt` for rust-analyzer to handle Bevy's heavy macro expansion gracefully:
   ```bash
   rustup component add rustfmt
   ```

### VS Code settings

In `.vscode/settings.json` of the project (or your user settings):

```json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.check.allTargets": true,
  "rust-analyzer.procMacro.enable": true,
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
```

The `cargo.features = "all"` line is important — Bevy has a `dynamic_linking` feature that rebuilds in 2-3 seconds instead of 30-60s. You can opt in via env var too:

```bash
export BEVY_FEATURES="dynamic_linking"
```

Or in Windows PowerShell:

```powershell
$env:BEVY_FEATURES = "dynamic_linking"
```

This is huge during iteration. The downside is the first build is slower.

### Forgejo sync

You mentioned you'll add a Forgejo origin for online backup. Suggested workflow:

```bash
# On VPS, after Forgejo is set up:
cd /root/projects/rustworn
git remote add origin git@forgejo.exora.dev:exora02/rustworn.git
git push -u origin main

# On Windows, clone:
git clone git@forgejo.exora.dev:exora02/rustworn.git
cd rustworn
cargo run
```

Pull/push from either side as needed.

### Linux build deps (if you ever build on Linux desktop)

```bash
sudo apt install -y build-essential pkg-config libudev-dev libasound2-dev \
    libwayland-dev libvulkan-dev libxkbcommon-dev mold
```

---

## 3. Useful cargo commands

```bash
cargo check                  # type-check only, fast (this is your VPS friend)
cargo build                  # full debug build, link required
cargo run                    # build + run (needs display)
cargo run --release          # optimized build + run
cargo test                   # unit + integration tests
cargo clippy                 # lints
cargo fmt                    # format
cargo fmt --check            # format check (CI-friendly)
cargo bench                  # benchmarks (we'll add some in Phase 2)
cargo build --profile dev-fast   # the "I just want to run it" profile
```

### Useful env vars

```bash
RUST_LOG=warn                # quiet the bevy log spam
RUST_LOG=info                # normal
RUST_LOG=rustworn=debug      # only our crate's debug logs
WGPU_BACKEND=vulkan          # explicit backend
WGPU_BACKEND=gl              # force OpenGL fallback
WGPU_ADAPTER_NAME="..."      # pin a specific GPU
```

---

## 4. Troubleshooting

### "could not find `2d` feature" on Bevy 0.16

You have the wrong feature list. See `Cargo.toml` — we cherry-pick features rather than use defaults. The `2d` flag is gone in 0.16; 2D rendering is bundled with the bevy_sprite / bevy_render features.

### First build is slow (5-10 min)

That's normal. Bevy pulls a lot of dependencies. Subsequent incremental builds are 5-30s with `dynamic_linking` enabled, 30-90s without.

### `mold: fatal: unknown -m argument: 64`

You have a stale `.cargo/config.toml` that uses the wrong mold invocation. We use `-C link-arg=-fuse-ld=mold` (passes through rustc's linker driver), not `linker = "/usr/bin/mold"` (which would invoke mold directly and choke on GCC-style args).

### Bevy panics about "no wgpu adapters"

Either no GPU is present, or Vulkan isn't working. Try `WGPU_BACKEND=gl` or check your graphics drivers.

### "I changed something and the build is now 5 minutes"

You probably changed `Cargo.toml` features. Cargo invalidates the dep graph. One-time pain — next build is back to normal.

---

## 5. Recommended editor layout

If you use Continue.dev on this project (you do), here's a working `.continue/config.json` sketch:

```json
{
  "models": [
    {
      "title": "Hermes",
      "provider": "openai",
      "apiBase": "http://xovps:PORT/v1",
      "model": "your-preferred-model"
    }
  ],
  "tabAutocompleteModel": {
    "title": "Copilot",
    "provider": "copilot"
  }
}
```

Your actual `xovps:PORT` will be whatever's in your Continue.dev config on Windows. Hermes itself runs on the VPS and the local Continue.dev talks to it over Tailscale.
