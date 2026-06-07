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

/// Marker for the main camera. Tags the camera so we can find it in
/// queries without relying on Camera2d alone (which Bevy may attach to
/// auxiliary views for UI layers in the future).
#[derive(Component)]
pub struct MainCamera;

/// Last-known aim direction in world space (unit vector). Written by
/// `aim_input_system`, read by `dash_state_system` and `aim_rotation_system`.
/// Stays at the previous value when the cursor is dead-on the player.
#[derive(Resource, Default)]
pub struct AimDirection(pub Vec2);

/// Brief invulnerability window after a dash. While present, contact
/// damage is ignored. Contact-damage wiring lands in Day 8-10; the
/// component is defined now so the dash system can attach it from day one.
#[derive(Component)]
pub struct Invulnerable(pub Timer);

/// Cooldown gate for the dash. Present between the end of an iframe
/// window and when the player can dash again. Dash input requires
/// neither this nor `Invulnerable` to be present.
#[derive(Component)]
pub struct DashCooldown(pub Timer);

/// Marker for the white "aim indicator" child entity spawned alongside
/// the player. Rotated each frame to point in the current `AimDirection`.
#[derive(Component)]
pub struct AimIndicator;

/// Player base color — the Exo orange from our protogen mascot palette.
/// Re-exposed here so systems (dash flash, future hit flash) can lerp
/// toward it without re-declaring the constant.
pub const PLAYER_COLOR: Color = Color::srgb(0.925, 0.525, 0.012); // ~#ec8603

// ─── Weapons & projectiles ────────────────────────────────────────────────

/// Active weapon attached to an entity (player today, enemies later).
/// Tracks its own cooldown so the firing system can pace shots.
#[derive(Component)]
pub struct Weapon {
    pub archetype: WeaponArchetype,
    pub damage: f32,
    pub fire_rate: f32, // shots per second
    pub projectile_speed: f32,
    pub fire_timer: Timer,
}

/// Available weapon archetypes. Pistol is the only one in Phase 0; the
/// enum exists so future weapons (Shotgun, SMG, etc.) can swap in
/// without touching the fire system.
#[derive(Default, Clone, Copy, Debug)]
pub enum WeaponArchetype {
    #[default]
    Pistol,
}

/// A projectile in flight. Owns its lifetime so the cleanup system
/// can despawn it without polling another structure. Has `Velocity`
/// but not `BaseVelocity` — player projectiles never cascade-scale
/// (per PLAN.md §2).
#[derive(Component)]
pub struct Projectile {
    pub damage: f32,
    pub lifetime: Timer,
    pub team: Team,
}

/// Faction. Phase 0 only spawns Player projectiles; Enemy is reserved
/// for Day 8-10 when chasers start shooting back. Used by the future
/// damage system to filter friendly fire.
#[derive(Default, Clone, Copy, Debug)]
pub enum Team {
    #[default]
    Player,
    Enemy,
}
