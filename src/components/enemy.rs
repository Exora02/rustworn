//! Enemy, AI, boss, beacon, and combat-visual components.

use bevy::prelude::*;

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
    /// Phase 1 mini-boss: single-instance per run, spawned
    /// once at `boss_spawn_time_s` (60s default). Two phases
    /// driven by `BossState` — Phase 1 (HP > half) chases at
    /// `boss_move_speed`; Phase 2 (HP <= half) enrages with
    /// 1.5× speed and a 3-burst spread every 2s. On death,
    /// `RunState::cascade_beacon_alive` flips to true to
    /// reserve the hook for the Phase 1 cascade.
    Boss,
    /// Phase 2 P5: Cascade beacon. Static (no AI movement, no
    /// contact damage) target that spawns the moment a
    /// Cascade window opens. Killing it during Cascade
    /// extends the window by
    /// `GameSettings::beacon_chain_extension_s` (5s) — see
    /// `combat.rs::projectile_damage_system` for the
    /// chain-extend hook. Outside Cascade the beacon is
    /// still hittable (the spawner doesn't check, the kill
    /// handler does) so a Cascade window that ends before
    /// the player kills the beacon leaves a "free" target
    /// on the field — that's fine, the player can mop up
    /// for a few XP and a stat-stick drop.
    Beacon,
    /// Phase 3d Bomber: walks toward the player at moderate
    /// speed. On death, detonates in an AoE circle that
    /// damages the player if caught inside. Creates a
    /// positioning puzzle — kill it at range or kite it
    /// away from your escape route.
    Bomber,
    /// Phase 3d Orbiter: maintains a fixed orbit radius
    /// around the player, firing projectiles tangent to
    /// the orbit at regular intervals. Multiple orbiters
    /// create layered bullet rings the player must
    /// navigate.
    Orbiter,
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

/// Phase 3d Bomber state. Tracks the explosion radius for
/// the death detonation. The bomber uses Chaser-style AI
/// (walk toward player); the `explosion_radius` field is
/// read by the death-AoE handler in combat.rs.
#[derive(Component)]
pub struct BomberState {
    pub explosion_radius: f32,
}

/// Phase 3d Orbiter state. Maintains a circular orbit
/// around the player at `orbit_radius` distance. Each
/// tick the `angle` advances by `angular_speed` delta.
/// Fires a projectile tangent to the orbit every
/// `fire_interval_s`.
#[derive(Component)]
pub struct OrbiterState {
    pub angle: f32,
    pub orbit_radius: f32,
    pub angular_speed: f32,
    pub fire_timer: Timer,
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
    /// Phase 1 mini-boss: single-instance per run, spawned
    /// directly by `boss_spawn_system` (not via a Spawner
    /// entity) at the run-midpoint. The variant exists here
    /// so the enum stays exhaustive over the full set of
    /// possible enemy types — the spawner dispatch (in
    /// `enemy.rs::spawn_enemy`) is a no-op for Boss.
    Boss,
    /// Phase 2 P5: see `AiBehavior::Beacon` for the full
    /// design. Same no-op dispatch as Boss — the beacon is
    /// spawned by the Cascade-start handler, not by the
    /// wave system.
    Beacon,
    /// Phase 3d Bomber: walks toward the player, explodes on death.
    Bomber,
    /// Phase 3d Orbiter: circles the player, fires tangent projectiles.
    Orbiter,
}

// ─── Mini-boss (Phase 1) ─────────────────────────────────────────────────

/// Marker: this entity is the run's mini-boss. Single-instance per
/// run — `boss_spawn_system` in the player plugin spawns exactly
/// one when `run.time_elapsed` crosses `boss_spawn_time_s`, and
/// the death path flips `run.cascade_beacon_alive` to true. The
/// HUD's boss HP bar query discriminates on this marker so the
/// bar is visible only while the boss is alive.
#[derive(Component)]
pub struct Boss;

/// Phase 2 P5: marker for the Cascade beacon. The kill
/// handler in `combat.rs::projectile_damage_system` reads
/// this via `Has<Beacon>` to fire the cascade-chain-extend
/// hook; the contact-damage handler reads it to skip
/// applying player damage (beacon is a "hit me" target, not
/// a "touch me" one). A simple unit struct — the beacon
/// has no per-entity state, the cascade window's `BeaconKilled`
/// logic only needs to know the entity WAS a beacon.
///
/// Only one beacon should exist at a time (per Cascade
/// window); `RunState::cascade_beacon_alive` is the
/// defensive guard, the marker is the discriminator.
#[derive(Component)]
pub struct Beacon;

