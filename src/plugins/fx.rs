//! World-space visual feedback: floating damage numbers, hit sparks.
//!
//! Both are world-space entities (Text2d / Sprite) spawned at the
//! hit point. They live for a short time, animate, and despawn.
//! Distinct from the HUD plugin (which is screen-anchored and lives
//! for the entire run) and from `HitFlash` / `DeathPuff` (which are
//! markers attached to existing entities).
//!
//! The `default_font` cargo feature (see Cargo.toml) is what makes
//! the Text2d path actually render. Without it the damage numbers
//! would be invisible. Verified on Bazzite (wgpu) 2026-06-08.

use bevy::prelude::*;

use crate::components::{
    AppState, DamageNumber, DeathParticle, HitSpark, MotionTrail, Player, TrailGhost,
};
use crate::resources::{CascadeState, GameSettings};

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (damage_number_update, hit_spark_update, spawn_damage_numbers)
                .chain()
                // Phase 2: damage numbers and hit sparks freeze
                // during MainMenu and GameOver. They were already
                // event-driven (only spawn on a damage event, and
                // damage events only fire during InRun), so this
                // is mostly defensive — the lifetime ticks for
                // already-spawned numbers / sparks shouldn't
                // advance while the world is frozen.
                .run_if(in_state(AppState::InRun)),
        );
        // Phase 2 P5.1: Sandevistan-style motion trail ghosts.
        // Runs during InRun only (no ghosts on menu/game-over).
        // The spawn system reads CascadeState to only produce
        // ghosts while the window is active. The fade system
        // runs every frame regardless of Cascade state so
        // ghosts that were spawned right as the window closed
        // still fade out properly.
        app.add_systems(
            Update,
            (trail_spawn_system, trail_fade_system)
                .chain()
                .run_if(in_state(AppState::InRun)),
        );
        // Phase 5 game juice: death particle burst ticks every
        // frame (also runs during hitstop so particles visually
        // freeze along with the action).
        app.add_systems(
            Update,
            (death_particle_tick_system, player_motion_trail_system)
                .run_if(in_state(AppState::InRun)),
        );
    }
}

/// Reads `DamageEvent` and spawns a `DamageNumber` Text2d entity at
/// the victim's position. Reads the position from the world; if the
/// victim has been despawned in the same frame the event fires (a
/// kill), the event carries the world-position via the enemy's
/// `Transform` lookup *before* despawn, so the position read here
/// is stable. The lifetime is short (~0.6s) and the drift is
/// upward — see `damage_number_update` for the animation.
fn spawn_damage_numbers(
    mut commands: Commands,
    mut events: EventReader<crate::events::DamageEvent>,
    settings: Res<GameSettings>,
    transforms: Query<&Transform>,
) {
    for ev in events.read() {
        let Ok(tf) = transforms.get(ev.victim) else {
            continue;
        };
        let pos = tf.translation.truncate();
        commands.spawn((
            DamageNumber {
                value: ev.amount.round() as i32,
                lifetime: Timer::from_seconds(settings.damage_number_lifetime_s, TimerMode::Once),
                drift_speed: settings.damage_number_drift_speed,
            },
            Text2d::new(format!("{}", ev.amount.round() as i32)),
            TextFont {
                font_size: settings.damage_number_font_size,
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.95, 0.30)),
            Transform::from_xyz(pos.x, pos.y, 1.0),
            Visibility::Inherited,
        ));
    }
}

