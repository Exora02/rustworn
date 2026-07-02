//! Damage systems: beam (capsule-vs-enemies per tick), breakable-damage,
//! and the shared kill-time bookkeeping helper.
//!
//! `beam_damage_system`: per-tick beam damage against enemies in the
//!   beam's capsule, with visual + HUD + kill-counter + drop-roll.
//! `breakable_damage_system`: projectile-vs-breakable (pots, crates,
//!   barrels) with the same AABB test and reward path as enemies.

use bevy::prelude::*;

use crate::components::{
    AiBehavior, AimDirection, AppState, Beacon, BeamVisual, Breakable, Collider, Elite, Enemy,
    EnemyAI, Health, HitFlash, PickupKind, Player, Projectile, Team, Weapon, WeaponArchetype,
};
use crate::events::{DamageEvent, EnemyKilledEvent};
use crate::plugins::enemy::spawn_death_puff;
use crate::plugins::fx::spawn_hit_spark;
use crate::plugins::pickups::{
    roll_gold_drop, roll_health_drop, roll_pickup_drop, roll_weapon_drop, spawn_pickup,
};

use super::aabb_overlap;
use crate::resources::{CascadeState, GameSettings, Hitstop, RunRng, RunState};

use super::{in_hitstop, on_enemy_killed, point_to_segment_distance};

