//! Door/tile transitions: teleport between rooms, floor transition
//! fade-out, and boss-death gate logic.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use log::info;

use crate::components::{
    AppState, Boss, Breakable, Collider, DeathPuff, Enemy, FloorTransitionText, Health,
    MainCamera, AimIndicator, Pickup, Player, Projectile, Room, RoomId, RoomKind, Shrine,
    TeleportPad, Velocity, Wall, WaveSpawner,
};
use crate::events::RoomClearedEvent;
use crate::resources::{BossRampTimer, FloorState, GameSettings, RunRng};
use crate::resources::RunState;

const PAD_SIZE: f32 = 60.0;
const ENTRY_INSET: f32 = 50.0;
const ROOM_SIZE: f32 = 1000.0;

// ─── Teleport transition ─────────────────────────────────────────────────

pub fn teleport_transition_system(
    mut commands: Commands,
    mut floor: ResMut<FloorState>,
    settings: Res<GameSettings>,
    spawners: Query<Entity, With<WaveSpawner>>,
    enemies: Query<Entity, With<Enemy>>,
    projectiles: Query<Entity, With<Projectile>>,
    puffs: Query<Entity, With<DeathPuff>>,
    pickups: Query<Entity, With<Pickup>>,
    shrines: Query<Entity, With<Shrine>>,
    breakables: Query<Entity, With<Breakable>>,
    pads: Query<(Entity, &Transform, &Collider, &TeleportPad), Without<Player>>,
    mut player_query: Query<(&mut Transform, &mut Health), (With<Player>, Without<AimIndicator>)>,
    walls: Query<Entity, With<Wall>>,
) {
    let Ok((mut player_tf, mut player_health)) = player_query.single_mut() else { return; };

    for (pad_entity, pad_tf, pad_col, teleport) in &pads {
        if !aabb_overlap(
            player_tf.translation.truncate(), Vec2::splat(20.0),
            pad_tf.translation.truncate(), pad_col.half_extents,
        ) { continue; }

        let next_room_id = RoomId(floor.rooms.len() as u8);

        // Check if the player is LEAVING a Boss room → floor advance.
        if let Some(current) = floor.rooms.iter().find(|r| r.id == floor.current_room) {
            if current.kind == RoomKind::Boss {
                floor.floor_number += 1;

                // Spawn floor transition text above the player.
                commands.spawn((
                    Text2d::new(format!("Floor {}", floor.floor_number)),
                    TextFont { font_size: 48.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.8, 0.2)),
                    TextLayout::new_with_justify(JustifyText::Center),
                    Anchor::Center,
                    Transform::from_xyz(
                        player_tf.translation.x,
                        player_tf.translation.y + 100.0,
                        15.0,
                    ),
                    FloorTransitionText(Timer::from_seconds(2.0, TimerMode::Once)),
                ));
            }
        }

        // Determine next room kind from pad label.
        let next_kind = match teleport.label.as_str() {
            "Treasure" => RoomKind::Fight,
            "Shop" => RoomKind::Shop,
            "Boss" => RoomKind::Boss,
            "Elite" => RoomKind::MiniBoss,
            "Horde" => RoomKind::Horde,
            _ => RoomKind::Fight,
        };

        // Despawn old room's transient entities.
        for e in &enemies { commands.entity(e).despawn(); }
        for e in &projectiles { commands.entity(e).despawn(); }
        for e in &puffs { commands.entity(e).despawn(); }
        for e in &pickups { commands.entity(e).despawn(); }
        for e in &shrines { commands.entity(e).despawn(); }
        for e in &breakables { commands.entity(e).despawn(); }
        for e in &spawners { commands.entity(e).despawn(); }
        for e in &walls { commands.entity(e).despawn(); }

        // Despawn all pads.
        commands.entity(pad_entity).despawn();
        // Also despawn any other pads still in the world.
        for (other_pad, _, _, _) in pads.iter().filter(|(e, ..)| *e != pad_entity) {
            commands.entity(other_pad).despawn();
        }

        // Generate next room.
        let new_room = super::generate_room(&mut commands, &floor, &settings, next_room_id, next_kind);
        floor.rooms.push(new_room);
        floor.current_room = next_room_id;

        // Treasure room bonus: extra breakables + guaranteed shrine.
        if teleport.label == "Treasure" {
            let Some(r) = floor.rooms.iter().find(|r| r.id == next_room_id) else {
                return;
            };
            super::spawn_breakables(&mut commands, r, 8);
            super::spawn_shrines(&mut commands, r, 2);
            info!("Treasure room {}: bonus breakables + shrines spawned", next_room_id.0);
        }

        // Boss room heal: leaving a Boss room heals the player fully.
        if let Some(current) = floor.rooms.iter().find(|r| r.id == floor.current_room) {
            if current.kind == RoomKind::Boss {
                player_health.current = player_health.max;
                info!("Boss room cleared — player fully healed");
            }
        }

        let entry = Vec2::new(
            new_room.bounds_min.x + ENTRY_INSET,
            (new_room.bounds_min.y + new_room.bounds_max.y) * 0.5,
        );
        player_tf.translation = entry.extend(0.0);

        info!(
            "Transition: room {} → {} ({:?})",
            floor.rooms.len() - 2, next_room_id.0, next_kind
        );
        return;
    }
}

// ─── Boss room clear (boss death gates room cleared) ───────────────────

pub fn boss_death_room_clear_system(
    mut commands: Commands,
    floor: Res<FloorState>,
    bosses: Query<(), With<Boss>>,
    spawners: Query<&WaveSpawner>,
    pads: Query<&TeleportPad>,
    mut events: EventWriter<RoomClearedEvent>,
) {
    let Some(current) = floor.rooms.iter().find(|r| r.id == floor.current_room) else {
        return;
    };
    if current.kind != RoomKind::Boss {
        return;
    }
    // Boss still alive → not cleared.
    if bosses.iter().next().is_some() {
        return;
    }
    // Already have a pad in this room → already cleared.
    if pads.iter().any(|p| p.label == "Boss" || p.label == "Fight") {
        return;
    }
    events.send(RoomClearedEvent {
        room_id: floor.current_room,
    });

    // Insert the BossRampTimer.
    let start_interval = spawners
        .iter()
        .find(|s| s.room_id == floor.current_room)
        .map(|s| s.farming_interval)
        .unwrap_or(0.3);
    commands.insert_resource(BossRampTimer {
        timer: Timer::from_seconds(15.0, TimerMode::Once),
        start_interval,
    });

    info!(
        "Boss room {} cleared (boss dead) — speed ramp started",
        floor.current_room.0
    );
}

// ─── Floor transition fade ────────────────────────────────────────────────

pub fn floor_transition_fade_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(
        Entity,
        &mut FloorTransitionText,
        &mut TextColor,
        &mut Transform,
    )>,
) {
    for (entity, mut state, mut color, mut tf) in &mut query {
        state.0.tick(time.delta());
        let progress = state.0.fraction();

        // Halfway through: start fading alpha.
        if progress > 0.5 {
            let fade = 1.0 - (progress - 0.5) * 2.0;
            color.0 = color.0.with_alpha(fade);
        }

        // When timer finishes: despawn.
        if state.0.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn aabb_overlap(a_center: Vec2, a_half: Vec2, b_center: Vec2, b_half: Vec2) -> bool {
    let a = a_center - b_center;
    a.x.abs() < a_half.x + b_half.x && a.y.abs() < a_half.y + b_half.y
}