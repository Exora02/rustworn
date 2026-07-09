use bevy::prelude::*;

/// Global Cascade Mode state. One per app.
///
/// Phase 1 prototype: every `cascade_interval_s` (default 75s) a
/// 30s window opens where every `CascadeAffected` entity (enemies
/// today, enemy projectiles + particles later) runs at
/// `cascade_scale` × normal speed. The player and player
/// projectiles are not `CascadeAffected` and run at full speed.
///
/// Two timers: `interval_timer` counts down to the next window
/// opening (used while `active` is false), `duration_timer` counts
/// down the current window (used while `active` is true).
/// `cascade_state_system` flips between them and resets the
/// appropriate one. The cycle is deterministic and asymmetric
/// (long quiet, short loud) by design — see PLAN.md §2.
///
/// Phase 2+ will also drive: 4× XP / 4× loot rarity on kills
/// during Cascade, +5s window extension on Beacon kills, and a
/// damage-taken multiplier. The `kills_during` and `chain_extends`
/// fields are in place now so we can land those without another
/// resource migration.
#[derive(Resource)]
pub struct CascadeState {
    pub active: bool,
    pub time_remaining: f32,
    pub kills_during: u32,
    pub chain_extends: u32,
    pub interval_timer: Timer,
    pub duration_timer: Timer,
}

impl Default for CascadeState {
    fn default() -> Self {
        Self {
            active: false,
            time_remaining: 0.0,
            kills_during: 0,
            chain_extends: 0,
            // Start with a 30s "warmup" so the first cascade doesn't
            // hit the player the moment they spawn. The interval
            // re-arms to its full length after each cascade ends,
            // so subsequent cascades are spaced by the full default.
            interval_timer: Timer::from_seconds(30.0, TimerMode::Once),
            duration_timer: Timer::from_seconds(0.0, TimerMode::Once),
        }
    }
}
