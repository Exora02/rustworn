//! Treasure, Challenge, and Secret room behavior systems.
//!
//! These complement the standard room-generation pipeline in `mod.rs`
//! by adding special gameplay mechanics:
//! - Treasure rooms: pedestals with items, player takes one
//! - Challenge rooms: locked arena, 3 waves, reward on clear
//! - Secret rooms: hidden entrance, free loot inside

use bevy::prelude::*;
use bevy::sprite::Anchor;
use log::info;

use crate::components::{
    AiBehavior, BaseVelocity, Biome, CascadeAffected, ChallengeElite, ChallengeRun,
    ChallengeState, ChallengeWall, Collider, Enemy, EnemyAI, EnemyArchetype, Health, ItemKind,
    Pickup, PickupKind, Player, Room, RoomId, RoomKind, SecretWall, TreasurePedestal, Velocity,
    Wall, WeaponArchetype,
};
use crate::resources::{FloorState, GameSettings, RunRng, RunState};

use super::pads::pad_bundle;
use super::spawn_breakables;

// ─── Constants ────────────────────────────────────────────────────────────

const PEDESTAL_SIZE: f32 = 28.0;
const PEDESTAL_COLOR: Color = Color::srgb(0.60, 0.50, 0.30);
const PEDESTAL_ACTIVE_COLOR: Color = Color::srgb(1.0, 0.85, 0.50);
const TREASURE_ITEM_COLOR: Color = Color::srgb(0.70, 0.40, 0.95);
const CHALLENGE_BLOCK_WALL_COLOR: Color = Color::srgb(0.85, 0.25, 0.15);
const SECRET_REVEAL_DIST: f32 = 60.0;
const SECRET_WALL_OPAQUE: f32 = 1.0;
const SECRET_WALL_REVEALED: f32 = 0.30;

// ─── Treasure room spawning ───────────────────────────────────────────────

/// Spawn 2-3 pedestals in a Treasure room, each holding a random item.
pub(crate) fn spawn_treasure_pedestals(commands: &mut Commands, room: &Room, rng: &mut RunRng) {
    let center = room.center();
    let count = 2 + (rng.next_unit() * 2.0) as usize; // 2 or 3
    let spread = room.size().x * 0.2;

    for i in 0..count {
        let offset_x = (i as f32 - (count as f32 - 1.0) * 0.5) * spread;
        let pos = center + Vec2::new(offset_x, 0.0);

        // Roll a random item from the ItemKind pool.
        let item_idx = (rng.next_unit() * 12.0) as usize;
        let item_kind = match item_idx {
            0 => ItemKind::RustyNails,
            1 => ItemKind::LuckyCharm,
            2 => ItemKind::Featherlight,
            3 => ItemKind::SteelToe,
            4 => ItemKind::Bandage,
            5 => ItemKind::MagnifyingGlass,
            6 => ItemKind::GlassCannon,
            7 => ItemKind::VampireFangs,
            8 => ItemKind::ExplosiveCharge,
            9 => ItemKind::EnergyShield,
            10 => ItemKind::LuckyHorseshoe,
            _ => ItemKind::CursedRing,
        };

        // Pedestal base (colored rectangle)
        commands.spawn((
            Sprite {
                color: PEDESTAL_COLOR,
                custom_size: Some(Vec2::new(PEDESTAL_SIZE * 1.2, PEDESTAL_SIZE * 0.6)),
                anchor: Anchor::Center,
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y - PEDESTAL_SIZE * 0.3, 0.35),
        ));

        // Item display (small colored square on top of pedestal)
        commands.spawn((
            Sprite {
                color: TREASURE_ITEM_COLOR,
                custom_size: Some(Vec2::splat(PEDESTAL_SIZE * 0.6)),
                anchor: Anchor::Center,
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 0.40),
            TreasurePedestal {
                item: PickupKind::Item(item_kind),
                taken: false,
            },
            Collider {
                half_extents: Vec2::splat(PEDESTAL_SIZE * 0.5),
            },
        ));
    }

    // Label text
    let label_pos = center + Vec2::new(0.0, room.size().y * 0.3);
    commands.spawn((
        Text2d::new("TREASURE — Take One Item"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.95, 0.75, 0.20)),
        TextLayout::new_with_justify(JustifyText::Center),
        Anchor::Center,
        Transform::from_xyz(label_pos.x, label_pos.y, 10.0),
    ));
}

