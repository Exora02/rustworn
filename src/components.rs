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
}

// Phase 2.3: replaced the 4 parallel `Spawner` entities (one
// per archetype) with a single `WaveSpawner` per Fight room.
// See `WaveSpawner` below. The old `Spawner` struct is removed.
//
// `EnemyArchetype` stays — it's the per-enemy tag that
// `spawn_chaser/dasher/shooter/sniper` dispatches on, and it's
// the value stored in `WaveDefinition.archetype`.

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

/// Phase 1 (Vampire Survivors fusion): a pickup dropped by a killed
/// enemy. Drifts gently toward the player each frame (or sits still
/// in the world) and auto-applies its effect on contact. The drop
/// chance is on `GameSettings` per archetype; chasers drop more
/// often than snipers. Effects are permanent for the run (Phase 1
/// is "kill → get stronger", not a slot system — Phase 2 will
/// introduce a choose-1-of-3 chest).
#[derive(Component)]
pub struct Pickup {
    pub kind: PickupKind,
    pub lifetime: Timer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupKind {
    /// +0.5 HP/sec regen for the rest of the run.
    Regen,
    /// +25% damage on all weapons.
    DamageUp,
    /// +30% fire rate on all weapons.
    FireRateUp,
    /// +20% move speed for the rest of the run.
    SpeedUp,
    /// Weapon pickup. Adds the weapon to the first empty
    /// inventory slot. If the inventory is full, the pickup
    /// is silently destroyed (no swap UI in P4).
    ///
    /// Phase 2.4: P4 loot drops. The weapon is a fresh copy
    /// of the archetype's base stats (no rarity yet — that's
    /// a future PLAN.md §5 mod-system patch). The pickup
    /// drops per-archetype chance on `GameSettings`
    /// (`pickup_drop_chance_weapon_*`).
    Weapon(WeaponArchetype),
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

/// Multi-slot weapon inventory. The player has 4 slots (PLAN.md
/// §5) — 2 to 6 is the meta-progression range, but 4 is the
/// Phase 2 default. `None` is an empty slot; `Some(archetype)`
/// is a weapon the player owns.
///
/// The player entity ALSO has a `Weapon` component that holds
/// the ACTIVE weapon's full state (damage, fire_timer, burst
/// state, etc.). The inventory is the source of truth for
/// "what the player owns"; the `Weapon` component is the
/// source of truth for "what the player is currently using".
/// Switching weapons (via 1-4 hotkeys) copies the inventory
/// slot's archetype + its base stats into the `Weapon` component.
///
/// P4 scope: picking up a weapon adds it to the first empty
/// slot. If the inventory is full, the pickup is silently
/// destroyed (no swap UI yet — that's a future patch that
/// needs a confirmation prompt). The active weapon is
/// unchanged on pickup.
#[derive(Component, Clone, Debug)]
pub struct WeaponInventory {
    pub slots: [Option<WeaponArchetype>; 4],
}

impl Default for WeaponInventory {
    fn default() -> Self {
        Self {
            slots: [Some(WeaponArchetype::Pistol), None, None, None],
        }
    }
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

/// Phase 1 beam: marker for the visual sprite entity that
/// represents the active beam. A single persistent entity
/// spawned once at startup; `beam_damage_system` updates its
/// position, rotation, scale, and visibility each frame
/// based on the player's position, aim direction, and the
/// `Weapon::beam_active` flag. Lives separately from the
/// player so the beam's z-order, size, and color are
/// independent of the player's transform. The sprite is
/// anchored at `Anchor::CenterLeft` (its transform position
/// is the player's position, the beam extends outward in
/// the +X direction after rotation), so a 0° rotation
/// points the beam along the +X axis and a 90° rotation
/// points it along +Y.
#[derive(Component)]
pub struct BeamVisual;

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
    // Phase 1 mini-boss: a prominent HP bar at top-center, 60%
    // of the viewport wide, 8px tall, deep red on a dark
    // background. Hidden by default; the HUD update system flips
    // it to `Visibility::Inherited` only when a `With<Boss>`
    // entity exists, and back to `Visibility::Hidden` when
    // the boss is killed. Fill scale = boss's
    // `Health.current / Health.max`. Lives alongside the
    // other bars in the HUD; doesn't break their layout.
    BossHpBar,
    // Phase 2: title-screen overlay. A dim full-screen background
    // + "PRESS ENTER TO START" text. Hidden by default; the
    // state_visibility_system flips it to `Visibility::Inherited`
    // only while `AppState == MainMenu`. Spawned once at startup
    // (alongside the camera) so the entity is always present and
    // we just toggle its visibility per state.
    MainMenu,
    // Phase 2 P5: XP counter bar, positioned above the kill bar.
    // Shows the player's XP-to-10 progress as a thin bar. Color
    // is a warm gold so it reads as "Cascade progress" rather
    // than "kill counter" (which is a dim white).
    XpBar,
}

// ─── AppState (Phase 2) ───────────────────────────────────────────────────

/// Top-level game state. Lifts the Phase 1 `RunState.game_over: bool`
/// into a proper Bevy `States` enum so each major UI/mode gets its
/// own system schedule.
///
/// Why a state machine (not a bool): Phase 1 used a single `game_over`
/// flag because there were only two modes (alive / dead). Phase 2
/// needs three more: a `MainMenu` title screen, a `Paused` state
/// (scaffolded for the future pause feature), and a clean separation
/// between the in-run gameplay systems and the death-screen systems.
/// `in_state(AppState::InRun).run_if(...)` is the canonical way to
/// gate the gameplay chain in Bevy 0.16; the bool pattern doesn't
/// scale past 2 modes.
///
/// `#[derive(Default)]` + `#[default]` on `MainMenu` makes the app
/// boot into the title screen — the player presses ENTER to start
/// the first run, not straight into combat.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    /// Title screen. Player + spawners exist but are hidden / not
    /// ticking; gameplay systems are gated off. ENTER transitions
    /// to InRun (which fires the per-run reset in
    /// `enter_in_run_system`).
    #[default]
    MainMenu,
    /// Active run. All gameplay systems tick, the HUD shows run
    /// progress, and `player_death_check_system` watches for HP <= 0
    /// to transition to GameOver.
    InRun,
    /// Player died. Gameplay systems are gated off (the world is
    /// frozen on a still frame), the game-over overlay is visible,
    /// and R restarts (→ InRun) or ESC returns to the title (→
    /// MainMenu). The transition InRun → GameOver fires
    /// `enter_game_over_system`, which can hook fade-in effects
    /// in a later patch.
    GameOver,
    /// Paused. Scaffolded for a future pause feature (ESC during
    /// InRun, ESC again to resume). No systems gate on it yet —
    /// it's here so the enum is a complete top-level state model
    /// and we can add the plumbing without another refactor.
    Paused,
}

// ─── Room system (Phase 2) ────────────────────────────────────────────────

/// Unique identifier for a room in a biome. `u8` gives 256 rooms
/// per biome — more than enough for Phase 2's 3 rooms and the
/// Phase 3+ multi-biome scaling.
///
/// Newtype (not a bare `u8`) so the type system catches "you
/// passed an enemy archetype id where a room id was expected"
/// at compile time. `#[derive(Default)]` defaults to `RoomId(0)`
/// so a freshly-spawned enemy / projectile etc. has a
/// placeholder room id (it'll be overwritten by the spawner
/// or filtered out by `RoomScope`-aware systems).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct RoomId(pub u8);

/// A rectangular room in a biome. The player can move freely
/// inside the `bounds` rectangle. Walls sit on the rectangle's
/// edges; doors are triggers on the floor in front of the walls.
///
/// `bounds_min` and `bounds_max` are the world-space corners.
/// The room is the closed set `[min, max]` on each axis. Walls
/// sit just outside the rectangle (offset by
/// `RoomsPlugin::WALL_THICKNESS` / 2) so the AABB is
/// `bounds_max + thickness` in extent.
#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub id: RoomId,
    pub kind: RoomKind,
    pub bounds_min: Vec2,
    pub bounds_max: Vec2,
}

