//! Combat: weapons, projectiles, and damage.
//!
//! Day 5-7: click-to-fire pistol, projectile with lifetime, no enemies yet.
//! Damage routing (Day 11-12) will land here too.

use bevy::prelude::*;
use log::warn;

use crate::components::{
    AimDirection, Collider, Enemy, Health, HitFlash, Invulnerable, Knockback, Player, Projectile,
    Team, Velocity, Weapon, WeaponArchetype,
};
use crate::plugins::enemy::spawn_death_puff;
use crate::resources::{GameSettings, Hitstop, RunState, ScreenShake};

/// Color of the player projectile — warm white, clearly readable as a
/// bullet against any background we'll add later.
const PROJECTILE_COLOR: Color = Color::srgb(1.0, 0.95, 0.6);

/// Sprite size of the projectile. Long axis aligns with travel direction.
const PROJECTILE_SIZE: Vec2 = Vec2::new(6.0, 12.0);

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // Order: cooldown tick (drives `fire_timer.finished()`) → fire
        // (consumes click + spawns projectile) → projectile lifetime
        // (despawns old ones) → projectile-damage (resolves hits
        // against enemies). Movement is handled by the generic
        // `movement_system` in the player plugin.
        //
        // `weapon_switch_system` runs ahead of the fire system so a
        // 1/2-key press that happens on the same frame as a click
        // sees the new archetype when the fire system processes the
        // click. (In practice the click + key press are mutually
        // exclusive in the same frame, but ordering it this way
        // makes the intent clear.)
        app.add_systems(
            Update,
            (
                weapon_cooldown_tick_system,
                weapon_switch_system,
                weapon_fire_system,
                projectile_lifetime_system,
                projectile_damage_system,
                // Phase 1 (shooter): mirror of `projectile_damage_system`
                // for `Team::Enemy` projectiles hitting the player.
                // Runs after `projectile_damage_system` so player-vs-enemy
                // damage resolves in the same frame as enemy-vs-player
                // damage (no priority either way — both happen on
                // contact). The two systems share the
                // `projectile_lifetime_system` for despawn-on-expire
                // so the lifetime logic isn't duplicated.
                enemy_projectile_damage_system,
            )
                .chain(),
        );
    }
}

// Phase 1 (shotgun + burst): per-archetype stat lookup. The single
// source of truth for "what does this weapon do" lives here; the
// Weapon component on the player gets refreshed from this table by
// `weapon_switch_system` and by initial spawn in `setup.rs`.
//
// Numbers are picked to feel distinct:
//   - Pistol: 3 shots/sec, 1.0 dmg/projectile, 700 u/s, 1 pellet,
//     1 round per click
//   - Shotgun: 1 shot/sec, 0.5 dmg/pellet (5 pellets = 2.5 total),
//     600 u/s, 5 pellets spread across 0.35 rad (~20°), 1 round
//     per click
//   - Burst: 2 shots/sec (post-burst cooldown), 0.7 dmg/round,
//     700 u/s, 1 pellet, 3 rounds per click with a 0.1s gap
//     between rounds. Effective DPS: 0.7 × 3 / 0.7s = 3.0,
//     same as pistol — but the damage arrives in a 3-hit
//     spike rather than a steady tick.
//
// The shotgun's per-shot DPS (2.5) is slightly below the pistol's
// (3.0) but the spread makes the shotgun much more forgiving against
// groups of enemies — close-range it deletes clusters, mid-range
// the spread misses, far-range it's worse than the pistol. That
// trade-off is the design. Burst occupies a third niche: precise
// like the pistol, but with a "punch" on click that the pistol
// lacks.
fn weapon_stats_for(archetype: WeaponArchetype) -> (f32, f32, f32, u32, f32, u32) {
    // (damage, fire_rate, projectile_speed, pellet_count, spread_rad, burst_rounds)
    match archetype {
        WeaponArchetype::Pistol => (1.0, 3.0, 700.0, 1, 0.0, 1),
        WeaponArchetype::Shotgun => (0.5, 1.0, 600.0, 5, 0.35, 1),
        WeaponArchetype::Burst => (0.7, 2.0, 700.0, 1, 0.0, 3),
    }
}

// Day 13-14: helper to early-return when a global hitstop is active.
// Used by every combat system that ticks timers, mutates transforms,
// or fires projectiles — the freeze is felt uniformly because all of
// them gate on this. The resource is removed by `hitstop_tick_system`
// in the player plugin once its timer elapses.
fn in_hitstop(hitstop: &Option<Res<Hitstop>>) -> bool {
    hitstop.is_some()
}

