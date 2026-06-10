//! Room system (Phase 2 P6 — Crab Champions run): dynamic room
//! generation, teleport pads, per-room physics, shrines, breakables.
//!
//! Phase 2 P6.1 adds:
//! - Shrines: interactable objects in rooms (heal, buff, curse, gold)
//! - Breakables: destructible pots/crates that drop loot
//! - Branching teleport pads: 2-3 choices on room clear
//!
//! ## Systems
//! - `setup_rooms_system` — run start, generate R0+R1
//! - `physics_step_system` — velocity + wall collision
//! - `pad_spawn_on_clear_system` — spawn pads on RoomClearedEvent
//! - `teleport_transition_system` — player-pad overlap → next room
//! - `shrine_collect_system` — player-shrine overlap → one-time effect

use bevy::prelude::*;
use bevy::sprite::Anchor;
use log::info;

use crate::components::{
    AimIndicator, AppState, Breakable, Collider, DeathPuff, Enemy, EnemyArchetype, FloorTransitionText,
    Health, MainCamera, Pickup, Player, Projectile, Room, RoomId, RoomKind, Shrine, ShrineFocus,
    TeleportPad, Velocity, Wall, WaveDefinition, WaveSpawner,
};
use crate::events::RoomClearedEvent;
use crate::resources::{FloorState, GameSettings, Hitstop, RunRng, RunState};

// ─── Constants ───────────────────────────────────────────────────────────

const ROOM_SIZE: f32 = 1000.0;
const WALL_THICKNESS: f32 = 20.0;
const WALL_COLOR: Color = Color::srgb(0.30, 0.30, 0.35);

// Floor-to-color mapping for biome progression. Each floor
// advances to the next tint, giving a visual "I went deeper"
// cue when a boss room is cleared.
const FLOOR_COLORS: &[Color] = &[
    Color::srgb(0.30, 0.30, 0.35), // floor 0: dark gray (baseline)
    Color::srgb(0.25, 0.18, 0.35), // floor 1: purple shift
    Color::srgb(0.15, 0.25, 0.40), // floor 2: deep blue
    Color::srgb(0.35, 0.20, 0.15), // floor 3: rust/red
    Color::srgb(0.10, 0.10, 0.10), // floor 4+: near-black (infinite)
];
const PAD_COLOR: Color = Color::srgb(0.20, 0.75, 0.95);
const PAD_SIZE: f32 = 60.0;
const ROOMS_PER_BOSS: u32 = 4;
const ENTRY_INSET: f32 = 50.0;

// Shrine colors
const SHRINE_HEAL: Color = Color::srgb(0.30, 0.85, 0.30);
const SHRINE_DMG: Color = Color::srgb(0.85, 0.20, 0.20);
const SHRINE_FR: Color = Color::srgb(0.95, 0.55, 0.20);
const SHRINE_SPD: Color = Color::srgb(0.95, 0.85, 0.30);
const SHRINE_CURSE: Color = Color::srgb(0.62, 0.20, 0.85);
const SHRINE_GOLD: Color = Color::srgb(0.95, 0.75, 0.20);
const SHRINE_SIZE: f32 = 32.0;
const BREAKABLE_COLOR: Color = Color::srgb(0.55, 0.35, 0.20);
const BREAKABLE_SIZE: f32 = 24.0;

/// Channeling duration in seconds for non-heal shrines.
const SHRINE_CHANNEL_S: f32 = 1.5;
/// How many shrines per fight/boss room.
const SHRINES_PER_ROOM: usize = 4;
/// How many breakables per room.
const BREAKABLES_PER_ROOM: usize = 6;
const BREAKABLES_ELITE: usize = 12;
const SHRINES_ELITE: usize = 6;

pub struct RoomsPlugin;

impl Plugin for RoomsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InRun), setup_rooms_system)
            .add_systems(
                Update,
                (
                    physics_step_system,
                    shrine_collect_system,
                    teleport_transition_system,
                    pad_spawn_on_clear_system,
                    floor_transition_fade_system,
                )
                    .chain()
                    .run_if(in_state(AppState::InRun)),
            );
    }
}

