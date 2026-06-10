//! Enemy plugin: spawner, AI, and contact-damage resolution.
//!
//! Day 8-10: one chaser archetype. Spawner ticks every N seconds and
//! creates a chaser at a random angle from the player. Chaser homes
//! in at a constant speed. On AABB contact with the player, deals
//! damage, applies knockback, and gives a brief i-frame window.

use bevy::prelude::*;
use log::warn;

use crate::components::{
    AiBehavior, AppState, BaseColor, BaseVelocity, Beacon, Boss, BossPhase, BossState,
    CascadeAffected, Collider, DasherPhase, DasherState, DeathPuff, Enemy, EnemyAI, EnemyArchetype,
    Health, HitFlash, Invulnerable, Knockback, MotionTrail, Player, Projectile, Room, RoomKind,
    ShooterState, SniperState, Team, Velocity, WaveSpawner,
};
use crate::events::{DamageEvent, RoomClearedEvent};
use crate::plugins::fx::spawn_hit_spark;
use crate::resources::{BossRampTimer, FloorState, GameSettings, Hitstop, RunRng, RunState, ScreenShake};

/// Color of the chaser — high-contrast red so it reads as "danger" at
/// a glance against the orange player and the dark background we'll
/// add later.
const CHASER_COLOR: Color = Color::srgb(0.84, 0.16, 0.16); // ~#d62828

/// Phase 1 dasher color. Purple/magenta so it's visually distinct
/// from the red chaser — the player learns "red = chaser, purple
/// = dasher" in one glance. The hue also telegraphs "burst damage"
/// (purple is the universal "fast/danger" color in arcade games).
const DASHER_COLOR: Color = Color::srgb(0.62, 0.20, 0.85); // ~#9e33d9

/// Phase 1 shooter color. Teal so it's distinct from the red
/// chaser and the purple dasher — the player learns "red =
/// melee rush, purple = lunge burst, teal = ranged plink".
/// Teal also reads as "ice/energy" in arcade-game color
/// shorthand, which fits a slower-firing, distance-keeping enemy.
const SHOOTER_COLOR: Color = Color::srgb(0.20, 0.75, 0.75); // ~#33bfbf

/// Color of the shooter's projectile — same teal as the body but
/// a touch lighter / less saturated so the bullet is visible
/// against any background. Distinct from the player's warm-white
/// bullet so the player can tell who's firing at them at a glance.
const SHOOTER_PROJECTILE_COLOR: Color = Color::srgb(0.45, 0.90, 0.90); // ~#73e6e6

/// Phase 1 sniper color. Yellow so it reads as "caution /
/// warning" in arcade-game color shorthand (think hazard tape,
/// construction signs). The AI lerps this toward a hot red
/// as the charge fills — the visual telegraph that a big
/// shot is incoming. Distinct from the red chaser, purple
/// dasher, and teal shooter so the four archetypes are
/// immediately identifiable by color.
const SNIPER_COLOR: Color = Color::srgb(0.95, 0.85, 0.25); // ~#f2d940

/// Sniper charged color (the end of the telegraph ramp).
/// Hot red so the player gets a clear "shot is coming NOW"
/// signal — the last 10% of the charge is a solid red
/// sprite, no yellow visible.
const SNIPER_CHARGED_COLOR: Color = Color::srgb(0.95, 0.20, 0.15); // ~#f22626

/// Sniper projectile color. Bright orange so the bullet is
/// distinct from the shooter's teal and the player's warm
/// white. The orange also reinforces the "this is a dangerous
/// shot" signal — same color family as the sniper's charge
/// ramp.
const SNIPER_PROJECTILE_COLOR: Color = Color::srgb(1.0, 0.55, 0.10); // ~#ff8c1a

/// Chaser sprite size. Slightly smaller than the player (40x40) so the
/// chaser feels more compact and the player has a small reach advantage
/// when kiting.
const CHASER_SIZE: f32 = 30.0;

/// Dasher sprite size. Same as the chaser — the visual difference
/// is color and behavior, not silhouette. Equal size also keeps
/// the AABB collision math symmetric (the contact-damage system
/// uses a `CHASER_SIZE_HALF` constant for chasers; we'd want a
/// `DASHER_SIZE_HALF` parallel or a single `ENEMY_HALF` if they
/// stay the same).
const DASHER_SIZE: f32 = 30.0;

/// Shooter sprite size. Same as chaser/dasher so all three
/// archetypes have a uniform silhouette. The visual difference
/// between them is color + behavior (chaser rushes, dasher
/// lunges, shooter plinks from range).
const SHOOTER_SIZE: f32 = 30.0;

/// Shooter projectile sprite size. Smaller than the player
/// projectile (6x12) so the shooter's bullets read as smaller
/// and faster-fading, reinforcing "these are weaker than yours".
const SHOOTER_PROJECTILE_SIZE: Vec2 = Vec2::new(5.0, 8.0);

/// Sniper sprite size. Same as the other three archetypes for
/// uniform silhouette — the difference is color + behavior
/// (sniper charges a big slow bullet).
const SNIPER_SIZE: f32 = 30.0;

// ─── Mini-boss constants (Phase 1) ─────────────────────────────────────

/// Boss color. Deep red, slightly darker than the chaser's
/// bright red, so the two don't blend. The boss reads as
/// "scarier version of the chaser" — the player learns
/// "bright red = chaser, deep red = boss" in one beat.
/// Spec: 0.70, 0.05, 0.05.
const BOSS_COLOR: Color = Color::srgb(0.70, 0.05, 0.05);

/// Boss sprite size. 50x50 — bigger than the player (40x40)
/// and the other enemies (30x30) so the boss is the visually
/// dominant enemy on screen. The bigger silhouette also
/// reads as "this is a different class of enemy" before the
/// player has read the HP bar.
const BOSS_SIZE: f32 = 50.0;

/// Boss projectile color. Same red family as the boss body
/// but lighter / more saturated so the bullet pops against
/// the world. Distinct from the shooter's teal and the
/// sniper's orange so the player can tell at a glance whose
/// round is incoming.
const BOSS_PROJECTILE_COLOR: Color = Color::srgb(0.95, 0.20, 0.15);

/// Boss projectile size. 12x12 — chunkier than the
/// shooter's 5x8 and the sniper's 8x14. The bigger AABB
/// means a player can't thread the needle between adjacent
/// bullets in a 3-burst — the spread is wide enough to
/// cover a 90° arc, the bullets are fat enough that any
/// attempt to step between them catches at least one.
const BOSS_PROJECTILE_SIZE: Vec2 = Vec2::new(12.0, 12.0);

// ─── Cascade beacon constants (Phase 2 P5) ──────────────────────────────

