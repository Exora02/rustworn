//! Wave spawner system — the main room-spawn tick.
//!
//! Ticks the WaveSpawner in the current room, spawning enemies
//! via the director budget system. Also contains the director
//! budget pick, credit rate calculation, elite color tint, and
//! the dispatch functions that call individual spawn functions.

use bevy::prelude::*;
use log::info;

use crate::components::{
    EnemyArchetype, Player, RoomKind,
    WaveSpawner,
};
use crate::events::RoomClearedEvent;
use crate::resources::{BossRampTimer, FloorState, GameSettings, Hitstop, RunRng, RunState};

// Re-use constants from spawn module
use super::spawn;

/// Per-room-type director credit accumulation rate (credits/s).
pub fn director_credit_rate(kind: RoomKind) -> f32 {
    match kind {
        RoomKind::Horde => spawn::CREDIT_RATE_HORDE,
        RoomKind::MiniBoss | RoomKind::Boss => spawn::CREDIT_RATE_ELITE,
        _ => spawn::CREDIT_RATE_FIGHT,
    }
}

/// Apply gold-tinted elite overlay to a base color.
pub fn elite_color_tint(base: Color) -> Color {
    let srgba = base.to_srgba();
    let elite = spawn::ELITE_TINT.to_srgba();
    Color::srgb(
        srgba.red * 0.6 + elite.red * 0.4,
        srgba.green * 0.6 + elite.green * 0.4,
        srgba.blue * 0.6 + elite.blue * 0.4,
    )
}

/// Director budget pick: spend accumulated credits to buy an
/// enemy archetype. Returns (archetype, is_elite). If credits
/// are too low for any enemy, falls back to a free chaser.
pub fn director_pick(spawner: &mut WaveSpawner, rng: &mut RunRng) -> (EnemyArchetype, bool) {
    let candidates = [
        (EnemyArchetype::Chaser, spawn::ENEMY_COST_CHASER),
        (EnemyArchetype::Dasher, spawn::ENEMY_COST_DASHER),
        (EnemyArchetype::Shooter, spawn::ENEMY_COST_SHOOTER),
        (EnemyArchetype::Sniper, spawn::ENEMY_COST_SNIPER),
        (EnemyArchetype::Bomber, spawn::ENEMY_COST_BOMBER),
        (EnemyArchetype::Orbiter, spawn::ENEMY_COST_ORBITER),
    ];

    let affordable: Vec<&(EnemyArchetype, f32)> =
        candidates.iter().filter(|(_, c)| *c <= spawner.director_credits).collect();
    if affordable.is_empty() {
        return (EnemyArchetype::Chaser, false);
    }

    let idx = (rng.next_unit() * affordable.len() as f32) as usize;
    let (archetype, base_cost) = *affordable[idx.min(affordable.len() - 1)];

    let is_elite = spawner.director_credits >= base_cost * spawn::ELITE_COST_MULTIPLIER
        && rng.next_unit() < spawn::ELITE_CHANCE;

    let cost = if is_elite {
        base_cost * spawn::ELITE_COST_MULTIPLIER
    } else {
        base_cost
    };
    spawner.director_credits = (spawner.director_credits - cost).max(0.0);

    (archetype, is_elite)
}