/// Phase 1 (beam): tick the beam's repeating damage timer and, on
/// each tick, apply `beam_damage_per_tick` to every enemy whose
/// center falls inside the beam's capsule. Also update the beam
/// visual entity's transform + visibility each frame.
///
/// The beam is a line segment from the player's position to
/// (player_pos + aim_dir * max_range). An enemy is hit when its
/// center is within (its half_extent + beam_thickness/2) of the
/// line, AND its projection lies on the segment (clamp(0, 1) in
/// `point_to_segment_distance`).
///
/// Hit-flash + HP + death-puff + kill counter are wired through
/// the same `HitFlash` insert + `spawn_death_puff` call that the
/// projectile path uses, so the beam feels like every other
/// damage source.
///
/// Frozen during hitstop + game over so the beam doesn't keep
/// dealing damage through a freeze or after death.
#[allow(clippy::too_many_arguments)]
pub fn beam_damage_system(
    time: Res<Time>,
    aim: Res<AimDirection>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    mut run_state: ResMut<RunState>,
    mut rng: ResMut<RunRng>,
    mut cascade: ResMut<CascadeState>,
    mut commands: Commands,
    mut weapons: Query<&mut Weapon, With<Player>>,
    player_query: Query<&Transform, With<Player>>,
    mut enemies: Query<
        (Entity, &Transform, &Collider, &mut Health, &EnemyAI, Has<Beacon>, Has<Elite>),
        With<Enemy>,
    >,
    mut beam_visual: Query<
        (&mut Transform, &mut Visibility),
        (With<BeamVisual>, Without<Player>, Without<Enemy>),
    >,
    mut damage_events: EventWriter<DamageEvent>,
    mut kill_events: EventWriter<EnemyKilledEvent>,
) {
    if in_hitstop(&hitstop) {
        // Hide the beam during freeze/death so the visual
        // doesn't keep painting a dead frame.
        if let Ok((_t, mut vis)) = beam_visual.single_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    }
    let Ok(mut weapon) = weapons.single_mut() else {
        return;
    };
    if weapon.archetype != WeaponArchetype::Beam {
        // Player is on a different weapon. Hide the beam sprite
        // so a stale visual doesn't linger.
        if let Ok((_t, mut vis)) = beam_visual.single_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    }
    let Ok(player_tf) = player_query.single() else {
        return;
    };
    let Ok((mut beam_tf, mut beam_vis)) = beam_visual.single_mut() else {
        return;
    };

    if !weapon.beam_active {
        *beam_vis = Visibility::Hidden;
        return;
    }
    *beam_vis = Visibility::Inherited;

    // Update the visual: position at player, rotate to aim dir,
    // size set once at spawn. The sprite's Anchor::CenterLeft
    // means the transform position is the beam's start, and
    // rotation 0 points along +X. We want +X rotated to the
    // aim direction.
    let player_pos = player_tf.translation.truncate();
    let center_dir = if aim.0 == Vec2::ZERO { Vec2::Y } else { aim.0 };
    beam_tf.translation.x = player_pos.x;
    beam_tf.translation.y = player_pos.y;
    let angle = center_dir.y.atan2(center_dir.x);
    beam_tf.rotation = Quat::from_rotation_z(angle);

    // Tick the damage timer; on each tick, sweep enemies.
    weapon.beam_tick_timer.tick(time.delta());
    if !weapon.beam_tick_timer.just_finished() {
        return;
    }
    let beam_end = player_pos + center_dir * settings.beam_max_range;
    let half_thickness = settings.beam_thickness * 0.5;
    let damage = settings.beam_damage_per_tick;

    for (enemy_entity, enemy_tf, enemy_col, mut health, enemy_ai, is_beacon, is_elite) in &mut enemies {
        // Same-frame corpse guard: see bevy-0.16-gotchas §9.
        if health.current <= 0.0 {
            continue;
        }
        let enemy_pos = enemy_tf.translation.truncate();
        let enemy_radius = enemy_col.half_extents.length();
        let dist = point_to_segment_distance(enemy_pos, player_pos, beam_end);
        if dist > half_thickness + enemy_radius {
            continue;
        }
        health.current -= damage;
        commands.entity(enemy_entity).insert(HitFlash {
            timer: Timer::from_seconds(0.1, TimerMode::Once),
        });
        spawn_hit_spark(&mut commands, &settings, enemy_pos);
        damage_events.send(DamageEvent {
            victim: enemy_entity,
            amount: damage,
        });
        if health.current <= 0.0 {
            spawn_death_puff(&mut commands, &settings, enemy_pos);
            run_state.enemies_killed += 1;
            // Phase 2 P5: XP + cascade bookkeeping shared
            // with the projectile kill branch. The helper
            // bumps XP (× cascade multiplier), increments
            // `kills_during` while in Cascade, and — on a
            // beacon kill during Cascade — extends the
            // duration_timer by 5s (capped at +15s).
            on_enemy_killed(&settings, &mut run_state, &mut cascade, is_beacon);
            // Phase 1 pickups: roll the drop chance on kill using
            // the enemy's archetype. RNG is `RunRng` for
            // per-run determinism.
            let mut rng = || rng.next_unit();
            if let Some(kind) = roll_pickup_drop(enemy_ai.behavior, &settings, cascade.active, run_state.curse, &mut rng) {
                spawn_pickup(&mut commands, &settings, enemy_pos, kind);
            }
            // Phase 2.4 (P4): independent weapon-drop roll.
            // Both rolls share the same `rng` closure so the
            // sequence is deterministic per kill count, but
            // the two rolls are independent (a single kill
            // can drop a stat-stick AND a weapon).
            if let Some(weapon_kind) =
                roll_weapon_drop(enemy_ai.behavior, &settings, cascade.active, run_state.curse, &mut rng)
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
            if roll_health_drop(&settings, run_state.curse, &mut rng) {
                spawn_pickup(&mut commands, &settings, enemy_pos, PickupKind::Heal);
            } else if is_elite && rng() < (settings.health_drop_chance).min(1.0) {
                spawn_pickup(&mut commands, &settings, enemy_pos, PickupKind::Heal);
            }
            // Phase 3c: gold drop.
            if let Some(amt) = roll_gold_drop(is_elite, &mut rng) {
                spawn_pickup(&mut commands, &settings, enemy_pos, PickupKind::Gold(amt));
            }
            kill_events.send(EnemyKilledEvent {
                victim: enemy_entity,
                killer: Entity::PLACEHOLDER,
                position: enemy_pos,
            });
            commands.entity(enemy_entity).despawn();
        }
    }
}

