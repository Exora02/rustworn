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

## Quick start (local — Windows / VS Code)

See [docs/setup.md](./docs/setup.md) for full instructions. The short version:

1. Install Rust 1.85+ via [rustup](https://rustup.rs/)
2. Install VS Code + the `rust-analyzer` extension
3. Clone the repo somewhere reachable from Tailscale (or sync via Forgejo)
4. `cargo run` from a local checkout

## Project structure

```
rustworn/
├── PLAN.md                       # full game design — read this first
├── Cargo.toml                    # dependencies, profiles
├── rust-toolchain.toml           # pinned Rust version
├── README.md                     # you are here
├── .gitignore
├── .cargo/config.toml            # mold linker config (Linux)
├── src/
│   ├── main.rs                   # thin binary, calls rustworn::run()
│   ├── lib.rs                    # boot, window, escape-to-exit
│   ├── components.rs             # Health, Velocity, BaseVelocity, CascadeAffected
│   ├── events.rs                 # DamageEvent, CascadeStartEvent, etc.
│   ├── resources.rs              # CascadeState, RunState, GameSettings
│   └── plugins/
│       ├── mod.rs                # plugin registry
│       └── setup.rs              # startup: camera + placeholder player
├── assets/                       # sprites, sounds, fonts (empty for now)
└── docs/
    ├── setup.md                  # dev environment walkthrough
    └── phase_0_prototype.md      # Phase 0 implementation plan
```

## Where to look

| Want to… | Read this |
|---|---|
| Understand the game design | `PLAN.md` |
| Set up a fresh dev environment | `docs/setup.md` |
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