/// Beacon color. Bright gold/yellow — distinct from every
/// other enemy on screen (chaser red, dasher orange, shooter
/// teal, sniper yellow, boss deep red). The beacon needs to
/// be visually obvious the moment the Cascade window opens
/// so the player can choose to push for the +5s extension
/// vs. focus on the slowed-down regular enemies. Gold also
/// reads as "objective" / "valuable" in arcade color
/// shorthand (coins, treasure, power-ups).
const BEACON_COLOR: Color = Color::srgb(1.0, 0.85, 0.20);

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        // Order: spawn (creates entities) → AI (writes their BaseVelocity)
        // → contact damage (resolves hits). Movement + visual feedback
        // happen via shared systems in the player and combat plugins.
        // Death-puff tick runs after contact so the puff's despawn
        // doesn't fight with a fresh spawn in the same frame.
        // Hit-flash tick runs last so the color reset on flash-end
        // doesn't fight with a fresh flash from a same-frame hit.
        app.add_systems(
            Update,
            (
                wave_spawner_tick_system,
                enemy_ai_system,
                enemy_contact_damage_system,
                death_puff_tick_system,
                hit_flash_tick_system,
            )
                .chain()
                // Phase 2: gates the whole enemy chain on
                // `in_state(AppState::InRun)`. Spawners don't tick,
                // AI doesn't move, contact damage doesn't resolve,
                // puffs don't fade, and hit flashes don't tick
                // during MainMenu or GameOver. This replaces the
                // Phase 1 `if hitstop.is_some() || run.game_over`
                // check at the top of every enemy system.
                .run_if(in_state(AppState::InRun)),
        );
    }
}
// ─── Wave Spawner (Phase 2.3) ─────────────────────────────────────────────

// Replaces the Phase 2.0 `spawner_tick_system` (4 parallel
// `Spawner` entities, one per archetype, all firing every
// 3s in parallel). Now there's one `WaveSpawner` per Fight
// room with a list of `WaveDefinition`s; this system ticks
// the spawner in the CURRENT room, spawning enemies of the
// current wave's archetype at the wave's `interval_s`.
//
// State machine (per frame):
// 1. If `current_wave >= waves.len()` → all waves done, skip.
// 2. Tick `wave_timer`.
// 3. If `wave_timer` fired AND `spawned_this_wave < waves[current_wave].count`:
//    - Spawn one enemy of `waves[current_wave].archetype`
//      at a random position in the current room
//    - Reset `wave_timer` to the current wave's `interval_s`
//    - Increment `spawned_this_wave`
// 4. If `spawned_this_wave == waves[current_wave].count` (wave done):
//    - Advance `current_wave`
//    - Reset `spawned_this_wave = 0`
//    - Re-arm `wave_timer` with the NEXT wave's `interval_s`
// 5. If we just advanced PAST the last wave (i.e. was
//    `waves.len() - 1`, now `waves.len()`): emit
//    `RoomClearedEvent { room_id }` so `door_unlock_system`
//    can unlock the room's forward door.
//
// Frozen during global hitstop so kills don't immediately
// trigger the next spawn. Frozen outside the current room
// (skipped by the `room_id != current_room` filter) so
// re-entering a room doesn't dump a backlog.
//
// On room transition, `door_transition_system` resets all
// `WaveSpawner.wave_timer` to the current wave's
// `interval_s` (so the new room gets a fresh warmup).
fn wave_spawner_tick_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    biome: Res<FloorState>,
    mut rng: ResMut<RunRng>,
    mut commands: Commands,
    mut spawners: Query<&mut WaveSpawner>,
    player_query: Query<&Transform, With<Player>>,
    mut room_cleared_events: EventWriter<RoomClearedEvent>,
    boss_ramp: Option<Res<BossRampTimer>>,
) {
    if hitstop.is_some() {
        return;
    }
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for mut spawner in &mut spawners {
        if spawner.room_id != biome.current_room {
            continue;
        }

        // ─── Horde-mode: skip the initial wave phase ──────
        // Jump straight to the continuous farming phase so
        // every room feels like a frantic horde from frame 1.
        // For non-Boss rooms, emit RoomClearedEvent immediately
        // so teleport pads appear and the player can leave
        // whenever. Boss rooms gate room-clear on boss death.
        if spawner.current_wave < spawner.waves.len() {
            spawner.current_wave = spawner.waves.len();

            // Don't clear Boss rooms from the wave system —
            // boss_death_room_clear_system handles that.
            let is_boss = biome
                .rooms
                .iter()
                .any(|r| r.id == biome.current_room && r.kind == RoomKind::Boss);
            if !is_boss {
                room_cleared_events.write(RoomClearedEvent {
                    room_id: spawner.room_id,
                });
            }
        }

        // ─── Farming phase ─────────────────────────────────
        spawner.wave_timer.tick(time.delta());
        if !spawner.wave_timer.finished() {
            continue;
        }

        // Compute the effective spawn interval:
        //   base = spawner.farming_interval (per room type)
        //   × ramp_factor from BossRampTimer (1.0 = normal,
        //     drops toward 0.28 over the ramp duration)
        let ramp_factor = boss_ramp
            .as_ref()
            .map(|ramp| {
                let progress = (ramp.timer.elapsed_secs() / ramp.timer.duration().as_secs_f32())
                    .clamp(0.0, 1.0);
                1.0 - (1.0 - 0.28) * progress
            })
            .unwrap_or(1.0);
        let interval = (spawner.farming_interval * ramp_factor).max(0.1);

        // Pick the next archetype based on room flavor.
        let room_kind = biome
            .rooms
            .iter()
            .find(|r| r.id == biome.current_room)
            .map(|r| r.kind)
            .unwrap_or(RoomKind::Fight);
        let archetype = farming_archetype(room_kind, spawner.spawned_this_wave);

        spawn_wave_enemy(
            &mut commands,
            &settings,
            &biome,
            &mut rng,
            player_pos,
            archetype,
        );
        spawner.spawned_this_wave += 1;
        spawner.wave_timer = Timer::from_seconds(interval, TimerMode::Once);
    }
}

/// Pick the enemy archetype for the next farming-spawn based on
/// the room's kind and the cumulative spawn counter. Each room
/// type has a different composition bias:
///   Fight:    balanced mix of all 4 archetypes
///   Horde:    melee-heavy (chasers + dashers), few ranged
///   MiniBoss: extra ranged pressure (more shooters + snipers)
///   Boss:     same as MiniBoss (ranged pressure + boss entity)
fn farming_archetype(kind: RoomKind, counter: u32) -> EnemyArchetype {
    match kind {
        RoomKind::Horde => match counter % 6 {
            0 | 3 => EnemyArchetype::Chaser,
            1 | 4 => EnemyArchetype::Dasher,
            2 => EnemyArchetype::Shooter,
            _ => EnemyArchetype::Chaser,
        },
        RoomKind::MiniBoss | RoomKind::Boss => match counter % 5 {
            0 => EnemyArchetype::Chaser,
            1 => EnemyArchetype::Dasher,
            2 => EnemyArchetype::Shooter,
            3 => EnemyArchetype::Sniper,
            _ => EnemyArchetype::Shooter,
        },
        _ => {
            // Fight rooms: balanced cycle
            const ALL: [EnemyArchetype; 4] = [
                EnemyArchetype::Chaser,
                EnemyArchetype::Dasher,
                EnemyArchetype::Shooter,
                EnemyArchetype::Sniper,
            ];
            ALL[counter as usize % 4]
        }
    }
}