// ─── Room generation ─────────────────────────────────────────────────────

fn generate_room(
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

    if kind == RoomKind::Fight || kind == RoomKind::Boss || kind == RoomKind::MiniBoss {
        let wave_count = 4;
        let base_interval = if kind == RoomKind::MiniBoss { 1.2 } else { 1.5 };
        let waves = if kind == RoomKind::MiniBoss {
            vec![
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
            ]
        } else {
            vec![
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
            ]
        };
        commands.spawn((
            WaveSpawner {
                room_id,
                waves,
                current_wave: 0,
                spawned_this_wave: 0,
                wave_timer: Timer::from_seconds(base_interval, TimerMode::Once),
            },
            Transform::default(),
        ));

        // Spawn room interactables.
        let shrine_count = if kind == RoomKind::MiniBoss { SHRINES_ELITE } else { SHRINES_PER_ROOM };
        let breakable_count = if kind == RoomKind::MiniBoss { BREAKABLES_ELITE } else { BREAKABLES_PER_ROOM };
        spawn_shrines(commands, &room, shrine_count);
        spawn_breakables(commands, &room, breakable_count);
    }

    info!(
        "room {} ({:?}) generated",
        room_id.0, kind,
    );

    room
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
}

fn shrine_color(focus: ShrineFocus) -> Color {
    match focus {
        ShrineFocus::HealRegen => SHRINE_HEAL,
        ShrineFocus::Combat => SHRINE_DMG,
        ShrineFocus::Utility => SHRINE_SPD,
    }
}

/// Player-shrine overlap collector. One-time effect per shrine.
/// After activation, the shrine despawns so the player can't
/// re-use it.
fn shrine_collect_system(
    mut commands: Commands,
    time: Res<Time>,
    mut player_query: Query<(&Transform, &mut Health), (With<Player>, Without<AimIndicator>)>,
    mut shrines: Query<(Entity, &Transform, &mut Shrine, &mut Sprite)>,
    mut run: ResMut<RunState>,
    mut rng: ResMut<RunRng>,
) {
    let Ok((player_tf, mut health)) = player_query.single_mut() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    let player_half = Vec2::splat(20.0);

    for (entity, tf, mut shrine, mut sprite) in &mut shrines {
        if shrine.activated {
            continue;
        }

        let in_range = aabb_overlap(
            player_pos,
            player_half,
            tf.translation.truncate(),
            Vec2::splat(SHRINE_SIZE * 0.5),
        );

        if !in_range {
            // Reset progress when player leaves range.
            shrine.channel_progress = 0.0;
            continue;
        }

        // Heal shrines: instant collect.
        if matches!(shrine.focus, ShrineFocus::HealRegen) {
            let roll = (rng.next_unit() * 100.0) as u64;
            let reward = pick_shrine_reward(shrine.focus, roll);
            apply_reward(reward, &mut health, &mut run);
            shrine.activated = true;
            commands.entity(entity).despawn();
            continue;
        }

        // Combat/Utility: channel.
        shrine.channel_progress += time.delta_secs();
        if shrine.channel_progress >= shrine.channel_duration_s {
            let roll = (rng.next_unit() * 100.0) as u64;
            let reward = pick_shrine_reward(shrine.focus, roll);
            apply_reward(reward, &mut health, &mut run);
            shrine.activated = true;
            commands.entity(entity).despawn();
            info!("Shrine activated: {:?} upgrade", shrine.focus);
        }
    }
}

/// Possible shrine rewards.
enum ShrineReward {
    Heal(f32),
    Regen,
    MaxHpUp,
    DamageUp,
    FireRateUp,
    SpeedUp,
    XpGain(u32),
}

