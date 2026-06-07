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

use crate::plugins::{CombatPlugin, PlayerPlugin, SetupPlugin};

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
        .add_plugins(SetupPlugin)
        .add_plugins(PlayerPlugin)
        .add_plugins(CombatPlugin)
        .add_systems(Update, exit_on_esc)
        .run();
}

/// Dev convenience: tap Escape to quit. Will be replaced by a real pause
/// menu once we have one.
fn exit_on_esc(keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
}
