//! Room generation (the big match on RoomKind) — creates walls,
//! wave spawners, shrines, breakables, and shop items for each room type.
//! Called by `setup_rooms_system` and `teleport_transition_system`.

use bevy::prelude::*;
use log::info;

use crate::components::{
    Breakable, Collider, EnemyArchetype, Room, RoomId, RoomKind, Shrine, ShrineFocus, Wall,
    WaveDefinition, WaveSpawner,
};
use crate::resources::{FloorState, GameSettings};

const ROOM_SIZE: f32 = 1000.0;
const WALL_THICKNESS: f32 = 20.0;
const WALL_COLOR: Color = Color::srgb(0.30, 0.30, 0.35);

/// Floor-to-color mapping for biome progression.
const FLOOR_COLORS: &[Color] = &[
    Color::srgb(0.30, 0.30, 0.35),
    Color::srgb(0.25, 0.18, 0.35),
    Color::srgb(0.15, 0.25, 0.40),
    Color::srgb(0.35, 0.20, 0.15),
    Color::srgb(0.10, 0.10, 0.10),
];

const SHRINE_HEAL: Color = Color::srgb(0.30, 0.85, 0.30);
const SHRINE_DMG: Color = Color::srgb(0.85, 0.20, 0.20);
const SHRINE_SPD: Color = Color::srgb(0.95, 0.55, 0.20);
const SHRINE_CURSE: Color = Color::srgb(0.62, 0.20, 0.85);
const SHRINE_GOLD: Color = Color::srgb(0.95, 0.75, 0.20);
const SHRINE_SIZE: f32 = 32.0;
const CURSED_SHRINE_SIZE: f32 = 40.0;
const CURSED_SHRINE_COLOR: Color = Color::srgb(0.55, 0.15, 0.75);
const SHRINE_CHANNEL_S: f32 = 1.5;
const SHRINES_PER_ROOM: usize = 4;
const SHRINES_ELITE: usize = 6;
const BREAKABLE_COLOR: Color = Color::srgb(0.55, 0.35, 0.20);
const BREAKABLE_SIZE: f32 = 24.0;
const BREAKABLES_PER_ROOM: usize = 6;
const BREAKABLES_ELITE: usize = 12;
const BREAKABLES_HORDE: usize = 16;

const PAD_SIZE: f32 = 60.0;
const PAD_COLOR: Color = Color::srgb(0.20, 0.75, 0.95);
const PAD_COLOR_FIGHT: Color = Color::srgb(0.20, 0.75, 0.95);
const PAD_COLOR_ELITE: Color = Color::srgb(0.75, 0.25, 0.85);
const PAD_COLOR_BOSS: Color = Color::srgb(0.85, 0.15, 0.15);
const PAD_COLOR_TREASURE: Color = Color::srgb(0.95, 0.75, 0.20);
const PAD_COLOR_HORDE: Color = Color::srgb(0.20, 0.85, 0.60);
const PAD_COLOR_ENTER: Color = Color::srgb(0.30, 0.85, 0.30);
const PAD_COLOR_SHOP: Color = Color::srgb(0.95, 0.85, 0.10);
const ROOMS_PER_BOSS: u32 = 6;
const ENTRY_INSET: f32 = 50.0;

// ─── Room generation ─────────────────────────────────────────────────────