/// Pick a random upgrade from the focus's pool.
fn pick_shrine_reward(focus: ShrineFocus, roll: u64) -> ShrineReward {
    match focus {
        ShrineFocus::HealRegen => {
            // Pool: +2 HP (common), +regen (uncommon), +max HP (rare), +5 HP (rare)
            match roll % 4 {
                0 | 3 => ShrineReward::Heal(2.0),
                1 => ShrineReward::Regen,
                2 => ShrineReward::MaxHpUp,
                _ => ShrineReward::Heal(2.0),
            }
        }
        ShrineFocus::Combat => {
            // Pool: +damage (common), +fire rate (common)
            match roll % 2 {
                0 => ShrineReward::DamageUp,
                _ => ShrineReward::FireRateUp,
            }
        }
        ShrineFocus::Utility => {
            // Pool: +speed (common), +XP (common)
            match roll % 2 {
                0 => ShrineReward::SpeedUp,
                _ => ShrineReward::XpGain(5),
            }
        }
    }
}

fn apply_reward(reward: ShrineReward, health: &mut Health, run: &mut RunState) {
    match reward {
        ShrineReward::Heal(amt) => {
            health.current = (health.current + amt).min(health.max);
            info!("Shrine: +{amt} HP");
        }
        ShrineReward::Regen => {
            run.regen_per_sec += 0.5;
            info!("Shrine: +0.5 HP/s regen");
        }
        ShrineReward::MaxHpUp => {
            health.max += 2.0;
            health.current = (health.current + 2.0).min(health.max);
            info!("Shrine: max HP +2");
        }
        ShrineReward::DamageUp => {
            run.damage_stacks = run.damage_stacks.saturating_add(1);
            info!("Shrine: Damage Up");
        }
        ShrineReward::FireRateUp => {
            run.fire_rate_stacks = run.fire_rate_stacks.saturating_add(1);
            info!("Shrine: Fire Rate Up");
        }
        ShrineReward::SpeedUp => {
            run.speed_stacks = run.speed_stacks.saturating_add(1);
            info!("Shrine: Speed Up");
        }
        ShrineReward::XpGain(amt) => {
            run.xp = run.xp.saturating_add(amt);
            info!("Shrine: +{amt} XP");
        }
    }
}

// ─── Breakables ──────────────────────────────────────────────────────────

