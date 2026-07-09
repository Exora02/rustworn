//! Cross-system event channels. Decouples producers from consumers and
//! makes behavior easy to observe from the dev console.

use bevy::prelude::*;

use crate::components::RoomId;

/// Fired when an entity takes damage. Carries the victim so a single
/// system can drive health bars, hit-flash, damage numbers, and audio.
#[derive(Event)]
pub struct DamageEvent {
    pub victim: Entity,
    pub amount: f32,
}

/// Fired when the player collects a pickup. Carries the kind so
/// the audio system can play the appropriate sound effect.
#[derive(Event)]
pub struct PickupCollectedEvent {
    pub kind: crate::components::PickupKind,
}

/// Fired when the player initiates a dash. Audio subscriber
/// plays the dash whoosh sound.
#[derive(Event)]
pub struct PlayerDashEvent;

/// Fired when the player fires a weapon shot. Carries the
/// archetype so the audio system can pick a different pitch /
/// sound per weapon type.
#[derive(Event)]
pub struct WeaponFiredEvent {
    pub archetype: crate::components::WeaponArchetype,
}

/// Fired when the player dashes through / hits an enemy during
/// a Reaver dash-strike. Audio subscriber plays the hit sound.
#[derive(Event)]
pub struct ReaverDashHitEvent;

/// Fired when a Cascade Mode window opens. Subscribers typically start
/// visual effects, audio sting, and UI transitions.
#[derive(Event)]
pub struct CascadeStartEvent;

/// Fired when a Cascade Mode window closes (timer expired or interrupted).
#[derive(Event)]
pub struct CascadeEndEvent {
    pub kills_during_cascade: u32,
}

/// Fired when an enemy dies. Consumed by loot drops, XP, and the
/// meta-progression's "Soul Imprint" tracker.
#[derive(Event)]
pub struct EnemyKilledEvent {
    pub victim: Entity,
    pub killer: Entity,
    pub position: Vec2,
}

// Phase 2.3: emitted by `wave_spawner_tick_system` when a
// room's last wave finishes. `door_unlock_system` consumes
// the event and sets `locked = false` on the room's forward
// door(s). Future subscribers (HUD "ROOM CLEAR" banner,
// Cascade beacon spawner, run-stat counters) can attach
// without changing the spawner.
#[derive(Event)]
pub struct RoomClearedEvent {
    pub room_id: RoomId,
}