/// Boss AI state. Carries the per-instance state that the shared
/// `enemy_ai_system` can't get from a component lookup:
///
///   - `phase`: the current phase (Phase1 chase or Phase2 enrage).
///     Detected each frame from `Health.current` crossing the
///     half-HP threshold; the transition resets `attack_timer`
///     so the first Phase 2 3-burst doesn't fire on the enrage
///     frame.
///   - `attack_timer`: unused in Phase 1. In Phase 2 counts down
///     from `boss_phase2_attack_interval_s` (2s default); when it
///     hits zero, the AI fires a 3-burst aimed at the player and
///     resets the timer.
///   - `current_hp`: mirrors `Health.current` so the phase
///     transition can be detected by comparing the previous
///     frame's value to the current. Stored on the entity
///     instead of re-querying `Health` to keep the AI tick
///     branchless.
#[derive(Component)]
pub struct BossState {
    pub phase: BossPhase,
    pub attack_timer: Timer,
    /// Separate timer for the second (health-gated) attack pattern.
    /// Runs independently so both patterns can coexist at different
    /// intervals even after the boss enters Phase 2.
    pub phase2_attack_timer: Timer,
    pub current_hp: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BossPhase {
    /// Slow chase. Movement = `boss_move_speed`. No attack.
    #[default]
    Phase1,
    /// Enraged. Movement = `boss_move_speed *
    /// boss_phase2_speed_multiplier` (1.5×). Fires a 3-burst
    /// aimed at the player every `boss_phase2_attack_interval_s`
    /// (2s).
    Phase2,
    /// Spawning minions (Lich death behavior).
    Summoning,
    /// Wind-up for charge attack (Yeti).
    Charging,
    /// Teleporting around the room (Void Walker).
    Teleporting,
}

// ─── Shared combat-visual components ───────────────────────────────────────

/// Short-lived fading-square visual spawned at an enemy's last
/// position when it dies. The system ticks the timer, fades the
/// alpha to 0, and despawns. Day 11-12 placeholder for the real
/// particle system that lands in Phase 1.
#[derive(Component)]
pub struct DeathPuff {
    pub lifetime: Timer,
}

// ─── Status Effects (Burn, Slow, Stun) ──────────────────────────────────────

/// Status effects applied to enemies by player projectiles and beam.
///
/// Each optional `Timer` represents an active effect:
/// - Burn: deals 0.5 damage/sec over 3s.
/// - Slow: halves movement speed over 2s.
/// - Stun: prevents all movement and attacks for 1s.
#[derive(Component, Default)]
pub struct StatusEffects {
    pub burn_timer: Option<Timer>,
    pub slow_timer: Option<Timer>,
    pub stun_timer: Option<Timer>,
}

// ─── Shared combat-visual components ───────────────────────────────────────

/// Phase 5 game juice: small burst particle spawned on enemy death.
/// Each particle has a fixed lifetime, an outward velocity, and
/// fades from its spawn color toward transparent. Spawned in groups
/// of 3-5 at the death position.
#[derive(Component)]
pub struct DeathParticle {
    pub lifetime: Timer,
    pub velocity: Vec2,
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

/// Phase 2 P5.1: marker for entities that should leave motion
/// trails (Sandevistan-style afterimages) during Cascade Mode.
/// Applied to the player and the Beacon. The
/// `trail_spawn_system` in fx.rs reads this marker and spawns
/// `TrailGhost` entities at the entity's current position.
#[derive(Component)]
pub struct MotionTrail;

/// Phase 2 P5.1: a ghost afterimage spawned by
/// `trail_spawn_system`. Fades from its spawn alpha to 0 over
/// `lifetime`, then is despawned by `trail_fade_system`.
#[derive(Component)]
pub struct TrailGhost {
    pub lifetime: Timer,
    pub initial_color: Color,
}

/// Phase 1 (Vampire Survivors fusion): floating damage number. Spawned
/// at the hit point on every `DamageEvent`, floats up + fades alpha
/// over `lifetime`, then despawns. The `value` is the integer-rounded
/// damage dealt (e.g. a 1.3-dmg pistol hit on a chaser shows "1",
/// not "1.3") — fractional damage is rare and rounding reads as
/// "definitely a number, not a calculation artifact" to the player.
/// Renders as a Text2d entity so it lives in the world space and
/// follows the camera naturally. The font is the default FiraMono
/// bundled with the `default_font` bevy feature (see Cargo.toml).
#[derive(Component)]
pub struct DamageNumber {
    pub value: i32,
    pub lifetime: Timer,
    pub drift_speed: f32,
}

/// Phase 1 hit feedback: a small white sprite that flashes briefly
/// at the hit point and despawns. Distinct from `HitFlash` (which
/// tints an existing enemy) — this is a standalone particle. Cheap
/// to spawn (single sprite, no physics), high readability ("I hit
/// it" signal on every pellet / beam tick / contact).
#[derive(Component)]
pub struct HitSpark {
    pub lifetime: Timer,
}

/// Phase 4: floating pickup popup. Spawned when the player collects a
/// pickup (gold, health, weapon, damage-up, etc.). Floats upward while
/// fading alpha over its lifetime, then despawns. Renders as a Text2d
/// entity in world space. Uses a queue system: a global resource tracks
/// how many popups are currently visible, and old popups are despawned
/// if the count exceeds the max.
#[derive(Component)]
pub struct PickupPopup {
    pub lifetime: Timer,
    pub drift_speed: f32,
}

/// Phase 3b: elite enemy variant. When an enemy is spawned as
/// an elite (via the director budget system), this marker is
/// attached. Elite enemies have:
/// - 2× base HP (bonus HP from floor scaling stacks on top)
/// - 1.5× base damage (from contact + projectile)
/// - Gold-tinted sprite (visual distinction)
/// - 2× loot drop chance bonus (stacked with curse)
/// - Visual aura ring (child sprite entity)
#[derive(Component)]
pub struct Elite;

/// Marker for boss-specific projectiles (spread, lob, homing, orbit).
/// Carries a lifetime after which the projectile despawns.
#[derive(Component)]
pub struct BossProjectile {
    pub lifetime: Timer,
}