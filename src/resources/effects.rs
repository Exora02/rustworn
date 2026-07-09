use bevy::prelude::*;

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

/// Phase 2 Patch 11: horde-mode boss speed ramp. Inserted by
/// `boss_death_room_clear_system` when a Boss room is cleared.
/// While present, `wave_spawner_tick_system` reduces the farming
/// phase spawn interval from the base `farming_interval` toward
/// 0.1s over `duration_s`. Removed when the player transitions
/// to the next room (teleport_transition_system despawns all
/// spawner entities, which clears the resource naturally).
#[derive(Resource)]
pub struct BossRampTimer {
    pub timer: Timer,
    /// Starting interval at ramp start (copied from spawner).
    pub start_interval: f32,
}
