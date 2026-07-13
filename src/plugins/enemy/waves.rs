//! Wave spawner system — the main room-spawn tick.
//!
//! Ticks the WaveSpawner in the current room, spawning enemies
//! via the director budget system. Also contains the director
//! budget pick, credit rate calculation, elite color tint, and
//! the dispatch functions that call individual spawn functions.

use bevy::prelude::*;
use log::info;

use crate::components::{
    Biome, Enemy, EnemyArchetype, Player, RoomKind,
    WaveSpawner,
};
use crate::events::RoomClearedEvent;
use crate::resources::{BossRampTimer, FloorState, GameSettings, Hitstop, RunRng, RunState};

// Re-use constants and types from spawn module
use super::spawn;

/// Per-room-type director credit accumulation rate (credits/s).
pub fn director_credit_rate(kind: RoomKind) -> f32 {
    match kind {
        RoomKind::Horde => spawn::CREDIT_RATE_HORDE,
        RoomKind::MiniBoss | RoomKind::Boss => spawn::CREDIT_RATE_ELITE,
        RoomKind::Challenge => spawn::CREDIT_RATE_FIGHT,
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
/// `biome` adjusts the preference weights for each archetype.
pub fn director_pick(spawner: &mut WaveSpawner, rng: &mut RunRng, biome: Biome) -> (EnemyArchetype, bool) {
    let candidates = [
        (EnemyArchetype::Chaser, spawn::ENEMY_COST_CHASER),
        (EnemyArchetype::Dasher, spawn::ENEMY_COST_DASHER),
        (EnemyArchetype::Shooter, spawn::ENEMY_COST_SHOOTER),
        (EnemyArchetype::Sniper, spawn::ENEMY_COST_SNIPER),
        (EnemyArchetype::Bomber, spawn::ENEMY_COST_BOMBER),
        (EnemyArchetype::Orbiter, spawn::ENEMY_COST_ORBITER),
        // Biome-specific enemies
        (EnemyArchetype::SkeletonArcher, spawn::ENEMY_COST_SKELETON_ARCHER),
        (EnemyArchetype::Ghost, spawn::ENEMY_COST_GHOST),
        (EnemyArchetype::SkullSwarm, spawn::ENEMY_COST_SKULL_SWARM),
        (EnemyArchetype::IceGolem, spawn::ENEMY_COST_ICE_GOLEM),
        (EnemyArchetype::SnowDrifter, spawn::ENEMY_COST_SNOW_DRIFTER),
        (EnemyArchetype::PenguinBomber, spawn::ENEMY_COST_PENGUIN_BOMBER),
        (EnemyArchetype::FireMage, spawn::ENEMY_COST_FIRE_MAGE),
        (EnemyArchetype::LavaElemental, spawn::ENEMY_COST_LAVA_ELEMENTAL),
        (EnemyArchetype::CinderWisp, spawn::ENEMY_COST_CINDER_WISP),
        (EnemyArchetype::Treant, spawn::ENEMY_COST_TREANT),
        (EnemyArchetype::VineCreeper, spawn::ENEMY_COST_VINE_CREEPER),
        (EnemyArchetype::SporePod, spawn::ENEMY_COST_SPORE_POD),
        (EnemyArchetype::VoidWalker, spawn::ENEMY_COST_VOID_WALKER),
        (EnemyArchetype::CorruptionOrb, spawn::ENEMY_COST_CORRUPTION_ORB),
        (EnemyArchetype::ShadowClone, spawn::ENEMY_COST_SHADOW_CLONE),
    ];

    // Per-biome weight multipliers. Default = 1.0 (balanced).
    // Weights for: Chaser, Dasher, Shooter, Sniper, Bomber, Orbiter,
    //              SkeletonArcher, Ghost, SkullSwarm, IceGolem, SnowDrifter, PenguinBomber,
    //              FireMage, LavaElemental, CinderWisp, Treant, VineCreeper, SporePod,
    //              VoidWalker, CorruptionOrb, ShadowClone
    let weights = match biome {
        Biome::Catacombs => [0.5, 0.5, 0.5, 1.0, 0.5, 0.5, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        Biome::Frost => [0.5, 1.0, 1.0, 0.3, 0.3, 0.5, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        Biome::Magma => [1.0, 0.3, 0.3, 0.3, 1.0, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        Biome::Verdant => [1.0, 0.3, 0.3, 0.3, 0.3, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0],
        Biome::Void => [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0],
    };

    // Build a weighted list of affordable candidates.
    let mut weighted: Vec<(EnemyArchetype, f32)> = Vec::new();
    for (i, (archetype, cost)) in candidates.iter().enumerate() {
        if *cost <= spawner.director_credits {
            let weight = weights[i];
            // Add weight copies to the selection pool for weighted random.
            let copies = (weight * 100.0) as usize; // avoid fractions
            for _ in 0..copies.max(1) {
                weighted.push((*archetype, *cost));
            }
        }
    }

    if weighted.is_empty() {
        return (EnemyArchetype::Chaser, false);
    }

    let idx = (rng.next_unit() * weighted.len() as f32) as usize;
    let (archetype, base_cost) = weighted[idx.min(weighted.len() - 1)];

    let is_elite = if biome == Biome::Void {
        // Void: all enemies are elite
        true
    } else {
        spawner.director_credits >= base_cost * spawn::ELITE_COST_MULTIPLIER
            && rng.next_unit() < spawn::ELITE_CHANCE
    };

    let mut cost = if is_elite {
        base_cost * spawn::ELITE_COST_MULTIPLIER
    } else {
        base_cost
    };
    // Void bonus: 50% more HP for elite enemies, cost adjusted
    if biome == Biome::Void {
        cost *= spawn::ELITE_COST_MULTIPLIER;
    }
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
    sprites: &spawn::GameSprites,
) {
    let room = biome
        .rooms
        .iter()
        .find(|r| r.id == biome.current_room)
        .expect("current room must exist in biome");
    let spawn_pos = spawn::random_spawn_position(settings, player_pos, room, rng);
    match archetype {
        EnemyArchetype::Chaser => spawn::spawn_chaser(commands, settings, room, spawn_pos, biome, is_elite, sprites),
        EnemyArchetype::Dasher => spawn::spawn_dasher(commands, settings, room, spawn_pos, biome, is_elite, sprites),
        EnemyArchetype::Shooter => spawn::spawn_shooter(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::Sniper => spawn::spawn_sniper(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::Bomber => spawn::spawn_bomber(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::Orbiter => spawn::spawn_orbiter(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::SkeletonArcher => spawn::spawn_skeleton_archer(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::Ghost => spawn::spawn_ghost(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::SkullSwarm => spawn::spawn_skull_swarm(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::IceGolem => spawn::spawn_ice_golem(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::SnowDrifter => spawn::spawn_snow_drifter(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::PenguinBomber => spawn::spawn_penguin_bomber(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::FireMage => spawn::spawn_fire_mage(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::LavaElemental => spawn::spawn_lava_elemental(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::CinderWisp => spawn::spawn_cinder_wisp(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::Treant => spawn::spawn_treant_enemy(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::VineCreeper => spawn::spawn_vine_creeper(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::SporePod => spawn::spawn_spore_pod(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::VoidWalker => spawn::spawn_void_walker_enemy(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::CorruptionOrb => spawn::spawn_corruption_orb(commands, settings, room, spawn_pos, biome, is_elite),
        EnemyArchetype::ShadowClone => spawn::spawn_shadow_clone(commands, settings, room, spawn_pos, biome, is_elite),
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
    sprites: &spawn::GameSprites,
) {
    let room = biome
        .rooms
        .iter()
        .find(|r| r.id == biome.current_room)
        .expect("current room must exist in biome");
    match archetype {
        EnemyArchetype::Chaser => spawn::spawn_chaser(commands, settings, room, player_pos, biome, false, sprites),
        EnemyArchetype::Dasher => spawn::spawn_dasher(commands, settings, room, player_pos, biome, false, sprites),
        EnemyArchetype::Shooter => spawn::spawn_shooter(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::Sniper => spawn::spawn_sniper(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::Bomber => spawn::spawn_bomber(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::Orbiter => spawn::spawn_orbiter(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::SkeletonArcher => spawn::spawn_skeleton_archer(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::Ghost => spawn::spawn_ghost(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::SkullSwarm => spawn::spawn_skull_swarm(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::IceGolem => spawn::spawn_ice_golem(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::SnowDrifter => spawn::spawn_snow_drifter(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::PenguinBomber => spawn::spawn_penguin_bomber(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::FireMage => spawn::spawn_fire_mage(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::LavaElemental => spawn::spawn_lava_elemental(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::CinderWisp => spawn::spawn_cinder_wisp(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::Treant => spawn::spawn_treant_enemy(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::VineCreeper => spawn::spawn_vine_creeper(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::SporePod => spawn::spawn_spore_pod(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::VoidWalker => spawn::spawn_void_walker_enemy(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::CorruptionOrb => spawn::spawn_corruption_orb(commands, settings, room, player_pos, biome, false),
        EnemyArchetype::ShadowClone => spawn::spawn_shadow_clone(commands, settings, room, player_pos, biome, false),
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
    enemy_query: Query<(), With<Enemy>>,
    mut room_cleared_events: EventWriter<RoomClearedEvent>,
    boss_ramp: Option<Res<BossRampTimer>>,
    run: Res<RunState>,
    sprites: Res<spawn::GameSprites>,
) {
    if hitstop.is_some() {
        return;
    }
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();
    let dt = time.delta_secs();

    // ─── Enemy cap: skip spawning when at max ──────────────
    let enemy_count = enemy_query.iter().count();
    if enemy_count >= 60 {
        return;
    }

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

        // ─── Horde Density: floor & curse multipliers ──────
        // Floor multiplier: floor 0 = 1.0x, floor 1 = 1.5x, ..., cap at 3.5x
        let floor_mult = (1.0 + biome.floor_number as f32 * 0.5).min(3.5);
        // Curse bonus: each curse point = +10% more credits
        let curse_credit_bonus = 1.0 + run.curse as f32 * 0.1;

        // ─── Spawn interval computation ───────────────────
        // Base interval: 0.8s, reduced by 0.02 per cleared room (min 0.25s)
        let cleared_rooms = biome.rooms.iter().filter(|r| r.kind != RoomKind::Boss).count().saturating_sub(1) as f32;
        let per_room_speedup = 0.02 * cleared_rooms;
        let base_interval = (0.80 - per_room_speedup).max(0.25);
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
        // Curse also speeds up spawn interval: each curse = 5% faster
        let curse_interval_bonus = 1.0 / (1.0 + run.curse as f32 * 0.05);
        let interval = (base_interval * ramp * curse_interval_bonus).max(0.08);

        // Director picks the next enemy archetype
        let room_kind = biome
            .rooms
            .iter()
            .find(|r| r.id == biome.current_room)
            .map(|r| r.kind)
            .unwrap_or(RoomKind::Fight);
        let credit_rate = director_credit_rate(room_kind);
        // Apply floor + curse multipliers to credit accumulation
        let effective_credit_rate = credit_rate * floor_mult * curse_credit_bonus;
        spawner.director_credits =
            (spawner.director_credits + effective_credit_rate * dt).min(spawn::CREDITS_MAX);
        let current_biome = crate::components::biome_from_floor(run.biome_index);
        let (archetype, is_elite) = director_pick(&mut spawner, &mut rng, current_biome);

        spawn_wave_enemy(
            &mut commands,
            &settings,
            &biome,
            &mut rng,
            player_pos,
            archetype,
            is_elite,
            &sprites,
        );
        spawner.spawned_this_wave += 1;
        spawner.wave_timer = Timer::from_seconds(interval, TimerMode::Once);
    }
}