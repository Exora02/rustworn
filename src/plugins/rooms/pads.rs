//! Teleport pad logic: spawning pads on room clear and generating
//! pad bundles with room-type-appropriate colors.

use bevy::prelude::*;
use log::info;

use crate::components::{
    AppState, Collider, Room, RoomId, RoomKind, TeleportPad,
};
use crate::events::RoomClearedEvent;
use crate::resources::{FloorState, RunRng};

const PAD_COLOR_BOSS: Color = Color::srgb(0.85, 0.15, 0.15);
const PAD_COLOR_EXTRACT: Color = Color::srgb(0.15, 0.90, 0.40); // bright emerald green
const PAD_COLOR_ELITE: Color = Color::srgb(0.75, 0.25, 0.85);
const PAD_COLOR_TREASURE: Color = Color::srgb(0.95, 0.75, 0.20);
const PAD_COLOR_HORDE: Color = Color::srgb(0.20, 0.85, 0.60);
const PAD_COLOR_ENTER: Color = Color::srgb(0.30, 0.85, 0.30);
const PAD_COLOR_SHOP: Color = Color::srgb(0.95, 0.85, 0.10);
const PAD_COLOR_FIGHT: Color = Color::srgb(0.20, 0.75, 0.95);
const PAD_SIZE: f32 = 60.0;
const ROOMS_PER_BOSS: u32 = 6;

// ─── Pad spawn on clear ─────────────────────────────────────────────────

pub fn pad_spawn_on_clear_system(
    mut commands: Commands,
    mut events: EventReader<RoomClearedEvent>,
    floor: Res<FloorState>,
    mut rng: ResMut<RunRng>,
) {
    for ev in events.read() {
        let Some(room) = floor.rooms.iter().find(|r| r.id == ev.room_id) else {
            continue;
        };
        let center = (room.bounds_min + room.bounds_max) * 0.5;
        let rooms_cleared = floor.rooms.len() as u32;

        // 6-room cycle: positions 1-5 choose room type, position 6 is boss.
        let cycle_pos = (rooms_cleared - 1) % ROOMS_PER_BOSS;

        let fight_label = match cycle_pos {
            2 => "Horde",
            4 => "Elite",
            5 => "Shop",
            _ => "Fight",
        };
        let next_is_boss = cycle_pos == 0;

        // Pad 1: always spawn a forward combat room.
        let fight_pos = center + Vec2::new(0.0, PAD_SIZE * 1.2);
        commands.spawn(pad_bundle(fight_pos, fight_label));

        // Pad 2: branching choice (50% nothing, 30% Elite, 20% Treasure).
        let branch_roll = rng.next_unit();
        let branch_label = if branch_roll < 0.5 {
            None
        } else if branch_roll < 0.8 {
            Some("Elite")
        } else {
            Some("Treasure")
        };
        if let Some(label) = branch_label {
            let branch_pos = center + Vec2::new(0.0, -PAD_SIZE * 1.2);
            commands.spawn(pad_bundle(branch_pos, label));
        }

        // Pad 3: Boss — every 6th room.
        if next_is_boss {
            let boss_pos = center + Vec2::new(PAD_SIZE * 1.2, 0.0);
            commands.spawn(pad_bundle(boss_pos, "Boss"));
        }

        // Pad 4: Extract — always appears on boss rooms after the
        // first boss kill (floor > 0). Player can step on it to
        // end the run with rewards.
        if next_is_boss && floor.floor_number > 0 {
            let extract_pos = center + Vec2::new(-PAD_SIZE * 1.2, 0.0);
            commands.spawn(pad_bundle(extract_pos, "Extract"));
        }

        info!("Teleport pads spawned at room {} center", ev.room_id.0);
    }
}

pub fn pad_bundle(pos: Vec2, label: &str) -> impl Bundle {
    let color = match label {
        "Boss" => PAD_COLOR_BOSS,
        "Extract" => PAD_COLOR_EXTRACT,
        "Elite" => PAD_COLOR_ELITE,
        "Treasure" => PAD_COLOR_TREASURE,
        "Horde" => PAD_COLOR_HORDE,
        "Enter" => PAD_COLOR_ENTER,
        "Shop" => PAD_COLOR_SHOP,
        _ => PAD_COLOR_FIGHT,
    };
    (
        Sprite { color, custom_size: Some(Vec2::splat(PAD_SIZE)), ..default() },
        Transform::from_xyz(pos.x, pos.y, 0.5),
        TeleportPad { label: label.to_string() },
        Collider { half_extents: Vec2::splat(PAD_SIZE * 0.5) },
    )
}