// Day 15-16: same idea, but for the run-over flag. Combat systems
// freeze when the player is dead so the on-screen overlay sits on
// a still frame and a stray fire+despawn chain can't wipe a fresh
// run that's about to start.
fn in_game_over(run: &RunState) -> bool {
    run.game_over
}

// Each frame, advance every weapon's fire_timer. A freshly fired weapon
// has its timer set to `1.0 / fire_rate`; once it ticks down to zero the
// weapon can fire again. Frozen during global hitstop so the player
// can't circumvent the freeze by firing.
fn weapon_cooldown_tick_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut query: Query<&mut Weapon>,
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
        return;
    }
    for mut weapon in &mut query {
        weapon.fire_timer.tick(time.delta());
    }
}

// Phase 1 (shotgun): 1/2 keys select a weapon archetype directly,
// Q cycles to the next. We refresh every Weapon field from
// `weapon_stats_for` so the fire system never has to look up the
// archetype mid-burst, and we reset the fire timer so the new
// weapon is ready immediately (no awkward "I switched and now
// I'm on cooldown from the previous weapon" feel).
//
// Same gating as the rest of combat: hitstop + game over freeze
// input so a stray keypress during the game-over screen doesn't
// swap the player's weapon before they restart.
fn weapon_switch_system(
    keys: Res<ButtonInput<KeyCode>>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut player_query: Query<&mut Weapon, With<Player>>,
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
        return;
    }
    let Ok(mut weapon) = player_query.single_mut() else {
        return;
    };

    // Press priority: 1/2/3 (direct) first, then Q (cycle). If both
    // Q and a number key are pressed on the same frame, the number
    // key wins (explicit slot is more direct than "next").
    let target: Option<WeaponArchetype> = if keys.just_pressed(KeyCode::Digit1) {
        Some(WeaponArchetype::Pistol)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(WeaponArchetype::Shotgun)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(WeaponArchetype::Burst)
    } else if keys.just_pressed(KeyCode::KeyQ) {
        Some(next_archetype(weapon.archetype))
    } else {
        None
    };

    let Some(target) = target else {
        return;
    };
    if target == weapon.archetype {
        // Same weapon selected — no-op. Avoids an unnecessary
        // fire-timer reset on a redundant press.
        return;
    }

    let (damage, fire_rate, projectile_speed, pellet_count, spread_rad, burst_rounds) =
        weapon_stats_for(target);
    weapon.archetype = target;
    weapon.damage = damage;
    weapon.fire_rate = fire_rate;
    weapon.projectile_speed = projectile_speed;
    weapon.pellet_count = pellet_count;
    weapon.spread_rad = spread_rad;
    weapon.burst_rounds = burst_rounds;
    // Mid-burst switch: cancel the in-flight burst so the new
    // weapon starts fresh. Without this, switching to the
    // pistol mid-burst would queue 2 more pistol rounds 0.1s
    // apart, which is unintuitive.
    weapon.burst_rounds_remaining = 0;
    // Zero-duration finished timer so the next click fires
    // immediately.
    weapon.fire_timer = Timer::from_seconds(0.0, TimerMode::Once);
}

// Cycle order: Pistol → Shotgun → Burst → Pistol. New weapons
// slot in here.
fn next_archetype(current: WeaponArchetype) -> WeaponArchetype {
    match current {
        WeaponArchetype::Pistol => WeaponArchetype::Shotgun,
        WeaponArchetype::Shotgun => WeaponArchetype::Burst,
        WeaponArchetype::Burst => WeaponArchetype::Pistol,
    }
}

