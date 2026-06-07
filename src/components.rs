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
}

/// Available weapon archetypes. Pistol is the single-shot starter;
/// Shotgun (Phase 1) fires 5 pellets in a tight spread. Burst
/// (Phase 1) fires 3 sequential rounds per click at a 0.1s
/// between-round gap — a "tapping" weapon that rewards rhythmic
/// timing and feels different from the other two. Future
/// weapons (Beam, Launcher, ...) will swap in by extending the
/// match in `weapon_stats_for` (combat.rs) — nothing else
/// needs to know the archetype enum.
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
    /// Phase 1 shooter: moves toward the player until at a
    /// preferred range, then stops and fires enemy projectiles
    /// on a cooldown. The first ranged enemy — tests the
    /// enemy-projectile pipeline that no other enemy uses yet
    /// (the player takes damage from `Team::Enemy` projectiles
    /// in `enemy_projectile_damage_system`). Lighter contact
    /// damage than the dasher, lower HP than the chaser.
    Shooter,
    /// Phase 1 sniper: stops at long range and charges a big
    /// shot, telegraphed by a color shift from the sniper's
    /// base color (yellow) to a hot red as the charge fills.
    /// Fires one slow high-damage bullet at the end of the
    /// charge. After firing, repeats. The "anticipation"
    /// pattern — the player sees the charge, decides whether
    /// to close the gap, sidestep the bullet, or use the
    /// charge window to land a free hit. First enemy that
    /// tests "stop moving and do something dangerous" (the
    /// shooter stops too, but its firing is continuous and
    /// unmarked).
    Sniper,
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

/// Phase 1 shooter state. Just a fire cooldown — when it
/// elapses, the shooter's AI spawns an enemy projectile and
/// resets the timer. Per-entity (not a resource) so multiple
/// shooters fire on independent cadences. The "approach until
/// in range, then stop" decision is made each frame in
/// `enemy_ai_system` from the player's position — no need to
/// track that here.
#[derive(Component)]
pub struct ShooterState {
    pub fire_timer: Timer,
}

/// Phase 1 sniper state. Tracks the charge-up phase and the
/// post-fire cooldown. The sniper's AI ticks the charge
/// timer; when it elapses, the sniper fires a big slow
/// bullet and resets the timer to a post-fire cooldown
/// (longer than the charge so the player has a window to
/// retaliate). The sprite color shift from base → red as
/// the charge fills is driven by the AI from the same
/// timer — the visual telegraph is the player's only
/// warning that a shot is incoming.
#[derive(Component)]
pub struct SniperState {
    pub charge_timer: Timer,
    pub is_charging: bool,
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
    /// Phase 1 shooter: ranged enemy that fires projectiles at
    /// the player. Same `Spawner` plumbing as Chaser/Dasher —
    /// the AI system dispatches on the `EnemyAI` component, the
    /// spawner just picks the archetype.
    Shooter,
    /// Phase 1 sniper: long-range, charges up for ~1.5s, fires
    /// one big slow bullet (2 dmg) with a telegraphed color
    /// shift. The "anticipation" pattern — different from any
    /// other enemy. Player has to read the charge and dodge.
    Sniper,
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
/// snaps the color back to the enemy's `BaseColor` when the timer
/// elapses so the next hit starts from the right base — and so a
/// purple dasher stays purple instead of getting hard-reset to
/// chaser red.
#[derive(Component)]
pub struct HitFlash {
    pub timer: Timer,
}

/// The enemy's "true" color — its archetype's base tint. Stored as a
/// component (rather than derived from `EnemyAI::behavior`) so the
/// hit-flash system can reset/lerp from it without re-running the
/// archetype match, and so future visual effects (status tints,
/// cascade tint) can read it the same way. Set at spawn time in
/// `spawn_chaser` / `spawn_dasher`; never changes for the lifetime
/// of the entity.
#[derive(Component, Clone, Copy, Debug)]
pub struct BaseColor(pub Color);

/// Marker for a HUD element. The HUD plugin positions each element
/// at a fixed offset from the player each frame (so the HUD stays
/// screen-stable as the camera follows) and updates its visual to
/// reflect current state (HP ratio, fire-rate progress, kills, time).
#[derive(Component)]
pub struct HudElement {
    pub kind: HudElementKind,
}

/// Phase 1 cascade telegraph: a full-screen overlay sprite that
/// tints the world during a Cascade window. Solid purple tint
/// while the window is open, pulsing purple in the 3s before
/// opening, hidden otherwise. Per-entity marker so the
/// cascade-telegraph update system can find the overlay via
/// `Query<&mut Sprite, With<CascadeTelegraph>>` and update its
/// color/alpha/visibility without disturbing any other HUD
/// element. The overlay is positioned at the player each
/// frame so it always covers the camera viewport (the overlay
/// is 2× the view size to ensure it covers even during
/// extreme camera-follow lag).
#[derive(Component)]
pub struct CascadeTelegraph;

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
    // Phase 1 cascade: a bar that shows the next Cascade window
    // — filling toward 1.0 while the cooldown counts down, then
    // draining back toward 0.0 while the window is active. Hidden
    // entirely when no cooldown is in flight (so the HUD is quiet
    // outside the cycle). Positioned just below the time bar.
    CascadeBar,
}
