//! Enemy plugin: spawner, AI, and contact-damage resolution.
//!
//! Day 8-10: one chaser archetype. Spawner ticks every N seconds and
//! creates a chaser at a random angle from the player. Chaser homes
//! in at a constant speed. On AABB contact with the player, deals
//! damage, applies knockback, and gives a brief i-frame window.

use bevy::prelude::*;
use log::warn;

use crate::components::{
    AiBehavior, BaseColor, BaseVelocity, CascadeAffected, Collider, DasherPhase, DasherState,
    DeathPuff, Enemy, EnemyAI, EnemyArchetype, Health, HitFlash, Invulnerable, Knockback, Player,
    Spawner, Velocity,
};
use crate::resources::{GameSettings, Hitstop, RunState, ScreenShake};

/// Color of the chaser — high-contrast red so it reads as "danger" at
/// a glance against the orange player and the dark background we'll
/// add later.
const CHASER_COLOR: Color = Color::srgb(0.84, 0.16, 0.16); // ~#d62828

/// Phase 1 dasher color. Purple/magenta so it's visually distinct
/// from the red chaser — the player learns "red = chaser, purple
/// = dasher" in one glance. The hue also telegraphs "burst damage"
/// (purple is the universal "fast/danger" color in arcade games).
const DASHER_COLOR: Color = Color::srgb(0.62, 0.20, 0.85); // ~#9e33d9

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
                spawner_tick_system,
                enemy_ai_system,
                enemy_contact_damage_system,
                death_puff_tick_system,
                hit_flash_tick_system,
            )
                .chain(),
        );
    }
}

// ─── Spawner ─────────────────────────────────────────────────────────────

// Each frame, advance every spawner's timer. When the timer elapses,
// spawn an enemy of the spawner's `archetype` at a random angle from
// the player at a random distance in [min_distance, max_distance],
// then reset the timer. Frozen during global hitstop so kills don't
// immediately spawn a new wave mid-freeze. Frozen during game over
// for the same reason.
fn spawner_tick_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut commands: Commands,
    mut spawners: Query<&mut Spawner>,
    player_query: Query<&Transform, With<Player>>,
) {
    if hitstop.is_some() || run.game_over {
        return;
    }
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for mut spawner in &mut spawners {
        spawner.interval.tick(time.delta());
        if !spawner.interval.finished() {
            continue;
        }
        spawn_enemy(&mut commands, &settings, player_pos, spawner.spawn);
    }
}

// Dispatch on the spawner's archetype. New archetypes slot in here.
fn spawn_enemy(
    commands: &mut Commands,
    settings: &GameSettings,
    player_pos: Vec2,
    archetype: EnemyArchetype,
) {
    match archetype {
        EnemyArchetype::Chaser => spawn_chaser(commands, settings, player_pos),
        EnemyArchetype::Dasher => spawn_dasher(commands, settings, player_pos),
    }
}

fn spawn_chaser(commands: &mut Commands, settings: &GameSettings, player_pos: Vec2) {
    let (spawn_pos, _) = random_spawn_position(settings, player_pos);

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
        Health::new(settings.enemy_max_hp),
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
fn spawn_dasher(commands: &mut Commands, settings: &GameSettings, player_pos: Vec2) {
    // Spawn position is the same as a chaser — random angle at a
    // random distance in [min, max]. We could spawn dashers
    // further out (so the player has time to read the charge
    // before they arrive) but that conflicts with the unified
    // spawner distance; Phase 2 will let each archetype override
    // its spawn range.
    let (spawn_pos, _) = random_spawn_position(settings, player_pos);

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
        Health::new(settings.dasher_max_hp),
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

// Shared spawn-position math: random angle, random distance in
// [min, max]. The xorshift state is seeded from the system clock
// at call time, so two spawns in the same frame get different
// values (they share the same `state` but advance it through the
// two `next_unit` calls).
fn random_spawn_position(settings: &GameSettings, player_pos: Vec2) -> (Vec2, u64) {
    let mut state = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xCAFEBABE);

    let next_unit = |s: &mut u64| -> f32 {
        let mut x = *s;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *s = x;
        ((x >> 40) as f32) / ((1u32 << 24) as f32)
    };

    let angle = next_unit(&mut state) * std::f32::consts::TAU;
    let distance = settings.spawner_min_distance
        + next_unit(&mut state) * (settings.spawner_max_distance - settings.spawner_min_distance);
    let offset = Vec2::new(angle.cos(), angle.sin()) * distance;
    (player_pos + offset, state)
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
    player_query: Query<&Transform, With<Player>>,
    mut enemies: Query<
        (
            &Transform,
            &EnemyAI,
            &mut BaseVelocity,
            Option<&mut DasherState>,
        ),
        With<Enemy>,
    >,
) {
    if hitstop.is_some() || run.game_over {
        return;
    }
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for (enemy_transform, ai, mut base, dasher_state) in &mut enemies {
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
) {
    if hitstop.is_some() || run.game_over {
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
        // every enemy type — chaser is 1, dasher is 2.
        let damage = match enemy_ai.behavior {
            AiBehavior::Chaser => settings.chaser_contact_damage,
            AiBehavior::Dasher => settings.dasher_contact_damage,
        };
        health.current = (health.current - damage).max(0.0);
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
