//! Shared component definitions. Kept centralized so the data model is
//! easy to audit against PLAN.md §8.

use bevy::prelude::*;

// ─── Core game components ──────────────────────────────────────────────────

/// Marks an entity as the player. Single-instance per run.
#[derive(Component)]
pub struct Player;

/// Health points. The `current` value never goes below 0; the damage
/// system enforces one-hit protection separately.
#[derive(Component, Clone, Copy, Debug)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }
}

/// True velocity — fed into the transform integration. Cascade-scaled
/// each frame from `BaseVelocity`, so we never compound the scale.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct Velocity(pub Vec2);

/// Physical "intended" velocity, not affected by Cascade Mode. The
/// cascade_scale_system reads from this and writes the scaled value
/// into `Velocity`. See PLAN.md §8 "ECS patterns".
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct BaseVelocity(pub Vec2);

/// Marker: this entity's velocity is scaled during Cascade Mode.
#[derive(Component, Default)]
pub struct CascadeAffected;