// Left-click on a ready weapon → spawn `pellet_count` Projectiles
// traveling in (or near) the current aim direction. The projectiles
// get a `Velocity` but no `BaseVelocity` (player projectiles don't
// cascade-scale). Frozen during global hitstop + game over.
//
// Day 15-16: also enforce the projectile cap. Before spawning, count
// live player projectiles; if we're already at the cap, silently
// drop the new shot. We pick "silent drop" over "despawn oldest"
// because the latter causes visible pops in the stream — for a
// 30-cap and a 3/sec fire rate, the cap is unreachable in normal
// play, so a dropped shot is genuinely a player error (machine
// lag, glitched click, etc.) and not worth the visual disruption.
//
// Phase 1 (shotgun): when `pellet_count > 1`, spawn N projectiles
// with aim directions evenly distributed across `[-spread/2,
// +spread/2]`. The center pellet fires exactly along the aim, the
// outer ones are rotated. Deterministic (no random angle) so the
// spread pattern is identical every shot — players can learn the
// pattern and aim with it.
//
// Phase 1 (burst): the state machine for `burst_rounds > 1`.
//
//   click       → fire round 1, set remaining = total - 1, set
//                 fire_timer to round gap (0.1s)
//   gap elapses → fire next round, decrement remaining, if 0
//                 set fire_timer to full cooldown, else set to
//                 round gap again
//   cooldown    → wait, weapon is busy
//   gap elapses → fire remaining > 0 path auto-fires the next
//                 round WITHOUT waiting for a click — the burst
//                 is "in flight" once started
//
// `burst_rounds_remaining == 0` and a click both satisfy the
// "should I fire this frame?" check; either one triggers a fire.
// Mid-burst, the click is ignored (the auto-fire path handles it)
// so spamming the mouse doesn't shorten the burst.
fn weapon_fire_system(
    mouse: Res<ButtonInput<MouseButton>>,
    aim: Res<AimDirection>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut commands: Commands,
    mut weapons: Query<(&mut Weapon, &Transform), With<Player>>,
    projectiles: Query<&Projectile>,
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
        return;
    }
    let Ok((mut weapon, player_transform)) = weapons.single_mut() else {
        return;
    };
    if !weapon.fire_timer.finished() {
        return;
    }

    // Phase 1 (burst): decide whether to fire this frame.
    //   - if a burst is in progress (remaining > 0), fire the
    //     next round
    //   - if a click just happened, start a new burst (or fire
    //     a single shot for pistol/shotgun)
    //   - otherwise, do nothing
    let burst_in_progress = weapon.burst_rounds_remaining > 0;
    let click = mouse.just_pressed(MouseButton::Left);
    if !burst_in_progress && !click {
        return;
    }

    // Projectile cap: count current live projectiles, skip the spawn
    // if we're already at the cap. Counts all teams since the cap is
    // a global perf budget and Phase 1+ will add enemy projectiles
    // that should also count against it. Skip the cap if the cap
    // is 0 (which would mean "disabled" — not currently used but
    // the safety net is cheap).
    if settings.projectile_cap > 0 {
        let live = projectiles.iter().count() as u32;
        if live >= settings.projectile_cap {
            return;
        }
    }

    // Spawn one round of the weapon (1 round for pistol/shotgun,
    // 1 round for each tap of the burst). The actual
    // `pellet_count` × spread is handled below in the spawn loop.
    let center_dir = if aim.0 == Vec2::ZERO { Vec2::Y } else { aim.0 };
    let player_pos = player_transform.translation.truncate();

    let pellet_count = weapon.pellet_count.max(1);
    for i in 0..pellet_count {
        // Spread angle: t goes 0..1 across the pellets, mapped to
        // -spread/2 .. +spread/2. With 1 pellet, t=0, angle=0.
        // With 5 pellets, t = {0, 0.25, 0.5, 0.75, 1.0}, angles =
        // {-spread/2, -spread/4, 0, +spread/4, +spread/2}.
        let angle_offset = if pellet_count == 1 {
            0.0
        } else {
            let t = i as f32 / (pellet_count - 1) as f32;
            (t - 0.5) * weapon.spread_rad
        };
        let dir = rotate_vec2(center_dir, angle_offset);

        let spawn_pos = player_pos + dir * settings.weapon_muzzle_offset;
        let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

        commands.spawn((
            Sprite {
                color: PROJECTILE_COLOR,
                custom_size: Some(PROJECTILE_SIZE),
                ..default()
            },
            Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
                .with_rotation(Quat::from_rotation_z(angle)),
            // No `BaseVelocity` / `CascadeAffected` markers, so the shared
            // `velocity_apply_system` won't touch this entity and the spawn-
            // time Velocity is preserved as the projectile's true motion.
            // Player projectiles run at full speed during Cascade Mode
            // (per PLAN.md §2).
            Velocity(dir * weapon.projectile_speed),
            // Day 11-12: 6x12 sprite → 3x6 half-extents for AABB collision
            // against enemy hitboxes. Kept slightly smaller than the visual
            // so close calls feel like hits rather than near-misses.
            Collider {
                half_extents: Vec2::new(PROJECTILE_SIZE.x * 0.5, PROJECTILE_SIZE.y * 0.5),
            },
            Projectile {
                damage: weapon.damage,
                lifetime: Timer::from_seconds(settings.projectile_lifetime_s, TimerMode::Once),
                team: Team::Player,
            },
        ));
    }

    // Update burst state. After this fire:
    //   - if a burst was in progress, decrement remaining
    //   - if a click started a new burst, set remaining to
    //     total - 1 (we just fired 1 round, total-1 left)
    //   - if neither, the weapon is single-shot (burst_rounds = 1)
    //     and remaining stays 0
    if burst_in_progress {
        weapon.burst_rounds_remaining -= 1;
    } else if click && weapon.burst_rounds > 1 {
        // Start of a new burst. We just fired round 1, so N-1
        // remain. (For single-shot weapons, burst_rounds == 1
        // and this branch doesn't fire, so remaining stays 0.)
        weapon.burst_rounds_remaining = weapon.burst_rounds - 1;
    }

    // Set the next fire_timer. Two cases:
    //   - remaining > 0: more rounds in this burst, wait the
    //     round gap
    //   - remaining == 0: burst complete (or single-shot
    //     weapon), wait the post-shot cooldown
    if weapon.burst_rounds_remaining > 0 {
        weapon.fire_timer = Timer::from_seconds(settings.burst_round_gap_s, TimerMode::Once);
    } else {
        let interval = 1.0 / weapon.fire_rate;
        weapon.fire_timer = Timer::from_seconds(interval, TimerMode::Once);
    }
}

