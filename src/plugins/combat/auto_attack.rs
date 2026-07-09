//! Auto-attack system (Vampire Survivors fusion).
//!
//! When the player has 2+ weapons in their inventory, each non-active
//! weapon slot automatically fires projectiles toward the nearest enemy
//! on its own cooldown. The active weapon slot is skipped — it's already
//! handled by `weapon_fire_system`.
//!
//! Runs after `weapon_fire_system` in the combat chain. Gated on the
//! same InRun + !hitstop conditions as the rest of the combat chain.

use bevy::prelude::*;

use crate::components::{
    AutoAttackCooldowns, Collider, Enemy, Player, Projectile, Team, Velocity, Weapon,
    WeaponArchetype, WeaponInventory,
};
use crate::plugins::enemy::GameSprites;
use crate::resources::{GameSettings, Hitstop, RunState};

use super::rotate_vec2;
use super::weapon::weapon_sprite_for;
use super::{in_hitstop, weapon_stats_for, PROJECTILE_COLOR};

/// Auto-attack system.
///
/// Reads `WeaponInventory` from the player, finds the nearest enemy,
/// and fires projectiles from each non-active weapon slot on its own
/// cooldown cadence.
#[allow(clippy::too_many_arguments)]
pub fn auto_attack_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    run: Res<RunState>,
    hitstop: Option<Res<Hitstop>>,
    mut commands: Commands,
    player_q: Query<(
        Entity,
        &Transform,
        &Weapon,
        &WeaponInventory,
        Option<&AutoAttackCooldowns>,
    ), With<Player>>,
    mut cooldown_q: Query<&mut AutoAttackCooldowns, With<Player>>,
    enemies: Query<&Transform, (With<Enemy>, Without<Player>)>,
    sprites: Res<GameSprites>,
) {
    if in_hitstop(&hitstop) {
        return;
    }

    let Ok((player_entity, player_tf, active_weapon, inventory, cooldowns_read)) =
        player_q.single()
    else {
        return;
    };

    let player_pos = player_tf.translation.truncate();

    // Count owned weapons. If < 2, remove cooldowns and bail.
    let owned_count = inventory.slots.iter().filter(|s| s.is_some()).count();
    if owned_count < 2 {
        if cooldowns_read.is_some() {
            commands.entity(player_entity).remove::<AutoAttackCooldowns>();
        }
        return;
    }

    // Insert AutoAttackCooldowns if not present.
    if cooldowns_read.is_none() {
        commands
            .entity(player_entity)
            .insert(AutoAttackCooldowns::default());
        return; // Insert applies next frame — bail so cooldown_q sees it then.
    }

    // Now fetch the cooldowns mutably via the dedicated query.
    let Ok(mut cooldowns) = cooldown_q.single_mut() else {
        return;
    };

    // Find the nearest enemy.
    let nearest = enemies
        .iter()
        .map(|e_tf| {
            let d = e_tf.translation.truncate().distance_squared(player_pos);
            (d, e_tf.translation.truncate())
        })
        .min_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let Some((_dist, target_pos)) = nearest else {
        return;
    };

    let aim_dir = (target_pos - player_pos).normalize_or_zero();
    if aim_dir == Vec2::ZERO {
        return;
    }

    // Tick all cooldowns.
    for cd in &mut cooldowns.cooldowns {
        cd.tick(time.delta());
    }

    // Fire each non-active weapon slot whose cooldown is ready.
    for (slot_idx, slot) in inventory.slots.iter().enumerate() {
        let Some(archetype) = slot else {
            continue;
        };

        // Skip the active weapon slot.
        if slot_idx == inventory.active_slot {
            continue;
        }
        // Also skip if the archetype matches the active weapon (defensive).
        if *archetype == active_weapon.archetype {
            continue;
        }

        if !cooldowns.cooldowns[slot_idx].finished() {
            continue;
        }

        // Look up weapon stats for this archetype.
        let (damage, fire_rate, projectile_speed, pellet_count, spread_rad, _burst_rounds) =
            weapon_stats_for(*archetype);

        // Auto-attack fires at half the weapon's fire rate.
        let interval = 1.0 / (fire_rate * 0.5);
        cooldowns.cooldowns[slot_idx] = Timer::from_seconds(interval, TimerMode::Once);

        // Apply pickup damage multiplier.
        let damage_mult = 1.0 + run.damage_stacks as f32 * settings.pickup_damage_per_stack;
        let final_damage = damage * damage_mult;

        // Spawn projectile(s) aimed at nearest enemy.
        spawn_projectile(
            &mut commands,
            &settings,
            player_pos,
            aim_dir,
            final_damage,
            projectile_speed,
            pellet_count,
            spread_rad,
            *archetype,
            &sprites,
        );
    }
}

/// Spawns a projectile entity — mirrors the spawn logic from
/// `weapon_fire_system` so auto-attack projectiles look and behave
/// identically to manual-fire projectiles.
pub(crate) fn spawn_projectile(
    commands: &mut Commands,
    settings: &GameSettings,
    player_pos: Vec2,
    center_dir: Vec2,
    damage: f32,
    projectile_speed: f32,
    pellet_count: u32,
    spread_rad: f32,
    archetype: WeaponArchetype,
    sprites: &GameSprites,
) {
    let count = pellet_count.max(1);
    let proj_sprite = weapon_sprite_for(archetype, sprites);
    for i in 0..count {
        let angle_offset = if count == 1 {
            0.0
        } else {
            let t = i as f32 / (count - 1) as f32;
            (t - 0.5) * spread_rad
        };
        let dir = rotate_vec2(center_dir, angle_offset);
        let spawn_pos = player_pos + dir * settings.weapon_muzzle_offset;
        let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

        commands.spawn((
            Sprite {
                image: proj_sprite.clone(),
                color: PROJECTILE_COLOR,
                custom_size: Some(Vec2::splat(12.0)),
                ..default()
            },
            Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
                .with_rotation(Quat::from_rotation_z(angle)),
            Velocity(dir * projectile_speed),
            Collider {
                half_extents: Vec2::splat(6.0),
            },
            Projectile {
                damage,
                lifetime: Timer::from_seconds(settings.projectile_lifetime_s, TimerMode::Once),
                team: Team::Player,
            },
        ));
    }
}