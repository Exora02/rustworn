//! Singleton resources. These are the state that lives across systems
//! but only once per app.
//!
//! For per-run state (current seed, biome, score, time remaining) see
//! `RunState` — it gets reset at the start of every run.

use bevy::prelude::*;

/// Global Cascade Mode state. One per app.
///
/// Phase 1 prototype: every `cascade_interval_s` (default 75s) a
/// 30s window opens where every `CascadeAffected` entity (enemies
/// today, enemy projectiles + particles later) runs at
/// `cascade_scale` × normal speed. The player and player
/// projectiles are not `CascadeAffected` and run at full speed.
///
/// Two timers: `interval_timer` counts down to the next window
/// opening (used while `active` is false), `duration_timer` counts
/// down the current window (used while `active` is true).
/// `cascade_state_system` flips between them and resets the
/// appropriate one. The cycle is deterministic and asymmetric
/// (long quiet, short loud) by design — see PLAN.md §2.
///
/// Phase 2+ will also drive: 4× XP / 4× loot rarity on kills
/// during Cascade, +5s window extension on Beacon kills, and a
/// damage-taken multiplier. The `kills_during` and `chain_extends`
/// fields are in place now so we can land those without another
/// resource migration.
#[derive(Resource)]
pub struct CascadeState {
    pub active: bool,
    pub time_remaining: f32,
    pub kills_during: u32,
    pub chain_extends: u32,
    pub interval_timer: Timer,
    pub duration_timer: Timer,
}

impl Default for CascadeState {
    fn default() -> Self {
        Self {
            active: false,
            time_remaining: 0.0,
            kills_during: 0,
            chain_extends: 0,
            // Start with a 30s "warmup" so the first cascade doesn't
            // hit the player the moment they spawn. The interval
            // re-arms to its full length after each cascade ends,
            // so subsequent cascades are spaced by the full default.
            interval_timer: Timer::from_seconds(30.0, TimerMode::Once),
            duration_timer: Timer::from_seconds(0.0, TimerMode::Once),
        }
    }
}

/// Per-run state. Reset on `InRun` state entry.
#[derive(Resource)]
pub struct RunState {
    pub seed: u64,
    pub biome_index: u32,
    pub time_elapsed: f32,
    pub time_cap: f32, // 300s for a 5-minute run
    pub score: u32,
    pub enemies_killed: u32,
    // The cascade beacon is gated on the boss's death — the
    // flag flips to `true` when the mini-boss's HP hits zero
    // (see `projectile_damage_system` in combat.rs). Reserved
    // for the Phase 1 cascade hook; no system reads it today,
    // but the slot is here so Phase 1 can land the spawn
    // without another resource migration.
    pub cascade_beacon_alive: bool,
    // Phase 1 mini-boss: set to `true` the frame the boss
    // spawns (via `boss_spawn_system` in player.rs). Used as
    // a one-shot gate so the boss spawns exactly once per run;
    // `restart_system` resets it back to `false` on death.
    pub boss_spawned: bool,
    // Day 15-16: when the player dies, gameplay systems freeze and a
    // restart prompt appears. The bool lives on RunState (rather than
    // a separate resource) because it's reset on the same lifecycle —
    // a proper Bevy State lands in Phase 2 alongside the main-menu /
    // game-over / pause split, and we'll lift this flag into that
    // state machine at the same time.
    pub game_over: bool,
    // Phase 1 (Vampire Survivors fusion) pickup stacks. Each
    // DamageUp / FireRateUp / SpeedUp pickup adds 1 to the
    // corresponding stack (capped at 8 per kind to prevent
    // runaway scaling). Regen is special: it stacks flat
    // HP/sec on top of a base of 0, no cap. All effects are
    // permanent for the run; `restart_system` resets them.
    pub damage_stacks: u32,
    pub fire_rate_stacks: u32,
    pub speed_stacks: u32,
    pub regen_per_sec: f32,
}

