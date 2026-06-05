//! Player movement, input, and camera follow. Day 1-2 of Phase 0.

use bevy::prelude::*;

use crate::components::{BaseVelocity, MainCamera, Player, Velocity};
use crate::resources::GameSettings;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                player_input_system,
                velocity_apply_system,
                player_movement_system,
                camera_follow_system,
            )
                .chain(),
        );
    }
}

fn player_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    mut query: Query<&mut BaseVelocity, With<Player>>,
) {
    let Ok(mut base_velocity) = query.single_mut() else {
        return;
    };

    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        dir.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        dir.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        dir.x += 1.0;
    }

    // Diagonal is the same speed as cardinal — normalize if there's input.
    base_velocity.0 = if dir.length() > 0.0 {
        dir.normalize() * settings.player_move_speed
    } else {
        Vec2::ZERO
    };
}

// In Phase 1, the Cascade scaling will land here — every cascade-affected
// entity gets its Velocity multiplied by 0.25 each frame.
fn velocity_apply_system(mut query: Query<(&BaseVelocity, &mut Velocity)>) {
    for (base, mut vel) in &mut query {
        vel.0 = base.0;
    }
}

fn player_movement_system(
    time: Res<Time>,
    mut query: Query<(&Velocity, &mut Transform), With<Player>>,
) {
    for (velocity, mut transform) in &mut query {
        transform.translation.x += velocity.0.x * time.delta_secs();
        transform.translation.y += velocity.0.y * time.delta_secs();
    }
}

// Frame-rate independent exponential smoothing. The camera approaches
// the player at `lerp_speed` units per second.
fn camera_follow_system(
    time: Res<Time>,
    player_query: Query<&Transform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };

    let lerp_speed = 8.0;
    let t = 1.0 - (-lerp_speed * time.delta_secs()).exp();
    camera_transform.translation = camera_transform
        .translation
        .lerp(player_transform.translation, t);
}