/// Player-over-pedestal collector. When the player walks over a pedestal,
/// the item is spawned as a real pickup and all pedestals despawn.
pub(crate) fn treasure_collect_system(
    mut commands: Commands,
    settings: Res<GameSettings>,
    player_query: Query<&Transform, (With<Player>, Without<TreasurePedestal>)>,
    pedestals: Query<(Entity, &Transform, &TreasurePedestal)>,
    mut collected_this_frame: Local<bool>,
) {
    if *collected_this_frame {
        return;
    }

    let Ok(player_tf) = player_query.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    let player_half = Vec2::splat(20.0);

    for (entity, tf, pedestal) in &pedestals {
        if pedestal.taken {
            continue;
        }
        let in_range = aabb_overlap(
            player_pos,
            player_half,
            tf.translation.truncate(),
            Vec2::splat(PEDESTAL_SIZE * 0.6),
        );
        if !in_range {
            continue;
        }

        info!("Treasure pedestal collected: {:?}", pedestal.item);

        // Spawn the item pickup at the pedestal position.
        let pos = tf.translation.truncate();
        crate::plugins::pickups::spawn_pickup(&mut commands, &settings, pos, pedestal.item);

        // Mark all pedestals as taken and despawn them.
        for (ped_entity, _, _) in &pedestals {
            commands.entity(ped_entity).despawn();
        }

        *collected_this_frame = true;
        return;
    }
}

// ─── Challenge room spawning ──────────────────────────────────────────────

