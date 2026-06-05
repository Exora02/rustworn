//! Phase 0 setup: window, camera, placeholder player, default resources.
//!
//! This is intentionally minimal. Future setup work (asset loading,
//! first-run tutorial, main menu) will land in dedicated plugins.

use bevy::prelude::*;
use log::info;

use crate::components::{BaseVelocity, Health, MainCamera, Player, Velocity};
use crate::resources::{CascadeState, GameSettings, RunState};

/// The placeholder player's color — Exo orange, our protogen mascot.
const PLAYER_COLOR: Color = Color::srgb(0.925, 0.525, 0.012); // ~#ec8603

/// Bevy plugin that wires up everything needed before a run can start.
pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CascadeState>()
            .init_resource::<RunState>()
            .init_resource::<GameSettings>()
            .add_systems(Startup, spawn_camera_and_player);
    }
}

fn spawn_camera_and_player(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera));

    commands.spawn((
        // Visual: a 40x40 orange square. Real sprite lands in Phase 1.
        Sprite {
            color: PLAYER_COLOR,
            custom_size: Some(Vec2::splat(40.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        // ECS data: same as a real player will have on day one.
        Player,
        Health::new(4.0),
        Velocity::default(),
        BaseVelocity::default(),
    ));

    info!("RUSTWORN booted — placeholder player spawned at origin");
}
