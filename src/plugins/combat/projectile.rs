//! Projectile systems: lifetime tracking and damage resolution.
//!
//! `projectile_lifetime_system` despawns expired projectiles.
//! `projectile_damage_system` resolves player-projectile-vs-enemy hits.
//! `enemy_projectile_damage_system` resolves enemy-projectile-vs-player hits.

use bevy::prelude::*;
use log::warn;

use crate::components::{
    ArcGunProjectile, Beacon, Boss, Collider, Elite, Enemy, EnemyAI, Health, HitFlash, Invulnerable,
    Knockback, PickupKind, Player, Projectile, Team,
};
use crate::events::{DamageEvent, EnemyKilledEvent};
use crate::plugins::enemy::spawn_death_puff;
use crate::plugins::enemy::status::apply_status_effects;
use crate::plugins::fx::spawn_hit_spark;
use crate::plugins::fx::spawn_death_particles;
use crate::plugins::pickups::{
    roll_gold_drop, roll_health_drop, roll_item_drop, roll_pickup_drop, roll_weapon_drop,
    spawn_pickup,
};
use crate::resources::{CascadeState, CrownSave, GameSettings, Hitstop, RunRng, RunState, ScreenShake};

use super::{aabb_overlap, in_hitstop, on_enemy_killed};