/// Spawn wall entities blocking the exits in a Challenge room.
pub(crate) fn spawn_challenge_blocking_walls(commands: &mut Commands, room: &Room) {
    let half = 10.0;
    let w = room.size().x;
    let h = room.size().y;
    let cx = (room.bounds_min.x + room.bounds_max.x) * 0.5;
    let cy = (room.bounds_min.y + room.bounds_max.y) * 0.5;

    // Block the four walls' central regions (where doors would be).
    commands.spawn((
        Sprite {
            color: CHALLENGE_BLOCK_WALL_COLOR,
            custom_size: Some(Vec2::new(150.0, 30.0)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(cx, room.bounds_max.y + half, 0.0),
        ChallengeWall,
        Collider {
            half_extents: Vec2::new(75.0, 15.0),
        },
    ));
    commands.spawn((
        Sprite {
            color: CHALLENGE_BLOCK_WALL_COLOR,
            custom_size: Some(Vec2::new(150.0, 30.0)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(cx, room.bounds_min.y - half, 0.0),
        ChallengeWall,
        Collider {
            half_extents: Vec2::new(75.0, 15.0),
        },
    ));
    commands.spawn((
        Sprite {
            color: CHALLENGE_BLOCK_WALL_COLOR,
            custom_size: Some(Vec2::new(30.0, 150.0)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(room.bounds_max.x + half, cy, 0.0),
        ChallengeWall,
        Collider {
            half_extents: Vec2::new(15.0, 75.0),
        },
    ));
    commands.spawn((
        Sprite {
            color: CHALLENGE_BLOCK_WALL_COLOR,
            custom_size: Some(Vec2::new(30.0, 150.0)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(room.bounds_min.x - half, cy, 0.0),
        ChallengeWall,
        Collider {
            half_extents: Vec2::new(15.0, 75.0),
        },
    ));
}

/// Spawn the challenge room label and register the ChallengeState.
pub(crate) fn spawn_challenge_setup(commands: &mut Commands, room: &Room) {
    let center = room.center();
    let label_pos = center + Vec2::new(0.0, room.size().y * 0.3);
    commands.spawn((
        Text2d::new("CHALLENGE — 3 Waves"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.25, 0.15)),
        TextLayout::new_with_justify(JustifyText::Center),
        Anchor::Center,
        Transform::from_xyz(label_pos.x, label_pos.y, 10.0),
    ));
}

/// Spawn challenge wave enemies directly. Called each frame by the
/// challenge_tick_system when the current wave needs enemies.
fn spawn_challenge_wave(
    commands: &mut Commands,
    settings: &GameSettings,
    floor: &FloorState,
    rng: &mut RunRng,
    room: &Room,
    wave: u32,
) -> bool {
    let center = room.center();
    let biome = crate::components::biome_from_floor(floor.floor_number);

    match wave {
        1 => {
            // Wave 1: 3 generic (chaser) enemies
            for i in 0..3 {
                let offset = Vec2::new((i as f32 - 1.0) * 80.0, -room.size().y * 0.2);
                let pos = center + offset;
                spawn_generic_enemy(
                    commands,
                    settings,
                    room,
                    pos,
                    &biome,
                    EnemyArchetype::Chaser,
                    false,
                );
            }
            info!("Challenge wave 1 spawned: 3 chasers");
        }
        2 => {
            // Wave 2: 5 enemies — mix of biome-specific + generic
            let archetypes = [
                EnemyArchetype::Chaser,
                EnemyArchetype::Dasher,
                EnemyArchetype::Shooter,
                EnemyArchetype::Chaser,
                EnemyArchetype::Dasher,
            ];
            for i in 0..5 {
                let offset = Vec2::new((i as f32 - 2.0) * 70.0, -room.size().y * 0.15);
                let pos = center + offset;
                spawn_generic_enemy(commands, settings, room, pos, &biome, archetypes[i], false);
            }
            info!("Challenge wave 2 spawned: 5 enemies");
        }
        3 => {
            // Wave 3: 7 enemies + 1 elite
            let archetypes = [
                EnemyArchetype::Chaser,
                EnemyArchetype::Dasher,
                EnemyArchetype::Shooter,
                EnemyArchetype::Chaser,
                EnemyArchetype::Dasher,
                EnemyArchetype::Shooter,
                EnemyArchetype::Chaser,
            ];
            for i in 0..7 {
                let offset = Vec2::new((i as f32 - 3.0) * 60.0, -room.size().y * 0.15);
                let pos = center + offset;
                spawn_generic_enemy(commands, settings, room, pos, &biome, archetypes[i], false);
            }
            // Spawn elite chaser
            let elite_pos = center + Vec2::new(0.0, room.size().y * 0.1);
            let elite_entity = spawn_generic_enemy(
                commands,
                settings,
                room,
                elite_pos,
                &biome,
                EnemyArchetype::Chaser,
                true,
            );
            // Tag the elite so the clear system can track it
            commands
                .entity(elite_entity)
                .insert(ChallengeElite { elite_entity });
            info!("Challenge wave 3 spawned: 7 enemies + 1 elite");
        }
        _ => {}
    }
    true
}

/// Spawn a single enemy at a specific position without Director budget.
fn spawn_generic_enemy(
    commands: &mut Commands,
    _settings: &GameSettings,
    _room: &Room,
    pos: Vec2,
    _biome: &Biome,
    _archetype: EnemyArchetype,
    is_elite: bool,
) -> Entity {
    let color = if is_elite {
        Color::srgb(0.95, 0.75, 0.20) // gold tint
    } else {
        Color::srgb(0.85, 0.15, 0.15) // red
    };
    let size = if is_elite { 22.0 } else { 16.0 };

    let hp = if is_elite { 8.0 } else { 3.0 };

    commands
        .spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::splat(size)),
                anchor: Anchor::Center,
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 0.3),
            Enemy,
            Health::new(hp),
            EnemyAI {
                behavior: AiBehavior::Chaser,
            },
            Velocity::default(),
            BaseVelocity::default(),
            CascadeAffected,
            Collider {
                half_extents: Vec2::splat(size * 0.5),
            },
        ))
        .id()
}

/// Track challenge wave progress and spawn enemies when needed.
/// On wave clear, the state advances. When all 3 waves are done,
/// the state.wave is set to 4 and challenge_complete_system handles rewards.
pub(crate) fn challenge_tick_system(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    floor: Res<FloorState>,
    mut rng: ResMut<RunRng>,
    enemy_query: Query<Entity, With<Enemy>>,
    challenge_elite_query: Query<&ChallengeElite>,
    mut challenge_states: Query<(Entity, &mut ChallengeState)>,
    player_query: Query<&Transform, (With<Player>, Without<Enemy>)>,
) {
    let Ok(player_tf) = player_query.single() else {
        return;
    };

    for (_entity, mut state) in &mut challenge_states {
        // Only process the current room's challenge state
        let room = floor.rooms.iter().find(|r| {
            r.bounds_min.x <= player_tf.translation.x
                && player_tf.translation.x <= r.bounds_max.x
                && r.bounds_min.y <= player_tf.translation.y
                && player_tf.translation.y <= r.bounds_max.y
        });
        let Some(room) = room else {
            continue;
        };
        if room.kind != RoomKind::Challenge {
            continue;
        }

        if state.wave > 3 {
            // All waves complete — skip
            continue;
        }

        if !state.enemies_spawned {
            // Spawn the current wave's enemies
            spawn_challenge_wave(&mut commands, &settings, &floor, &mut rng, room, state.wave);
            state.enemies_spawned = true;
            info!("Challenge wave {} — enemies spawned", state.wave);
        } else {
            // Check if all enemies for this wave are dead
            let enemies_alive = enemy_query.iter().count();
            if enemies_alive == 0 {
                // Wave cleared
                if state.wave < 3 {
                    // Advance to next wave
                    state.wave += 1;
                    state.enemies_spawned = false;
                    info!(
                        "Challenge wave {} cleared — advancing to wave {}",
                        state.wave - 1,
                        state.wave
                    );
                } else {
                    // Wave 3 cleared — challenge complete!
                    info!("Challenge complete — all 3 waves cleared!");
                    state.wave = 4; // Mark complete, triggers challenge_complete_system
                }
            }
        }
    }
}

/// Separate system to despawn ChallengeWalls and spawn rewards.
/// Runs after challenge_tick_system so the state.wave=4 flag is visible.
pub(crate) fn challenge_complete_system(
    mut commands: Commands,
    floor: Res<FloorState>,
    settings: Res<GameSettings>,
    challenge_states: Query<&ChallengeState>,
    challenge_walls: Query<Entity, With<ChallengeWall>>,
    player_query: Query<&Transform, (With<Player>, Without<Enemy>)>,
) {
    let Ok(player_tf) = player_query.single() else {
        return;
    };

    let room = floor.rooms.iter().find(|r| {
        r.bounds_min.x <= player_tf.translation.x
            && player_tf.translation.x <= r.bounds_max.x
            && r.bounds_min.y <= player_tf.translation.y
            && player_tf.translation.y <= r.bounds_max.y
    });
    let Some(room) = room else {
        return;
    };
    if room.kind != RoomKind::Challenge {
        return;
    }

    // Check if any challenge state in this room has wave=4 (complete)
    let complete = challenge_states.iter().any(|s| s.wave > 3);

    if complete {
        // Despawn blocking walls
        for entity in &challenge_walls {
            commands.entity(entity).despawn();
        }

        // Spawn reward at room center
        let center = room.center();
        // Weapon pickup
        let weapon_archetype = WeaponArchetype::Shotgun;
        commands.spawn((
            Pickup {
                kind: PickupKind::Weapon(weapon_archetype),
                lifetime: Timer::from_seconds(30.0, TimerMode::Once),
            },
            Sprite {
                color: Color::srgb(1.0, 0.55, 0.10),
                custom_size: Some(Vec2::splat(24.0)),
                ..default()
            },
            Transform::from_xyz(center.x, center.y + 20.0, 0.3),
            crate::components::Velocity(Vec2::ZERO),
        ));

        // Gold reward
        commands.spawn((
            Pickup {
                kind: PickupKind::Gold(25),
                lifetime: Timer::from_seconds(30.0, TimerMode::Once),
            },
            Sprite {
                color: Color::srgb(0.95, 0.85, 0.10),
                custom_size: Some(Vec2::splat(14.0)),
                ..default()
            },
            Transform::from_xyz(center.x, center.y - 20.0, 0.3),
            crate::components::Velocity(Vec2::ZERO),
        ));

        // Label
        commands.spawn((
            Text2d::new("CHALLENGE COMPLETE!"),
            TextFont {
                font_size: 28.0,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.85, 0.10)),
            TextLayout::new_with_justify(JustifyText::Center),
            Anchor::Center,
            Transform::from_xyz(center.x, center.y + 60.0, 10.0),
        ));

        info!("Challenge reward spawned: weapon + 25 gold");
    }
}

// ─── Secret room spawning ────────────────────────────────────────────────

/// Spawn a small Secret room (400x400) adjacent to an existing room.
/// Places the entrance wall as a special SecretWall entity.
pub(crate) fn spawn_secret_room(
    commands: &mut Commands,
    _floor: &FloorState,
    room: &Room,
    rng: &mut RunRng,
) -> Room {
    // Position the secret room to the right of the parent room
    let secret_size = 400.0;
    let secret_min = Vec2::new(room.bounds_max.x + 40.0, room.bounds_min.y + 100.0);
    let secret_max = Vec2::new(secret_min.x + secret_size, secret_min.y + secret_size);

    let secret_room = Room {
        id: RoomId(255), // Special ID for secret rooms
        kind: RoomKind::Secret,
        bounds_min: secret_min,
        bounds_max: secret_max,
    };

    // Spawn walls for the secret room
    let cx = (secret_min.x + secret_max.x) * 0.5;
    let cy = (secret_min.y + secret_max.y) * 0.5;
    let w = secret_size;
    let h = secret_size;
    let half = 10.0;

    // Floor background
    commands.spawn((
        Sprite {
            color: Color::srgba(0.15, 0.12, 0.15, 0.8),
            custom_size: Some(Vec2::new(w, h)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(cx, cy, -0.1),
        Wall,
    ));

    // Entrance wall (left side) — this is the secret wall that toggles transparency
    commands.spawn((
        Sprite {
            color: Color::srgb(0.30, 0.30, 0.35),
            custom_size: Some(Vec2::new(20.0, h)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(secret_min.x - half, cy, 0.0),
        Wall,
        Collider {
            half_extents: Vec2::new(10.0, h * 0.5),
        },
        SecretWall {
            revealed: false,
            room_id: room.id,
        },
    ));

    // Right wall
    commands.spawn((
        Sprite {
            color: Color::srgb(0.30, 0.30, 0.35),
            custom_size: Some(Vec2::new(20.0, h)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(secret_max.x + half, cy, 0.0),
        Wall,
        Collider {
            half_extents: Vec2::new(10.0, h * 0.5),
        },
    ));
    // Top wall
    commands.spawn((
        Sprite {
            color: Color::srgb(0.30, 0.30, 0.35),
            custom_size: Some(Vec2::new(w + 40.0, 20.0)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(cx, secret_max.y + half, 0.0),
        Wall,
        Collider {
            half_extents: Vec2::new(w * 0.5 + 20.0, 10.0),
        },
    ));
    // Bottom wall
    commands.spawn((
        Sprite {
            color: Color::srgb(0.30, 0.30, 0.35),
            custom_size: Some(Vec2::new(w + 40.0, 20.0)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(cx, secret_min.y - half, 0.0),
        Wall,
        Collider {
            half_extents: Vec2::new(w * 0.5 + 20.0, 10.0),
        },
    ));

    // Spawn loot inside: 2-3 breakables that always drop gold
    spawn_breakables(commands, &secret_room, 3);

    // Spawn a free item or heal pedestal
    let is_heal = rng.next_unit() < 0.5;
    let pedestal_pos = secret_room.center();
    let (label, item) = if is_heal {
        ("Heal Orb", PickupKind::Heal)
    } else {
        let item_idx = (rng.next_unit() * 12.0) as usize;
        let kind = match item_idx {
            0 => ItemKind::RustyNails,
            1 => ItemKind::LuckyCharm,
            2 => ItemKind::Featherlight,
            3 => ItemKind::SteelToe,
            4 => ItemKind::Bandage,
            5 => ItemKind::MagnifyingGlass,
            6 => ItemKind::GlassCannon,
            7 => ItemKind::VampireFangs,
            8 => ItemKind::ExplosiveCharge,
            9 => ItemKind::EnergyShield,
            10 => ItemKind::LuckyHorseshoe,
            _ => ItemKind::CursedRing,
        };
        ("Free Item", PickupKind::Item(kind))
    };
    info!("Secret room free loot: {}", label);

    // Pedestal
    commands.spawn((
        Sprite {
            color: PEDESTAL_COLOR,
            custom_size: Some(Vec2::new(PEDESTAL_SIZE * 1.2, PEDESTAL_SIZE * 0.6)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(pedestal_pos.x, pedestal_pos.y - PEDESTAL_SIZE * 0.3, 0.35),
    ));
    commands.spawn((
        Sprite {
            color: TREASURE_ITEM_COLOR,
            custom_size: Some(Vec2::splat(PEDESTAL_SIZE * 0.6)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(pedestal_pos.x, pedestal_pos.y, 0.40),
        TreasurePedestal { item, taken: false },
        Collider {
            half_extents: Vec2::splat(PEDESTAL_SIZE * 0.5),
        },
    ));

    secret_room
}

/// Toggle the secret wall transparency when the player is near it.
pub(crate) fn secret_wall_reveal_system(
    player_query: Query<&Transform, (With<Player>, Without<SecretWall>)>,
    mut secret_walls: Query<(&Transform, &mut Sprite, &SecretWall)>,
) {
    let Ok(player_tf) = player_query.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (wall_tf, mut sprite, secret_wall) in &mut secret_walls {
        let dist = player_pos.distance(wall_tf.translation.truncate());
        if dist < SECRET_REVEAL_DIST {
            if !secret_wall.revealed {
                // Make the wall semi-transparent
                let mut color = sprite.color.to_srgba();
                color.alpha = SECRET_WALL_REVEALED;
                sprite.color = Color::from(color);
                info!("Secret wall revealed!");
            }
        } else if secret_wall.revealed {
            // Make it opaque again
            let mut color = sprite.color.to_srgba();
            color.alpha = SECRET_WALL_OPAQUE;
            sprite.color = Color::from(color);
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────

fn aabb_overlap(a_center: Vec2, a_half: Vec2, b_center: Vec2, b_half: Vec2) -> bool {
    let delta = a_center - b_center;
    delta.x.abs() < a_half.x + b_half.x && delta.y.abs() < a_half.y + b_half.y
}