// Spawn a single enemy of `archetype` at a random
// position in the current room. Reads randomness from
// the per-run `RunRng` so the same seed gives the same
// enemy positions (Phase 2.3 determinism).
fn spawn_wave_enemy(
    commands: &mut Commands,
    settings: &GameSettings,
    biome: &FloorState,
    rng: &mut RunRng,
    player_pos: Vec2,
    archetype: EnemyArchetype,
) {
    let room = biome
        .rooms
        .iter()
        .find(|r| r.id == biome.current_room)
        .expect("current room must exist in biome");
    let spawn_pos = random_spawn_position(settings, player_pos, room, rng);
    match archetype {
        EnemyArchetype::Chaser => spawn_chaser(commands, settings, room, spawn_pos, biome),
        EnemyArchetype::Dasher => spawn_dasher(commands, settings, room, spawn_pos, biome),
        EnemyArchetype::Shooter => spawn_shooter(commands, settings, room, spawn_pos, biome),
        EnemyArchetype::Sniper => spawn_sniper(commands, settings, room, spawn_pos, biome),
        // Phase 2.3: Boss is still on the timer-driven
        // path (boss_spawn_system at run midpoint), not
        // a wave. Future patch will fold it into the
        // wave system.
        // Phase 2 P5: Beacon is spawned by the cascade-
        // start handler in `player.rs`, not via the wave
        // system (cascade windows don't align with wave
        // starts). No-op here, like Boss.
        EnemyArchetype::Boss | EnemyArchetype::Beacon => {}
    }
}

// Dispatch on the spawner's archetype. New archetypes slot in here.
//
// Phase 2: takes `biome` so the per-archetype spawners can
// clamp the spawn position to the current room's bounds
// (see `random_spawn_position`). Without the clamp, an enemy
// could spawn in a wall or in the next room over.
fn spawn_enemy(
    commands: &mut Commands,
    settings: &GameSettings,
    biome: &FloorState,
    player_pos: Vec2,
    archetype: EnemyArchetype,
) {
    let room = biome
        .rooms
        .iter()
        .find(|r| r.id == biome.current_room)
        .expect("current room must exist in biome");
    match archetype {
        EnemyArchetype::Chaser => spawn_chaser(commands, settings, room, player_pos, biome),
        EnemyArchetype::Dasher => spawn_dasher(commands, settings, room, player_pos, biome),
        EnemyArchetype::Shooter => spawn_shooter(commands, settings, room, player_pos, biome),
        EnemyArchetype::Sniper => spawn_sniper(commands, settings, room, player_pos, biome),
        // Note: `Boss` is not spawned via the random spawner —
        // it spawns at the run-midpoint (60s) at a fixed
        // distance from the player. See `boss_spawn_system`
        // in the player plugin. Same for `Beacon` — see
        // `cascade_start_handler` in the player plugin.
        EnemyArchetype::Boss | EnemyArchetype::Beacon => {}
    }
}

fn floor_difficulty_mult(floor: u32) -> (f32, f32) {
    let hp = 1.0 + floor as f32 * 0.08;
    let dmg = 1.0 + floor as f32 * 0.05;
    (hp, dmg)
}