/// Phase 2.5+: damage system for `Breakable` entities (pots,
/// crates, barrels). Queries breakables with `Health` + `Collider`,
/// and `Team::Player` projectiles. On AABB overlap: reduces the
/// breakable's HP by the projectile's damage, applies a `HitFlash`,
/// and — if HP hits 0 — spawns a death puff, rolls pickup/weapon/
/// health drops (same as enemy kills), and despawns the breakable.
/// One projectile can hit multiple breakables before being
/// despawned (unlike enemies where the projectile stops on first
/// contact). The projectile is despawned after any hit.
///
/// Early-returns on hitstop, same as other damage systems.
#[allow(clippy::too_many_arguments)]
pub fn breakable_damage_system(
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    mut run: ResMut<RunState>,
    mut rng: ResMut<RunRng>,
    mut cascade: ResMut<CascadeState>,
    mut commands: Commands,
    projectiles: Query<(Entity, &Transform, &Collider, &Projectile)>,
    mut breakables: Query<(Entity, &Transform, &Collider, &mut Breakable)>,
    mut damage_events: EventWriter<DamageEvent>,
    mut kill_events: EventWriter<EnemyKilledEvent>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    for (proj_entity, proj_tf, proj_col, proj) in &projectiles {
        // Only `Team::Player` projectiles damage breakables.
        if !matches!(proj.team, Team::Player) {
            continue;
        }
        let proj_pos = proj_tf.translation.truncate();

        let mut hit_any = false;
        for (breakable_entity, breakable_tf, breakable_col, mut breakable) in &mut breakables {
            if !aabb_overlap(
                proj_pos,
                proj_col.half_extents,
                breakable_tf.translation.truncate(),
                breakable_col.half_extents,
            ) {
                continue;
            }
            // Skip breakables already at 0 HP (same-frame guard).
            if breakable.hp == 0 {
                continue;
            }
            breakable.hp = breakable.hp.saturating_sub(proj.damage as u32);
            let destroyed = breakable.hp == 0;
            hit_any = true;

            // Visual feedback on every hit.
            commands.entity(breakable_entity).insert(HitFlash {
                timer: Timer::from_seconds(settings.hit_flash_duration_s, TimerMode::Once),
            });
            let breakable_pos = breakable_tf.translation.truncate();
            spawn_hit_spark(&mut commands, &settings, breakable_pos);
            damage_events.send(DamageEvent {
                victim: breakable_entity,
                amount: proj.damage,
            });

            if destroyed {
                // Breakable destroyed — same reward path as enemies.
                commands.entity(breakable_entity).despawn();
                spawn_death_puff(&mut commands, &settings, breakable_pos);
                run.enemies_killed = run.enemies_killed.saturating_add(1);
                on_enemy_killed(&settings, &mut run, &mut cascade, false);

                // Roll stat-stick pickup drop (use AiBehavior::Chaser
                // as a stand-in for breakable drop rates).
                let mut rng = || rng.next_unit();
                if let Some(kind) =
                    roll_pickup_drop(AiBehavior::Chaser, &settings, cascade.active, run.curse, &mut rng)
                {
                    spawn_pickup(&mut commands, &settings, breakable_pos, kind);
                }
                // Roll weapon drop (same stand-in).
                if let Some(weapon_kind) =
                    roll_weapon_drop(AiBehavior::Chaser, &settings, cascade.active, run.curse, &mut rng)
                {
                    spawn_pickup(
                        &mut commands,
                        &settings,
                        breakable_pos,
                        PickupKind::Weapon(weapon_kind),
                    );
                }
                // Roll health drop (flat 15%).
                if roll_health_drop(&settings, run.curse, &mut rng) {
                    spawn_pickup(&mut commands, &settings, breakable_pos, PickupKind::Heal);
                }
                // Phase 3c: gold drop from breakables (25% chance, 1-3 gold).
                let gold_roll: f32 = rng();
                if gold_roll < 0.25 {
                    let amt = 1 + (rng() * 3.0) as u32;
                    spawn_pickup(&mut commands, &settings, breakable_pos, PickupKind::Gold(amt));
                }
                kill_events.send(EnemyKilledEvent {
                    victim: breakable_entity,
                    killer: Entity::PLACEHOLDER,
                    position: breakable_pos,
                });
                // Brief hitstop on destruction.
                commands.insert_resource(Hitstop {
                    timer: Timer::from_seconds(settings.hitstop_duration_s, TimerMode::Once),
                });
            }
        }
        // Despawn the projectile if it hit any breakable.
        if hit_any {
            commands.entity(proj_entity).despawn();
        }
    }
}