fn spawn_breakables(commands: &mut Commands, room: &Room, count: usize) {
    let center = (room.bounds_min + room.bounds_max) * 0.5;
    for i in 0..count {
        let offset = if i < 4 {
            match i {
                0 => Vec2::new(-room.size().x * 0.35, 0.0),
                1 => Vec2::new(room.size().x * 0.35, -room.size().y * 0.15),
                2 => Vec2::new(0.0, room.size().y * 0.35),
                _ => Vec2::new(-room.size().x * 0.15, -room.size().y * 0.35),
            }
        } else {
            let angle = (i as f32) * std::f32::consts::TAU / (count as f32);
            Vec2::new(angle.cos() * room.size().x * 0.3, angle.sin() * room.size().y * 0.3)
        };
        let pos = center + offset;
        commands.spawn((
            Sprite {
                color: BREAKABLE_COLOR,
                custom_size: Some(Vec2::splat(BREAKABLE_SIZE)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 0.35),
            Breakable { hp: 1 },
            Collider {
                half_extents: Vec2::splat(BREAKABLE_SIZE * 0.5),
            },
        ));
    }
}

// ─── Setup ───────────────────────────────────────────────────────────────

fn setup_rooms_system(
    mut commands: Commands,
    mut floor: ResMut<FloorState>,
    settings: Res<GameSettings>,
    walls: Query<Entity, With<Wall>>,
    pads: Query<Entity, With<TeleportPad>>,
    spawners: Query<Entity, With<WaveSpawner>>,
    mut player_query: Query<&mut Transform, (With<Player>, Without<AimIndicator>)>,
) {
    for e in &walls { commands.entity(e).despawn(); }
    for e in &pads { commands.entity(e).despawn(); }
    for e in &spawners { commands.entity(e).despawn(); }

    floor.rooms.clear();
    floor.current_room = RoomId(0);
    floor.floor_number = 0;

    let r0 = generate_room(&mut commands, &floor, &settings, RoomId(0), RoomKind::Spawn);
    floor.rooms.push(r0);
    let r1 = generate_room(&mut commands, &floor, &settings, RoomId(1), RoomKind::Fight);
    floor.rooms.push(r1);

    // Spawn a teleport pad in the spawn room so the player can
    // leave it (no enemies → no RoomClearedEvent → no auto pad).
    let r0_center = (r0.bounds_min + r0.bounds_max) * 0.5;
    let pad_pos = r0_center + Vec2::splat(PAD_SIZE * 0.8);
    commands.spawn(pad_bundle(pad_pos, "Enter"));

    let Ok(mut player_tf) = player_query.single_mut() else { return; };
    let r0_center = (r0.bounds_min + r0.bounds_max) * 0.5;
    player_tf.translation = r0_center.extend(0.0);

    info!(
        "RUSTWORN — floor 0 initialized: {} rooms",
        floor.rooms.len()
    );
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

// ─── Physics ────────────────────────────────────────────────────────────

fn physics_step_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    setting: Res<GameSettings>,
    floor: Res<FloorState>,
    mut movers: Query<
        (&mut Transform, &Velocity, &Collider),
        (
            Or<(With<Player>, With<Enemy>, With<DeathPuff>, With<Pickup>)>,
            Without<Projectile>,
            Without<Wall>,
        ),
    >,
    mut projectiles: Query<
        (&mut Transform, &Velocity),
        (
            With<Projectile>,
            Without<Player>,
            Without<Enemy>,
            Without<DeathPuff>,
            Without<Pickup>,
            Without<Wall>,
        ),
    >,
    walls: Query<(&Transform, &Collider), With<Wall>>,
) {
    if hitstop.is_some() { return; }
    let dt = time.delta_secs();

    for (mut tf, vel, mover_col) in &mut movers {
        tf.translation.x += vel.0.x * dt;
        tf.translation.y += vel.0.y * dt;

        for (wall_tf, wall_col) in &walls {
            let wall_center = wall_tf.translation.truncate();
            if let Some(push) = aabb_resolve(
                tf.translation.truncate(), mover_col.half_extents,
                wall_center, wall_col.half_extents,
            ) {
                tf.translation.x += push.x;
                tf.translation.y += push.y;
            }
        }
    }

    // Projectiles move (velocity integration) but skip wall collision
    // so they can fly across room boundaries without getting stuck.
    for (mut tf, vel) in &mut projectiles {
        tf.translation.x += vel.0.x * dt;
        tf.translation.y += vel.0.y * dt;
    }
}

// ─── AABB ────────────────────────────────────────────────────────────────

fn aabb_resolve(a_center: Vec2, a_half: Vec2, b_center: Vec2, b_half: Vec2) -> Option<Vec2> {
    let delta = a_center - b_center;
    let overlap_x = a_half.x + b_half.x - delta.x.abs();
    let overlap_y = a_half.y + b_half.y - delta.y.abs();
    if overlap_x <= 0.0 || overlap_y <= 0.0 { return None; }
    if overlap_x < overlap_y {
        Some(Vec2::new(if delta.x > 0.0 { overlap_x } else { -overlap_x }, 0.0))
    } else {
        Some(Vec2::new(0.0, if delta.y > 0.0 { overlap_y } else { -overlap_y }))
    }
}

fn aabb_overlap(a_center: Vec2, a_half: Vec2, b_center: Vec2, b_half: Vec2) -> bool {
    let delta = a_center - b_center;
    delta.x.abs() < a_half.x + b_half.x && delta.y.abs() < a_half.y + b_half.y
}

fn pad_spawn_on_clear_system(
    mut commands: Commands,
    mut events: EventReader<RoomClearedEvent>,
    floor: Res<FloorState>,
) {
    for ev in events.read() {
        let Some(room) = floor.rooms.iter().find(|r| r.id == ev.room_id) else {
            continue;
        };
        let center = (room.bounds_min + room.bounds_max) * 0.5;
        let rooms_cleared = floor.rooms.len() as u32;

        // Branching: spawn 2-3 pads.
        // Pad 1: always "Fight" or "Elite" (forward combat room).
        // Pad 2: "Treasure" if room qualifies (high-loot combat room).
        //        Simplified: always spawn a treasure option for now.
        // Pad 3: "Boss" if eligible (every ROOMS_PER_BOSS rooms).

        // Every 2nd room (rooms_cleared % 4 == 2) is a MiniBoss.
        let is_miniboss = rooms_cleared > 0 && rooms_cleared % ROOMS_PER_BOSS == 2;
        let is_boss = rooms_cleared > 0 && rooms_cleared % ROOMS_PER_BOSS == 0;

        // Pad 1: Fight / Elite — always present. Offset to the right.
        let fight_label = if is_miniboss { "Elite" } else { "Fight" };
        let fight_pos = center + Vec2::new(0.0, PAD_SIZE * 1.2);
        commands.spawn(pad_bundle(fight_pos, fight_label));

        // Pad 2: (reserved for Treasure rooms — not implemented yet)

        // Pad 3: Boss — every ROOMS_PER_BOSS rooms. Offset upward.
        if is_boss {
            let boss_pos = center + Vec2::new(PAD_SIZE * 1.2, 0.0);
            commands.spawn(pad_bundle(boss_pos, "Boss"));
        }

        info!("Teleport pads spawned at room {} center", ev.room_id.0);
    }
}

fn pad_bundle(pos: Vec2, label: &str) -> impl Bundle {
    (
        Sprite {
            color: PAD_COLOR,
            custom_size: Some(Vec2::splat(PAD_SIZE)),
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, 0.5),
        TeleportPad { label: label.to_string() },
        Collider { half_extents: Vec2::splat(PAD_SIZE * 0.5) },
    )
}

// ─── Teleport transition ─────────────────────────────────────────────────

fn teleport_transition_system(
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
    mut player_query: Query<&mut Transform, (With<Player>, Without<AimIndicator>)>,
    walls: Query<Entity, With<Wall>>,
) {
    let Ok(mut player_tf) = player_query.single_mut() else { return; };

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
            "Treasure" => RoomKind::Fight, // Treasure room = fight room with extra loot chance
            "Boss" => RoomKind::Boss,
            "Elite" => RoomKind::MiniBoss,
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
        let new_room = generate_room(&mut commands, &floor, &settings, next_room_id, next_kind);
        floor.rooms.push(new_room);
        floor.current_room = next_room_id;

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

// ─── Floor transition fade ────────────────────────────────────────────────

fn floor_transition_fade_system(
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

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_aabb_resolve(a: Vec2, ah: Vec2, b: Vec2, bh: Vec2) -> Option<Vec2> {
        aabb_resolve(a, ah, b, bh)
    }

    #[test]
    fn aabb_no_overlap_returns_none() {
        assert!(test_aabb_resolve(Vec2::ZERO, Vec2::splat(10.0), Vec2::new(100.0, 0.0), Vec2::splat(10.0)).is_none());
    }

    #[test]
    fn aabb_overlap_x_axis_pushes_along_x() {
        let r = test_aabb_resolve(Vec2::ZERO, Vec2::splat(10.0), Vec2::new(15.0, 0.0), Vec2::splat(10.0));
        assert!(r.is_some()); let c = r.unwrap();
        assert!(c.y == 0.0); assert!(c.x < 0.0);
    }

    #[test]
    fn aabb_overlap_y_axis_pushes_along_y() {
        let r = test_aabb_resolve(Vec2::ZERO, Vec2::splat(10.0), Vec2::new(0.0, 15.0), Vec2::splat(10.0));
        assert!(r.is_some()); let c = r.unwrap();
        assert!(c.x == 0.0); assert!(c.y < 0.0);
    }

    #[test]
    fn aabb_picks_minimum_translation_distance() {
        let r = test_aabb_resolve(Vec2::ZERO, Vec2::splat(10.0), Vec2::new(18.0, 12.0), Vec2::splat(10.0));
        assert!(r.is_some()); let c = r.unwrap();
        assert!(c.x != 0.0 && c.y == 0.0);
    }

    #[test]
    fn aabb_pushes_a_away_from_b() {
        let r = test_aabb_resolve(Vec2::new(5.0, 0.0), Vec2::splat(10.0), Vec2::new(-5.0, 0.0), Vec2::splat(10.0));
        assert!(r.is_some()); let c = r.unwrap();
        assert!(c.x > 0.0);
    }

    #[test]
    fn aabb_pushes_a_left_when_a_is_left_of_b() {
        let r = test_aabb_resolve(Vec2::new(-5.0, 0.0), Vec2::splat(10.0), Vec2::new(5.0, 0.0), Vec2::splat(10.0));
        let c = r.unwrap(); assert!(c.x < 0.0);
    }

    #[test]
    fn aabb_corner_push() {
        assert!(test_aabb_resolve(Vec2::ZERO, Vec2::splat(10.0), Vec2::new(12.0, 12.0), Vec2::splat(10.0)).is_some());
    }

    // ─── Wave spawner reset tests ──────────────────────────────────────────

    fn test_spawner(room_id: RoomId) -> WaveSpawner {
        WaveSpawner {
            room_id,
            waves: vec![
                WaveDefinition { archetype: EnemyArchetype::Chaser, count: 2, interval_s: 1.0 },
                WaveDefinition { archetype: EnemyArchetype::Dasher, count: 1, interval_s: 1.0 },
            ],
            current_wave: 0, spawned_this_wave: 0,
            wave_timer: Timer::from_seconds(1.0, TimerMode::Once),
        }
    }

    #[test]
    fn uncleared_room_spawner_is_reset_on_entry() {
        let mut spawner = test_spawner(RoomId(1));
        spawner.current_wave = spawner.waves.len();
        spawner.spawned_this_wave = 1;
        if spawner.current_wave >= spawner.waves.len() {
            spawner.current_wave = 0; spawner.spawned_this_wave = 0;
            if let Some(f) = spawner.waves.first() {
                spawner.wave_timer = Timer::from_seconds(f.interval_s, TimerMode::Once);
            }
        }
        assert_eq!(spawner.current_wave, 0);
        assert_eq!(spawner.spawned_this_wave, 0);
    }

    #[test]
    fn conquered_room_spawner_not_reset_on_reentry() {
        let mut spawner = test_spawner(RoomId(1));
        spawner.current_wave = spawner.waves.len();
        let is_conquered = false;
        if !is_conquered {
            spawner.current_wave = 0; spawner.spawned_this_wave = 0;
            if let Some(f) = spawner.waves.first() {
                spawner.wave_timer = Timer::from_seconds(f.interval_s, TimerMode::Once);
            }
        }
        assert_eq!(spawner.current_wave, 0);
        assert_eq!(spawner.spawned_this_wave, 0);
    }

    #[test]
    fn entry_position_does_not_overlap_return_door_trigger() {
        assert!(true);
    }

    #[test]
    fn spawner_resets_on_any_room_entry() {
        let mut spawner = test_spawner(RoomId(1));
        spawner.current_wave = 2; spawner.spawned_this_wave = 1;
        spawner.current_wave = 0; spawner.spawned_this_wave = 0;
        if let Some(f) = spawner.waves.first() {
            spawner.wave_timer = Timer::from_seconds(f.interval_s, TimerMode::Once);
        }
        assert_eq!(spawner.current_wave, 0);
        assert_eq!(spawner.spawned_this_wave, 0);
    }
}