fn spawn_chaser(commands: &mut Commands, settings: &GameSettings, room: &Room, spawn_pos: Vec2, floor: &FloorState) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);

    commands.spawn((
        Sprite {
            color: CHASER_COLOR,
            custom_size: Some(Vec2::splat(CHASER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Chaser,
        },
        Collider {
            half_extents: Vec2::splat(CHASER_SIZE * 0.5),
        },
        // Day 11-12: enemies carry Health so the projectile-damage
        // system can drop them. `max` is a constant per-archetype
        // (read from settings on spawn) and `current` starts at max.
        Health::new(settings.enemy_max_hp * hp_mult),
        Velocity::default(),
        // Chasers carry BaseVelocity so the future Cascade scaler can
        // slow them to 0.25× during Cascade Mode (per PLAN.md §2).
        BaseVelocity::default(),
        // Phase 1 cascade: chasers are CascadeAffected so the
        // velocity_apply_system scales their Velocity by 0.25×
        // during a Cascade window. The marker has no other effect.
        CascadeAffected,
        // Per-entity base color so the hit-flash system can reset
        // the sprite tint back to chaser red (not the wrong
        // archetype's color) when a flash ends without killing.
        BaseColor(CHASER_COLOR),
    ));
}

// Phase 1 dasher. Same shape as the chaser, plus a `DasherState`
// component that the AI ticks each frame to advance the
// approach → dash → recover cycle. The dasher starts in Approach.
fn spawn_dasher(commands: &mut Commands, settings: &GameSettings, room: &Room, spawn_pos: Vec2, floor: &FloorState) {
    // Spawn position is the same as a chaser — random angle at a
    // random distance in [min, max]. We could spawn dashers
    // further out (so the player has time to read the charge
    // before they arrive) but that conflicts with the unified
    // spawner distance; Phase 2 will let each archetype override
    // its spawn range.
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);

    commands.spawn((
        Sprite {
            color: DASHER_COLOR,
            custom_size: Some(Vec2::splat(DASHER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Dasher,
        },
        Collider {
            half_extents: Vec2::splat(DASHER_SIZE * 0.5),
        },
        Health::new(settings.dasher_max_hp * hp_mult),
        Velocity::default(),
        BaseVelocity::default(),
        // Phase 1 cascade: dashers are CascadeAffected for the same
        // reason as chasers — their Velocity is scaled by 0.25×
        // during Cascade windows. The dash itself still completes
        // in the same wall-clock time (it ticks against `Time`,
        // not against the scaled Velocity), so the player's
        // "approach → dash → recover" rhythm stays readable.
        CascadeAffected,
        DasherState {
            phase: DasherPhase::Approach,
            phase_timer: Timer::from_seconds(settings.dasher_approach_duration_s, TimerMode::Once),
        },
        // Per-entity base color — the dasher's purple tint, not the
        // chaser's red. Without this, the hit-flash system would
        // hard-reset every enemy to chaser red on every non-killing
        // hit (see `hit_flash_tick_system`).
        BaseColor(DASHER_COLOR),
    ));
}

// Phase 1 shooter. Same base bundle as the dasher (sprite +
// BaseColor + CascadeAffected + Collider + Health + Velocity +
// BaseVelocity) plus a `ShooterState` carrying the fire cooldown.
// The fire timer starts at 0.5× interval (random spread) so a
// wave of shooters doesn't all open fire on the same frame —
// Phase 2 will use seeded RNG to make this deterministic per
// run.
fn spawn_shooter(commands: &mut Commands, settings: &GameSettings, room: &Room, spawn_pos: Vec2, floor: &FloorState) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);

    // Spread the first shot over the fire interval. Without
    // this, a fresh wave of 3 shooters all open fire on the
    // same frame and the player eats a triple-tap before they
    // can react. Half-interval spread is enough to break the
    // sync without feeling random.
    let interval = 1.0 / settings.shooter_fire_rate;
    let initial_cooldown = interval * 0.5;

    commands.spawn((
        Sprite {
            color: SHOOTER_COLOR,
            custom_size: Some(Vec2::splat(SHOOTER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Shooter,
        },
        Collider {
            half_extents: Vec2::splat(SHOOTER_SIZE * 0.5),
        },
        Health::new(settings.shooter_max_hp * hp_mult),
        Velocity::default(),
        BaseVelocity::default(),
        // Phase 1 cascade: shooters are CascadeAffected so the
        // velocity_apply_system scales their approach speed by
        // 0.25× during a Cascade window. The shooter's own
        // projectiles also carry the marker (see
        // `spawn_shooter_projectile`) so the bullets slow too —
        // the player can sidestep them in slow-mo.
        CascadeAffected,
        BaseColor(SHOOTER_COLOR),
        ShooterState {
            fire_timer: Timer::from_seconds(initial_cooldown, TimerMode::Once),
        },
    ));
}

// Phase 1 sniper. Same base bundle as the dasher (sprite +
// BaseColor + CascadeAffected + Collider + Health + Velocity +
// BaseVelocity) plus a `SniperState` carrying the charge +
// post-fire timer. Starts in the "post-fire cooldown" state
// (not charging) so a wave of snipers doesn't all open fire
// on the same frame — the spawn-time spread is the cooldown
// duration itself, so the first shot from a fresh sniper
// lands ~1.0s after spawn.
fn spawn_sniper(commands: &mut Commands, settings: &GameSettings, room: &Room, spawn_pos: Vec2, floor: &FloorState) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);

    commands.spawn((
        Sprite {
            color: SNIPER_COLOR,
            custom_size: Some(Vec2::splat(SNIPER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Sniper,
        },
        Collider {
            half_extents: Vec2::splat(SNIPER_SIZE * 0.5),
        },
        Health::new(settings.sniper_max_hp * hp_mult),
        Velocity::default(),
        BaseVelocity::default(),
        // Phase 1 cascade: snipers are CascadeAffected so the
        // velocity_apply_system scales their approach speed
        // by 0.25× during a Cascade window. The sniper's
        // own projectiles also carry the marker so the
        // bullets slow too.
        CascadeAffected,
        BaseColor(SNIPER_COLOR),
        SniperState {
            // Start in cooldown (not charging) so the wave
            // doesn't all open fire on the same frame.
            charge_timer: Timer::from_seconds(
                settings.sniper_post_fire_cooldown_s,
                TimerMode::Once,
            ),
            is_charging: false,
        },
    ));
}

// Spawn a `Team::Enemy` sniper projectile at `enemy_pos` aimed
// at the player. Mirrors `spawn_shooter_projectile` but with
// sniper-tuned values: bigger sprite, slower speed, higher
// damage. The bigger sprite matters for AABB collision — the
// player has to actually dodge this one, not just sidestep.
fn spawn_sniper_projectile(
    commands: &mut Commands,
    settings: &GameSettings,
    enemy_pos: Vec2,
    dir: Vec2,
) {
    let muzzle = SNIPER_SIZE * 0.5 + settings.sniper_projectile_size_y * 0.5 + 2.0;
    let spawn_pos = enemy_pos + dir * muzzle;
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

    let speed = settings.sniper_projectile_speed;

    commands.spawn((
        Sprite {
            color: SNIPER_PROJECTILE_COLOR,
            custom_size: Some(Vec2::new(
                settings.sniper_projectile_size_x,
                settings.sniper_projectile_size_y,
            )),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
            .with_rotation(Quat::from_rotation_z(angle)),
        BaseVelocity(dir * speed),
        Velocity(dir * speed),
        CascadeAffected,
        Collider {
            half_extents: Vec2::new(
                settings.sniper_projectile_size_x * 0.5,
                settings.sniper_projectile_size_y * 0.5,
            ),
        },
        Projectile {
            damage: settings.sniper_projectile_damage,
            lifetime: Timer::from_seconds(settings.sniper_projectile_lifetime_s, TimerMode::Once),
            team: Team::Enemy,
        },
    ));
}
fn spawn_shooter_projectile(
    commands: &mut Commands,
    settings: &GameSettings,
    enemy_pos: Vec2,
    dir: Vec2,
) {
    // Muzzle offset = half the shooter's width + half the
    // projectile's length, so the bullet visually starts in
    // front of the shooter's body, not overlapping it.
    let muzzle = SHOOTER_SIZE * 0.5 + SHOOTER_PROJECTILE_SIZE.y * 0.5 + 2.0;
    let spawn_pos = enemy_pos + dir * muzzle;
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

    let speed = settings.shooter_projectile_speed;
    commands.spawn((
        Sprite {
            color: SHOOTER_PROJECTILE_COLOR,
            custom_size: Some(SHOOTER_PROJECTILE_SIZE),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
            .with_rotation(Quat::from_rotation_z(angle)),
        // The cascade-scaled velocity lives in BaseVelocity;
        // Velocity is the "current" value the cascade scaler
        // overwrites each frame. The cascade scaler reads
        // CascadeAffected and applies the 0.25× scale only when
        // a Cascade window is open.
        BaseVelocity(dir * speed),
        Velocity(dir * speed),
        CascadeAffected,
        Collider {
            half_extents: Vec2::new(
                SHOOTER_PROJECTILE_SIZE.x * 0.5,
                SHOOTER_PROJECTILE_SIZE.y * 0.5,
            ),
        },
        Projectile {
            damage: settings.shooter_projectile_damage,
            lifetime: Timer::from_seconds(settings.shooter_projectile_lifetime_s, TimerMode::Once),
            team: Team::Enemy,
        },
    ));
}

// Phase 1 mini-boss: spawn a `Team::Enemy` boss projectile
// aimed in `dir`. Mirrors `spawn_shooter_projectile` /
// `spawn_sniper_projectile` but with boss-tuned values: fatter
// sprite (12x12), slower speed (200 vs shooter's 300), and
// CascadeAffected so Cascade Mode slows the projectiles too.
// The bigger sprite means a player can't thread the needle
// between adjacent bullets in a 3-burst.
fn spawn_boss_projectile(
    commands: &mut Commands,
    settings: &GameSettings,
    enemy_pos: Vec2,
    dir: Vec2,
) {
    let muzzle = BOSS_SIZE * 0.5 + settings.boss_projectile_size_y * 0.5 + 2.0;
    let spawn_pos = enemy_pos + dir * muzzle;
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;
    let speed = settings.boss_projectile_speed;

    commands.spawn((
        Sprite {
            color: BOSS_PROJECTILE_COLOR,
            custom_size: Some(Vec2::new(
                settings.boss_projectile_size_x,
                settings.boss_projectile_size_y,
            )),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
            .with_rotation(Quat::from_rotation_z(angle)),
        BaseVelocity(dir * speed),
        Velocity(dir * speed),
        CascadeAffected,
        Collider {
            half_extents: Vec2::new(
                settings.boss_projectile_size_x * 0.5,
                settings.boss_projectile_size_y * 0.5,
            ),
        },
        Projectile {
            damage: settings.boss_projectile_damage,
            lifetime: Timer::from_seconds(settings.boss_projectile_lifetime_s, TimerMode::Once),
            team: Team::Enemy,
        },
    ));
}

// Phase 1 mini-boss: spawn the boss at `position` with full
// HP. The single instance lives until killed (cascade beacon
// flips on death) or the player dies (restart_system despawns
// it). `boss_spawn_system` in the player plugin handles the
// "spawn once at the run-midpoint" gate via
// `RunState.boss_spawned`.
//
// The boss carries: Boss marker (so the HUD's boss HP bar
// query can find it), BossState (mirrored HP + phase +
// attack timer for the AI state machine), BossPhase
// (Phase1/Phase2 enum), EnemyAI + Enemy markers, full Health,
// standard physics + collision components, CascadeAffected
// (so Cascade Mode slows it AND its projectiles), and
// BaseColor (so the hit-flash system can reset the sprite
// tint back to deep red, not the chaser's bright red).
pub fn spawn_boss(commands: &mut Commands, settings: &GameSettings, floor: &FloorState, position: Vec2) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    commands.spawn((
        Sprite {
            color: BOSS_COLOR,
            custom_size: Some(Vec2::splat(BOSS_SIZE)),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 0.0),
        Boss,
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Boss,
        },
        Collider {
            half_extents: Vec2::splat(BOSS_SIZE * 0.5),
        },
        Health::new(settings.boss_max_hp * hp_mult),
        Velocity::default(),
        BaseVelocity::default(),
        // Phase 1 cascade: boss is CascadeAffected so
        // Cascade Mode slows the chase AND the 3-burst
        // projectiles (since they also carry the marker).
        // This gives the player a free-hits window during
        // a Cascade window, which is the intended
        // difficulty curve — Cascade is your "I can deal
        // with the boss" moment.
        CascadeAffected,
        // Per-entity base color so the hit-flash system
        // resets the sprite tint back to deep red (not the
        // chaser's bright red) when a flash ends without
        // killing.
        BaseColor(BOSS_COLOR),
        BossState {
            phase: BossPhase::Phase1,
            attack_timer: Timer::from_seconds(
                settings.boss_phase2_attack_interval_s,
                TimerMode::Once,
            ),
            current_hp: settings.boss_max_hp * hp_mult,
        },
    ));
}

// Phase 2 P5: spawn a Cascade beacon at `position`. Called
// by the cascade-start handler in `player.rs` the frame a
// Cascade window opens. The beacon is a static, hittable
// target — no AI movement, no contact damage, no
// CascadeAffected (velocity is always zero so scaling is
// irrelevant). Killing it during Cascade extends the
// window by `beacon_chain_extension_s` (5s, capped at
// +15s total) — see `combat.rs::projectile_damage_system`
// for the kill-side hook.
//
// The beacon's `Beacon` marker component is the
// discriminator the kill handler reads via `Has<Beacon>`
// to fire the chain-extend. The marker is intentionally
// separate from `AiBehavior::Beacon` so future "spawn a
// moving beacon" patches don't accidentally fire the
// chain hook on the wrong archetype.
pub fn spawn_beacon(commands: &mut Commands, settings: &GameSettings, position: Vec2) {
    commands.spawn((
        Sprite {
            color: BEACON_COLOR,
            custom_size: Some(Vec2::splat(settings.beacon_size)),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 0.0),
        Beacon,
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Beacon,
        },
        Collider {
            half_extents: Vec2::splat(settings.beacon_size * 0.5),
        },
        Health::new(settings.beacon_hp),
        // Phase 2 P5.1: the beacon has BaseVelocity + Velocity
        // and flees from the player via the AI system below.
        // It does NOT get CascadeAffected, so during Cascade
        // it moves at full speed while every other enemy is
        // slowed to 0.25× — the beacon visibly "runs fast"
        // in the slowed world.
        Velocity::default(),
        BaseVelocity::default(),
        BaseColor(BEACON_COLOR),
        // Motion trail: Sandevistan-style afterimages while
        // Cascade is active. The trail system checks for both
        // the marker AND cascade.active before spawning ghosts.
        MotionTrail,
    ));
}

// Shared spawn-position math: random angle, random distance
// in [min, max]. Reads randomness from the per-run `RunRng`
// resource (Phase 2.3 determinism: same seed → same enemy
// positions, so recorded runs can be replayed).
//
// Returns just the position (not `(Vec2, u64)` as in
// Phase 2) because the RNG state lives in the `RunRng`
// resource, not in the return value. Simpler API for the
// same behavior.
//
// Phase 2.1 also added the room-bounds clamp: enemies
// can't spawn in a wall or in an adjacent room. The
// per-axis clamp is the cheapest correct behavior.
fn random_spawn_position(
    settings: &GameSettings,
    player_pos: Vec2,
    room: &Room,
    rng: &mut RunRng,
) -> Vec2 {
    let angle = rng.next_unit() * std::f32::consts::TAU;
    let distance = settings.spawner_min_distance
        + rng.next_unit() * (settings.spawner_max_distance - settings.spawner_min_distance);
    let offset = Vec2::new(angle.cos(), angle.sin()) * distance;
    let raw = player_pos + offset;

    // Margin: half the largest enemy (40px chaser) + a small
    // buffer. Keeps the enemy AABB entirely inside the room
    // after spawn (so the wall_collision_system doesn't push
    // them on the first frame).
    const SPAWN_MARGIN: f32 = 25.0;
    Vec2::new(
        raw.x.clamp(
            room.bounds_min.x + SPAWN_MARGIN,
            room.bounds_max.x - SPAWN_MARGIN,
        ),
        raw.y.clamp(
            room.bounds_min.y + SPAWN_MARGIN,
            room.bounds_max.y - SPAWN_MARGIN,
        ),
    )
}

// ─── AI ──────────────────────────────────────────────────────────────────

// For each enemy, dispatch on its AI behavior. The chaser path is
// the simple "move toward player" from Phase 0. The dasher path
// runs a three-phase state machine (Approach → Dash → Recover)
// and writes its current speed into BaseVelocity; the shared
// `velocity_apply_system` then copies it into `Velocity`, and
// `movement_system` integrates it into the transform.
//
// Frozen during global hitstop so enemies halt during the freeze.
// Frozen during game over so the dead player doesn't get bumped
// by a straggler.
//
// `Option<&mut DasherState>` lets the same query match both
// archetypes — chasers get None, dashers get Some.
fn enemy_ai_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    // `Option<&mut DasherState>` lets the same query match both
    // archetypes — chasers get None, dashers get Some. The new
    // `Option<&mut SniperState>` adds the 4th archetype the
    // same way: chasers/dashers/shooters get None, snipers get
    // Some. `&mut Sprite` lets the sniper's AI lerp the sprite
    // color from yellow to red during the charge (the visual
    // telegraph) — non-sniper entities never have their color
    // touched in this system.
    mut enemies: Query<
        (
            &Transform,
            &EnemyAI,
            &mut BaseVelocity,
            &mut Sprite,
            &Health,
            Option<&mut DasherState>,
            Option<&mut ShooterState>,
            Option<&mut SniperState>,
            Option<&mut BossState>,
        ),
        With<Enemy>,
    >,
) {
    if hitstop.is_some() {
        return;
    }
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for (
        enemy_transform,
        ai,
        mut base,
        mut sprite,
        health,
        dasher_state,
        shooter_state,
        sniper_state,
        boss_state,
    ) in &mut enemies
    {
        match ai.behavior {
            AiBehavior::Chaser => {
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                base.0 = dir * settings.chaser_move_speed;
            }
            AiBehavior::Dasher => {
                let Some(mut state) = dasher_state else {
                    // Defensive: a dasher without DasherState shouldn't
                    // exist (spawn_dasher always attaches one) but
                    // skipping is safer than panicking.
                    continue;
                };
                // Tick the phase timer. If it just elapsed, advance
                // to the next phase and reset the timer to the new
                // phase's duration. The to_player direction is
                // re-computed each phase (so the dash re-aims if
                // the player moved during Approach).
                let to_player = player_pos - enemy_transform.translation.truncate();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };
                let (speed, next_phase, next_duration) = match state.phase {
                    DasherPhase::Approach => (
                        settings.dasher_approach_speed,
                        DasherPhase::Dash,
                        settings.dasher_dash_duration_s,
                    ),
                    DasherPhase::Dash => (
                        settings.dasher_dash_speed,
                        DasherPhase::Recover,
                        settings.dasher_recover_duration_s,
                    ),
                    DasherPhase::Recover => (
                        0.0,
                        DasherPhase::Approach,
                        settings.dasher_approach_duration_s,
                    ),
                };
                base.0 = dir * speed;
                state.phase_timer.tick(time.delta());
                if state.phase_timer.finished() {
                    state.phase = next_phase;
                    state.phase_timer = Timer::from_seconds(next_duration, TimerMode::Once);
                }
            }
            AiBehavior::Shooter => {
                let Some(mut state) = shooter_state else {
                    // Defensive: a shooter without ShooterState
                    // shouldn't exist (spawn_shooter always
                    // attaches one) but skipping is safer than
                    // panicking.
                    continue;
                };
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dist = to_player.length();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };

                // Approach until at preferred range, then stop.
                // The "stop" is just `speed = 0.0` — the enemy
                // doesn't need a separate state for "in range",
                // the distance check is enough. This keeps the
                // Shooter AI stateless (only the fire timer is
                // tracked, and that's per-entity on
                // `ShooterState`).
                let speed = if dist > settings.shooter_preferred_range {
                    settings.shooter_move_speed
                } else {
                    0.0
                };
                base.0 = dir * speed;

                // Tick the fire timer always. Only spawn a
                // projectile when (a) the timer elapsed AND
                // (b) we're in range. We deliberately do NOT
                // reset the timer if we're out of range —
                // `fire_timer.finished()` stays true, so the
                // moment we enter range we fire on the next
                // frame. The cap is implicit: a shooter that's
                // out of range for 5 seconds doesn't burst-fire
                // when it finally gets in range; it fires once
                // and resumes the normal cadence.
                state.fire_timer.tick(time.delta());
                if dist <= settings.shooter_preferred_range && state.fire_timer.finished() {
                    spawn_shooter_projectile(&mut commands, &settings, enemy_pos, dir);
                    let interval = 1.0 / settings.shooter_fire_rate;
                    state.fire_timer = Timer::from_seconds(interval, TimerMode::Once);
                }
            }
            AiBehavior::Sniper => {
                let Some(mut state) = sniper_state else {
                    // Defensive: a sniper without SniperState
                    // shouldn't exist (spawn_sniper always
                    // attaches one) but skipping is safer than
                    // panicking.
                    continue;
                };
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dist = to_player.length();
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };

                // Sniper movement: approach until at preferred
                // range, then plant and charge. While in
                // cooldown (between shots), the sniper still
                // moves toward the player so it can re-plant
                // for the next charge — but slower than its
                // approach speed, so a determined player can
                // keep pushing during the cooldown window.
                let speed = if dist > settings.sniper_preferred_range {
                    settings.sniper_move_speed
                } else {
                    0.0
                };
                base.0 = dir * speed;

                // Charge / fire / cooldown state machine.
                //
                //   !is_charging, timer not finished → in
                //     cooldown after a recent shot, keep
                //     ticking the timer (and approaching the
                //     player if needed)
                //   !is_charging, timer finished → cooldown
                //     done, transition to charging
                //   is_charging, timer not finished → mid-
                //     charge, tick the timer, update the
                //     sprite color telegraph (lerp from
                //     SNIPER_COLOR to SNIPER_CHARGED_COLOR
                //     over the charge duration)
                //   is_charging, timer finished → fire the
                //     bullet, transition to cooldown
                state.charge_timer.tick(time.delta());

                if !state.is_charging {
                    // Cooldown phase
                    if state.charge_timer.finished() {
                        // Transition: cooldown done, start
                        // charging. Reset the timer to the
                        // full charge duration.
                        state.is_charging = true;
                        state.charge_timer =
                            Timer::from_seconds(settings.sniper_charge_duration_s, TimerMode::Once);
                    }
                    // While in cooldown, the sprite stays
                    // at SNIPER_COLOR (no telegraph).
                } else {
                    // Charging phase. Lerp the sprite color
                    // from base yellow to charged red over
                    // the charge duration — the visual
                    // telegraph that a shot is incoming.
                    let dur = state.charge_timer.duration().as_secs_f32();
                    if dur > 0.0 {
                        let progress =
                            (1.0 - state.charge_timer.remaining_secs() / dur).clamp(0.0, 1.0);
                        let base_srgba = SNIPER_COLOR.to_srgba();
                        let charged_srgba = SNIPER_CHARGED_COLOR.to_srgba();
                        sprite.color = Color::srgba(
                            base_srgba.red + (charged_srgba.red - base_srgba.red) * progress,
                            base_srgba.green + (charged_srgba.green - base_srgba.green) * progress,
                            base_srgba.blue + (charged_srgba.blue - base_srgba.blue) * progress,
                            base_srgba.alpha,
                        );
                    }
                    // When cooldown ends and we transition
                    // back to !is_charging, the sprite stays
                    // at the post-fire value (charged red)
                    // for one frame, then snaps back to
                    // base yellow on the next frame's "while
                    // in cooldown, sprite stays at base"
                    // branch. (We don't explicitly reset it
                    // here — the next frame's branch handles
                    // it because the visual only matters
                    // when is_charging is true.)
                    // Actually we DO need to reset it: the
                    // cooldown should show the sniper at
                    // base color, not stuck at red. Add the
                    // reset.
                    if state.charge_timer.finished() {
                        // Fire! Spawn a sniper projectile
                        // aimed at the player and transition
                        // to cooldown. We aim at the
                        // CURRENT player position (not the
                        // position when the charge started)
                        // — the sniper re-aims each frame
                        // during the charge (handled in the
                        // AI tick above, not here, since
                        // `dir` is recomputed each frame).
                        spawn_sniper_projectile(&mut commands, &settings, enemy_pos, dir);
                        state.is_charging = false;
                        state.charge_timer = Timer::from_seconds(
                            settings.sniper_post_fire_cooldown_s,
                            TimerMode::Once,
                        );
                        // Reset sprite to base yellow so
                        // the cooldown phase doesn't show
                        // a red sniper between shots.
                        sprite.color = SNIPER_COLOR;
                    }
                }
            }
            AiBehavior::Boss => {
                let Some(mut state) = boss_state else {
                    continue;
                };
                let enemy_pos = enemy_transform.translation.truncate();
                let to_player = player_pos - enemy_pos;
                let dir = if to_player.length_squared() > 0.0 {
                    to_player.normalize()
                } else {
                    Vec2::ZERO
                };

                // Sync the mirrored HP first so the phase
                // transition below can read it.
                state.current_hp = health.current;

                // Phase detection: switch from Phase1 to
                // Phase2 the frame HP crosses the half-HP
                // threshold. We carry `current_hp` on the
                // state component (mirroring `Health.current`)
                // so the transition triggers on the
                // crossing, not on every frame the boss is
                // at low HP. Reset the attack timer so the
                // first Phase 2 burst doesn't fire on the
                // enrage frame.
                let half_hp = settings.boss_max_hp * 0.5;
                if state.phase == BossPhase::Phase1 && state.current_hp <= half_hp {
                    state.phase = BossPhase::Phase2;
                    state.attack_timer = Timer::from_seconds(
                        settings.boss_phase2_attack_interval_s,
                        TimerMode::Once,
                    );
                }

                // Movement: chase the player. Phase 1 at
                // base speed, Phase 2 at multiplier speed.
                let speed = match state.phase {
                    BossPhase::Phase1 => settings.boss_move_speed,
                    BossPhase::Phase2 => {
                        settings.boss_move_speed * settings.boss_phase2_speed_multiplier
                    }
                };
                base.0 = dir * speed;

                // Phase 2: 3-burst spread every interval.
                // Tick the timer; on finish, fire a 3-round
                // fan aimed at the current player pos, then
                // reset. The burst is a 17° spread (same
                // arc as the shotgun) so a player running
                // perpendicular dodges cleanly.
                if state.phase == BossPhase::Phase2 {
                    state.attack_timer.tick(time.delta());
                    if state.attack_timer.finished() {
                        let n = settings.boss_phase2_burst_count.max(1) as i32;
                        let total_spread: f32 = 0.30; // ~17°
                        for i in 0..n {
                            let t = if n == 1 {
                                0.0
                            } else {
                                i as f32 / (n - 1) as f32 - 0.5
                            };
                            let angle_offset = t * total_spread;
                            let (s, c) = angle_offset.sin_cos();
                            let burst_dir = Vec2::new(dir.x * c - dir.y * s, dir.x * s + dir.y * c);
                            spawn_boss_projectile(&mut commands, &settings, enemy_pos, burst_dir);
                        }
                        state.attack_timer = Timer::from_seconds(
                            settings.boss_phase2_attack_interval_s,
                            TimerMode::Once,
                        );
                    }
                }
            }
            // Phase 2 P5: Beacon is a static enemy (no movement)
                        // that extends the Cascade window on kill. The
                        // `Beacon` marker on the entity is what the kill
                        // handler in `combat.rs` reads via `Has<Beacon>` to fire
                        // the cascade-chain-extend hook; the contact-damage
                        // handler skips the beacon via the same marker (see the
                        // early-continue in `enemy_contact_damage_system`).
                        //
                        // Phase 2 P5.1: the beacon now FLEES from the player
                        // instead of sitting still. It runs at
                        // `beacon_flee_speed` directly away from the player's
                        // position. During Cascade, the beacon is NOT
                        // CascadeAffected, so it moves at full speed while
                        // other enemies are slowed to 0.25× — the player
                        // has to chase it down to get the chain extend.
                        AiBehavior::Beacon => {
                            let to_player = player_pos - enemy_transform.translation.truncate();
                            let dir = if to_player.length_squared() > 0.0 {
                                -to_player.normalize()
                            } else {
                                Vec2::ZERO
                            };
                            base.0 = dir * settings.beacon_flee_speed;
                        }
        }
    }
}

