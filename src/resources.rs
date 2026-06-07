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
    // Day 15-16: when the player dies, gameplay systems freeze and a
    // restart prompt appears. The bool lives on RunState (rather than
    // a separate resource) because it's reset on the same lifecycle —
    // a proper Bevy State lands in Phase 2 alongside the main-menu /
    // game-over / pause split, and we'll lift this flag into that
    // state machine at the same time.
    pub game_over: bool,
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
            game_over: false,
        }
    }
}

/// Brief global freeze when an enemy dies. Inserted by the
/// projectile-damage system on a killing blow; ticks down each frame;
/// any gameplay system (movement, AI, fire, contact) early-returns
/// while the resource is present, so the hitstop is felt uniformly.
/// The resource is removed when the timer elapses.
#[derive(Resource)]
pub struct Hitstop {
    pub timer: Timer,
}

/// Global screen-shake state. Set to a non-zero intensity by the
/// contact-damage system when the player takes a hit; the camera
/// follow system reads the current value, adds a decaying random
/// offset, and decays the remaining time each frame.
#[derive(Resource, Default)]
pub struct ScreenShake {
    pub intensity: f32,
    pub duration: f32,
    pub remaining: f32,
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
    // Day 8-10 — chaser + spawner
    pub chaser_move_speed: f32,
    pub chaser_contact_damage: f32,
    pub chaser_knockback_speed: f32,
    pub chaser_knockback_duration_s: f32,
    pub hit_iframe_duration_s: f32,
    pub spawner_interval_s: f32,
    pub spawner_min_distance: f32,
    pub spawner_max_distance: f32,
    // Day 11-12 — enemy HP + death
    pub enemy_max_hp: f32,
    pub death_puff_lifetime_s: f32,
    // Day 13-14 — HUD + juice
    pub hit_flash_duration_s: f32,
    pub hitstop_duration_s: f32,
    pub screen_shake_intensity: f32,
    pub screen_shake_duration_s: f32,
    // Day 15-16 — polish
    pub projectile_cap: u32,
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
            // Day 8-10 — chaser is slower than the player (220),
            // deals 1 hp on contact with 0.4s knockback + 0.5s i-frames.
            chaser_move_speed: 100.0,
            chaser_contact_damage: 1.0,
            chaser_knockback_speed: 400.0,
            chaser_knockback_duration_s: 0.15,
            hit_iframe_duration_s: 0.5,
            // One chaser every 3s, spawning 400-600 units from the player.
            spawner_interval_s: 3.0,
            spawner_min_distance: 400.0,
            spawner_max_distance: 600.0,
            // Day 11-12 — chaser takes 3 hits to kill; death puff
            // is a 0.3s fading white square.
            enemy_max_hp: 3.0,
            death_puff_lifetime_s: 0.3,
            // Day 13-14 — enemy hit flash is a 0.1s white pulse,
            // killing blows trigger a 50ms global hitstop, and
            // taking damage shakes the camera for 0.2s at intensity 12.
            hit_flash_duration_s: 0.1,
            hitstop_duration_s: 0.05,
            screen_shake_intensity: 12.0,
            screen_shake_duration_s: 0.2,
            // Day 15-16 — hard cap of 30 live projectiles. Holding
            // fire and running around no longer spawns an unbounded
            // stream; the 31st shot is silently dropped. Pick this
            // high enough that legit gameplay (clearing a room) never
            // hits it, but low enough that pathological cases don't
            // tank the frame rate on weaker machines.
            projectile_cap: 30,
        }
    }
}
