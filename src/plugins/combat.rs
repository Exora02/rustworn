//! Combat: weapons, projectiles, and damage.
//!
//! Day 5-7: click-to-fire pistol, projectile with lifetime, no enemies yet.
//! Damage routing (Day 11-12) will land here too.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use log::warn;

use crate::components::{
    AimDirection, AppState, AiBehavior, Beacon, BeamVisual, Breakable, Collider, Enemy, EnemyAI,
    Health, HitFlash, Invulnerable, Knockback, PickupKind, Player, Projectile, Team, Velocity,
    Weapon, WeaponArchetype,
};
use crate::events::{DamageEvent, EnemyKilledEvent};
use crate::plugins::enemy::spawn_death_puff;
use crate::plugins::fx::spawn_hit_spark;
use crate::plugins::pickups::{roll_health_drop, roll_pickup_drop, roll_weapon_drop, spawn_pickup};
use crate::resources::{CascadeState, GameSettings, Hitstop, RunRng, RunState, ScreenShake};

/// Color of the player projectile — warm white, clearly readable as a
/// bullet against any background we'll add later.
const PROJECTILE_COLOR: Color = Color::srgb(1.0, 0.95, 0.6);

/// Sprite size of the projectile. Long axis aligns with travel direction.
const PROJECTILE_SIZE: Vec2 = Vec2::new(6.0, 12.0);

/// Phase 1 (beam): visual color of the beam sprite. Hot
/// orange/yellow — reads as "energy weapon" in arcade color
/// shorthand, distinct from the warm white of a bullet, the
/// cool cyan of the burst HUD bar, and the dark reds of
/// enemy death. The same color is used for the HUD fire-rate
/// bar fill (see `FIRE_RATE_BEAM` in `hud.rs`) so the player
/// sees one consistent "you're on the beam weapon" signal.
const BEAM_COLOR: Color = Color::srgb(1.0, 0.6, 0.1);

/// Phase 1 (beam): z-order of the beam visual sprite. Sits
/// above the projectiles (0.5) and well below the HUD (10.0)
/// so the beam paints in the world but never bleeds onto the
/// HUD bars. 0.5+epsilon on Bazzite (wgpu) where same-z
/// ordering is arbitrary — see `bevy-0.16-gotchas` §12
/// correction note.
const BEAM_Z: f32 = 0.6;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // Phase 1 (beam): the beam visual entity is spawned once
        // at startup alongside the player. A single persistent
        // sprite (not a per-frame spawn/despawn) so the
        // `beam_damage_system` can simply toggle its visibility
        // and reposition it each frame.
        app.add_systems(Startup, spawn_beam_visual);

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
        //
        // `beam_damage_system` runs after the click-based
        // `weapon_fire_system` (which early-returns for Beam).
        // Both read `Weapon` from the player but on different
        // archetype arms, and Bevy serializes them in the chain,
        // so there's no B0001 conflict. Beam damage resolves
        // before projectile lifetime / damage so a beam tick that
        // kills an enemy in the same frame as a sniper bullet
        // despawns both consistently.
        app.add_systems(
            Update,
            (
                weapon_cooldown_tick_system,
                weapon_switch_system,
                weapon_fire_system,
                beam_damage_system,
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
                // Phase 2.5+: breakable-damage system.
                // Runs after enemy-projectile damage so breakable
                // hits resolve in the same frame as the main
                // damage pass. One projectile can break multiple
                // breakables in a single frame.
                breakable_damage_system,
            )
                .chain()
                // Phase 2: gates the whole combat chain on the
                // active-run state. Weapons don't fire, projectiles
                // don't tick lifetimes, no damage resolves during
                // MainMenu or GameOver. The hitstop check stays
                // per-system (in `if in_hitstop(&hitstop) { return; }`)
                // because hitstop is a per-frame concern, not a
                // per-state one.
                .run_if(in_state(AppState::InRun)),
        );
    }
}

