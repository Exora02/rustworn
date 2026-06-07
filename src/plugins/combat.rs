//! Combat: weapons, projectiles, and damage.
//!
//! Day 5-7: click-to-fire pistol, projectile with lifetime, no enemies yet.
//! Damage routing (Day 11-12) will land here too.

use bevy::prelude::*;

use crate::components::{AimDirection, BaseVelocity, Player, Projectile, Team, Velocity, Weapon};
use crate::resources::GameSettings;

/// Color of the player projectile — warm white, clearly readable as a
/// bullet against any background we'll add later.
const PROJECTILE_COLOR: Color = Color::srgb(1.0, 0.95, 0.6);

/// Sprite size of the projectile. Long axis aligns with travel direction.
const PROJECTILE_SIZE: Vec2 = Vec2::new(6.0, 12.0);

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // Order: cooldown tick (drives `fire_timer.finished()`) → fire
        // (consumes click + spawns projectile) → lifetime (despawns old
        // ones). Movement is handled by the generic `movement_system` in
        // the player plugin.
        app.add_systems(
            Update,
            (
                weapon_cooldown_tick_system,
                weapon_fire_system,
                projectile_lifetime_system,
            )
                .chain(),
        );
    }
}

// Each frame, advance every weapon's fire_timer. A freshly fired weapon
// has its timer set to `1.0 / fire_rate`; once it ticks down to zero the
// weapon can fire again.
fn weapon_cooldown_tick_system(time: Res<Time>, mut query: Query<&mut Weapon>) {
    for mut weapon in &mut query {
        weapon.fire_timer.tick(time.delta());
    }
}

// Left-click on a ready weapon → spawn a Projectile traveling in the
// current aim direction. The projectile gets a `Velocity` but no
// `BaseVelocity` (player projectiles don't cascade-scale).
fn weapon_fire_system(
    mouse: Res<ButtonInput<MouseButton>>,
    aim: Res<AimDirection>,
    settings: Res<GameSettings>,
    mut commands: Commands,
    mut weapons: Query<(&mut Weapon, &Transform), With<Player>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok((mut weapon, player_transform)) = weapons.single_mut() else {
        return;
    };
    if !weapon.fire_timer.finished() {
        return;
    }

    // Reset cooldown. The combat plugin's tick system will count it down
    // each frame; the next fire becomes possible once the timer elapses.
    let interval = 1.0 / weapon.fire_rate;
    weapon.fire_timer = Timer::from_seconds(interval, TimerMode::Once);

    // Default to +Y if the player hasn't aimed yet.
    let dir = if aim.0 == Vec2::ZERO { Vec2::Y } else { aim.0 };

    // Spawn at the muzzle (player center + aim × offset) and rotate the
    // long axis of the sprite to match the aim direction.
    let spawn_pos = player_transform.translation.truncate() + dir * settings.weapon_muzzle_offset;
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

    commands.spawn((
        Sprite {
            color: PROJECTILE_COLOR,
            custom_size: Some(PROJECTILE_SIZE),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
            .with_rotation(Quat::from_rotation_z(angle)),
        // Player projectiles run at full speed during Cascade Mode.
        // No `BaseVelocity`/`CascadeAffected` markers, so neither the
        // velocity apply step nor the future cascade scaler will touch them.
        Velocity(dir * weapon.projectile_speed),
        BaseVelocity::default(),
        Projectile {
            damage: weapon.damage,
            lifetime: Timer::from_seconds(settings.projectile_lifetime_s, TimerMode::Once),
            team: Team::Player,
        },
    ));
}

fn projectile_lifetime_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Projectile)>,
) {
    for (entity, mut proj) in &mut query {
        proj.lifetime.tick(time.delta());
        if proj.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}