// ─── Contact damage ──────────────────────────────────────────────────────

// AABB-vs-AABB overlap test between two transformed entities. Returns
// true if the rectangles overlap on both axes.
fn aabb_overlap(pos_a: Vec2, half_a: Vec2, pos_b: Vec2, half_b: Vec2) -> bool {
    let delta = (pos_a - pos_b).abs();
    delta.x < half_a.x + half_b.x && delta.y < half_a.y + half_b.y
}

/// Player query used by the contact-damage system: needs the entity
/// handle (to attach Knockback / Invulnerable) plus position, collider,
/// mutable health, and the Invulnerable presence flag.
type PlayerForContact = (
    Entity,
    &'static Transform,
    &'static Collider,
    &'static mut Health,
    Has<Invulnerable>,
);

// Each frame, check if any enemy overlaps the player. On contact:
//   - if the player is invulnerable, skip (dash iframe / hit iframe)
//   - otherwise, deal damage, attach Knockback, attach hit Invulnerable
// One hit per frame — breaks after the first overlap so the player
// can't be hit twice on the same contact frame.
// Frozen during game over so an enemy that's standing on the dead
// player doesn't keep resetting the run state or causing a "you
// died" log spam.
//
// Day 8-10 used a hard-coded `CHASER_SIZE_HALF` for the AABB
// half-extents. Phase 1 reads the per-entity `Collider` instead,
// which lets future enemy types (shooter, sniper, mini-boss) have
// their own sizes without touching this system. Contact damage
// also dispatches on the enemy's AI behavior — chasers deal
// `chaser_contact_damage`, dashers deal `dasher_contact_damage`.
// The knockback and screen-shake triggers are identical.
fn enemy_contact_damage_system(
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut commands: Commands,
    mut player_query: Query<PlayerForContact, With<Player>>,
    enemies: Query<(&Transform, &Collider, &EnemyAI), (With<Enemy>, With<Collider>)>,
    mut shake: ResMut<ScreenShake>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    if hitstop.is_some() {
        return;
    }
    let Ok((player_entity, player_tf, player_col, mut health, invulnerable)) =
        player_query.single_mut()
    else {
        return;
    };
    if invulnerable {
        return;
    }

    let player_pos = player_tf.translation.truncate();

    for (enemy_tf, enemy_col, enemy_ai) in &enemies {
        // Phase 2 P5: the beacon is a "hit me" target, not
        // a "touch me" one. Skip contact damage entirely
        // (no damage, no log, no shake) so the player can
        // walk up to the beacon during Cascade without
        // losing HP for it. The beacon is still hittable
        // by projectiles / the beam (those paths use
        // `With<Enemy>` filtering that includes the
        // beacon, and the kill handler applies the
        // chain-extend hook on a beacon kill).
        if matches!(enemy_ai.behavior, AiBehavior::Beacon) {
            continue;
        }
        let enemy_pos = enemy_tf.translation.truncate();
        if !aabb_overlap(
            player_pos,
            player_col.half_extents,
            enemy_pos,
            enemy_col.half_extents,
        ) {
            continue;
        }

        // Dispatch contact damage by archetype. Add a new arm for
        // every enemy type — chaser is 1, dasher is 2, shooter is 1,
        // sniper is 1.
        let damage = match enemy_ai.behavior {
            AiBehavior::Chaser => settings.chaser_contact_damage,
            AiBehavior::Dasher => settings.dasher_contact_damage,
            AiBehavior::Shooter => settings.shooter_contact_damage,
            AiBehavior::Sniper => settings.sniper_contact_damage,
            AiBehavior::Boss => settings.boss_contact_damage,
            // Unreachable: filtered by the early-continue
            // above. Listed to keep the match exhaustive.
            AiBehavior::Beacon => 0.0,
        };
        health.current = (health.current - damage).max(0.0);
        damage_events.send(DamageEvent {
            victim: player_entity,
            amount: damage,
        });
        spawn_hit_spark(&mut commands, &settings, enemy_pos);
        warn!(
            "Player took {:.1} damage ({:?}) — HP: {:.1}",
            damage, enemy_ai.behavior, health.current
        );

        // Day 13-14: trigger screen shake. Set both the duration
        // (full shake window) and the remaining time so the camera
        // follow system can decay the magnitude linearly.
        shake.intensity = settings.screen_shake_intensity;
        shake.duration = settings.screen_shake_duration_s;
        shake.remaining = settings.screen_shake_duration_s;

        // Knockback direction = away from the enemy.
        let from_enemy = (player_pos - enemy_pos).normalize_or_zero();
        commands.entity(player_entity).insert(Knockback {
            direction: if from_enemy == Vec2::ZERO {
                Vec2::Y
            } else {
                from_enemy
            },
            timer: Timer::from_seconds(settings.chaser_knockback_duration_s, TimerMode::Once),
        });
        // Hit i-frames reuse the existing `Invulnerable` so the dash
        // visual flash + iframe gate work the same way.
        commands
            .entity(player_entity)
            .insert(Invulnerable(Timer::from_seconds(
                settings.hit_iframe_duration_s,
                TimerMode::Once,
            )));

        // One hit per frame is enough — break out so the player can't
        // be hit twice on the same contact frame.
        break;
    }
}