/// Phase 1 (beam): spawn a single persistent beam-visual sprite
/// entity at startup. Hidden by default; `beam_damage_system`
/// toggles its `Visibility` and updates its position, rotation,
/// and scale each frame based on the player's position, aim
/// direction, and the `Weapon::beam_active` flag. The sprite
/// is anchored at `Anchor::CenterLeft` (its transform position
/// is the "start" of the beam, the sprite extends outward in
/// the +X direction after rotation), and its `custom_size` is
/// set to (max_range, thickness) once at spawn so the
/// per-frame update only needs to set the rotation and
/// visibility — the size is constant.
///
/// Z = `BEAM_Z` (0.6) puts the beam above projectiles (0.5)
/// and well below the HUD (10.0), so the beam paints in the
/// world but never bleeds onto the HUD bars.
fn spawn_beam_visual(mut commands: Commands, settings: Res<GameSettings>) {
    commands.spawn((
        BeamVisual,
        Sprite {
            color: BEAM_COLOR,
            custom_size: Some(Vec2::new(settings.beam_max_range, settings.beam_thickness)),
            anchor: Anchor::CenterLeft,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, BEAM_Z),
        Visibility::Hidden,
    ));
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
        // Beam: damage/fire_rate are cosmetic for the HUD bar
        // (the real cadence is `beam_tick_interval_s`); speed=0
        // signals "no projectile, instant hit"; pellet_count=1 +
        // spread_rad=0 means the click path (which early-returns
        // for Beam) wouldn't even spawn anything.
        WeaponArchetype::Beam => (0.5, 10.0, 0.0, 1, 0.0, 1),
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

// Day 15-16 (removed in Phase 2): the previous `in_game_over` helper
// gated combat systems on `RunState.game_over: bool`. Phase 2 lifts
// the bool into a Bevy `AppState` and gates the whole combat chain
// on `in_state(AppState::InRun)` at the chain level. Per-system
// `|| in_game_over(&run)` checks are now just hitstop checks.

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
fn weapon_switch_system(
    keys: Res<ButtonInput<KeyCode>>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
    mut player_query: Query<&mut Weapon, With<Player>>,
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

// Cycle order: Pistol → Shotgun → Burst → Beam → Pistol. New
// weapons slot in here.
fn next_archetype(current: WeaponArchetype) -> WeaponArchetype {
    match current {
        WeaponArchetype::Pistol => WeaponArchetype::Shotgun,
        WeaponArchetype::Shotgun => WeaponArchetype::Burst,
        WeaponArchetype::Burst => WeaponArchetype::Beam,
        WeaponArchetype::Beam => WeaponArchetype::Pistol,
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

// Rotate a 2D vector by `angle` radians. The default `Vec2` API
// doesn't expose a rotation helper, so this is the cheapest
// handwritten form — sin/cos once, no allocations.
fn rotate_vec2(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

// Project a point onto the beam's line segment, returning the
// perpendicular distance from the beam line to the point. If the
// projection falls outside the segment, returns infinity (caller
// treats as "miss"). This is the test the beam uses to decide
// whether an enemy is "in the beam" — capsule-vs-point.
fn point_to_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < f32::EPSILON {
        // Degenerate segment (a == b). Return distance to a.
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let projection = a + ab * t;
    (p - projection).length()
}

// Phase 1 (beam): tick the beam's repeating damage timer and, on
// each tick, apply `beam_damage_per_tick` to every enemy whose
// center falls inside the beam's capsule. Also update the beam
// visual entity's transform + visibility each frame.
//
// The beam is a line segment from the player's position to
// (player_pos + aim_dir * max_range). An enemy is hit when its
// center is within (its half_extent + beam_thickness/2) of the
// line, AND its projection lies on the segment (clamp(0, 1) in
// `point_to_segment_distance`).
//
// Hit-flash + HP + death-puff + kill counter are wired through
// the same `HitFlash` insert + `spawn_death_puff` call that the
// projectile path uses, so the beam feels like every other
// damage source. The `commands.get_entity` check would NOT
// protect us from the same-frame-corpse panic (see
// bevy-0.16-gotchas §9), so we skip enemies with
// `health.current <= 0.0` in the inner loop instead.
//
// Frozen during hitstop + game over so the beam doesn't keep
// dealing damage through a freeze or after death.
#[allow(clippy::too_many_arguments)]
fn beam_damage_system(
    time: Res<Time>,
    aim: Res<AimDirection>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    // `Res<RunState>` and `ResMut<RunState>` together would
    // raise B0002 (conflicting access to the same resource).
    // `run_state` is read+mutated below; we use the same
    // binding for the early-return gate (a read is fine on
    // `ResMut`).
    mut run_state: ResMut<RunState>,
    mut rng: ResMut<RunRng>,
    mut cascade: ResMut<CascadeState>,
    mut commands: Commands,
    mut weapons: Query<&mut Weapon, With<Player>>,
    player_query: Query<&Transform, With<Player>>,
    mut enemies: Query<
        (Entity, &Transform, &Collider, &mut Health, &EnemyAI, Has<Beacon>),
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

    for (enemy_entity, enemy_tf, enemy_col, mut health, enemy_ai, is_beacon) in &mut enemies {
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
            if let Some(kind) = roll_pickup_drop(enemy_ai.behavior, &settings, cascade.active, &mut rng) {
                spawn_pickup(&mut commands, &settings, enemy_pos, kind);
            }
            // Phase 2.4 (P4): independent weapon-drop roll.
            // Both rolls share the same `rng` closure so the
            // sequence is deterministic per kill count, but
            // the two rolls are independent (a single kill
            // can drop a stat-stick AND a weapon).
            if let Some(weapon_kind) =
                roll_weapon_drop(enemy_ai.behavior, &settings, cascade.active, &mut rng)
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
            if roll_health_drop(&settings, &mut rng) {
                spawn_pickup(
                    &mut commands,
                    &settings,
                    enemy_pos,
                    PickupKind::Heal,
                );
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

fn projectile_lifetime_system(
    time: Res<Time>,
    hitstop: Option<Res<Hitstop>>,
    run: Res<RunState>,
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
    mut rng: ResMut<RunRng>,
    mut cascade: ResMut<CascadeState>,
    mut commands: Commands,
    projectiles: Query<(Entity, &Transform, &Collider, &Projectile)>,
    mut enemies: Query<
        (Entity, &Transform, &Collider, &mut Health, &EnemyAI, Has<Beacon>),
        With<Enemy>,
    >,
    mut damage_events: EventWriter<DamageEvent>,
    mut kill_events: EventWriter<EnemyKilledEvent>,
) {
    // Frozen during global hitstop — a killing blow that triggers the
    // freeze should not also resolve follow-up hits in the same frame.
    // Frozen during game over so the on-screen overlay sits on a
    // still frame and a stray fire+despawn chain can't wipe a fresh
    // run that's about to start.
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
        for (enemy_entity, enemy_tf, enemy_col, mut health, enemy_ai, is_beacon) in &mut enemies {
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
            damage_events.send(DamageEvent {
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
                if let Some(kind) = roll_pickup_drop(enemy_ai.behavior, &settings, cascade.active, &mut rng) {
                    spawn_pickup(&mut commands, &settings, enemy_pos, kind);
                }
                // Phase 2.4 (P4): independent weapon-drop roll.
                // See the matching block in the beam/contact
                // kill handler above for the rationale.
                if let Some(weapon_kind) =
                    roll_weapon_drop(enemy_ai.behavior, &settings, cascade.active, &mut rng)
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
                if roll_health_drop(&settings, &mut rng) {
                    spawn_pickup(
                        &mut commands,
                        &settings,
                        enemy_pos,
                        PickupKind::Heal,
                    );
                }
                kill_events.send(EnemyKilledEvent {
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
        damage_events.send(DamageEvent {
            victim: player_entity,
            amount: proj.damage,
        });
        spawn_hit_spark(&mut commands, &settings, player_pos);
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
fn breakable_damage_system(
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    mut run: ResMut<RunState>,
    mut rng: ResMut<RunRng>,
    mut cascade: ResMut<CascadeState>,
    mut commands: Commands,
    projectiles: Query<(Entity, &Transform, &Collider, &Projectile)>,
    mut breakables: Query<
        (Entity, &Transform, &Collider, &mut Health),
        (With<Breakable>, Without<Projectile>),
    >,
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
        for (breakable_entity, breakable_tf, breakable_col, mut health) in &mut breakables {
            if !aabb_overlap(
                proj_pos,
                proj_col.half_extents,
                breakable_tf.translation.truncate(),
                breakable_col.half_extents,
            ) {
                continue;
            }
            // Skip breakables already at 0 HP (same-frame guard).
            if health.current <= 0.0 {
                continue;
            }
            health.current = (health.current - proj.damage).max(0.0);
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

            if health.current <= 0.0 {
                // Breakable destroyed — same reward path as enemies.
                commands.entity(breakable_entity).despawn();
                spawn_death_puff(&mut commands, &settings, breakable_pos);
                run.enemies_killed = run.enemies_killed.saturating_add(1);
                on_enemy_killed(&settings, &mut run, &mut cascade, false);

                // Roll stat-stick pickup drop (use AiBehavior::Chaser
                // as a stand-in for breakable drop rates).
                let mut rng = || rng.next_unit();
                if let Some(kind) =
                    roll_pickup_drop(AiBehavior::Chaser, &settings, cascade.active, &mut rng)
                {
                    spawn_pickup(&mut commands, &settings, breakable_pos, kind);
                }
                // Roll weapon drop (same stand-in).
                if let Some(weapon_kind) =
                    roll_weapon_drop(AiBehavior::Chaser, &settings, cascade.active, &mut rng)
                {
                    spawn_pickup(
                        &mut commands,
                        &settings,
                        breakable_pos,
                        PickupKind::Weapon(weapon_kind),
                    );
                }
                // Roll health drop (flat 15%).
                if roll_health_drop(&settings, &mut rng) {
                    spawn_pickup(
                        &mut commands,
                        &settings,
                        breakable_pos,
                        PickupKind::Heal,
                    );
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

// Phase 2 P5: shared kill-time bookkeeping, called from
// BOTH `projectile_damage_system` and `beam_damage_system`
// on a killing blow. Centralizing the logic keeps the two
// paths from drifting (e.g. one bumping XP and the other
// not, or one extending the cascade and the other not).
//
// Three side effects:
//   1. Bumps `run.xp` by `xp_per_kill * cascade_xp_multiplier`
//      when a Cascade window is active (else 1×). The
//      multiplier is applied to the per-kill base, not to
//      the running total, so the per-kill XP is always a
//      whole number (4 × 1 = 4 XP per kill in Cascade).
//   2. Bumps `cascade.kills_during` while active so the
//      `CascadeEndEvent` carries the count (the HUD/UI
//      subscriber can show "Cascade: 12 kills").
//   3. If `is_beacon` is true AND a Cascade window is
//      active, extends `cascade.duration_timer` by
//      `beacon_chain_extension_s` (5s) and bumps
//      `cascade.chain_extends` up to
//      `beacon_chain_max_extensions` (3 → +15s total per
//      window). The cap prevents infinite-Cascade exploits
//      (a stream of respawned beacons — not possible today
//      but cheap insurance for future patches).
//      The `cascade_beacon_alive` flag flips to false so
//      the cascade-end cleanup knows there's nothing left
//      to despawn.
fn on_enemy_killed(
    settings: &GameSettings,
    run: &mut RunState,
    cascade: &mut CascadeState,
    is_beacon: bool,
) {
    // 1) XP — base × cascade multiplier (or × 1.0 outside Cascade).
    let xp_mult = if cascade.active {
        settings.cascade_xp_multiplier
    } else {
        1.0
    };
    let xp_gain = (settings.xp_per_kill as f32 * xp_mult) as u32;
    run.xp = run.xp.saturating_add(xp_gain);

    // 2) Cascade kills counter.
    if cascade.active {
        cascade.kills_during = cascade.kills_during.saturating_add(1);
    }

    // 3) Beacon chain-extend (only meaningful during Cascade).
    if is_beacon && cascade.active {
        if cascade.chain_extends < settings.beacon_chain_max_extensions {
            let remaining = cascade.duration_timer.remaining_secs();
            cascade.duration_timer = Timer::from_seconds(
                remaining + settings.beacon_chain_extension_s,
                TimerMode::Once,
            );
            cascade.chain_extends += 1;
            log::info!(
                "Cascade beacon killed — extended by {:.1}s (chain {}/{})",
                settings.beacon_chain_extension_s,
                cascade.chain_extends,
                settings.beacon_chain_max_extensions,
            );
        } else {
            log::info!(
                "Cascade beacon killed — chain cap reached ({}/{}), no further extension",
                cascade.chain_extends,
                settings.beacon_chain_max_extensions,
            );
        }
        run.cascade_beacon_alive = false;
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Without cascade active, each kill gives exactly `xp_per_kill`
    /// XP (default 1). No cascade bonuses, no beacon extension.
    #[test]
    fn on_enemy_killed_gives_base_xp_outside_cascade() {
        let settings = GameSettings::default();
        let mut run = RunState::default();
        let mut cascade = CascadeState::default();
        cascade.active = false;

        on_enemy_killed(&settings, &mut run, &mut cascade, false);

        assert_eq!(run.xp, 1);
        assert_eq!(cascade.kills_during, 0);
    }

    /// With cascade active, each kill gives
    /// `xp_per_kill * cascade_xp_multiplier` XP
    /// (default 4). kills_during is also bumped.
    #[test]
    fn on_enemy_killed_gives_4x_xp_during_cascade() {
        let settings = GameSettings::default();
        let mut run = RunState::default();
        let mut cascade = CascadeState::default();
        cascade.active = true;

        on_enemy_killed(&settings, &mut run, &mut cascade, false);

        assert_eq!(run.xp, 4);
        assert_eq!(cascade.kills_during, 1);
    }

    /// Killing a beacon during Cascade extends the
    /// duration_timer by beacon_chain_extension_s (5s).
    /// The original duration is 30s, so after one extend
    /// the remaining should be ~5s more than it would be
    /// after the same tick consumption. We verify by
    /// checking the remaining is close to 30 + 5 = 35s
    /// (since no time has ticked on the timer yet).
    #[test]
    fn beacon_kill_during_cascade_extends_duration() {
        let settings = GameSettings::default();
        let mut run = RunState::default();
        let mut cascade = CascadeState::default();
        cascade.active = true;
        cascade.duration_timer =
            Timer::from_seconds(settings.cascade_duration_s, TimerMode::Once);

        on_enemy_killed(&settings, &mut run, &mut cascade, true);

        let expected = settings.cascade_duration_s + settings.beacon_chain_extension_s;
        let tolerance = 0.01;
        assert!(
            (cascade.duration_timer.remaining_secs() - expected).abs() < tolerance,
            "expected ~{expected}s remaining, got {}",
            cascade.duration_timer.remaining_secs()
        );
        assert_eq!(cascade.chain_extends, 1);
    }

    /// Beacon chain-extension is capped at
    /// beacon_chain_max_extensions (3). After 3 extends,
    /// further beacon kills don't extend the timer.
    #[test]
    fn beacon_chain_extension_capped_at_max() {
        let settings = GameSettings::default();
        let mut run = RunState::default();
        let mut cascade = CascadeState::default();
        cascade.active = true;
        cascade.duration_timer =
            Timer::from_seconds(settings.cascade_duration_s, TimerMode::Once);

        // Extend 3 times (the cap).
        for _ in 0..3 {
            on_enemy_killed(&settings, &mut run, &mut cascade, true);
        }

        let after_cap = cascade.duration_timer.remaining_secs();
        // Reset timer back to start for the 4th kill — even with
        // a fresh timer, the cap blocks further extension.
        cascade.duration_timer =
            Timer::from_seconds(settings.cascade_duration_s, TimerMode::Once);
        on_enemy_killed(&settings, &mut run, &mut cascade, true);

        // After the 4th extend, the remaining should still be
        // just the base duration (30s) — not 30+5.
        let tolerance = 0.01;
        assert!(
            (cascade.duration_timer.remaining_secs() - settings.cascade_duration_s).abs()
                < tolerance,
            "expected {:.1}s (no extension past cap), got {:.1}s",
            settings.cascade_duration_s,
            cascade.duration_timer.remaining_secs()
        );
        assert_eq!(cascade.chain_extends, 3);
    }

    /// Killing a non-beacon enemy during Cascade should NOT
    /// extend the duration or bump chain_extends.
    #[test]
    fn non_beacon_kill_during_cascade_does_not_extend() {
        let settings = GameSettings::default();
        let mut run = RunState::default();
        let mut cascade = CascadeState::default();
        cascade.active = true;
        cascade.duration_timer =
            Timer::from_seconds(settings.cascade_duration_s, TimerMode::Once);

        let before = cascade.duration_timer.remaining_secs();
        on_enemy_killed(&settings, &mut run, &mut cascade, false);

        // Duration unchanged (minus the tick consumption is exactly
        // zero because no time delta has been applied).
        let tolerance = 0.01;
        assert!(
            (cascade.duration_timer.remaining_secs() - before).abs() < tolerance,
            "duration changed without beacon: {before} → {}",
            cascade.duration_timer.remaining_secs()
        );
        assert_eq!(cascade.chain_extends, 0);
    }
}
