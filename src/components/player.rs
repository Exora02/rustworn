//! Player, weapons, projectiles, health, collision, and core movement components.

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

/// Last non-zero movement direction (WASD input). Updated every
/// frame by `player_input_system`; read by `dash_state_system`
/// when a dash fires. The dash direction is the movement
/// direction (where you're going), NOT the aim direction
/// (where the mouse is) — dashing toward the mouse feels
/// disconnected from WASD movement. Stays at the last
/// non-zero value while no WASD is pressed, so a dash with
/// no input goes the way you were last moving. Initialized
/// to `Vec2::Y` (up) at spawn so the very first dash has a
/// sensible direction before the player has touched WASD.
#[derive(Component, Default)]
pub struct LastMoveDirection(pub Vec2);

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
///
/// Phase 1 (shotgun): `pellet_count` and `spread_rad` carry the
/// multi-pellet behavior. Pistol keeps them at 1 / 0.0; shotgun gets
/// 5 pellets spread across `~17°`. Stored on the component (rather
/// than looked up per archetype) so the fire system has a single
/// source of truth and the switch system can refresh all fields at
/// once.
///
/// Phase 1 (burst): `burst_rounds` and `burst_rounds_remaining`
/// carry the multi-shot-per-click behavior. Pistol and shotgun
/// have `burst_rounds = 1` (a single click = a single shot; the
/// "burst" of 1 happens in zero time, the fire system treats
/// it the same as a one-shot). Burst has `burst_rounds = 3`,
/// so a single click fires 3 sequential rounds with a
/// `BurstRoundGapS` gap between each. `burst_rounds_remaining`
/// tracks the in-flight burst: 0 when no burst is in progress
/// (weapon ready to fire on next click), N>0 when the burst
/// has N more rounds to fire (the fire system auto-fires the
/// next round when the gap timer elapses, without waiting
/// for a click).
///
/// Phase 1 (beam): `beam_active` + `beam_tick_timer` carry
/// the held-trigger instant-hit behavior. Unlike the other
/// three weapons, beam does NOT spawn a Projectile — it
/// applies damage directly to enemies in line of sight on
/// a per-tick cadence. `beam_active` is true while the
/// mouse is held (set by `beam_damage_system` on
/// `mouse.just_pressed`, cleared on `just_released`),
/// false otherwise. `beam_tick_timer` is a Repeating
/// timer that elapses every `GameSettings::beam_tick_interval_s`
/// (0.1s = 10Hz) while the beam is active; each "tick"
/// applies `beam_damage_per_tick` to every enemy in the
/// beam's capsule. Effective DPS = `beam_damage_per_tick /
/// beam_tick_interval_s` = 0.5 / 0.1 = 5.0.
#[derive(Component)]
pub struct Weapon {
    pub archetype: WeaponArchetype,
    pub damage: f32,
    pub fire_rate: f32, // shots per second (used for post-burst cooldown)
    pub projectile_speed: f32,
    pub pellet_count: u32,
    pub spread_rad: f32,
    pub burst_rounds: u32,
    pub fire_timer: Timer,
    pub burst_rounds_remaining: u32,
    /// Phase 1 (beam): true while the player is holding the
    /// left mouse button. Beam damage ticks only fire while
    /// this is true. Toggled by `beam_damage_system`.
    pub beam_active: bool,
    /// Phase 1 (beam): repeating timer that fires the
    /// per-tick damage pass. Each `just_finished()` transition
    /// = one damage tick (0.5 dmg applied to all enemies in
    /// the beam's capsule). Reset by the press handler so
    /// the first tick is exactly `tick_interval` after the
    /// press, not immediate.
    pub beam_tick_timer: Timer,
}

/// Available weapon archetypes. Pistol is the single-shot starter;
/// Shotgun (Phase 1) fires 5 pellets in a tight spread. Burst
/// (Phase 1) fires 3 sequential rounds per click at a 0.1s
/// between-round gap — a "tapping" weapon that rewards rhythmic
/// timing and feels different from the other two. Beam
/// (Phase 1) is a held-trigger instant-hit laser — the player
/// holds the mouse and a continuous orange beam fires from
/// the player to max_range, dealing 0.5 damage per tick
/// (~5 DPS) to every enemy in its line of sight. The fourth
/// archetype is the only "DPS hose" — it scales with how many
/// enemies are in a line, where the other three deal discrete
/// shots to one enemy at a time. Future weapons (Launcher,
/// Sniper, etc.) will swap in by extending the match in
/// `weapon_stats_for` (combat.rs) — nothing else needs to
/// know the archetype enum.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponArchetype {
    #[default]
    Pistol,
    Shotgun,
    /// Phase 1 burst: 3 sequential rounds per click, 0.7 dmg
    /// per round, 0.1s between rounds, 0.5s post-burst cooldown
    /// (fire_rate = 2.0). Effective DPS: 0.7 × 3 / 0.7s = 3.0,
    /// same as pistol — but the damage arrives in a 3-hit
    /// spike rather than a steady tick. The player feels the
    /// "punch" of the burst more than the shotgun's spread
    /// (which feels like a "cloud").
    Burst,
    /// Phase 1 beam: held trigger, instant-hit (no projectile),
    /// 0.5 dmg/tick at 10Hz (5 DPS), max range 500 units,
    /// visualized as a long thin orange line. The
    /// `beam_damage_system` reads the player's position +
    /// aim direction each tick and applies damage directly
    /// to every enemy whose center falls inside the beam's
    /// capsule. fire_rate=10 in `weapon_stats_for` is a
    /// cosmetic read for the HUD bar (it shows a "fast"
    /// color-coded tick rate); the actual damage cadence
    /// comes from `GameSettings::beam_tick_interval_s`
    /// (0.1s).
    Beam,
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

/// Axis-aligned bounding box for contact-damage detection. `half_extents`
/// is the half-size in each axis — so a 40x40 sprite gets (20, 20).
/// Component-based so projectiles (Day 11-12) and pickups (later) can
/// have their own sizes without sharing the sprite's `custom_size`.
#[derive(Component, Clone, Copy, Debug)]
pub struct Collider {
    pub half_extents: Vec2,
}

/// Tracks which enemies have been hit by the Reaver's dash-strike
/// during the current dash. Removed when the dash ends (Invulnerable
/// removed). Cleared on each new dash.
#[derive(Component, Default)]
pub struct ReaverDashHit {
    pub hit_enemies: Vec<Entity>,
}

/// Brief knockback window after taking a hit. While present, the
/// player's `BaseVelocity` is overridden to the knockback direction.
/// Removed when the timer elapses. Hit i-frames are attached as a
/// separate `Invulnerable` so a player can dash-cancel out of a
/// knockback without losing the i-frame protection.
#[derive(Component)]
pub struct Knockback {
    pub direction: Vec2,
    pub timer: Timer,
}

// ─── Playable characters ────────────────────────────────────────────────────

/// Which playable character the player selected this session.
/// Determines starting stats, passive, and active ability.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerClass {
    #[default]
    Marauder,
    Reaver,
    Spectre,
    Engineer,
}
