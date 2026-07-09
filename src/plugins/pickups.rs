//! Pickup system. Pickups are world-space entities that drift toward
//! the player over their lifetime and apply a permanent-for-the-run
//! effect on contact. The four kinds (Regen, DamageUp, FireRateUp,
//! SpeedUp) are defined in `components::PickupKind`.
//!
//! The drop happens at the kill site (in `combat.rs` / `enemy.rs`),
//! not here. This module owns the *lifecycle* of a pickup: drift,
//! auto-collect, despawn on lifetime expiry, and applying the
//! effect to the player on collect.

use bevy::prelude::*;

use crate::components::{
    AiBehavior, AppState, Health, Pickup, PickupKind, Player, Velocity, WeaponArchetype,
    WeaponInventory,
};
use crate::events::PickupCollectedEvent;
use crate::resources::{GameSettings, RunState};

const PICKUP_SIZE: f32 = 14.0;

pub struct PickupsPlugin;

impl Plugin for PickupsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                pickup_drift_system,
                pickup_collect_system,
                pickup_lifetime_system,
                regen_tick_system,
            )
                .chain()
                // Phase 2: pickups freeze during MainMenu
                // (no run to lose them in) and GameOver (world
                // is frozen, pickups shouldn't drift off-screen
                // while the player reads the death screen).
                .run_if(in_state(AppState::InRun)),
        );
    }
}