pub fn generate_room(
    commands: &mut Commands,
    floor: &FloorState,
    settings: &GameSettings,
    room_id: RoomId,
    kind: RoomKind,
) -> Room {
    let offset_x = room_id.0 as f32 * ROOM_SIZE;
    let bounds_min = Vec2::new(offset_x, -ROOM_SIZE);
    let bounds_max = Vec2::new(offset_x + ROOM_SIZE, 0.0);
    let room = Room {
        id: room_id,
        kind,
        bounds_min,
        bounds_max,
    };

    spawn_walls(commands, &room, floor.floor_number);

    // Non-spawn rooms get enemies + interactables.
    if kind != RoomKind::Spawn {
        let (waves, breakable_count, shrine_count) = match kind {
            RoomKind::MiniBoss | RoomKind::Boss => {
                // Tougher / mini-boss waves.
                let base_interval = 1.2;
                let waves = vec![
                    WaveDefinition {
                        archetype: EnemyArchetype::Chaser,
                        count: (6 + floor.floor_number).min(14),
                        interval_s: base_interval * 0.5,
                    },
                    WaveDefinition {
                        archetype: EnemyArchetype::Dasher,
                        count: (3 + floor.floor_number / 2).min(10),
                        interval_s: base_interval * 0.4,
                    },
                    WaveDefinition {
                        archetype: EnemyArchetype::Shooter,
                        count: (3 + floor.floor_number / 2).min(10),
                        interval_s: base_interval * 0.4,
                    },
                    WaveDefinition {
                        archetype: EnemyArchetype::Sniper,
                        count: (2 + floor.floor_number / 3).min(7),
                        interval_s: base_interval * 0.4,
                    },
                ];
                (waves, BREAKABLES_ELITE, SHRINES_ELITE)
            }
            RoomKind::Horde => {
                // Many weak enemies, fast intervals. Chaser/dasher heavy.
                let base_interval = 0.5;
                let waves = vec![
                    WaveDefinition {
                        archetype: EnemyArchetype::Chaser,
                        count: (8 + floor.floor_number * 2).min(24),
                        interval_s: base_interval * 1.2,
                    },
                    WaveDefinition {
                        archetype: EnemyArchetype::Dasher,
                        count: (4 + floor.floor_number).min(12),
                        interval_s: base_interval * 1.0,
                    },
                    WaveDefinition {
                        archetype: EnemyArchetype::Chaser,
                        count: (8 + floor.floor_number * 2).min(24),
                        interval_s: base_interval * 0.8,
                    },
                    WaveDefinition {
                        archetype: EnemyArchetype::Shooter,
                        count: (2 + floor.floor_number / 2).min(6),
                        interval_s: base_interval * 1.0,
                    },
                ];
                (waves, BREAKABLES_HORDE, SHRINES_PER_ROOM)
            }
            RoomKind::Shop => {
                // Shop rooms have no enemies — player walks in, buys
                // upgrades with gold, then leaves. Spawn breakables
                // so the player has a chance to earn a bit more gold.
                (vec![], BREAKABLES_PER_ROOM, 0)
            }
            _ => {
                // Fight room — standard combat.
                let base_interval = 1.5;
                let waves = vec![
                    WaveDefinition {
                        archetype: EnemyArchetype::Chaser,
                        count: (4 + floor.floor_number).min(10),
                        interval_s: base_interval,
                    },
                    WaveDefinition {
                        archetype: EnemyArchetype::Dasher,
                        count: (2 + floor.floor_number / 2).min(8),
                        interval_s: base_interval * 0.7,
                    },
                    WaveDefinition {
                        archetype: EnemyArchetype::Shooter,
                        count: (2 + floor.floor_number / 2).min(8),
                        interval_s: base_interval * 0.7,
                    },
                    WaveDefinition {
                        archetype: EnemyArchetype::Sniper,
                        count: (1 + floor.floor_number / 3).min(5),
                        interval_s: base_interval * 0.35,
                    },
                ];
                (waves, BREAKABLES_PER_ROOM, SHRINES_PER_ROOM)
            }
        };

        // Per-room-type farming interval (seconds between enemies).
        // Lower = faster spawns for a more frantic horde feel.
        let farming_interval = match kind {
            RoomKind::Horde => 0.25,
            RoomKind::MiniBoss | RoomKind::Boss => 0.30,
            _ => 0.35,
        };
        // Clear delay: how many seconds before RoomClearedEvent fires
        // and teleport pads appear. Boss rooms gate on boss death
        // instead (set delay to 0, boss_death_room_clear_system handles it).
        let clear_delay = if kind == RoomKind::Boss { 0.0 } else { 60.0 };
        commands.spawn((
            WaveSpawner {
                room_id,
                waves,
                current_wave: 0,
                spawned_this_wave: 0,
                wave_timer: Timer::from_seconds(0.2, TimerMode::Once), // quick first spawn
                farming_interval,
                farming_elapsed: 0.0,
                clear_delay,
                director_credits: 8.0,
            },
            Transform::default(),
        ));

        // Spawn room interactables.
        super::spawn_shrines(commands, &room, shrine_count);
        super::spawn_breakables(commands, &room, breakable_count);

        if kind == RoomKind::Shop {
            // Shop rooms: no enemies, no WaveSpawner. Spawn items
            // the player can buy with gold, plus a Boss exit pad.
            // Use super::shop module for shop item spawning.
            super::shop::spawn_shop_items(commands, &room);

            // Boss exit pad near the top of the room.
            let exit_pos = Vec2::new(
                room.center().x,
                room.bounds_max.y - PAD_SIZE * 1.5,
            );
            commands.spawn(super::pads::pad_bundle(exit_pos, "Boss"));
        }
    }

    info!("room {} ({:?}) generated", room_id.0, kind);

    room
}

