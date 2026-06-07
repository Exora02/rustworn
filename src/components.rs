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
///
/// Phase 1 (shotgun): `pellet_count` and `spread_rad` carry the
/// multi-pellet behavior. Pistol keeps them at 1 / 0.0; shotgun gets
/// 5 pellets spread across `~17°`. Stored on the component (rather
/// than looked up per archetype) so the fire system has a single
/// source of truth and the switch system can refresh all fields at
/// once.
#[derive(Component)]
pub struct Weapon {
    pub archetype: WeaponArchetype,
    pub damage: f32,
    pub fire_rate: f32, // shots per second
    pub projectile_speed: f32,
    pub pellet_count: u32,
    pub spread_rad: f32,
    pub fire_timer: Timer,
}

/// Available weapon archetypes. Pistol is the single-shot starter;
/// Shotgun (Phase 1) fires 5 pellets in a tight spread. Future
/// weapons (Burst, Beam, Launcher, ...) will swap in by extending
/// the match in `weapon_stats_for` (combat.rs) — nothing else
/// needs to know the archetype enum.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponArchetype {
    #[default]
    Pistol,
    Shotgun,
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

// ─── Enemies & combat resolution ─────────────────────────────────────────

/// Marker: this entity is an enemy. Used by contact-damage and (later)
/// projectile-damage systems to find valid targets.
#[derive(Component)]
pub struct Enemy;

/// AI behavior state. Chaser is the simple "move toward player"
/// behavior from Phase 0. Dasher (Phase 1) alternates between a
/// slow approach and a fast lunge, forcing the player to time
/// their dashes against the burst. The enum is set up so future
/// archetypes (Shooter, Orbit, Sniper) can swap in by extending
/// the AI dispatch.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct EnemyAI {
    pub behavior: AiBehavior,
}

#[derive(Default, Clone, Copy, Debug)]
pub enum AiBehavior {
    #[default]
    Chaser,
    /// Phase 1 dasher: slow approach for a beat, then a fast
    /// straight-line lunge, then a brief recovery. The dash is
    /// fast enough that the player has to react (not just react
    /// to the chaser). Lower HP than the chaser, slightly more
    /// contact damage.
    Dasher,
}

/// Phase 1 dasher state. Tracks which phase of the
/// approach → dash → recover cycle the dasher is in, and the
/// timer that gates the next phase transition. Lives on the
/// enemy entity (rather than a resource) so multiple dashers
/// can be in different phases simultaneously.
#[derive(Component)]
pub struct DasherState {
    pub phase: DasherPhase,
    pub phase_timer: Timer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DasherPhase {
    /// Slow movement toward the player. Lasts `approach_duration_s`
    /// seconds, then transitions to Dash.
    Approach,
    /// Fast straight-line lunge toward the player (the player can
    /// hear this one coming). Lasts `dash_duration_s` seconds, then
    /// transitions to Recover.
    Dash,
    /// Brief pause after the dash — gives the player a window to
    /// counter-attack and makes the dash feel committed (a
    /// committed move is scarier than a continuous one).
    Recover,
}

/// Periodic spawner. Owns a Timer; when it elapses, the spawner system
/// creates one enemy at a random angle from the player and resets the
/// timer. The `spawn` field picks which archetype comes out — Chaser
/// (Phase 0) or Dasher (Phase 1). Persists for the whole run (no
/// despawn) — Day 15-16 polish will add a max-alive cap and
/// per-spawner despawn.
#[derive(Component)]
pub struct Spawner {
    pub interval: Timer,
    pub spawn: EnemyArchetype,
}

/// What a Spawner produces. Mirrors AiBehavior (every spawned enemy
/// has a corresponding AI behavior) but lives separately so the
/// spawner doesn't need to know about AI internals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyArchetype {
    Chaser,
    Dasher,
}

/// Axis-aligned bounding box for contact-damage detection. `half_extents`
/// is the half-size in each axis — so a 40x40 sprite gets (20, 20).
/// Component-based so projectiles (Day 11-12) and pickups (later) can
/// have their own sizes without sharing the sprite's `custom_size`.
#[derive(Component, Clone, Copy, Debug)]
pub struct Collider {
    pub half_extents: Vec2,
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

/// Short-lived fading-square visual spawned at an enemy's last
/// position when it dies. The system ticks the timer, fades the
/// alpha to 0, and despawns. Day 11-12 placeholder for the real
/// particle system that lands in Phase 1.
#[derive(Component)]
pub struct DeathPuff {
    pub lifetime: Timer,
}

/// Brief white flash on an enemy when hit by a projectile. Inserted
/// by the projectile-damage system on every hit (not just kills).
/// Drives a per-frame color lerp in the enemy plugin; the system
/// snaps the color back to the chaser base color when the timer
/// elapses so the next hit starts from the right base.
#[derive(Component)]
pub struct HitFlash {
    pub timer: Timer,
}

/// Marker for a HUD element. The HUD plugin positions each element
/// at a fixed offset from the player each frame (so the HUD stays
/// screen-stable as the camera follows) and updates its visual to
/// reflect current state (HP ratio, fire-rate progress, kills, time).
#[derive(Component)]
pub struct HudElement {
    pub kind: HudElementKind,
}

#[derive(Clone, Copy, Debug)]
pub enum HudElementKind {
    HpBar,
    FireRate,
    // Day 15-16: top-right pair showing the player's run progress
    // (kills + elapsed time) and a centered game-over overlay that
    // appears when `RunState.game_over` flips.
    KillBar,
    TimeBar,
    GameOver,
    RestartPrompt,
}