/// Per-frame update for live damage numbers: drift up, fade alpha,
/// despawn at end of lifetime. Two simple `Query`s: one to read the
/// timer (immutable) and one to mutate the transform + visibility.
fn damage_number_update(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut DamageNumber, &mut Transform, &mut TextColor)>,
) {
    for (entity, mut dmg, mut tf, mut color) in &mut q {
        dmg.lifetime.tick(time.delta());
        let t = if dmg.lifetime.duration().as_secs_f32() > 0.0 {
            (dmg.lifetime.elapsed_secs() / dmg.lifetime.duration().as_secs_f32()).clamp(0.0, 1.0)
        } else {
            1.0
        };
        // Drift up at a constant rate; the lifetime controls how
        // far the number travels. Alpha fades from 1.0 → 0.0 across
        // the lifetime so the number doesn't pop out of existence.
        tf.translation.y += dmg.drift_speed * time.delta_secs();
        let alpha = (1.0 - t).powf(0.7);
        color.0 = color.0.with_alpha(alpha);
        if dmg.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Per-frame update for hit sparks: shrink + fade + despawn. A
/// hit spark is a small white square that briefly marks a hit point.
/// Lives in world space (no physics), scales down over its lifetime
/// to give a "spark fading" feel.
fn hit_spark_update(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut HitSpark, &mut Transform, &mut Sprite)>,
) {
    for (entity, mut spark, mut tf, mut sprite) in &mut q {
        spark.lifetime.tick(time.delta());
        let t = if spark.lifetime.duration().as_secs_f32() > 0.0 {
            (spark.lifetime.elapsed_secs() / spark.lifetime.duration().as_secs_f32())
                .clamp(0.0, 1.0)
        } else {
            1.0
        };
        // Shrink from 1.0 → 0.3 over the lifetime. Constant-size
        // sparks read as "blip" — shrinking reads as "burst".
        let scale = 1.0 - 0.7 * t;
        tf.scale = Vec3::splat(scale);
        let alpha = (1.0 - t).powf(0.5);
        sprite.color = sprite.color.with_alpha(alpha);
        if spark.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Convenience spawner used by combat + enemy plugins at the
/// moment of a hit. Spawns a `HitSpark` at `pos` with the
/// configured lifetime and size.
pub fn spawn_hit_spark(commands: &mut Commands, settings: &GameSettings, pos: Vec2) {
    commands.spawn((
        HitSpark {
            lifetime: Timer::from_seconds(settings.hit_spark_lifetime_s, TimerMode::Once),
        },
        Sprite {
            color: Color::srgb(1.0, 1.0, 1.0).with_alpha(0.9),
            custom_size: Some(Vec2::splat(settings.hit_spark_size)),
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, 0.6),
        Visibility::Inherited,
    ));
}

/// Ghost trail lifetime: how long each afterimage persists
/// before fading to transparent. 0.2s gives ~12 visible
/// ghosts at 60fps — a readable trail without overwhelming
/// the screen.
const TRAIL_GHOST_LIFETIME_S: f32 = 0.2;

/// Sandevistan-style afterimage system. Every frame during
/// Cascade, spawns a semi-transparent ghost sprite at each
/// entity tagged with `MotionTrail`. The ghost copies the
/// entity's current sprite color and fades to transparent
/// over `TRAIL_GHOST_LIFETIME_S` seconds.
///
/// The ghost sprite is 90% of the entity's size and has a
/// slight blue bias (Sandevistan aesthetic) via an 0.15
/// lerp toward blue. Visible at z=0.3 (above walls/floor,
/// below player/z=0).
fn trail_spawn_system(
    cascade: Res<CascadeState>,
    mut commands: Commands,
    entities: Query<(&Transform, &Sprite), With<MotionTrail>>,
) {
    if !cascade.active {
        return;
    }
    for (tf, sprite) in &entities {
        let pos = tf.translation;
        let color = sprite.color;
        // Entity's own color with a slight blue boost for the
        // Sandevistan aesthetic. Alpha at 0.35 so the ghost is
        // clearly translucent — readable trail without
        // overwhelming the source entity.
        let trail_color = Color::srgba(
            color.to_srgba().red,
            color.to_srgba().green,
            (color.to_srgba().blue + 0.15).min(1.0),
            0.35,
        );
        let size = sprite.custom_size.unwrap_or(Vec2::splat(30.0));
        commands.spawn((
            TrailGhost {
                lifetime: Timer::from_seconds(TRAIL_GHOST_LIFETIME_S, TimerMode::Once),
                initial_color: trail_color,
            },
            Sprite {
                color: trail_color.with_alpha(0.35),
                custom_size: Some(size * 0.9),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 0.3),
            Visibility::Inherited,
        ));
    }
}

/// Per-frame update for live trail ghosts: tick lifetime,
/// fade alpha toward 0, despawn on finish.
fn trail_fade_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut TrailGhost, &mut Sprite)>,
) {
    for (entity, mut ghost, mut sprite) in &mut q {
        ghost.lifetime.tick(time.delta());
        let t = if ghost.lifetime.duration().as_secs_f32() > 0.0 {
            (ghost.lifetime.elapsed_secs() / ghost.lifetime.duration().as_secs_f32()).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let alpha = ghost.initial_color.alpha() * (1.0 - t);
        sprite.color = ghost.initial_color.with_alpha(alpha);
        if ghost.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

// ─── Phase 5 game juice ─────────────────────────────────────────────

/// Color palette for death burst particles. Picked to be distinct
/// from enemy colours and easy to read against any background.
const DEATH_PARTICLE_COLORS: [Color; 5] = [
    Color::srgb(1.0, 0.5, 0.0),   // orange
    Color::srgb(1.0, 0.2, 0.1),   // red
    Color::srgb(1.0, 0.8, 0.2),   // gold
    Color::srgb(1.0, 1.0, 0.4),   // yellow
    Color::srgb(1.0, 0.6, 0.4),   // salmon
];

/// Death-particle lifetime in seconds.
const DEATH_PARTICLE_LIFETIME_S: f32 = 0.4;

/// Spawn 3-5 small colored particles at `position` that fly outward
/// in random directions and fade. Called from the death path in both
/// `projectile_damage_system` and `beam_damage_system`.
pub fn spawn_death_particles(commands: &mut Commands, position: Vec2) {
    use std::f32::consts::TAU;
    let count = 4;
    let base_angle = position.x + position.y; // cheap pseudo-random seed
    for i in 0..count {
        let angle = base_angle + (i as f32 / count as f32) * TAU;
        let speed = 60.0 + (i as f32 * 17.3) % 40.0;
        let vel = Vec2::new(angle.cos(), angle.sin()) * speed;
        let color = DEATH_PARTICLE_COLORS[i % DEATH_PARTICLE_COLORS.len()];
        commands.spawn((
            DeathParticle {
                lifetime: Timer::from_seconds(DEATH_PARTICLE_LIFETIME_S, TimerMode::Once),
                velocity: vel,
            },
            Sprite {
                color,
                custom_size: Some(Vec2::splat(5.0)),
                ..default()
            },
            Transform::from_xyz(position.x, position.y, 0.55),
            Visibility::Inherited,
        ));
    }
}

/// Per-frame update for death burst particles: move outward, fade
/// alpha, despawn on expiry.
fn death_particle_tick_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut DeathParticle, &mut Transform, &mut Sprite)>,
) {
    for (entity, mut particle, mut tf, mut sprite) in &mut q {
        particle.lifetime.tick(time.delta());
        let t = if particle.lifetime.duration().as_secs_f32() > 0.0 {
            (particle.lifetime.elapsed_secs() / particle.lifetime.duration().as_secs_f32())
                .clamp(0.0, 1.0)
        } else {
            1.0
        };
        // Move outward.
        tf.translation.x += particle.velocity.x * time.delta_secs();
        tf.translation.y += particle.velocity.y * time.delta_secs();
        // Fade alpha.
        let alpha = (1.0 - t).powf(0.6);
        sprite.color = sprite.color.with_alpha(alpha);
        if particle.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Always-on player trail: every frame, spawn a semi-transparent
/// copy of the player sprite at the player's current position with
/// a short 0.1s lifetime. Unlike the Cascade-gated `trail_spawn_system`,
/// this runs independently whenever the player is alive, providing a
/// constant movement-juice trail effect.
fn player_motion_trail_system(
    mut commands: Commands,
    player_query: Query<(&Transform, &Sprite), (With<Player>, Without<TrailGhost>)>,
) {
    let Ok((tf, sprite)) = player_query.single() else {
        return;
    };
    let pos = tf.translation;
    let color = sprite.color.with_alpha(0.20);
    let size = sprite.custom_size.unwrap_or(Vec2::splat(30.0));
    commands.spawn((
        TrailGhost {
            lifetime: Timer::from_seconds(0.1, TimerMode::Once),
            initial_color: color,
        },
        Sprite {
            color,
            custom_size: Some(size * 0.85),
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, 0.25),
        Visibility::Inherited,
    ));
}