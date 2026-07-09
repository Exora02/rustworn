//! Weapon systems: cooldown ticking, archetype switching, and firing.
//!
//! These three systems form the "input → weapon state → projectile spawn"
//! pipeline. All are frozen during global hitstop.

use bevy::prelude::*;

use crate::components::{
    AimDirection, AppState, Collider, Player, Projectile, Team, Velocity, Weapon, WeaponArchetype,
    WeaponInventory,
    AiBehavior, Beacon, Elite, EnemyAI, Health, HitFlash, PickupKind,
};
use crate::events::{DamageEvent, EnemyKilledEvent};
use crate::plugins::enemy::spawn_death_puff;
use crate::plugins::fx::spawn_hit_spark;
use crate::plugins::pickups::{roll_gold_drop, roll_health_drop, roll_pickup_drop, roll_weapon_drop, spawn_pickup};
use crate::resources::{CascadeState, GameSettings, Hitstop, RunRng, RunState, ScreenShake};

use super::{
    aabb_overlap, in_hitstop, next_archetype, on_enemy_killed, rotate_vec2, weapon_stats_for,
    PROJECTILE_COLOR, PROJECTILE_SIZE,
};

// Each frame, advance every weapon's fire_timer. A freshly fired weapon
// has its timer set to `1.0 / fire_rate`; once it ticks down to zero the
// weapon can fire again. Frozen during global hitstop so the player
// can't circumvent the freeze by firing.
pub fn weapon_cooldown_tick_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut query: Query<&mut Weapon>,
) {
    if in_hitstop(&hitstop) {
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
pub fn weapon_switch_system(
    keys: Res<ButtonInput<KeyCode>>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut player_query: Query<&mut Weapon, With<Player>>,
    mut inventory_q: Query<&mut WeaponInventory, With<Player>>,
) {
    if in_hitstop(&hitstop) {
        return;
    }
    let Ok(mut weapon) = player_query.single_mut() else {
        return;
    };

    // Press priority: 1/2/3/4 (direct) first, then Q (cycle). If both
    // Q and a number key are pressed on the same frame, the number
    // key wins (explicit slot is more direct than "next").
    let target: Option<WeaponArchetype> = if keys.just_pressed(KeyCode::Digit1) {
        Some(WeaponArchetype::Pistol)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(WeaponArchetype::Shotgun)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(WeaponArchetype::Burst)
    } else if keys.just_pressed(KeyCode::Digit4) {
        Some(WeaponArchetype::Beam)
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
    // Update the active_slot on WeaponInventory so auto_attack_system
    // knows which slot is the active weapon (and should be skipped).
    if let Ok(mut inventory) = inventory_q.single_mut() {
        inventory.active_slot = inventory
            .slots
            .iter()
            .position(|s| *s == Some(target))
            .unwrap_or(0);
    }
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
#[allow(clippy::too_many_arguments)]
pub fn weapon_fire_system(
    mouse: Res<ButtonInput<MouseButton>>,
    aim: Res<AimDirection>,
    settings: Res<GameSettings>,
    run: Res<RunState>,
    mut commands: Commands,
    mut weapons: Query<(&mut Weapon, &mut Transform), With<Player>>,
    projectiles: Query<&Projectile>,
) {
    // Phase 2: no per-system game-over early-return needed — the
    // whole combat chain is gated on `in_state(AppState::InRun)` in
    // `CombatPlugin::build()`. The `run` resource is still in scope
    // because weapon_fire uses it for the `run.enemies_killed += 1`
    // bookkeeping on a kill.
    let Ok((mut weapon, player_transform)) = weapons.single_mut() else {
        return;
    };
    // Phase 1 (beam): beam is held-trigger — its damage cadence is
    // driven entirely by `beam_damage_system` (per-tick damage to
    // enemies in the beam's capsule). The click path doesn't fire
    // a projectile for beam; instead, the press/release toggles
    // `Weapon::beam_active`, which the damage system reads each
    // frame.
    if weapon.archetype == WeaponArchetype::Beam {
        if mouse.just_pressed(MouseButton::Left) {
            weapon.beam_active = true;
            weapon.beam_tick_timer.reset();
        }
        if mouse.just_released(MouseButton::Left) {
            weapon.beam_active = false;
        }
        // Beam never gates on the fire_timer; the fire_timer
        // for beam is purely cosmetic (not used).
        return;
    }
    // Phase 1 (Vampire Survivors fusion): weapons auto-fire
    // on their `fire_timer` cadence regardless of mouse input.
    // Click is now an "interrupt / force-fire" — it resets the
    // timer so the next frame fires immediately. For burst,
    // a click interrupts any in-flight burst and starts a
    // fresh one. The click never suppresses auto-fire: even
    // if the player mashes the mouse, the next frame will
    // fire because the timer was just reset.
    let click = mouse.just_pressed(MouseButton::Left);
    if click {
        weapon.fire_timer = Timer::from_seconds(0.0, TimerMode::Once);
    }
    if !weapon.fire_timer.finished() {
        return;
    }
    // Phase 1 (burst): continue a burst if one is in progress.
    // The auto-fire loop sets `burst_rounds_remaining` when a
    // burst starts; each fire decrements it until it hits 0.
    // For single-shot weapons, `burst_rounds_remaining` stays
    // at 0 and the auto-fire loop just fires once per cooldown.
    let burst_in_progress = weapon.burst_rounds_remaining > 0;

    // Apply the Phase 1 pickup multipliers. Damage and fire
    // rate both stack multiplicatively: 2 DamageUp pickups
    // give 1.0 × 1.25 × 1.25 = 1.5625× damage. The base
    // `weapon.damage` and `weapon.fire_rate` are the
    // archetype's stats (set in setup.rs); the multipliers
    // come from `RunState` which `apply_pickup` updates.
    let damage_mult = 1.0 + run.damage_stacks as f32 * settings.pickup_damage_per_stack;
    let fire_rate_mult = 1.0 + run.fire_rate_stacks as f32 * settings.pickup_fire_rate_per_stack;
    let damage = weapon.damage * damage_mult;

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
                // Pickup-multiplied damage: each DamageUp adds
                // `pickup_damage_per_stack` (0.25) to a 1.0 base.
                // The damage is stamped at spawn time; pickups
                // picked up later apply to subsequent projectiles
                // (not retroactively to ones already in flight).
                damage,
                lifetime: Timer::from_seconds(settings.projectile_lifetime_s, TimerMode::Once),
                team: Team::Player,
            },
        ));
    }

    // Update burst state. After this fire:
    //   - if a burst was in progress, decrement remaining
    //   - if we just started a burst (single-shot weapons skip
    //     this branch because burst_rounds == 1), set
    //     remaining to total - 1
    if burst_in_progress {
        weapon.burst_rounds_remaining -= 1;
    } else if weapon.burst_rounds > 1 {
        // Auto-fire starting a new burst. We just fired round 1
        // (this system fires one round per fire_timer tick), so
        // N-1 remain. The "click is required to start" rule is
        // gone with auto-fire — the auto-fire loop starts the
        // burst on its own.
        weapon.burst_rounds_remaining = weapon.burst_rounds - 1;
    }
    // (Single-shot weapons: burst_rounds == 1, neither branch
    // fires, remaining stays 0, every fire is independent.)

    // Set the next fire_timer. Two cases:
    //   - remaining > 0: more rounds in this burst, wait the
    //     round gap
    //   - remaining == 0: burst complete (or single-shot
    //     weapon), wait the post-shot cooldown. The cooldown
    //     is divided by the fire-rate multiplier (1.0 +
    //     stacks × per_stack), so 1 FireRateUp makes the
    //     cooldown 1/1.3 ≈ 0.77× as long.
    if weapon.burst_rounds_remaining > 0 {
        weapon.fire_timer = Timer::from_seconds(settings.burst_round_gap_s, TimerMode::Once);
    } else {
        let interval = 1.0 / (weapon.fire_rate * fire_rate_mult);
        weapon.fire_timer = Timer::from_seconds(interval, TimerMode::Once);
    }
}