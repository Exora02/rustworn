//! Projectile systems: lifetime tracking and damage resolution.
//!
//! `projectile_lifetime_system` despawns expired projectiles.
//! `projectile_damage_system` resolves player-projectile-vs-enemy hits.
//! `enemy_projectile_damage_system` resolves enemy-projectile-vs-player hits.

use bevy::prelude::*;
use log::warn;

use crate::components::{
    Beacon, Collider, Elite, Enemy, EnemyAI, Health, HitFlash, Invulnerable, Knockback,
    PickupKind, Player, Projectile, Team,
};
use crate::events::{DamageEvent, EnemyKilledEvent};
use crate::plugins::enemy::spawn_death_puff;
use crate::plugins::fx::spawn_hit_spark;
use crate::plugins::pickups::{
    roll_gold_drop, roll_health_drop, roll_pickup_drop, roll_weapon_drop, spawn_pickup,
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
    projectiles: Query<(Entity, &Transform, &Collider, &Projectile)>,
    mut enemies: Query<
        (Entity, &Transform, &Collider, &mut Health, &EnemyAI, Has<Beacon>, Has<Elite>),
        With<Enemy>,
    >,
    mut damage_events: EventWriter<DamageEvent>,
    mut kill_events: EventWriter<EnemyKilledEvent>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for (proj_entity, proj_tf, proj_col, proj) in &projectiles {
        // Projectiles are Player team today; the team check is in
        // place so enemy projectiles (Phase 1+) can co-exist without
        // damaging their own faction.
        if !matches!(proj.team, Team::Player) {
            continue;
        }
        let proj_pos = proj_tf.translation.truncate();

        let mut hit = false;
        for (enemy_entity, enemy_tf, enemy_col, mut health, enemy_ai, is_beacon, is_elite) in &mut enemies {
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
            if health.current <= 0.0 {
                commands.entity(enemy_entity).despawn();
                spawn_death_puff(&mut commands, &settings, enemy_pos);
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
    }
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