// Iterate live projectiles and tick their lifetimes. When a
// projectile's timer finishes, despawn it. Projectiles of all
// teams are despawned — nobody's projectiles live forever.
pub fn projectile_lifetime_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    _run: Res<RunState>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Projectile)>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for (entity, mut proj) in &mut query {
        proj.lifetime.tick(time.delta());
        if proj.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Iterate live player projectiles and check each against every
/// enemy (chaser, dasher, etc.). On AABB overlap: despawn the
/// projectile, decrement the enemy's health, and — if HP hits
/// zero — despawn the enemy, spawn a death puff, increment
/// `RunState::enemies_killed`, and roll the full pickup/weapon/
/// health/gold drop sequence via `on_enemy_killed`.
/// One projectile = at most one enemy hit; the projectile
/// despawns on first contact.
///
/// Day 15-16: increment `RunState::enemies_killed` on a killing
/// blow so the HUD kill bar reflects progress.
///
/// Frozen during global hitstop so a killing blow that triggers
/// the freeze should not also resolve follow-up hits in the
/// same frame. Frozen during game over so the on-screen overlay
/// sits on a still frame.
#[allow(clippy::too_many_arguments)]
pub fn projectile_damage_system(
    save: Res<CrownSave>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    mut run: ResMut<RunState>,
    mut rng: ResMut<RunRng>,
    mut cascade: ResMut<CascadeState>,
    mut commands: Commands,
    mut shake: ResMut<ScreenShake>,
    projectiles: Query<(Entity, &Transform, &Collider, &Projectile, Has<ArcGunProjectile>)>,
    mut enemies: Query<
        (
            Entity,
            &Transform,
            &Collider,
            &mut Health,
            &EnemyAI,
            Has<Beacon>,
            Has<Elite>,
            Has<Boss>,
        ),
        With<Enemy>,
    >,
    mut damage_events: EventWriter<DamageEvent>,
    mut kill_events: EventWriter<EnemyKilledEvent>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for (proj_entity, proj_tf, proj_col, proj, is_arc) in &projectiles {
        // Projectiles are Player team today; the team check is in
        // place so enemy projectiles (Phase 1+) can co-exist without
        // damaging their own faction.
        if !matches!(proj.team, Team::Player) {
            continue;
        }
        let proj_pos = proj_tf.translation.truncate();

        let mut hit = false;
        // Store data for Arc Gun chain (needs to survive the enemies iter scope).
        let mut hit_enemy_entity = Entity::PLACEHOLDER;
        let mut hit_enemy_pos = Vec2::ZERO;
        for (enemy_entity, enemy_tf, enemy_col, mut health, enemy_ai, is_beacon, is_elite, is_boss) in
            &mut enemies
        {
            if !aabb_overlap(
                proj_pos,
                proj_col.half_extents,
                enemy_tf.translation.truncate(),
                enemy_col.half_extents,
            ) {
                continue;
            }
            // Skip enemies already at 0 HP from a same-frame hit
            // (e.g., shotgun 5 pellets landing on a 2-HP dasher:
            // pellet #4 despawns it, pellet #5's HitFlash insert
            // would otherwise panic on the now-queued-for-despawn
            // entity when the command queue flushes). The pellet
            // keeps flying in case there's another enemy in its
            // path; it just doesn't double-process a corpse.
            if health.current <= 0.0 {
                continue;
            }
            health.current = (health.current - proj.damage).max(0.0);
            hit = true;
            hit_enemy_entity = enemy_entity;
            hit_enemy_pos = enemy_tf.translation.truncate();
            // Day 13-14: white pulse on every hit (kill or not).
            commands.entity(enemy_entity).insert(HitFlash {
                timer: Timer::from_seconds(settings.hit_flash_duration_s, TimerMode::Once),
            });
            let enemy_pos = enemy_tf.translation.truncate();
            spawn_hit_spark(&mut commands, &settings, enemy_pos);
            damage_events.write(DamageEvent {
                victim: enemy_entity,
                amount: proj.damage,
            });
            // Phase 5.1: roll for status effects on hit.
            apply_status_effects(&mut commands, enemy_entity, &mut || rng.next_unit());
            if health.current <= 0.0 {
                commands.entity(enemy_entity).despawn();
                spawn_death_puff(&mut commands, &settings, enemy_pos);
                // Phase 5 game juice: spawn death burst particles.
                spawn_death_particles(&mut commands, enemy_pos);
                // Phase 5 game juice: stronger screen shake on boss
                // kills for a meatier feel.
                if is_boss {
                    shake.intensity = settings.screen_shake_intensity * 2.5;
                    shake.duration = settings.screen_shake_duration_s * 2.0;
                    shake.remaining = shake.duration;
                }
                // Day 15-16: bump the kill counter so the HUD bar
                // reflects progress. Saturating add would be cleaner
                // if we ever expected a "win" state; for now an
                // overflow at 4 billion kills is acceptable.
                run.enemies_killed = run.enemies_killed.saturating_add(1);
                // Phase 2 P5: XP + cascade bookkeeping. See
                // `on_enemy_killed` for the full semantics
                // (XP multiplier, beacon chain-extend). The
                // helper is called from both projectile and
                // beam kill paths so the two stay in sync.
                on_enemy_killed(&settings, &mut run, &mut cascade, is_beacon);
                // Phase 1 pickup drop. Deterministic via
                // `RunRng`.
                let mut rng = || rng.next_unit();
                if let Some(kind) = roll_pickup_drop(enemy_ai.behavior, &settings, cascade.active, run.curse, &mut rng) {
                    spawn_pickup(&mut commands, &settings, enemy_pos, kind);
                }
                // Phase 2.4 (P4): independent weapon-drop roll.
                if let Some(weapon_kind) =
                    roll_weapon_drop(enemy_ai.behavior, &settings, cascade.active, run.curse, &save.unlocks, &mut rng)
                {
                    spawn_pickup(
                        &mut commands,
                        &settings,
                        enemy_pos,
                        PickupKind::Weapon(weapon_kind),
                    );
                }
                // Health drop: flat 15% roll on every kill,
                // independent of stat-stick and weapon drops.
                // Elite kills get a second roll for double chance.
                if roll_health_drop(&settings, run.curse, &mut rng) {
                    spawn_pickup(&mut commands, &settings, enemy_pos, PickupKind::Heal);
                } else if is_elite && rng() < (settings.health_drop_chance).min(1.0) {
                    spawn_pickup(&mut commands, &settings, enemy_pos, PickupKind::Heal);
                }
                // Phase 3c: gold drop.
                if let Some(amt) = roll_gold_drop(is_elite, &mut rng) {
                    spawn_pickup(&mut commands, &settings, enemy_pos, PickupKind::Gold(amt));
                }
                // Phase 3e: item drop. Bosses: 100%, Elites: 15%, Regulars: 3% * (1 + luck).
                if let Some(item_kind) = roll_item_drop(is_boss, is_elite, run.luck, &mut rng) {
                    spawn_pickup(
                        &mut commands,
                        &settings,
                        enemy_pos,
                        PickupKind::Item(item_kind),
                    );
                }
                kill_events.write(EnemyKilledEvent {
                    victim: enemy_entity,
                    killer: Entity::PLACEHOLDER,
                    position: enemy_pos,
                });
                // Day 13-14: global hitstop on a killing blow. Insert
                // (or refresh) the resource — every combat/player/
                // enemy system gates on its presence, so the freeze
                // is felt uniformly across the game.
                commands.insert_resource(Hitstop {
                    timer: Timer::from_seconds(settings.hitstop_duration_s, TimerMode::Once),
                });
            }
            break;
        }
        if hit {
            commands.entity(proj_entity).despawn();
        }
        // Arc Gun chain: after the projectile hits and before we finish
        // processing this projectile, chain to nearby enemies. We do it
        // here (after the enemies iterator drops) so there's no borrow
        // conflict. The chain uses its own query iteration.
        if is_arc {
            chain_to_nearby_enemies(
                &mut commands,
                &settings,
                &mut enemies,
                hit_enemy_entity,
                hit_enemy_pos,
            );
        }
    }
}

/// After an Arc Gun primary hit at `hit_pos`, find the nearest living enemy
/// within 100px and chain lightning to it. Chains up to `MAX_CHAIN` targets
/// (2 in addition to the primary), dealing `CHAIN_DAMAGE` (0.8) each.
/// Must be called OUTSIDE the enemies iterator scope to avoid borrow conflicts.
fn chain_to_nearby_enemies(
    commands: &mut Commands,
    settings: &GameSettings,
    enemies: &mut Query<
        (
            Entity,
            &Transform,
            &Collider,
            &mut Health,
            &EnemyAI,
            Has<Beacon>,
            Has<Elite>,
            Has<Boss>,
        ),
        With<Enemy>,
    >,
    hit_enemy: Entity,
    hit_pos: Vec2,
) {
    const CHAIN_RANGE: f32 = 100.0;
    const CHAIN_DAMAGE: f32 = 0.8;
    const MAX_CHAIN: u32 = 2;

    let mut current_pos = hit_pos;
    let mut chain_exclude = vec![hit_enemy];

    for _chain_idx in 0..MAX_CHAIN {
        // Find the nearest living enemy (not already chained) within range.
        let mut nearest: Option<(Entity, Vec2)> = None;
        let mut nearest_dist = CHAIN_RANGE;

        for (enemy_entity, enemy_tf, _enemy_col, health, _enemy_ai, _is_beacon, _is_elite, _is_boss) in
            enemies.iter_mut()
        {
            if chain_exclude.contains(&enemy_entity) {
                continue;
            }
            if health.current <= 0.0 {
                continue;
            }
            let pos = enemy_tf.translation.truncate();
            let dist = pos.distance(current_pos);
            if dist <= nearest_dist {
                nearest_dist = dist;
                nearest = Some((enemy_entity, pos));
            }
        }

        if let Some((target, target_pos)) = nearest {
            // Apply damage to the chained enemy.
            if let Ok((_, _, _, mut health, _, _, _, _)) = enemies.get_mut(target) {
                health.current = (health.current - CHAIN_DAMAGE).max(0.0);
            }
            // Spawn a small yellow lightning visual (a thin line).
            spawn_chain_lightning(commands, settings, current_pos, target_pos);
            // Mark as excluded so we don't chain to it again.
            chain_exclude.push(target);
            current_pos = target_pos;
        } else {
            break; // No more enemies in range to chain to.
        }
    }
}

/// Spawn a small yellow line from `from` to `to` representing a chain
/// lightning bolt. Lives for 0.15s then despawns. Uses a stretched
/// thin sprite aligned along the from→to axis.
fn spawn_chain_lightning(
    commands: &mut Commands,
    _settings: &GameSettings,
    from: Vec2,
    to: Vec2,
) {
    use crate::components::HitSpark;
    use crate::plugins::fx::spawn_hit_spark;

    let midpoint = (from + to) * 0.5;
    let diff = to - from;
    let length = diff.length();
    let angle = diff.y.atan2(diff.x);

    // Lightning bolt visual: a thin yellow rectangle connecting the two points.
    commands.spawn((
        HitSpark {
            lifetime: Timer::from_seconds(0.15, TimerMode::Once),
        },
        Sprite {
            color: Color::srgb(1.0, 0.95, 0.20),
            custom_size: Some(Vec2::new(length, 3.0)),
            ..default()
        },
        Transform::from_xyz(midpoint.x, midpoint.y, 0.55)
            .with_rotation(Quat::from_rotation_z(angle)),
        Visibility::Inherited,
    ));

    // Also spawn a hit spark at the chained target for visual feedback.
    spawn_hit_spark(commands, _settings, to);
}

/// Mirror of `projectile_damage_system` but for `Team::Enemy`
/// projectiles hitting the player. Same AABB test, but:
///   - filters projectiles by `Team::Enemy`
///   - targets the Player
///   - applies knockback + i-frames + screen shake on hit
///   - one hit per frame (breaks after first overlap so a
///     cluster of bullets can't multi-hit the player faster
///     than the iframe gate allows)
///
/// Frozen during global hitstop + game over.
pub fn enemy_projectile_damage_system(
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    _run: Res<RunState>,
    mut commands: Commands,
    projectiles: Query<(Entity, &Transform, &Collider, &Projectile)>,
    mut player_query: Query<
        (
            Entity,
            &Transform,
            &Collider,
            &mut Health,
            Has<Invulnerable>,
        ),
        With<Player>,
    >,
    mut shake: ResMut<ScreenShake>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    let Ok((player_entity, player_tf, player_col, mut health, invulnerable)) =
        player_query.single_mut()
    else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (proj_entity, proj_tf, proj_col, proj) in &projectiles {
        // Only `Team::Enemy` projectiles. `Team::Player` projectiles
        // are handled by `projectile_damage_system` and hit enemies,
        // not the player.
        if !matches!(proj.team, Team::Enemy) {
            continue;
        }
        let proj_pos = proj_tf.translation.truncate();

        if !aabb_overlap(
            proj_pos,
            proj_col.half_extents,
            player_pos,
            player_col.half_extents,
        ) {
            continue;
        }

        // Bullet overlapped the player. Despawn it on every contact
        // (i-framed or not) so we don't re-check it next frame.
        commands.entity(proj_entity).despawn();

        if invulnerable {
            // Don't deal damage, don't apply knockback, don't
            // trigger screen shake. The visual flash from
            // `player_color_system` (dash iframe) still rides the
            // existing Invulnerable timer — we don't add
            // anything new.
            continue;
        }

        health.current = (health.current - proj.damage).max(0.0);
        let player_pos = player_tf.translation.truncate();
        damage_events.write(DamageEvent {
            victim: player_entity,
            amount: proj.damage,
        });
        spawn_hit_spark(&mut commands, &settings, player_pos);
        warn!(
            "Player took {:.1} damage from enemy projectile — HP: {:.1}",
            proj.damage, health.current
        );

        // Same screen-shake + knockback + i-frame trifecta as
        // the contact-damage system. The shake intensity and
        // knockback duration come from the chaser settings
        // (a shooter's bullet hitting you is roughly as
        // impactful as a chaser touching you, both are 1
        // damage; differentiating them here is over-tuning
        // for Phase 1).
        shake.intensity = settings.screen_shake_intensity;
        shake.duration = settings.screen_shake_duration_s;
        shake.remaining = settings.screen_shake_duration_s;

        // Knockback direction = away from the bullet's incoming
        // direction. The bullet was moving toward the player, so
        // "away from the bullet" is the bullet's velocity direction.
        let bullet_dir = if proj_pos == player_pos {
            Vec2::Y
        } else {
            (player_pos - proj_pos).normalize_or_zero()
        };
        commands.entity(player_entity).insert(Knockback {
            direction: if bullet_dir == Vec2::ZERO {
                Vec2::Y
            } else {
                bullet_dir
            },
            timer: Timer::from_seconds(settings.chaser_knockback_duration_s, TimerMode::Once),
        });
        commands
            .entity(player_entity)
            .insert(Invulnerable(Timer::from_seconds(
                settings.hit_iframe_duration_s,
                TimerMode::Once,
            )));

        // One hit per frame per bullet, but we keep iterating
        // to despawn any other overlapping bullets (they all
        // hit at the same instant). Multiple bullets in the
        // same frame = multiple despawns, but only the first
        // damages (the rest see `invulnerable` true and skip
        // the damage/knockback/iframe insert).
    }
}