/// Per-frame: drift each live pickup toward the player, and
/// despawn on lifetime expiry. Drift speed and lifetime are
/// on `GameSettings`. A pickup that touches the player is
/// collected (handled in `pickup_collect_system`); one that
/// expires is silently despawned (its drop was missed).
fn pickup_drift_system(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    player_q: Query<&Transform, With<Player>>,
    mut pickup_q: Query<(Entity, &mut Pickup, &mut Transform, &mut Velocity), Without<Player>>,
) {
    let Ok(player_tf) = player_q.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    let dt = time.delta_secs();
    for (entity, mut pickup, mut tf, mut vel) in &mut pickup_q {
        // Drift toward player only if outside collect radius.
        // Inside the radius, the collect system handles the
        // pickup this frame and we don't want to also pull
        // it inward.
        let to_player = player_pos - tf.translation.truncate();
        let dist = to_player.length();
        if dist > settings.pickup_collect_radius {
            let dir = if dist > 0.0 {
                to_player / dist
            } else {
                Vec2::ZERO
            };
            vel.0 = dir * settings.pickup_drift_speed;
        } else {
            vel.0 = Vec2::ZERO;
        }
        tf.translation += vel.0.extend(0.0) * dt;
        pickup.lifetime.tick(time.delta());
        if pickup.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Per-frame: collect any pickup whose center is inside the
/// collect radius. On collect: apply the effect, despawn the
/// pickup. Effects are stored on `RunState` so they persist
/// across the run; weapons read the multipliers in
/// `combat::weapon_fire_system` (for damage and fire rate) and
/// `player::player_movement` reads the speed multiplier.
fn pickup_collect_system(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut run: ResMut<RunState>,
    player_q: Query<&Transform, With<Player>>,
    pickup_q: Query<(Entity, &Pickup, &Transform)>,
    mut health_q: Query<&mut Health, With<Player>>,
    mut inventory_q: Query<&mut WeaponInventory, With<Player>>,
    mut pickup_events: EventWriter<PickupCollectedEvent>,
) {
    let Ok(player_tf) = player_q.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    for (entity, pickup, tf) in &pickup_q {
        let pos = tf.translation.truncate();
        if pos.distance(player_pos) > settings.pickup_collect_radius {
            continue;
        }
        apply_pickup(
            pickup.kind,
            &mut run,
            &settings,
            &mut health_q,
            &mut inventory_q,
        );
        // Emit pickup-collected event for the audio system.
        pickup_events.write(PickupCollectedEvent {
            kind: pickup.kind,
        });
        // Phase 4 HUD juice: spawn a floating pickup popup text
        // at the pickup's position before it despawns.
        let pos = tf.translation.truncate();
        crate::plugins::hud::juice::push_pickup_popup(&mut commands, pickup.kind, pos);
        commands.entity(entity).despawn();
    }
}

/// Apply the effect of a picked-up `PickupKind` to the run
/// state. The damage/fire-rate/speed multipliers stack
/// multiplicatively (1.0 + n × per_stack), so 2 DamageUp =
/// 1.5625× damage. Regen is special-cased: it doesn't stack
/// a multiplier, it stacks a flat HP/sec regen. Each Regen
/// pickup adds another +0.5 HP/sec to the running total.
///
/// Weapon pickups are special-cased too: they add the
/// archetype to the first empty `WeaponInventory` slot.
/// If the inventory is full, the pickup is silently
/// discarded (no swap UI in P4 — that's a future patch
/// with a confirmation prompt).
fn apply_pickup(
    kind: PickupKind,
    run: &mut RunState,
    settings: &GameSettings,
    health_q: &mut Query<&mut Health, With<Player>>,
    inventory_q: &mut Query<&mut WeaponInventory, With<Player>>,
) {
    match kind {
        PickupKind::DamageUp => {
            run.damage_stacks = (run.damage_stacks + 1).min(8);
        }
        PickupKind::FireRateUp => {
            run.fire_rate_stacks = (run.fire_rate_stacks + 1).min(8);
        }
        PickupKind::SpeedUp => {
            run.speed_stacks = (run.speed_stacks + 1).min(8);
        }
        PickupKind::Regen => {
            run.regen_per_sec += settings.pickup_regen_hp_per_sec;
        }
        PickupKind::Heal => {
            // Heal for +3 HP (one-shot heal orb). The universal
            // +1 heal below also applies for non-weapon pickups,
            // giving +4 total — intentional.
            if let Ok(mut h) = health_q.single_mut() {
                h.current = (h.current + 3.0).min(h.max);
            }
        }
        PickupKind::Weapon(archetype) => {
            if let Ok(mut inv) = inventory_q.single_mut() {
                if let Some(slot) = inv.slots.iter_mut().find(|s| s.is_none()) {
                    *slot = Some(archetype);
                }
                // Inventory full → silently discard the
                // pickup. The despawn already happened in
                // `pickup_collect_system`; we just don't
                // modify the inventory. Future P5+ patch
                // can add a swap prompt here.
            }
        }
        PickupKind::Gold(amt) => {
            run.gold = run.gold.saturating_add(amt);
        }
    }
    // Heal a small chunk on every pickup so the player can
    // feel the collect even when the stacked effect is small.
    // Skipped for weapon pickups and gold: the player already has the
    // weapon as the reward, the +1 HP is redundant; gold is currency.
    if !matches!(kind, PickupKind::Weapon(_) | PickupKind::Gold(_)) {
        if let Ok(mut h) = health_q.single_mut() {
            h.current = (h.current + 1.0).min(h.max);
        }
    }
}

/// Per-frame regen: if `RunState.regen_per_sec > 0`, add
/// `regen_per_sec * dt` to the player's HP, capped at max.
/// Capped stacks on the multipliers (max 8 per kind) keep
/// the run from snowballing; regen has no cap because the
/// HP cap is the natural ceiling.
pub fn regen_tick_system(
    time: Res<Time>,
    run: Res<RunState>,
    mut health_q: Query<&mut Health, With<Player>>,
) {
    if run.regen_per_sec <= 0.0 {
        return;
    }
    let dt = time.delta_secs();
    if let Ok(mut h) = health_q.single_mut() {
        h.current = (h.current + run.regen_per_sec * dt).min(h.max);
    }
}

/// Convenience: roll whether a kill of `behavior` should drop a
/// pickup. Returns `Some(PickupKind)` on a successful roll, `None`
/// otherwise. The drop chance is per-archetype on `GameSettings`.
/// The `rng` parameter is a closure that returns a `f32` in [0, 1)
/// for testability — production calls a deterministic hash of
/// the kill counter (Phase 2 will swap in seeded RNG).
///
/// Phase 2 P5: when `cascade_active` is true, the drop
/// chance is multiplied by `GameSettings::cascade_loot_multiplier`
/// (default 4.0). This is the "4× loot rarity roll"
/// promise from `phase_2_run_structure.md` P5 — during a
/// Cascade window, the same enemy is 4× as likely to
/// drop a stat-stick. Capped at 1.0 so a chaser's
/// 0.25 base × 4 = 1.0 (guaranteed drop) — at that point
/// the cap kicks in and the chance stops growing. The
/// stat-stick KIND roll (DamageUp / FireRateUp / etc.) is
/// unchanged; "rarity" in the design sense is "more
/// drops per kill", not "better tier drops".
pub fn roll_pickup_drop(
    behavior: AiBehavior,
    settings: &GameSettings,
    cascade_active: bool,
    curse: u32,
    rng: &mut impl FnMut() -> f32,
) -> Option<PickupKind> {
    let chance = match behavior {
        AiBehavior::Chaser => settings.pickup_drop_chance_chaser,
        AiBehavior::Dasher => settings.pickup_drop_chance_dasher,
        AiBehavior::Shooter => settings.pickup_drop_chance_shooter,
        AiBehavior::Sniper => settings.pickup_drop_chance_sniper,
        AiBehavior::Boss => settings.pickup_drop_chance_boss,
        AiBehavior::Beacon => 0.0,
        // Phase 3d: Bomber and Orbiter use the same drop
        // chance as their closest archetype (Chaser for
        // Bomber, Sniper for Orbiter) — no unique loot
        // tables yet. Phase 4 will differentiate.
        AiBehavior::Bomber => settings.pickup_drop_chance_chaser,
        AiBehavior::Orbiter => settings.pickup_drop_chance_sniper,
    };
    let cascade_mult = if cascade_active {
        settings.cascade_loot_multiplier
    } else {
        1.0
    };
    // Curse scales loot: each curse point adds ~15% drop chance.
    let curse_mult = 1.0 + curse as f32 * 0.15;
    if rng() >= (chance * cascade_mult * curse_mult).min(1.0) {
        return None;
    }
    // Equal-weight roll for the kind. Phase 1 has 4 kinds;
    // Phase 2 will weight these (more DamageUp / Regen
    // than FireRateUp / SpeedUp, since the weapons feel
    // less differentiated and stat boosts feel more
    // impactful). The shape is "1 of N" — `rng` × 4
    // truncated.
    let r = (rng() * 4.0) as u32;
    Some(match r {
        0 => PickupKind::DamageUp,
        1 => PickupKind::FireRateUp,
        2 => PickupKind::SpeedUp,
        _ => PickupKind::Regen,
    })
}

/// Phase 2.4 (P4 loot drops): roll whether a kill of
/// `behavior` should drop a WEAPON pickup (vs the stat-stick
/// roll in `roll_pickup_drop`). Both rolls are independent —
/// a single kill can drop a stat-stick AND a weapon.
///
/// Returns `Some(WeaponArchetype)` on a successful roll,
/// `None` otherwise. The dropped archetype is
/// **enemies-don't-carry-weapons** for Phase 2.4: we
/// roll uniformly across the 4 player weapon archetypes
/// (Pistol / Shotgun / Burst / Beam). Future patches can
/// tie drops to the killing archetype (e.g. snipers
/// always drop Sniper-class weapons) but that's a
/// design decision for the rarity/mod-system patch.
///
/// Like `roll_pickup_drop`, the `rng` is a closure
/// returning a `f32` in [0, 1) for testability.
///
/// Phase 2 P5: `cascade_active` multiplies the chance
/// by `cascade_loot_multiplier` (default 4.0). The 4× loot
/// applies to BOTH the stat-stick roll (above) and the
/// weapon roll (here) so a Cascade window is a meaningful
/// "burst of progress" for both kinds of drop. Capped at
/// 1.0 so the boss's 1.0 base × 4 = 1.0 (no change) and
/// the snipers' 0.25 × 4 = 1.0 (guaranteed drop in
/// cascade).
pub fn roll_weapon_drop(
    behavior: AiBehavior,
    settings: &GameSettings,
    cascade_active: bool,
    curse: u32,
    rng: &mut impl FnMut() -> f32,
) -> Option<WeaponArchetype> {
    let chance = match behavior {
        AiBehavior::Chaser => settings.pickup_drop_chance_weapon_chaser,
        AiBehavior::Dasher => settings.pickup_drop_chance_weapon_dasher,
        AiBehavior::Shooter => settings.pickup_drop_chance_weapon_shooter,
        AiBehavior::Sniper => settings.pickup_drop_chance_weapon_sniper,
        AiBehavior::Boss => settings.pickup_drop_chance_weapon_boss,
        // Beacon: no weapon drop (its reward is the
        // cascade chain extension). Listed for exhaustiveness.
        AiBehavior::Beacon => 0.0,
        // Phase 3d: same treatment as roll_pickup_drop —
        // Bomber uses chaser rate, Orbiter uses sniper rate.
        AiBehavior::Bomber => settings.pickup_drop_chance_weapon_chaser,
        AiBehavior::Orbiter => settings.pickup_drop_chance_weapon_sniper,
    };
    let cascade_mult = if cascade_active {
        settings.cascade_loot_multiplier
    } else {
        1.0
    };
    // Curse scales loot: each curse point adds ~15% drop chance.
    let curse_mult = 1.0 + curse as f32 * 0.15;
    if rng() >= (chance * cascade_mult * curse_mult).min(1.0) {
        return None;
    }
    // Roll uniformly across the 4 player weapon
    // archetypes. `as u32` truncates, so 0.9999 → 3
    // (Beam), giving the roll a tiny upward bias toward
    // Beam but it's a 4-way uniform for practical
    // purposes.
    let r = (rng() * 4.0) as u32;
    Some(match r {
        0 => WeaponArchetype::Pistol,
        1 => WeaponArchetype::Shotgun,
        2 => WeaponArchetype::Burst,
        _ => WeaponArchetype::Beam,
    })
}

/// Roll a flat `health_drop_chance` to drop a Heal orb.
/// Unlike stat-stick / weapon drops, the health drop is
/// NOT per-archetype — it's a flat 15% roll on every
/// enemy (and breakable) kill, independent of the other
/// rolls. A single kill can drop a stat-stick, a weapon,
/// AND a health orb.
///
/// Returns `true` if the roll succeeded.
pub fn roll_health_drop(settings: &GameSettings, curse: u32, rng: &mut impl FnMut() -> f32) -> bool {
    let curse_mult = 1.0 + curse as f32 * 0.15;
    rng() < (settings.health_drop_chance * curse_mult).min(1.0)
}

/// Phase 3c: roll a gold drop on kill. 50% base chance for
/// regular enemies, 75% for elites. Drops 2-5 gold.
/// Called from the kill handlers in combat.rs.
pub fn roll_gold_drop(is_elite: bool, rng: &mut impl FnMut() -> f32) -> Option<u32> {
    let chance = if is_elite { 0.75 } else { 0.50 };
    if rng() < chance {
        Some(2 + (rng() * 4.0) as u32) // 2-5 gold
    } else {
        None
    }
}

/// Lifetime: separate from the per-frame drift + tick, so the
/// drift system can focus on motion and the lifetime is a
/// background tick. (Both are checked each frame in the same
/// pass; the split is logical, not physical.) Exposed as a
/// system label in case Phase 2 wants to insert pickup visual
/// effects (glow, audio sting) on the despawn frame.
fn pickup_lifetime_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Pickup)>,
) {
    for (entity, mut pickup) in &mut q {
        pickup.lifetime.tick(time.delta());
        if pickup.lifetime.finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Spawn a pickup of `kind` at `pos`. The pickup is a small
/// world-space sprite that drifts toward the player over its
/// lifetime. The visual is a colored square (one color per
/// kind) so the player can read the kind at a glance — no
/// text needed.
///
/// Phase 2.4 (P4): weapon pickups get a distinct
/// orange-amber color and a slightly larger size (24u
/// square vs 14u) so they read as "rare" — bigger and
/// warmer than the stat-stick pickups.
pub fn spawn_pickup(commands: &mut Commands, settings: &GameSettings, pos: Vec2, kind: PickupKind) {
    let (color, size) = match kind {
        PickupKind::Regen => (Color::srgb(0.30, 0.95, 0.30), PICKUP_SIZE),
        PickupKind::DamageUp => (Color::srgb(0.95, 0.30, 0.30), PICKUP_SIZE),
        PickupKind::FireRateUp => (Color::srgb(0.95, 0.95, 0.30), PICKUP_SIZE),
        PickupKind::SpeedUp => (Color::srgb(0.30, 0.65, 0.95), PICKUP_SIZE),
        PickupKind::Weapon(_) => (
            Color::srgb(1.0, 0.55, 0.10), // orange — same family as the
            // player's PLAYER_COLOR but
            // brighter, so weapon pickups
            // pop against the background
            // but don't blend with the
            // player sprite.
            24.0, // bigger than stat-sticks
        ),
        PickupKind::Heal => (
            Color::srgb(0.20, 0.85, 0.60), // teal/green — reads as
            // "health" in arcade color
            // shorthand, distinct from
            // Regen's green.
            PICKUP_SIZE,
        ),
        PickupKind::Gold(_) => (
            Color::srgb(0.95, 0.85, 0.10), // bright gold — coin
            PICKUP_SIZE,
        ),
    };
    commands.spawn((
        Pickup {
            kind,
            lifetime: Timer::from_seconds(settings.pickup_lifetime_s, TimerMode::Once),
        },
        Sprite {
            color,
            custom_size: Some(Vec2::splat(size)),
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, 0.3),
        Velocity(Vec2::ZERO),
        Visibility::Inherited,
    ));
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::WeaponArchetype;

    fn settings() -> GameSettings {
        // All weapon drop chances at 1.0 so the roll
        // always succeeds (we're testing the archetype
        // distribution, not the drop chance).
        GameSettings {
            pickup_drop_chance_weapon_chaser: 1.0,
            pickup_drop_chance_weapon_dasher: 1.0,
            pickup_drop_chance_weapon_shooter: 1.0,
            pickup_drop_chance_weapon_sniper: 1.0,
            pickup_drop_chance_weapon_boss: 1.0,
            ..Default::default()
        }
    }

    // `roll_weapon_drop` returns one of the 4 player
    // weapon archetypes. Smoke-test that the roll
    // actually fires (returns Some) when the chance
    // is 1.0 and the rng always returns 0.0.
    #[test]
    fn roll_weapon_drop_returns_some_when_chance_is_one() {
        let s = settings();
        let mut rng = || 0.0_f32; // first rng() < 1.0 → drop
        let result = roll_weapon_drop(AiBehavior::Chaser, &s, false, 0, &mut rng);
        assert!(result.is_some(), "chance=1.0 must always drop");
        assert!(matches!(
            result.unwrap(),
            WeaponArchetype::Pistol | WeaponArchetype::Shotgun | WeaponArchetype::Burst | WeaponArchetype::Beam
        ));
    }

    // `roll_weapon_drop` returns None when the chance
    // is 0.0 (or when the rng() returns >= chance).
    #[test]
    fn roll_weapon_drop_returns_none_when_chance_is_zero() {
        let s = GameSettings {
            pickup_drop_chance_weapon_chaser: 0.0,
            ..Default::default()
        };
        let mut rng = || 0.0_f32;
        assert!(roll_weapon_drop(AiBehavior::Chaser, &s, false, 0, &mut rng).is_none());
    }

    // `roll_weapon_drop` returns None when rng() >= chance.
    // With chance = 0.5 and rng = 0.6, no drop.
    #[test]
    fn roll_weapon_drop_respects_rng_threshold() {
        let s = GameSettings {
            pickup_drop_chance_weapon_chaser: 0.5,
            ..Default::default()
        };
        let mut rng = || 0.6_f32;
        assert!(roll_weapon_drop(AiBehavior::Chaser, &s, false, 0, &mut rng).is_none());

        // And with rng = 0.4 < 0.5, drop.
        let mut rng = || 0.4_f32;
        assert!(roll_weapon_drop(AiBehavior::Chaser, &s, false, 0, &mut rng).is_some());
    }

    // `WeaponInventory::default()` has Pistol in slot 0
    // and 3 empty slots. This is the starting state
    // for every run.
    #[test]
    fn weapon_inventory_default_has_pistol_in_slot_zero() {
        let inv = WeaponInventory::default();
        assert_eq!(inv.slots[0], Some(WeaponArchetype::Pistol));
        assert_eq!(inv.slots[1], None);
        assert_eq!(inv.slots[2], None);
        assert_eq!(inv.slots[3], None);
    }

    // Pickup behavior: a weapon pickup adds the
    // archetype to the first empty slot. We test
    // by simulating the same code path that
    // `apply_pickup` uses (the actual apply_pickup
    // function takes a Query, which is hard to
    // construct in a unit test without an App).
    #[test]
    fn weapon_pickup_fills_first_empty_slot() {
        let mut inv = WeaponInventory {
            slots: [Some(WeaponArchetype::Pistol), None, None, None],
        };
        // Simulate the apply_pickup logic for
        // PickupKind::Weapon(Shotgun).
        if let Some(slot) = inv.slots.iter_mut().find(|s| s.is_none()) {
            *slot = Some(WeaponArchetype::Shotgun);
        }
        assert_eq!(inv.slots[0], Some(WeaponArchetype::Pistol));
        assert_eq!(inv.slots[1], Some(WeaponArchetype::Shotgun));
        assert_eq!(inv.slots[2], None);
        assert_eq!(inv.slots[3], None);
    }

    // Pickup behavior: if the inventory is full, a
    // weapon pickup is silently discarded. The slot
    // array is unchanged.
    #[test]
    fn weapon_pickup_silently_discarded_when_inventory_full() {
        let mut inv = WeaponInventory {
            slots: [
                Some(WeaponArchetype::Pistol),
                Some(WeaponArchetype::Shotgun),
                Some(WeaponArchetype::Burst),
                Some(WeaponArchetype::Beam),
            ],
        };
        // Simulate the apply_pickup logic for
        // PickupKind::Weapon(Shotgun).
        if let Some(slot) = inv.slots.iter_mut().find(|s| s.is_none()) {
            *slot = Some(WeaponArchetype::Shotgun);
        }
        // Inventory unchanged.
        assert_eq!(inv.slots[0], Some(WeaponArchetype::Pistol));
        assert_eq!(inv.slots[1], Some(WeaponArchetype::Shotgun));
        assert_eq!(inv.slots[2], Some(WeaponArchetype::Burst));
        assert_eq!(inv.slots[3], Some(WeaponArchetype::Beam));
    }
}
