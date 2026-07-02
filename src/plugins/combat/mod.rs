//! Combat: weapons, projectiles, and damage.
//!
//! Day 5-7: click-to-fire pistol, projectile with lifetime, no enemies yet.
//! Damage routing (Day 11-12) will land here too.

pub mod weapon;
pub mod projectile;
pub mod damage;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use log::warn;

use crate::components::{
    AimDirection, AppState, AiBehavior, Beacon, BeamVisual, Breakable, Collider, Elite, Enemy, EnemyAI,
    Health, HitFlash, Invulnerable, Knockback, PickupKind, Player, Projectile, Team, Velocity,
    Weapon, WeaponArchetype,
};
use crate::events::{DamageEvent, EnemyKilledEvent};
use crate::plugins::enemy::spawn_death_puff;
use crate::plugins::fx::spawn_hit_spark;
use crate::plugins::pickups::{roll_gold_drop, roll_health_drop, roll_pickup_drop, roll_weapon_drop, spawn_pickup};
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
                weapon::weapon_cooldown_tick_system,
                weapon::weapon_switch_system,
                weapon::weapon_fire_system,
                damage::beam_damage_system,
                projectile::projectile_lifetime_system,
                projectile::projectile_damage_system,
                // Phase 1 (shooter): mirror of `projectile_damage_system`
                // for `Team::Enemy` projectiles hitting the player.
                // Runs after `projectile_damage_system` so player-vs-enemy
                // damage resolves in the same frame as enemy-vs-player
                // damage (no priority either way — both happen on
                // contact). The two systems share the
                // `projectile_lifetime_system` for despawn-on-expire
                // so the lifetime logic isn't duplicated.
                projectile::enemy_projectile_damage_system,
                // Phase 2.5+: breakable-damage system.
                // Runs after enemy-projectile damage so breakable
                // hits resolve in the same frame as the main
                // damage pass. One projectile can break multiple
                // breakables in a single frame.
                damage::breakable_damage_system,
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
pub(crate) fn spawn_beam_visual(mut commands: Commands, settings: Res<GameSettings>) {
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

// Day 13-14: helper to early-return when a global hitstop is active.
// Used by every combat system that ticks timers, mutates transforms,
// or fires projectiles — the freeze is felt uniformly because all of
// them gate on this. The resource is removed by `hitstop_tick_system`
// in the player plugin once its timer elapses.
pub(crate) fn in_hitstop(hitstop: &Option<Res<Hitstop>>) -> bool {
    hitstop.is_some()
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
pub(crate) fn weapon_stats_for(archetype: WeaponArchetype) -> (f32, f32, f32, u32, f32, u32) {
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

// Cycle order: Pistol → Shotgun → Burst → Beam → Pistol. New
// weapons slot in here.
pub(crate) fn next_archetype(current: WeaponArchetype) -> WeaponArchetype {
    match current {
        WeaponArchetype::Pistol => WeaponArchetype::Shotgun,
        WeaponArchetype::Shotgun => WeaponArchetype::Burst,
        WeaponArchetype::Burst => WeaponArchetype::Beam,
        WeaponArchetype::Beam => WeaponArchetype::Pistol,
    }
}

// Rotate a 2D vector by `angle` radians. The default `Vec2` API
// doesn't expose a rotation helper, so this is the cheapest
// handwritten form — sin/cos once, no allocations.
pub(crate) fn rotate_vec2(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

// Project a point onto the beam's line segment, returning the
// perpendicular distance from the beam line to the point. If the
// projection falls outside the segment, returns infinity (caller
// treats as "miss"). This is the test the beam uses to decide
// whether an enemy is "in the beam" — capsule-vs-point.
pub(crate) fn point_to_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
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

// AABB-vs-AABB overlap test between two transformed entities.
// Same math as the player contact-damage system, but duplicated here
// rather than promoted to a shared util — there's only two callers
// and they live in different plugins.
pub(crate) fn aabb_overlap(pos_a: Vec2, half_a: Vec2, pos_b: Vec2, half_b: Vec2) -> bool {
    let delta = (pos_a - pos_b).abs();
    delta.x < half_a.x + half_b.x && delta.y < half_a.y + half_b.y
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
pub(crate) fn on_enemy_killed(
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

// ─── Tests ───────────────────────────────────────────────────────────

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