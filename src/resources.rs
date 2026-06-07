//! Singleton resources. These are the state that lives across systems
//! but only once per app.
//!
//! For per-run state (current seed, biome, score, time remaining) see
//! `RunState` — it gets reset at the start of every run.

use bevy::prelude::*;

/// Global Cascade Mode state. One per app; reset between runs by
/// `RunState::reset()`.
#[derive(Resource, Default)]
pub struct CascadeState {
    pub active: bool,
    pub time_remaining: f32,
    pub kills_during: u32,
    pub chain_extends: u32,
}

/// Per-run state. Reset on `InRun` state entry.
#[derive(Resource)]
pub struct RunState {
    pub seed: u64,
    pub biome_index: u32,
    pub time_elapsed: f32,
    pub time_cap: f32, // 300s for a 5-minute run
    pub score: u32,
    pub enemies_killed: u32,
    pub cascade_beacon_alive: bool,
}

impl Default for RunState {
    fn default() -> Self {
        Self {
            seed: 0,
            biome_index: 0,
            time_elapsed: 0.0,
            time_cap: 300.0,
            score: 0,
            enemies_killed: 0,
            cascade_beacon_alive: true,
        }
    }
}

/// Tunable gameplay constants. Lives in a resource so we can tweak
/// values from the dev console or a settings menu without recompiling.
#[derive(Resource)]
pub struct GameSettings {
    pub dash_iframe_ms: u32,
    pub cascade_scale: f32,
    pub cascade_duration_s: f32,
    pub cascade_interval_s: f32,
    pub player_move_speed: f32,
    pub player_dash_speed: f32,
    pub player_dash_duration_s: f32,
    pub player_dash_cooldown_s: f32,
    // Day 5-7 — pistol
    pub weapon_damage: f32,
    pub weapon_fire_rate: f32, // shots per second
    pub weapon_projectile_speed: f32,
    pub weapon_muzzle_offset: f32,
    pub projectile_lifetime_s: f32,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            dash_iframe_ms: 200,
            cascade_scale: 0.25,
            cascade_duration_s: 30.0,
            cascade_interval_s: 75.0, // 60-90s, mid-range
            player_move_speed: 220.0,
            player_dash_speed: 600.0,
            player_dash_duration_s: 0.15,
            player_dash_cooldown_s: 1.5,
            // Day 5-7 — pistol: 3 shots/sec, 2s range.
            weapon_damage: 1.0,
            weapon_fire_rate: 3.0,
            weapon_projectile_speed: 700.0,
            weapon_muzzle_offset: 25.0,
            projectile_lifetime_s: 2.0,
        }
    }
}