impl Default for RunState {
    fn default() -> Self {
        Self {
            seed: 0,
            biome_index: 0,
            time_elapsed: 0.0,
            time_cap: 300.0,
            score: 0,
            enemies_killed: 0,
            cascade_beacon_alive: false,
            boss_spawned: false,
            game_over: false,
            damage_stacks: 0,
            fire_rate_stacks: 0,
            speed_stacks: 0,
            regen_per_sec: 0.0,
        }
    }
}

/// Brief global freeze when an enemy dies. Inserted by the
/// projectile-damage system on a killing blow; ticks down each frame;
/// any gameplay system (movement, AI, fire, contact) early-returns
/// while the resource is present, so the hitstop is felt uniformly.
/// The resource is removed when the timer elapses.
#[derive(Resource)]
pub struct Hitstop {
    pub timer: Timer,
}

/// Global screen-shake state. Set to a non-zero intensity by the
/// contact-damage system when the player takes a hit; the camera
/// follow system reads the current value, adds a decaying random
/// offset, and decays the remaining time each frame.
#[derive(Resource, Default)]
pub struct ScreenShake {
    pub intensity: f32,
    pub duration: f32,
    pub remaining: f32,
}

/// Tunable gameplay constants. Lives in a resource so we can tweak
/// values from the dev console or a settings menu without recompiling.
#[derive(Resource)]
pub struct GameSettings {
    pub dash_iframe_ms: u32,
    pub cascade_scale: f32,
    pub cascade_duration_s: f32,
    pub cascade_interval_s: f32,
    pub player_move_speed: f32,
    pub player_dash_speed: f32,
    pub player_dash_duration_s: f32,
    pub player_dash_cooldown_s: f32,
    // Day 5-7 — pistol
    pub weapon_damage: f32,
    pub weapon_fire_rate: f32, // shots per second
    pub weapon_projectile_speed: f32,
    pub weapon_muzzle_offset: f32,
    pub projectile_lifetime_s: f32,
    // Day 8-10 — chaser + spawner
    pub chaser_move_speed: f32,
    pub chaser_contact_damage: f32,
    pub chaser_knockback_speed: f32,
    pub chaser_knockback_duration_s: f32,
    pub hit_iframe_duration_s: f32,
    pub spawner_interval_s: f32,
    pub spawner_min_distance: f32,
    pub spawner_max_distance: f32,
    // Day 11-12 — enemy HP + death
    pub enemy_max_hp: f32,
    pub death_puff_lifetime_s: f32,
    // Phase 1 — dasher enemy
    pub dasher_approach_speed: f32,
    pub dasher_dash_speed: f32,
    pub dasher_approach_duration_s: f32,
    pub dasher_dash_duration_s: f32,
    pub dasher_recover_duration_s: f32,
    pub dasher_max_hp: f32,
    pub dasher_contact_damage: f32,
    // Phase 1 — shooter enemy. First ranged enemy: moves toward
    // the player until at `shooter_preferred_range`, then stops
    // and fires `Team::Enemy` projectiles on a cooldown. The
    // projectile speed is intentionally slower than the player's
    // 700 so a player who sees the muzzle flash can sidestep.
    // 1 contact damage (same as chaser) so a shooter that catches
    // the player in melee range is a non-lethal trade.
    pub shooter_move_speed: f32,
    pub shooter_preferred_range: f32,
    pub shooter_fire_rate: f32, // shots per second
    pub shooter_projectile_speed: f32,
    pub shooter_projectile_damage: f32,
    pub shooter_projectile_lifetime_s: f32,
    pub shooter_max_hp: f32,
    pub shooter_contact_damage: f32,
    // Phase 1 — sniper enemy. Stops at long range and charges
    // a single high-damage shot, telegraphed by a color shift.
    // 2 damage per bullet (player has 4 HP base, so 2 sniper
    // hits = death). Bullet speed 250 is slower than the
    // shooter's 300 — but the sniper's bullet is bigger and
    // does more damage, so the trade is "easier to dodge, more
    // punishing if you don't". Post-fire cooldown of 1.0s
    // gives the player a window to close the gap and land a
    // free hit before the next charge starts.
    pub sniper_move_speed: f32,
    pub sniper_preferred_range: f32,
    pub sniper_charge_duration_s: f32,
    pub sniper_post_fire_cooldown_s: f32,
    pub sniper_projectile_speed: f32,
    pub sniper_projectile_damage: f32,
    pub sniper_projectile_lifetime_s: f32,
    pub sniper_projectile_size_x: f32,
    pub sniper_projectile_size_y: f32,
    pub sniper_max_hp: f32,
    pub sniper_contact_damage: f32,
    // Phase 1 — mini-boss. Single-instance per run, spawned
    // once at `boss_spawn_time_s` (60s default) at a random
    // angle at `boss_spawn_distance` from the player. 10 HP
    // total split evenly across two phases (5/5). Contact
    // damage 2 (matches the dasher) so a successful boss
    // contact deletes half the player's 4 HP. Phase 1
    // chases at `boss_move_speed` (60 — slower than the
    // chaser's 100, so the player can kite the boss on the
    // approach). Phase 2 enrages with a
    // `boss_phase2_speed_multiplier` (1.5×) speed boost and
    // a `boss_phase2_burst_count` (3) projectile spread
    // every `boss_phase2_attack_interval_s` (2s). The
    // boss's projectiles are slower than the shooter's
    // (200 vs 300) and do the same damage (1 HP) so a
    // clean 3-burst hit can chunk the player for 3 HP. The
    // boss itself is CascadeAffected (so Cascade Mode slows
    // both the chase and the 3-burst projectiles), and its
    // death flips `RunState::cascade_beacon_alive` to true.
    pub boss_max_hp: f32,
    pub boss_contact_damage: f32,
    pub boss_move_speed: f32,
    pub boss_phase2_speed_multiplier: f32,
    pub boss_phase2_attack_interval_s: f32,
    pub boss_phase2_burst_count: u32,
    pub boss_projectile_speed: f32,
    pub boss_projectile_damage: f32,
    pub boss_projectile_lifetime_s: f32,
    pub boss_projectile_size_x: f32,
    pub boss_projectile_size_y: f32,
    pub boss_spawn_time_s: f32,
    pub boss_spawn_distance: f32,
    // Day 13-14 — HUD + juice
    pub hit_flash_duration_s: f32,
    pub hitstop_duration_s: f32,
    pub screen_shake_intensity: f32,
    pub screen_shake_duration_s: f32,
    // Day 15-16 — polish
    pub projectile_cap: u32,
    // Phase 1 — burst weapon. Time between rounds in a single
    // burst (3 rounds per click on the burst archetype). 0.1s
    // is fast enough that the player perceives the burst as
    // "one big shot" but slow enough that the three rounds
    // are distinct visual events (you can see each bullet
    // fly). Smaller than this and it feels like a single
    // multi-pellet shot like the shotgun; larger and the
    // burst becomes "click, click, click, click" which
    // defeats the point.
    pub burst_round_gap_s: f32,
    // Phase 1 — beam weapon. The beam is a held-trigger
    // instant-hit "DPS hose" — it does NOT spawn a
    // Projectile, it applies damage directly to every
    // enemy in its line of sight on a per-tick cadence.
    //
    // beam_max_range (500 units): the line segment
    // extends from the player to player + max_range *
    // aim_dir. Enemies past this distance are not hit.
    // 500 is well past the player's view radius (the
    // view is 1280×720) so a long beam can paint across
    // the visible screen; it also matches the shooter's
    // projectile range (300 u/s × 1.5s) so the beam
    // trades DPS for "no projectile to dodge" (good
    // against clustered enemies, weak against spread-
    // out enemies).
    //
    // beam_damage_per_tick (0.5) ×
    // (1.0 / beam_tick_interval_s) (10Hz) = 5.0 DPS.
    // Slightly above the pistol's 3.0 DPS, much below
    // the shotgun's per-shot 2.5 (which is paid all at
    // once). The beam's DPS is felt as continuous
    // pressure rather than spike damage — different
    // gameplay niche.
    //
    // beam_thickness (4.0) is the visual width of
    // the beam sprite and the half-thickness used in
    // the capsule-vs-AABB damage test (an enemy is hit
    // when the perpendicular distance from the beam
    // line to the enemy center is less than
    // enemy_half_extent + beam_thickness/2). 4.0 is
    // thin enough that the beam looks like a "laser"
    // rather than a "tube", thick enough that close-
    // range fights don't feel like pixel-perfect
    // dodges.
    pub beam_max_range: f32,
    pub beam_damage_per_tick: f32,
    pub beam_tick_interval_s: f32,
    pub beam_thickness: f32,
    // Phase 1 — pickups (Vampire Survivors fusion). Each killed
    // enemy has a per-archetype chance to drop one pickup, which
    // drifts toward the player over its lifetime and auto-applies
    // its permanent-for-the-run effect on contact. The 4 kinds:
    //
    //   Regen       — +0.5 HP/sec
    //   DamageUp    — +25% on all weapons
    //   FireRateUp  — +30% on all weapons
    //   SpeedUp     — +20% move speed
    //
    // Drop rates are per-archetype: chasers are 25% (low-threat,
    // common drops), snipers are 8% (high-threat, rare drops so
    // killing them doesn't feel mandatory for the build). The
    // boss has its own higher rate so killing it always feels
    // rewarding.
    pub pickup_drop_chance_chaser: f32,
    pub pickup_drop_chance_dasher: f32,
    pub pickup_drop_chance_shooter: f32,
    pub pickup_drop_chance_sniper: f32,
    pub pickup_drop_chance_boss: f32,
    pub pickup_lifetime_s: f32,
    pub pickup_drift_speed: f32,
    pub pickup_collect_radius: f32,
    // Multipliers applied as 1.0 + value on pickup collect.
    // E.g. damage_multiplier_per_stack = 0.25 means each
    // DamageUp pickup adds 25% to base damage. Stacks
    // multiplicatively (2 DamageUp = 1.5625×, not 1.5×) so the
    // scaling has a soft ceiling and the run doesn't snowball
    // out of control.
    pub pickup_damage_per_stack: f32,
    pub pickup_fire_rate_per_stack: f32,
    pub pickup_speed_per_stack: f32,
    pub pickup_regen_hp_per_sec: f32,
    // Phase 1 — damage number visual tunables.
    pub damage_number_lifetime_s: f32,
    pub damage_number_drift_speed: f32,
    pub damage_number_font_size: f32,
    // Phase 1 — hit spark visual tunables.
    pub hit_spark_lifetime_s: f32,
    pub hit_spark_size: f32,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            dash_iframe_ms: 200,
            cascade_scale: 0.25,
            cascade_duration_s: 30.0,
            cascade_interval_s: 75.0, // 60-90s, mid-range
            player_move_speed: 220.0,
            player_dash_speed: 600.0,
            player_dash_duration_s: 0.15,
            player_dash_cooldown_s: 1.5,
            // Day 5-7 — pistol: 3 shots/sec, 2s range.
            weapon_damage: 1.0,
            weapon_fire_rate: 3.0,
            weapon_projectile_speed: 700.0,
            weapon_muzzle_offset: 25.0,
            projectile_lifetime_s: 2.0,
            // Day 8-10 — chaser is slower than the player (220),
            // deals 1 hp on contact with 0.4s knockback + 0.5s i-frames.
            chaser_move_speed: 100.0,
            chaser_contact_damage: 1.0,
            chaser_knockback_speed: 400.0,
            chaser_knockback_duration_s: 0.15,
            hit_iframe_duration_s: 0.5,
            // One chaser every 3s, spawning 400-600 units from the player.
            spawner_interval_s: 3.0,
            spawner_min_distance: 400.0,
            spawner_max_distance: 600.0,
            // Day 11-12 — chaser takes 3 hits to kill; death puff
            // is a 0.3s fading white square.
            enemy_max_hp: 3.0,
            death_puff_lifetime_s: 0.3,
            // Phase 1 — dasher. Slower approach (150 vs chaser's
            // 100) so it doesn't overtake the chaser; much faster
            // dash (450 — slower than the player's dash, faster
            // than their walk) so it punishes panicky players who
            // stand still. 0.7s approach + 0.25s dash + 0.5s
            // recover is a ~1.45s cycle that's readable but not
            // slow. Two hits to kill (vs chaser's three) so the
            // player can burst it down during Recover. 2 contact
            // damage (vs chaser's 1) so a successful dash deletes
            // half the player's HP.
            dasher_approach_speed: 150.0,
            dasher_dash_speed: 450.0,
            dasher_approach_duration_s: 0.7,
            dasher_dash_duration_s: 0.25,
            dasher_recover_duration_s: 0.5,
            dasher_max_hp: 2.0,
            dasher_contact_damage: 2.0,
            // Phase 1 — shooter. Slower than chaser (80 vs 100)
            // because the shooter doesn't need to close distance
            // quickly — it sits at 250 and plinks. 0.8 shots/sec
            // is slower than the player's pistol (3/sec) so a
            // single shooter is rarely lethal. 300 u/s projectile
            // speed is slow enough to dodge if you saw the muzzle
            // flash; combined with a 1.5s lifetime the projectile
            // range is ~450 units, well past the preferred range
            // of 250. 2 HP (one pistol shot + the second) so a
            // focused burst takes a shooter out fast.
            shooter_move_speed: 80.0,
            shooter_preferred_range: 250.0,
            shooter_fire_rate: 0.8,
            shooter_projectile_speed: 300.0,
            shooter_projectile_damage: 1.0,
            shooter_projectile_lifetime_s: 1.5,
            shooter_max_hp: 2.0,
            shooter_contact_damage: 1.0,
            // Phase 1 — sniper. Move-speed 70 (slowest of the
            // four — the sniper never wants to close the gap
            // quickly, it wants to plant and charge). Range 400
            // is further than the shooter (250) so the two
            // archetypes naturally space out and the player
            // faces ranged pressure from multiple distances.
            // Charge 1.5s is long enough to read and react
            // (the color shift starts yellow and ramps to
            // red over the full 1.5s) but short enough that
            // the sniper doesn't feel like a wait.
            // Post-fire cooldown 1.0s gives the player a clear
            // window to push in. Bullet speed 250 is dodgeable
            // even without i-frames.
            sniper_move_speed: 70.0,
            sniper_preferred_range: 400.0,
            sniper_charge_duration_s: 1.5,
            sniper_post_fire_cooldown_s: 1.0,
            sniper_projectile_speed: 250.0,
            sniper_projectile_damage: 2.0,
            sniper_projectile_lifetime_s: 2.0,
            sniper_projectile_size_x: 8.0,
            sniper_projectile_size_y: 14.0,
            sniper_max_hp: 1.0, // one pistol hit kills (1 HP)
            sniper_contact_damage: 1.0,
            // Phase 1 — mini-boss. See `boss_max_hp` above for
            // the full design notes. Numbers chosen so:
            //   - 10 HP is 5× the chaser's 3 HP, 5× the
            //     dasher's 2, 5× the shooter's 2, 10× the
            //     sniper's 1. The boss is meaningfully
            //     tankier than any other enemy.
            //   - 60s spawn time lands the boss ~halfway
            //     through the first cascade cycle (which
            //     opens at t=30s + 75s interval, so the
            //     first cascade after the boss is in
            //     progress). Cascade Mode slows the boss
            //     + projectiles, giving the player a
            //     window to land free hits.
            //   - Phase 2's 1.5× speed (90) is still
            //     slower than the chaser's 100, so the
            //     enrage doesn't make the boss faster
            //     than the smallest enemy — it just
            //     speeds up the chase so a player who
            //     was kiting has to start dodging the
            //     3-burst instead.
            //   - 3-burst spread is ~17° total (same as
            //     the shotgun), so a player running
            //     perpendicular sidesteps cleanly.
            //   - Boss projectile size 12x12 (vs
            //     shooter's 5x8) reads as a chunky
            //     bullet; the bigger AABB means the
            //     player can't thread the needle
            //     between them.
            boss_max_hp: 10.0,
            boss_contact_damage: 2.0,
            boss_move_speed: 60.0,
            boss_phase2_speed_multiplier: 1.5,
            boss_phase2_attack_interval_s: 2.0,
            boss_phase2_burst_count: 3,
            boss_projectile_speed: 200.0,
            boss_projectile_damage: 1.0,
            boss_projectile_lifetime_s: 1.5,
            boss_projectile_size_x: 12.0,
            boss_projectile_size_y: 12.0,
            boss_spawn_time_s: 60.0,
            boss_spawn_distance: 500.0,
            // Day 13-14 — enemy hit flash is a 0.1s white pulse,
            // killing blows trigger a 50ms global hitstop, and
            // taking damage shakes the camera for 0.2s at intensity 12.
            hit_flash_duration_s: 0.1,
            hitstop_duration_s: 0.05,
            screen_shake_intensity: 12.0,
            screen_shake_duration_s: 0.2,
            // Day 15-16 — hard cap of 50 live projectiles. Holding
            // fire and running around no longer spawns an unbounded
            // stream; the 51st shot is silently dropped. Bumped
            // from 30 in Phase 1 to leave headroom for the shooter's
            // own `Team::Enemy` bullets plus the player's 3-5/sec
            // + the shotgun's 5-pellet bursts. Pick this high
            // enough that legit gameplay (clearing a room) never
            // hits it, but low enough that pathological cases
            // don't tank the frame rate on weaker machines.
            projectile_cap: 50,
            // Phase 1 — burst round gap. See `Weapon::burst_rounds`
            // for the full design.
            burst_round_gap_s: 0.1,
            // Phase 1 — beam tunables. See the field-level
            // doc comments for the full design rationale.
            // 500 max range, 0.5 dmg/tick at 10Hz = 5 DPS,
            // 4-unit visual + collision thickness.
            beam_max_range: 500.0,
            beam_damage_per_tick: 0.5,
            beam_tick_interval_s: 0.1,
            beam_thickness: 4.0,
            // Phase 1 — pickup drop rates. 25% on chasers means a
            // wave of 3 chasers will drop on average ~0.75 pickups
            // (so 4 chasers → 1 pickup, 8 chasers → 2 pickups).
            // Boss drop rate is 100% so killing the boss always
            // hands the player a meaningful reward.
            pickup_drop_chance_chaser: 0.25,
            pickup_drop_chance_dasher: 0.20,
            pickup_drop_chance_shooter: 0.15,
            pickup_drop_chance_sniper: 0.08,
            pickup_drop_chance_boss: 1.0,
            pickup_lifetime_s: 8.0,
            pickup_drift_speed: 60.0,
            pickup_collect_radius: 25.0,
            // Multipliers: each pickup adds X to a 1.0 base.
            // 2 DamageUp = 1.0 × 1.25 × 1.25 = 1.5625× damage.
            // 4 of each = 1.0 × 1.25^4 = 2.44× — strong but not
            // run-winning.
            pickup_damage_per_stack: 0.25,
            pickup_fire_rate_per_stack: 0.30,
            pickup_speed_per_stack: 0.20,
            pickup_regen_hp_per_sec: 0.5,
            // Phase 1 — damage number visual tunables.
            damage_number_lifetime_s: 0.6,
            damage_number_drift_speed: 50.0,
            damage_number_font_size: 20.0,
            // Phase 1 — hit spark visual tunables.
            hit_spark_lifetime_s: 0.12,
            hit_spark_size: 8.0,
        }
    }
}