impl Room {
    pub fn center(&self) -> Vec2 {
        (self.bounds_min + self.bounds_max) * 0.5
    }
    pub fn size(&self) -> Vec2 {
        self.bounds_max - self.bounds_min
    }
}

/// Semantic role of a room. Phase 2 ships 3 rooms (Spawn /
/// Fight / Extract); Patch 6 will add the "Extract" exit portal
/// inside the Extract room. New room kinds (Treasure, Gauntlet
/// per PLAN.md §6) slot in here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    /// Where the player spawns at the start of a run. No
    /// enemies, no pickups. The "calm before the storm".
    Spawn,
    /// Where the bulk of combat happens. Spawners are
    /// scoped to this room (Phase 2 minimum: all 4 spawners
    /// in the Fight room).
    Fight,
    /// Where the run ends. Phase 2: just a labeled room
    /// with no enemies. Phase 6 adds the extraction portal
    /// here.
    Extract,
}

/// Door trigger: a small AABB on the floor in front of a wall.
/// When the player's AABB overlaps a door,
/// `door_transition_system` teleports them to `to_room` at
/// `entry_position` and despawns the old room's transient
/// entities (enemies, projectiles, puffs, pickups).
///
/// Doors don't have a collider of their own that blocks the
/// player — the surrounding wall does that. The door's
/// `Collider` is just the AABB used for the overlap check.
/// (Walls are full-perimeter; doors are floor triggers on
/// the inside of the room, not gaps in the walls.)
///
/// Phase 2.3: added `from_room`, `locked`, and `forward`.
/// - `from_room`: the room the player is IN when they step
///   on this door's trigger. The "source" of the transition.
///   Symmetric to `to_room` (the destination). Without this
///   field, `door_unlock_system` would have to infer the
///   source from the door's position relative to a room,
///   which is brittle and breaks for branching paths in
///   Phase 3+.
/// - `locked = true` means the door refuses to fire the
///   transition even if the player overlaps its AABB.
///   Set to `false` by `door_unlock_system` when the
///   source room's waves are cleared.
/// - `forward = true` marks this door as a "progression"
///   door — it's locked by default and unlocks on clear.
///   Backward doors (`forward = false`) are never locked:
///   the player can always retreat to a previous room.
#[derive(Component, Clone, Copy, Debug)]
pub struct Door {
    pub from_room: RoomId,
    pub to_room: RoomId,
    pub entry_position: Vec2,
    pub locked: bool,
    pub forward: bool,
}