/// Spawn a single enemy of `archetype` at a random position in
/// the current room. Reads randomness from the per-run `RunRng`.
pub fn spawn_wave_enemy(
    commands: &mut Commands,
    settings: &GameSettings,
    biome: &FloorState,
    rng: &mut RunRng,
    player_pos: Vec2,
    archetype: EnemyArchetype,
    is_elite: bool,
) {
    let room = biome
        .rooms
        .iter()
        .find(|r| r.id == biome.current_room)
        .expect("current room must exist in biome");
    let spawn_pos = spawn::random_spawn_position(settings, player_pos, room, rng);
    match archetype {
        EnemyArchetype::Chaser => spawn::spawn_chaser(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::Dasher => spawn::spawn_dasher(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::Shooter => spawn::spawn_shooter(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::Sniper => spawn::spawn_sniper(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::Bomber => spawn::spawn_bomber(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::Orbiter => spawn::spawn_orbiter(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::Boss | EnemyArchetype::Beacon => {}
    }
}

/// Dispatch on the spawner's archetype. New archetypes slot in here.
pub fn spawn_enemy(
    commands: &mut Commands,
    settings: &GameSettings,
    biome: &FloorState,
    player_pos: Vec2,
    archetype: EnemyArchetype,
) {
    let room = biome
        .rooms
        .iter()
        .find(|r| r.id == biome.current_room)
        .expect("current room must exist in biome");
    match archetype {
        EnemyArchetype::Chaser => spawn::spawn_chaser(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::Dasher => spawn::spawn_dasher(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::Shooter => spawn::spawn_shooter(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::Sniper => spawn::spawn_sniper(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::Bomber => spawn::spawn_bomber(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::Orbiter => spawn::spawn_orbiter(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::Boss | EnemyArchetype::Beacon => {}
    }
}

/// The main wave-spawner tick system. Runs once per frame per room.
pub fn wave_spawner_tick_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    hitstop: Option<Res<Hitstop>>,
    biome: Res<FloorState>,
    mut rng: ResMut<RunRng>,
    mut commands: Commands,
    mut spawners: Query<&mut WaveSpawner>,
    player_query: Query<&Transform, With<Player>>,
    mut room_cleared_events: EventWriter<RoomClearedEvent>,
    boss_ramp: Option<Res<BossRampTimer>>,
    run: Res<RunState>,
) {
    if hitstop.is_some() {
        return;
    }
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();
    let dt = time.delta_secs();

    for mut spawner in &mut spawners {
        if spawner.room_id != biome.current_room {
            continue;
        }

        // ─── Horde-mode: skip the initial wave phase ──────
        if spawner.current_wave < spawner.waves.len() {
            spawner.current_wave = spawner.waves.len();
        }

        // ─── Farming phase ─────────────────────────────────
        spawner.wave_timer.tick(time.delta());
        if !spawner.wave_timer.finished() {
            continue;
        }

        // Accumulate elapsed farming time for universal ramp.
        spawner.farming_elapsed += dt;

        // Clear delay countdown
        if spawner.clear_delay > 0.0 {
            spawner.clear_delay -= dt;
            if spawner.clear_delay <= 0.0 {
                spawner.clear_delay = 0.0;
                room_cleared_events.write(RoomClearedEvent {
                    room_id: spawner.room_id,
                });
                info!(
                    "Room {} cleared after {:.0}s — teleport pads open",
                    spawner.room_id.0, spawner.farming_elapsed
                );
            }
        }

        // ─── Spawn interval computation ───────────────────
        let time_before_clear = (60.0 - spawner.clear_delay).max(0.0);
        let slow_factor = 1.0 / (1.0 + time_before_clear * 0.008);
        let fast_factor = if spawner.clear_delay <= 0.0 {
            let post_clear = spawner.farming_elapsed - 60.0;
            1.0 / (1.0 + post_clear.max(0.0) * 0.04)
        } else {
            1.0
        };
        let universal_factor = slow_factor * fast_factor;
        let boss_factor = boss_ramp
            .as_ref()
            .map(|ramp| {
                let progress = (ramp.timer.elapsed_secs() / ramp.timer.duration().as_secs_f32())
                    .clamp(0.0, 1.0);
                1.0 - (1.0 - 0.28) * progress
            })
            .unwrap_or(1.0);
        let ramp = (universal_factor * boss_factor).max(0.08);
        let curse_factor = 1.0 / (1.0 + run.curse as f32 * 0.10);
        let interval = (spawner.farming_interval * ramp * curse_factor).max(0.08);

        // Director picks the next enemy archetype
        let room_kind = biome
            .rooms
            .iter()
            .find(|r| r.id == biome.current_room)
            .map(|r| r.kind)
            .unwrap_or(RoomKind::Fight);
        let credit_rate = director_credit_rate(room_kind);
        spawner.director_credits = (spawner.director_credits + credit_rate * dt).min(spawn::CREDITS_MAX);
        let (archetype, is_elite) = director_pick(&mut spawner, &mut rng);

        spawn_wave_enemy(
            &mut commands,
            &settings,
            &biome,
            &mut rng,
            player_pos,
            archetype,
            is_elite,
        );
        spawner.spawned_this_wave += 1;
        spawner.wave_timer = Timer::from_seconds(interval, TimerMode::Once);
    }
}