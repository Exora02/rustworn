# RUSTWORN

> A 2D top-down roguelike in Bevy. Nuclear Throne aim × Enter the Gungeon feel × Vampire Survivors scaling × Megabonk mobility — wrapped in a slow-motion **Cascade Mode** that flips the tempo every 60-90 seconds.

This is the development repo. The full game design lives in [PLAN.md](./PLAN.md). Implementation lives phase by phase in [docs/](./docs/).

## Status

**Phase 0 — scaffolding.** A Bevy window opens with a placeholder orange player sprite. ECS components, events, and resources are declared but not yet driven by systems. This is the "does the toolchain work?" check.

Next: [docs/phase_0_prototype.md](./docs/phase_0_prototype.md) walks through building the first playable slice (move + dash + shoot + kill).

## Quick start (VPS — headless)

```bash
cd /root/projects/rustworn
cargo check         # fast: type-check only
cargo build         # slower: full debug build
cargo run           # needs a display; won't work over Tailscale
```

The project is pinned to Rust 1.85+ via `rust-toolchain.toml`. If you `cd` into the directory and your toolchain is older, rustup will fetch the right one automatically.

## Quick start (local — any dev machine)

```bash
git clone git@github.com:Exora02/slop_rustworn.git   # or pull from VPS
cd rustworn
direnv allow              # auto-activates devenv.sh on cd
cargo run                 # builds + runs
```

One `devenv.nix` config provides Rust + mold + wayland + vulkan + alsa + everything Bevy needs. Works on Bazzite, Ubuntu, WSL2, macOS. See [docs/devenv.md](./docs/devenv.md) for install steps.

### Bazzite specific

Bazzite's read-only rootfs can't host Nix directly. One-shot setup:

```bash
git clone git@github.com:Exora02/slop_rustworn.git
cd rustworn
./scripts/setup-nuked-dev.sh
distrobox enter nuked-dev
```

The script creates a `nuked-dev` distrobox, installs Nix + devenv + direnv + cargo inside, and prints a quick-reference at the end. Idempotent — safe to re-run.

## Quick start (local — Windows / VS Code without WSL)

See [docs/setup.md](./docs/setup.md) for the manual path (Visual Studio Build Tools + Rust). Prefer WSL2 + devenv if you can.

## Project structure

```
rustworn/
├── PLAN.md                       # full game design — read this first
├── Cargo.toml                    # dependencies, profiles
├── rust-toolchain.toml           # pinned Rust version
├── devenv.nix                    # cross-platform dev environment (Nix)
├── devenv.yaml                   # devenv inputs pin
├── .envrc                        # direnv auto-activation
├── README.md                     # you are here
├── .gitignore
├── .gitattributes
├── .vscode/                      # rust-analyzer + format config
├── .cargo/config.toml            # mold linker config (Linux)
├── src/
│   ├── main.rs                   # thin binary, calls rustworn::run()
│   ├── lib.rs                    # boot, window, escape-to-exit
│   ├── components.rs             # Health, Velocity, BaseVelocity, CascadeAffected
│   ├── events.rs                 # DamageEvent, CascadeStartEvent, etc.
│   ├── resources.rs              # CascadeState, RunState, GameSettings
│   └── plugins/
│       ├── mod.rs                # plugin registry
│       ├── setup.rs              # startup: camera + placeholder player
│       └── player.rs             # movement, input, camera follow
├── assets/                       # sprites, sounds, fonts (empty for now)
├── scripts/
│   └── setup-nuked-dev.sh        # one-shot Bazzite distrobox setup
└── docs/
    ├── setup.md                  # dev environment walkthrough (manual path)
    ├── devenv.md                 # devenv.sh — recommended cross-platform setup
    └── phase_0_prototype.md      # Phase 0 implementation plan
```

## Where to look

| Want to… | Read this |
|---|---|
| Understand the game design | `PLAN.md` |
| Set up a fresh dev environment (recommended) | `docs/devenv.md` |
| Set up a dev environment manually (Windows native, etc.) | `docs/setup.md` |
| Implement the first playable slice | `docs/phase_0_prototype.md` |
| Add a new ECS component | follow the pattern in `src/components.rs` |
| Add a new Bevy plugin | follow the pattern in `src/plugins/setup.rs` |
| Tweak gameplay constants | `src/resources.rs` → `GameSettings` |

## Tooling choices

- **Bevy 0.16** — the 2D-friendly, ECS-native, single-crate-everything engine. Pinned to `=0.16.1` for now.
- **mold linker** — Linux-only, ~50% faster link times than `ld`. Configured in `.cargo/config.toml`.
- **log crate** — we use the `log` facade directly so logging works the same in `info!`, `warn!`, `error!` calls.
- **No physics yet** — `bevy_xpbd_2d` lands in Phase 1 when we need proper collision queries.
- **No editor** — this is a code-only project. Use `bevy_egui` for in-game dev menus later.

## License

MIT OR Apache-2.0, at your option.
