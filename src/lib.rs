//! RUSTWORN — a 2D top-down roguelike built on Bevy ECS.
//!
//! Phase 0 is intentionally tiny: a window, a camera, a placeholder
//! player sprite, and the ECS scaffolding we will grow into. The goal
//! is to verify the toolchain on the VPS (cargo check passes) and to
//! establish module boundaries that match the long-term architecture
//! in PLAN.md.
//!
//! Long-term layout:
//!   - `components`  — data definitions (Health, Velocity, CascadeAffected, ...)
//!   - `resources`   — singleton state (CascadeState, RunState, GameSettings)
//!   - `plugins/`    — feature modules, each a Bevy plugin
//!       - `setup`     — window, camera, default resources
//!       - `player`    — player movement, weapons
//!       - `combat`    — damage, projectiles, i-frames
//!       - `cascade`   — Cascade Mode time scaling
//!       - `procgen`   — biomes, waves, rooms
//!       - `meta`      — saves, currency, imprints
//!   - `events`      — cross-system event channels

mod components;
mod events;
mod plugins;
mod resources;

use bevy::prelude::*;
use bevy::window::WindowResolution;

use crate::plugins::{
    CombatPlugin, EnemyPlugin, FxPlugin, HudPlugin, PickupsPlugin, PlayerPlugin, RoomsPlugin,
    SetupPlugin,
};

/// Boot the Bevy app. Called from `main.rs` and (eventually) from tests.
pub fn run() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "RUSTWORN".into(),
                        resolution: WindowResolution::new(1280.0, 720.0),
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    // No remote asset processing in dev — keeps things simple.
                    meta_check: bevy::asset::AssetMetaCheck::Never,
                    ..default()
                }),
        )
        // Event channels used by multiple plugins. Registered
        // here once so each plugin doesn't have to remember
        // to add them. `DamageEvent` drives the damage-number
        // system in `fx`; `EnemyKilledEvent` drives pickup
        // drops; `CascadeStart` / `CascadeEnd` are reserved
        // for Phase 2 (the cascade UI transition).
        .add_event::<events::DamageEvent>()
        .add_event::<events::EnemyKilledEvent>()
        .add_event::<events::CascadeStartEvent>()
        .add_event::<events::CascadeEndEvent>()
        .add_plugins(SetupPlugin)
        // Phase 2: RoomsPlugin must be added AFTER SetupPlugin so
        // its `setup_rooms_system` (OnEnter InRun) fires after
        // `enter_in_run_system`. The OnEnter systems run in
        // registration order; we want the player reset (HP,
        // position, weapon, etc.) to happen first, then the
        // room walls + doors to spawn on top of that
        // already-reset world, then `setup_rooms_system` to
        // re-anchor the player to R0's center.
        .add_plugins(RoomsPlugin)
        .add_plugins(PlayerPlugin)
        .add_plugins(CombatPlugin)
        .add_plugins(EnemyPlugin)
        .add_plugins(FxPlugin)
        .add_plugins(PickupsPlugin)
        .add_plugins(HudPlugin)
        // Phase 2: ESC handling moved into the per-state input
        // systems in `setup.rs`:
        //   - MainMenu: ESC quits the binary
        //   - InRun:    ESC → MainMenu (abandon run)
        //   - GameOver: ESC → MainMenu (skip restart)
        // The previous `exit_on_esc` here was a global ESC-to-quit
        // that fired from any state; that's now wrong (quitting
        // mid-run is jarring, and the title screen has a clean
        // "press ESC to quit" affordance instead).
        .run();
}