// Rotate a 2D vector by `angle` radians. The default `Vec2` API
// doesn't expose a rotation helper, so this is the cheapest
// handwritten form — sin/cos once, no allocations.
fn rotate_vec2(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

fn projectile_lifetime_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Projectile)>,
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
        return;
    }
    for (entity, mut proj) in &mut query {
        proj.lifetime.tick(time.delta());
        if proj.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

// AABB-vs-AABB overlap test between two transformed entities.
// Same math as the player contact-damage system, but duplicated here
// rather than promoted to a shared util — there's only two callers
// and they live in different plugins.
fn aabb_overlap(pos_a: Vec2, half_a: Vec2, pos_b: Vec2, half_b: Vec2) -> bool {
    let delta = (pos_a - pos_b).abs();
    delta.x < half_a.x + half_b.x && delta.y < half_a.y + half_b.y
}

// Iterate live projectiles and check each against every chaser. On
// hit: despawn the projectile, decrement the chaser's health, and
// (if HP hits zero) despawn the chaser and spawn a fading DeathPuff
// at its last position. One projectile = at most one enemy hit; the
// projectile despawns on first contact, so the inner loop exits
// naturally.
//
// Day 15-16: increment `RunState::enemies_killed` on a killing blow
// so the HUD kill bar reflects progress.
fn projectile_damage_system(
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    mut run: ResMut<RunState>,
    mut commands: Commands,
    projectiles: Query<(Entity, &Transform, &Collider, &Projectile)>,
    mut enemies: Query<(Entity, &Transform, &Collider, &mut Health), With<Enemy>>,
) {
    // Frozen during global hitstop — a killing blow that triggers the
    // freeze should not also resolve follow-up hits in the same frame.
    // Frozen during game over so the on-screen overlay sits on a
    // still frame and a stray fire+despawn chain can't wipe a fresh
    // run that's about to start.
    if in_hitstop(&hitstop) || in_game_over(&run) {
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
        for (enemy_entity, enemy_tf, enemy_col, mut health) in &mut enemies {
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
            if health.current <= 0.0 {
                let pos = enemy_tf.translation.truncate();
                commands.entity(enemy_entity).despawn();
                spawn_death_puff(&mut commands, &settings, pos);
                // Day 15-16: bump the kill counter so the HUD bar
                // reflects progress. Saturating add would be cleaner
                // if we ever expected a "win" state; for now an
                // overflow at 4 billion kills is acceptable.
                run.enemies_killed = run.enemies_killed.saturating_add(1);
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

// Phase 1 (shooter): mirror of `projectile_damage_system` but for
// `Team::Enemy` projectiles hitting the player. Same AABB test, but:
//   - filters projectiles by `Team::Enemy` (the other system
//     filters by `Team::Player`)
//   - targets the Player instead of `With<Enemy>`
//   - applies knockback + i-frames + screen shake on hit, the
//     same contact-damage trifecta the chaser/dasher use
//   - does NOT trigger global hitstop or kill-counter (those
//     are reserved for player kills; damage-taken feedback is
//     the screen shake alone)
//   - one hit per frame — breaks after the first overlap so a
//     cluster of bullets can't multi-hit the player faster
//     than the iframe gate allows (they all despawn on first
//     contact, the rest are wasted)
//
// Frozen during global hitstop (so a cascade kill mid-bullet-
// rain doesn't trigger a follow-up hit during the freeze) and
// during game over (so the dead player doesn't keep eating
// bullets from a shooter that's still firing).
fn enemy_projectile_damage_system(
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
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
) {
    if in_hitstop(&hitstop) || in_game_over(&run) {
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
        warn!(
            "Player took {:.1} damage from enemy projectile — HP: {:.1}",
            proj.damage, health.current
        );

        // Same screen-shake + knockback + i-frame trigemma as
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