/// Bright white death-puff color. Stands out against the chaser red,
/// the dasher purple, and the player's orange; the alpha fade is what
/// makes it readable as a "puff" rather than a static square.
const DEATH_PUFF_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);

/// Spawn a DeathPuff entity at `position`. Called from the projectile
/// damage system when an enemy's HP hits zero.
pub fn spawn_death_puff(commands: &mut Commands, settings: &GameSettings, position: Vec2) {
    commands.spawn((
        Sprite {
            color: DEATH_PUFF_COLOR,
            custom_size: Some(Vec2::splat(CHASER_SIZE)),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 0.0),
        DeathPuff {
            lifetime: Timer::from_seconds(settings.death_puff_lifetime_s, TimerMode::Once),
        },
    ));
}

/// Tick every DeathPuff: fade alpha from 1.0 to 0.0 over its lifetime,
/// then despawn. Each puff is independent (its own Timer) so they can
/// overlap without interference.
fn death_puff_tick_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut DeathPuff, &mut Sprite)>,
) {
    for (entity, mut puff, mut sprite) in &mut query {
        puff.lifetime.tick(time.delta());
        if puff.lifetime.finished() {
            commands.entity(entity).despawn();
            continue;
        }
        let progress = puff.lifetime.elapsed_secs() / puff.lifetime.duration().as_secs_f32();
        let alpha = 1.0 - progress.clamp(0.0, 1.0);
        sprite.color = sprite.color.with_alpha(alpha);
    }
}

