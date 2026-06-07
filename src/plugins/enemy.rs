//! Enemy plugin: spawner, AI, and contact-damage resolution.
//!
//! Day 8-10: one chaser archetype. Spawner ticks every N seconds and
//! creates a chaser at a random angle from the player. Chaser homes
//! in at a constant speed. On AABB contact with the player, deals
//! damage, applies knockback, and gives a brief i-frame window.

use bevy::prelude::*;
use log::warn;

use crate::components::{
    AiBehavior, BaseVelocity, Collider, Enemy, EnemyAI, Health, Invulnerable, Knockback, Player,
    Spawner, Velocity,
};
use crate::resources::GameSettings;

/// Color of the chaser — high-contrast red so it reads as "danger" at
/// a glance against the orange player and the dark background we'll
/// add later.
const CHASER_COLOR: Color = Color::srgb(0.84, 0.16, 0.16); // ~#d62828

/// Chaser sprite size. Slightly smaller than the player (40x40) so the
/// chaser feels more compact and the player has a small reach advantage
/// when kiting.
const CHASER_SIZE: f32 = 30.0;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        // Order: spawn (creates entities) → AI (writes their BaseVelocity)
        // → contact damage (resolves hits). Movement + visual feedback
        // happen via shared systems in the player and combat plugins.
        app.add_systems(
            Update,
            (
                spawner_tick_system,
                enemy_ai_system,
                enemy_contact_damage_system,
            )
                .chain(),
        );
    }
}

// ─── Spawner ─────────────────────────────────────────────────────────────

// Each frame, advance every spawner's timer. When the timer elapses,
// spawn a chaser at a random angle from the player at a random distance
// in [min_distance, max_distance], then reset the timer.
fn spawner_tick_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut commands: Commands,
    mut spawners: Query<&mut Spawner>,
    player_query: Query<&Transform, With<Player>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for mut spawner in &mut spawners {
        spawner.interval.tick(time.delta());
        if !spawner.interval.finished() {
            continue;
        }
        spawn_chaser(&mut commands, &settings, player_pos);
    }
}

fn spawn_chaser(commands: &mut Commands, settings: &GameSettings, player_pos: Vec2) {
    // Tiny xorshift64 LCG — no extra dep, plenty of variety for a
    // placeholder spawn pattern. Seeded from the system clock so two
    // runs always get different sequences.
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
        // Top 24 bits normalized to [0, 1).
        ((x >> 40) as f32) / ((1u32 << 24) as f32)
    };

    // Random angle and distance for the spawn.
    let angle = next_unit(&mut state) * std::f32::consts::TAU;
    let distance = settings.spawner_min_distance
        + next_unit(&mut state) * (settings.spawner_max_distance - settings.spawner_min_distance);
    let offset = Vec2::new(angle.cos(), angle.sin()) * distance;
    let spawn_pos = player_pos + offset;

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
        Velocity::default(),
        // Chasers carry BaseVelocity so the future Cascade scaler can
        // slow them to 0.25× during Cascade Mode (per PLAN.md §2).
        BaseVelocity::default(),
    ));
}

// ─── AI ──────────────────────────────────────────────────────────────────

// For each chaser, compute the direction from the chaser to the player
// and write it into BaseVelocity (scaled by chaser_move_speed). The
// shared `velocity_apply_system` then copies it into `Velocity`, and
// `movement_system` integrates it into the transform.
fn enemy_ai_system(
    settings: Res<GameSettings>,
    player_query: Query<&Transform, With<Player>>,
    mut enemies: Query<(&Transform, &EnemyAI, &mut BaseVelocity), With<Enemy>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for (enemy_transform, ai, mut base) in &mut enemies {
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

// Each frame, check if any chaser overlaps the player. On contact:
//   - if the player is invulnerable, skip (dash iframe / hit iframe)
//   - otherwise, deal damage, attach Knockback, attach hit Invulnerable
// One hit per frame — breaks after the first overlap so the player
// can't be hit twice on the same contact frame.
fn enemy_contact_damage_system(
    settings: Res<GameSettings>,
    mut commands: Commands,
    mut player_query: Query<PlayerForContact, With<Player>>,
    enemies: Query<&Transform, (With<Enemy>, With<Collider>)>,
) {
    let Ok((player_entity, player_tf, player_col, mut health, invulnerable)) =
        player_query.single_mut()
    else {
        return;
    };
    if invulnerable {
        return;
    }

    let player_pos = player_tf.translation.truncate();

    for enemy_tf in &enemies {
        let enemy_pos = enemy_tf.translation.truncate();
        if !aabb_overlap(player_pos, player_col.half_extents, enemy_pos, CHASER_SIZE_HALF) {
            continue;
        }

        // Apply damage.
        health.current = (health.current - settings.chaser_contact_damage).max(0.0);
        warn!(
            "Player took {:.1} damage — HP: {:.1}",
            settings.chaser_contact_damage, health.current
        );

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
        commands.entity(player_entity).insert(Invulnerable(Timer::from_seconds(
            settings.hit_iframe_duration_s,
            TimerMode::Once,
        )));

        // One hit per frame is enough — break out so the player can't
        // be hit twice on the same contact frame.
        break;
    }
}

/// Pre-computed half-extent of the chaser AABB, used in the contact
/// damage AABB test. Mirrors the value the spawner writes into the
/// chaser's `Collider` component, so a single source of truth is
/// implicit (the `CHASER_SIZE` constant).
const CHASER_SIZE_HALF: Vec2 = Vec2::new(CHASER_SIZE * 0.5, CHASER_SIZE * 0.5);
