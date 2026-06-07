//! Phase 0 setup: window, camera, placeholder player, default resources.
//!
//! This is intentionally minimal. Future setup work (asset loading,
//! first-run tutorial, main menu) will land in dedicated plugins.

use bevy::prelude::*;
use log::info;

use crate::components::{
    AimIndicator, BaseVelocity, Collider, EnemyArchetype, Health, MainCamera, Player, Spawner,
    Velocity, Weapon, WeaponArchetype, PLAYER_COLOR,
};
use crate::resources::{CascadeState, GameSettings, RunState, ScreenShake};

/// Color of the aim indicator line — high-contrast white so it stays
/// readable against any background we'll add later.
const AIM_INDICATOR_COLOR: Color = Color::WHITE;

/// Bevy plugin that wires up everything needed before a run can start.
pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CascadeState>()
            .init_resource::<RunState>()
            .init_resource::<GameSettings>()
            // Day 13-14: ScreenShake is Default, so a single
            // init_resource call seeds it (intensity=0, no shake).
            .init_resource::<ScreenShake>()
            .add_systems(Startup, spawn_camera_and_player);
    }
}

fn spawn_camera_and_player(mut commands: Commands, settings: Res<GameSettings>) {
    commands.spawn((Camera2d, MainCamera));

    commands
        .spawn((
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
            // Day 8-10: 40x40 sprite → 20x20 half-extents for AABB.
            Collider {
                half_extents: Vec2::splat(20.0),
            },
            // Day 5-7: starting pistol. Timer is set to a zero-duration
            // finished timer so the first click fires immediately.
            // Phase 1 (shotgun): `pellet_count` + `spread_rad` round
            // out the Weapon fields. Pistol = 1 pellet, no spread.
            Weapon {
                archetype: WeaponArchetype::Pistol,
                damage: settings.weapon_damage,
                fire_rate: settings.weapon_fire_rate,
                projectile_speed: settings.weapon_projectile_speed,
                pellet_count: 1,
                spread_rad: 0.0,
                fire_timer: Timer::from_seconds(0.0, TimerMode::Once),
            },
        ))
        .with_children(|parent| {
            // White line sticking out the top of the player; rotated each
            // frame to match the current aim direction. This is the
            // placeholder for a future gun sprite / hand attachment.
            parent.spawn((
                Sprite {
                    color: AIM_INDICATOR_COLOR,
                    custom_size: Some(Vec2::new(6.0, 30.0)),
                    ..default()
                },
                // +Y offset so the line sits ahead of the player body,
                // not centered on it. Z slightly forward to avoid z-fighting.
                Transform::from_xyz(0.0, 20.0, 1.0),
                AimIndicator,
            ));
        });

    // Day 8-10: chaser spawner entity — persists for the run, ticks
    // every `spawner_interval_s` and creates a chaser at a random
    // angle from the player, at a random distance in [min_distance,
    // max_distance].
    commands.spawn(Spawner {
        interval: Timer::from_seconds(settings.spawner_interval_s, TimerMode::Repeating),
        spawn: EnemyArchetype::Chaser,
    });

    // Phase 1: dasher spawner. Same interval, separate entity — the
    // two spawners tick independently, so the player sees a chaser
    // every 3s and a dasher every 3s in alternating (well, parallel)
    // waves. Phase 2 will fold this into a single wave-spawner
    // entity that picks archetypes from a wave definition.
    commands.spawn(Spawner {
        interval: Timer::from_seconds(settings.spawner_interval_s, TimerMode::Repeating),
        spawn: EnemyArchetype::Dasher,
    });

    info!("RUSTWORN booted — player + chaser/dasher spawners spawned at origin");
}