/// Day 13-14: brief white pulse on an enemy when hit by a projectile.
/// Inserted by the projectile-damage system; here we tick the timer
/// and lerp the enemy color toward white (stronger at the start,
/// fading back to the enemy's `BaseColor` by the time the timer
/// elapses). When the timer finishes, the color is snapped back to
/// the per-entity base so subsequent hits start from a clean state
/// AND the dasher's purple stays purple (the previous hardcoded
/// `CHASER_COLOR` reset was turning every non-killed enemy red).
fn hit_flash_tick_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut HitFlash, &mut Sprite, &BaseColor), With<Enemy>>,
) {
    for (entity, mut flash, mut sprite, base_color) in &mut query {
        flash.timer.tick(time.delta());
        if flash.timer.finished() {
            sprite.color = base_color.0;
            commands.entity(entity).remove::<HitFlash>();
            continue;
        }
        let progress = flash.timer.elapsed_secs() / flash.timer.duration().as_secs_f32();
        // Inverse-progress lerp: 100% flash (full white) at the
        // start of the window, 0% (back to base) at the end.
        // The previous 0.7 cap was tuned for the chaser's red
        // base but made the flash barely visible on dark
        // colors (e.g. a dasher lerping 70% from purple toward
        // white = a pale lavender — no "I hit it!" signal).
        // Full white at peak is the universal hit feedback.
        let t = 1.0 - progress;
        let base = base_color.0.to_srgba();
        sprite.color = Color::srgba(
            base.red + (1.0 - base.red) * t,
            base.green + (1.0 - base.green) * t,
            base.blue + (1.0 - base.blue) * t,
            base.alpha,
        );
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{RoomId, RoomKind};

    // Spawn position clamp (Phase 2.1): the enemy must
    // spawn inside the current room, with a margin from
    // each wall. The clamp is a simple Vec2::clamp on
    // each axis. We test by running the function 50
    // times (the xorshift PRNG gives different outputs
    // each call) and verifying every result stays inside
    // the room with the margin.

    fn test_room() -> Room {
        Room {
            id: RoomId(1),
            kind: RoomKind::Fight,
            bounds_min: Vec2::new(0.0, 0.0),
            bounds_max: Vec2::new(800.0, 800.0),
        }
    }

    fn test_settings() -> GameSettings {
        GameSettings {
            spawner_min_distance: 100.0,
            spawner_max_distance: 300.0,
            ..Default::default()
        }
    }

    fn test_rng() -> RunRng {
        // Phase 2.3: deterministic seed so the test is reproducible.
        // Each call site uses a different seed so the 3 tests don't
        // all hit the same first-spawn position.
        RunRng::new(0xDEADBEEF)
    }

    #[test]
    fn spawn_position_clamp_when_player_in_center() {
        let room = test_room();
        let settings = test_settings();
        let player = Vec2::new(400.0, 400.0);
        for _ in 0..50 {
            let mut rng = test_rng();
            let pos = random_spawn_position(&settings, player, &room, &mut rng);
            assert!(
                pos.x >= 25.0 && pos.x <= 775.0,
                "x outside room: {:?}",
                pos
            );
            assert!(
                pos.y >= 25.0 && pos.y <= 775.0,
                "y outside room: {:?}",
                pos
            );
        }
    }

    #[test]
    fn spawn_position_clamp_when_player_at_wall() {
        // Player at the corner of the room. Even with the
        // longest possible distance, the raw spawn position
        // would be outside the room without clamping.
        let room = test_room();
        let settings = test_settings();
        let player = Vec2::new(0.0, 0.0); // corner
        for _ in 0..50 {
            let mut rng = test_rng();
            let pos = random_spawn_position(&settings, player, &room, &mut rng);
            assert!(
                pos.x >= 25.0 && pos.x <= 775.0,
                "x outside room from corner: {:?}",
                pos
            );
            assert!(
                pos.y >= 25.0 && pos.y <= 775.0,
                "y outside room from corner: {:?}",
                pos
            );
        }
    }

    // Phase 2.1 regression: the player at a wall edge +
    // a long spawn distance used to spawn the enemy
    // outside the current room (visible as "enemies in
    // the Spawn room while the player is in the Fight
    // room"). The clamp fixes this.
    #[test]
    fn spawn_position_never_exits_room_bounds() {
        let room = test_room();
        let settings = test_settings();
        for player_x in [0.0, 200.0, 400.0, 600.0, 800.0] {
            for player_y in [0.0, 200.0, 400.0, 600.0, 800.0] {
                let player = Vec2::new(player_x, player_y);
                for _ in 0..10 {
                    let mut rng = test_rng();
                    let pos = random_spawn_position(&settings, player, &room, &mut rng);
                    assert!(
                        pos.x >= 25.0 && pos.x <= 775.0,
                        "player ({}, {}) → spawn x outside: {:?}",
                        player_x,
                        player_y,
                        pos
                    );
                    assert!(
                        pos.y >= 25.0 && pos.y <= 775.0,
                        "player ({}, {}) → spawn y outside: {:?}",
                        player_x,
                        player_y,
                        pos
                    );
                }
            }
        }
    }

    // Phase 2.3: same seed → same spawn position.
    // The RunRng-based function is deterministic. Two
    // fresh RNGs with the same seed should produce
    // identical first-spawn positions.
    #[test]
    fn spawn_position_is_deterministic_for_same_seed() {
        let room = test_room();
        let settings = test_settings();
        let player = Vec2::new(400.0, 400.0);
        let mut rng_a = RunRng::new(0xCAFEBABE);
        let mut rng_b = RunRng::new(0xCAFEBABE);
        let pos_a = random_spawn_position(&settings, player, &room, &mut rng_a);
        let pos_b = random_spawn_position(&settings, player, &room, &mut rng_b);
        assert_eq!(pos_a, pos_b, "same seed must give same spawn pos");
    }
}