/// Marker on entities that belong to a specific room. Used
/// by `spawner_tick_system` to filter which spawners tick
/// (only the ones in the current room) and by future per-
/// room scoping systems (e.g. per-room decoration lights,
/// per-room audio cues).
///
/// Phase 2 attaches `RoomScope` only to spawner entities
/// (one per archetype, all in the Fight room). Enemies
/// don't need it — they're spawned in the current room and
/// despawned on transition.
#[derive(Component, Clone, Copy, Debug)]
pub struct RoomScope {
    pub room_id: RoomId,
}

/// Static wall entity. Walls block player + enemy movement
/// (but not projectiles, which fly through freely in Patch 2).
/// Has `Transform + Sprite + Collider` like other colliders,
/// plus this marker so `physics_step_system` and the door
/// system can find them.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Wall;

// ─── Wave spawning (Phase 2.3) ────────────────────────────────────────────

/// One wave in a room's progression: spawn `count` enemies of
/// `archetype`, spaced `interval_s` apart.
///
/// A wave is "done" when `count` enemies have been spawned.
/// The `WaveSpawner` then advances to the next wave in its
/// `waves` vec. When all waves are done, the room is "cleared"
/// and its forward door(s) unlock.
///
/// `interval_s = 0.0` means "spawn all enemies of this wave
/// on the same frame" — useful for cinematic openings or
/// scripted "ambush" waves. The `wave_spawner_tick_system`
/// treats 0.0 as a special case (spawn all remaining, then
/// advance immediately).
#[derive(Clone, Copy, Debug)]
pub struct WaveDefinition {
    pub archetype: EnemyArchetype,
    pub count: u32,
    pub interval_s: f32,
}

/// One wave spawner per room that has enemies. Replaces the
/// Phase 1 / 2.0 design of 4 parallel `Spawner` entities
/// (one per archetype, all firing every 3s in parallel).
///
/// State machine:
/// 1. `current_wave < waves.len()` → ticking `wave_timer`
/// 2. When the timer fires and `spawned_this_wave < waves[current_wave].count`,
///    spawn one enemy and reset the timer to `interval_s`.
/// 3. When `spawned_this_wave == waves[current_wave].count`, advance:
///    `current_wave += 1`, `spawned_this_wave = 0`, re-arm `wave_timer`
///    with the NEXT wave's `interval_s`.
/// 4. When `current_wave == waves.len()` (all waves done), the
///    spawner stops ticking. `door_unlock_system` observes this
///    and sets `locked = false` on the room's forward door(s).
///
/// `room_id` replaces the Phase 2.0 `RoomScope` marker for
/// spawners — the spawner is now a struct with its own room
/// field, so it doesn't need a separate component.
///
/// `wave_timer` is `TimerMode::Repeating`-equivalent: each
/// spawn resets it to the current wave's `interval_s` (so
/// the next spawn is `interval_s` later, not on the next
/// frame). This matches the Phase 1 `Spawner.interval`
/// semantics.
#[derive(Component, Debug)]
pub struct WaveSpawner {
    pub room_id: RoomId,
    pub waves: Vec<WaveDefinition>,
    pub current_wave: usize,
    pub spawned_this_wave: u32,
    pub wave_timer: Timer,
}