// ─── Wall spawning ──────────────────────────────────────────────────────

fn spawn_walls(commands: &mut Commands, room: &Room, floor_number: u32) {
    let cx = (room.bounds_min.x + room.bounds_max.x) * 0.5;
    let cy = (room.bounds_min.y + room.bounds_max.y) * 0.5;
    let w = room.size().x;
    let h = room.size().y;
    let half = WALL_THICKNESS * 0.5;
    // Pick wall color from the floor palette.
    let color = FLOOR_COLORS[(floor_number as usize).min(FLOOR_COLORS.len() - 1)];

    let walls_data = [
        (Vec3::new(cx, room.bounds_max.y + half, 0.0), Vec2::new(w + WALL_THICKNESS * 2.0, WALL_THICKNESS)),
        (Vec3::new(cx, room.bounds_min.y - half, 0.0), Vec2::new(w + WALL_THICKNESS * 2.0, WALL_THICKNESS)),
        (Vec3::new(room.bounds_max.x + half, cy, 0.0), Vec2::new(WALL_THICKNESS, h)),
        (Vec3::new(room.bounds_min.x - half, cy, 0.0), Vec2::new(WALL_THICKNESS, h)),
    ];

    for (pos, size) in walls_data {
        commands.spawn((
            Sprite { color, custom_size: Some(size), ..default() },
            Transform::from_translation(pos),
            Wall,
            Collider { half_extents: size * 0.5 },
        ));
    }
}

// ─── Shrines ─────────────────────────────────────────────────────────────

fn spawn_shrines(commands: &mut Commands, room: &Room, count: usize) {
    let center = (room.bounds_min + room.bounds_max) * 0.5;
    let focuses = [
        ShrineFocus::HealRegen,
        ShrineFocus::Combat,
        ShrineFocus::Utility,
        ShrineFocus::Combat, // 4th shrine: another combat for variety
    ];
    for i in 0..count {
        let focus = focuses[i.min(focuses.len() - 1)];
        let color = shrine_color(focus);
        let offset = if i < 4 {
            match i {
                0 => Vec2::new(-room.size().x * 0.3, room.size().y * 0.2),
                1 => Vec2::new(room.size().x * 0.25, -room.size().y * 0.25),
                2 => Vec2::new(-room.size().x * 0.2, -room.size().y * 0.3),
                _ => Vec2::new(room.size().x * 0.3, room.size().y * 0.25),
            }
        } else {
            let angle = (i as f32) * std::f32::consts::TAU / (count as f32);
            Vec2::new(angle.cos() * room.size().x * 0.28, angle.sin() * room.size().y * 0.28)
        };
        let pos = center + offset;
        let channel = if matches!(focus, ShrineFocus::HealRegen) {
            0.0
        } else {
            SHRINE_CHANNEL_S
        };
        commands.spawn((
            Sprite { color, custom_size: Some(Vec2::splat(SHRINE_SIZE)), ..default() },
            Transform::from_xyz(pos.x, pos.y, 0.4),
            Shrine {
                focus,
                channel_duration_s: channel,
                channel_progress: 0.0,
                activated: false,
            },
            Collider { half_extents: Vec2::splat(SHRINE_SIZE * 0.5) },
        ));
    }

    // Spawn one cursed shrine in non-spawn rooms.
    // Cursed shrines are opt-in: distinct purple color, slightly
    // larger, positioned near the room entrance for visibility.
    if room.kind != RoomKind::Spawn {
        let cursed_pos = center + Vec2::new(0.0, room.size().y * 0.1);
        commands.spawn((
            Sprite {
                color: CURSED_SHRINE_COLOR,
                custom_size: Some(Vec2::splat(CURSED_SHRINE_SIZE)),
                ..default()
            },
            Transform::from_xyz(cursed_pos.x, cursed_pos.y, 0.4),
            Shrine {
                focus: ShrineFocus::Cursed,
                channel_duration_s: SHRINE_CHANNEL_S,
                channel_progress: 0.0,
                activated: false,
            },
            Collider {
                half_extents: Vec2::splat(CURSED_SHRINE_SIZE * 0.5),
            },
        ));
        info!("Cursed shrine spawned at room center-bottom");
    }
}

fn shrine_color(focus: ShrineFocus) -> Color {
    match focus {
        ShrineFocus::HealRegen => SHRINE_HEAL,
        ShrineFocus::Combat => SHRINE_DMG,
        ShrineFocus::Utility => SHRINE_SPD,
        ShrineFocus::Cursed => CURSED_SHRINE_COLOR,
    }
}