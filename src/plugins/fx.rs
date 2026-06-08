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

use crate::components::{DamageNumber, HitSpark};
use crate::resources::GameSettings;

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (damage_number_update, hit_spark_update, spawn_damage_numbers)
                .chain(),
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
                lifetime: Timer::from_seconds(
                    settings.damage_number_lifetime_s,
                    TimerMode::Once,
                ),
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
            (dmg.lifetime.elapsed_secs() / dmg.lifetime.duration().as_secs_f32())
                .clamp(0.0, 1.0)
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
