//! Shop-related logic: spawning shop items and the collect system.
//! Shop rooms have no enemies — the player walks in, buys upgrades,
//! then leaves.

use bevy::prelude::*;
use log::info;

use crate::components::{AppState, Collider, Health, Player, Room, ShopItem, ShopReward, AimIndicator};
use crate::resources::{RunState, RunRng};

use crate::events::RoomClearedEvent;

const SHOP_ITEM_COLOR_BUFF: Color = Color::srgb(0.30, 0.75, 0.95);
const SHOP_ITEM_COLOR_HEAL: Color = Color::srgb(0.30, 0.85, 0.30);
const SHOP_ITEM_SIZE: f32 = 36.0;

// ─── Shop room (Phase 3c) ──────────────────────────────────────────────────

pub fn spawn_shop_items(commands: &mut Commands, room: &Room) {
    let center = room.center();
    let items = [
        (ShopReward::DamageUp, 50, SHOP_ITEM_COLOR_BUFF),
        (ShopReward::FireRateUp, 50, SHOP_ITEM_COLOR_BUFF),
        (ShopReward::SpeedUp, 40, SHOP_ITEM_COLOR_BUFF),
        (ShopReward::HealFull, 30, SHOP_ITEM_COLOR_HEAL),
        (ShopReward::Regen, 60, SHOP_ITEM_COLOR_HEAL),
    ];
    for (i, (reward, cost, color)) in items.iter().enumerate() {
        let angle = i as f32 * std::f32::consts::TAU / items.len() as f32;
        let offset = Vec2::new(angle.cos(), angle.sin()) * room.size().x * 0.25;
        let pos = center + offset;
        commands.spawn((
            Sprite { color: *color, custom_size: Some(Vec2::splat(SHOP_ITEM_SIZE)), ..default() },
            Transform::from_xyz(pos.x, pos.y, 0.4),
            ShopItem { cost: *cost, reward: *reward },
            Collider { half_extents: Vec2::splat(SHOP_ITEM_SIZE * 0.5) },
        ));
    }
}

// ─── Shop collect system ────────────────────────────────────────────────

pub fn shop_collect_system(
    mut commands: Commands,
    mut player_query: Query<(&Transform, &mut Health), (With<Player>, Without<AimIndicator>)>,
    mut run: ResMut<RunState>,
    mut shop_items: Query<(Entity, &Transform, &ShopItem)>,
) {
    let Ok((player_tf, mut health)) = player_query.single_mut() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    let player_half = Vec2::splat(20.0);

    for (entity, tf, item) in &mut shop_items {
        if !aabb_overlap(
            player_pos, player_half,
            tf.translation.truncate(), Vec2::splat(SHOP_ITEM_SIZE * 0.5),
        ) {
            continue;
        }
        if run.gold < item.cost {
            continue;
        }
        run.gold = run.gold.saturating_sub(item.cost);
        match item.reward {
            ShopReward::DamageUp => { run.damage_stacks = (run.damage_stacks + 1).min(8); }
            ShopReward::FireRateUp => { run.fire_rate_stacks = (run.fire_rate_stacks + 1).min(8); }
            ShopReward::SpeedUp => { run.speed_stacks = (run.speed_stacks + 1).min(8); }
            ShopReward::MaxHpUp => {
                health.max += 2.0;
                health.current = (health.current + 2.0).min(health.max);
            }
            ShopReward::Regen => { run.regen_per_sec += 0.5; }
            ShopReward::HealFull => { health.current = health.max; }
            ShopReward::Weapon(_) => {}
        }
        info!("Shop: bought {:?} for {} gold ({} remaining)", item.reward, item.cost, run.gold);
        commands.entity(entity).despawn();
    }
}

fn aabb_overlap(a_center: Vec2, a_half: Vec2, b_center: Vec2, b_half: Vec2) -> bool {
    let delta = a_center - b_center;
    delta.x.abs() < a_half.x + b_half.x && delta.y.abs() < a_half.y + b_half.y
}