//! Spawn functions and constants.
//!
//! Color constants, size constants, cost constants, and all spawn
//! functions for every enemy archetype.

use bevy::prelude::*;

use crate::components::{
    AiBehavior, BaseColor, BaseVelocity, Beacon, BomberState, Boss, BossPhase, BossState,
    CascadeAffected, Collider, DasherPhase, DasherState, DeathPuff, Elite, Enemy, EnemyAI, Health, HitFlash, MotionTrail, OrbiterState, Projectile, Room, ShooterState, SniperState, Team, Velocity,
};
use crate::resources::{FloorState, GameSettings, RunRng};

/// Color of the chaser — high-contrast red so it reads as "danger" at
/// a glance against the orange player and the dark background we'll
/// add later.
pub const CHASER_COLOR: Color = Color::srgb(0.84, 0.16, 0.16); // ~#d62828

/// Phase 1 dasher color. Purple/magenta so it's visually distinct
/// from the red chaser — the player learns "red = chaser, purple
/// = dasher" in one glance. The hue also telegraphs "burst damage"
/// (purple is the universal "fast/danger" color in arcade games).
pub const DASHER_COLOR: Color = Color::srgb(0.62, 0.20, 0.85); // ~#9e33d9

/// Phase 1 shooter color. Teal so it's distinct from the red chaser
/// and the purple dasher — the player learns "red = melee rush,
/// purple = lunge burst, teal = ranged plink".
pub const SHOOTER_COLOR: Color = Color::srgb(0.20, 0.75, 0.75); // ~#33bfbf

/// Color of the shooter's projectile — same teal as the body but
/// a touch lighter so the bullet is visible against any background.
pub const SHOOTER_PROJECTILE_COLOR: Color = Color::srgb(0.45, 0.90, 0.90); // ~#73e6e6

/// Phase 1 sniper color. Yellow so it reads as "caution / warning"
/// in arcade-game color shorthand.
pub const SNIPER_COLOR: Color = Color::srgb(0.95, 0.85, 0.25); // ~#f2d940

/// Sniper charged color (the end of the telegraph ramp).
pub const SNIPER_CHARGED_COLOR: Color = Color::srgb(0.95, 0.20, 0.15); // ~#f22626

/// Phase 3d bomber color. Orange — distinct from all other archetypes.
pub const BOMBER_COLOR: Color = Color::srgb(0.95, 0.60, 0.15); // ~#f29926

/// Phase 3d orbiter color. Cyan — distinct from every existing archetype.
pub const ORBITER_COLOR: Color = Color::srgb(0.20, 0.80, 0.90); // ~#33cce6

/// Bomber sprite size. Slightly smaller than the chaser (30) so the
/// player can visually distinguish the two.
pub const BOMBER_SIZE: f32 = 20.0;

/// Orbiter sprite size. Smaller than the bomber (20) so the orbiter
/// reads as "small / fast / annoying".
pub const ORBITER_SIZE: f32 = 14.0;

/// Orbiter projectile color. Same cyan as the body but slightly brighter.
pub const ORBITER_PROJECTILE_COLOR: Color = Color::srgb(0.35, 0.90, 0.95); // ~#59e6f2

/// Sniper projectile color. Bright orange so the bullet is distinct
/// from the shooter's teal and the player's warm white.
pub const SNIPER_PROJECTILE_COLOR: Color = Color::srgb(1.0, 0.55, 0.10); // ~#ff8c1a

/// Chaser sprite size. Slightly smaller than the player (40x40).
pub const CHASER_SIZE: f32 = 30.0;

/// Dasher sprite size. Same as the chaser.
pub const DASHER_SIZE: f32 = 30.0;

/// Shooter sprite size. Same as chaser/dasher.
pub const SHOOTER_SIZE: f32 = 30.0;

/// Shooter projectile sprite size. Smaller than the player projectile (6x12).
pub const SHOOTER_PROJECTILE_SIZE: Vec2 = Vec2::new(5.0, 8.0);

/// Sniper sprite size. Same as the other three archetypes.
pub const SNIPER_SIZE: f32 = 30.0;

// ─── Mini-boss constants (Phase 1) ─────────────────────────────────────

/// Boss color. Deep red, slightly darker than the chaser's bright red.
pub const BOSS_COLOR: Color = Color::srgb(0.70, 0.05, 0.05);

/// Boss sprite size. 50x50 — bigger than the player (40x40) and other
/// enemies (30x30).
pub const BOSS_SIZE: f32 = 50.0;

/// Boss projectile color. Same red family as the boss body but lighter.
pub const BOSS_PROJECTILE_COLOR: Color = Color::srgb(0.95, 0.20, 0.15);

/// Boss projectile size. 12x12 — chunkier than the shooter's 5x8.
pub const BOSS_PROJECTILE_SIZE: Vec2 = Vec2::new(12.0, 12.0);

// ─── Cascade beacon constants (Phase 2 P5) ──────────────────────────────

/// Phase 3b director-cost constants. Each archetype costs a number of
/// director credits to spawn. Elite variants cost 2×.
pub const ENEMY_COST_CHASER: f32 = 5.0;
pub const ENEMY_COST_DASHER: f32 = 8.0;
pub const ENEMY_COST_SHOOTER: f32 = 7.0;
pub const ENEMY_COST_SNIPER: f32 = 12.0;
pub const ENEMY_COST_BOMBER: f32 = 8.0;
pub const ENEMY_COST_ORBITER: f32 = 10.0;
pub const ELITE_COST_MULTIPLIER: f32 = 2.0;

/// Per-room-type director credit rates (credits per second).
pub const CREDIT_RATE_FIGHT: f32 = 8.0;
pub const CREDIT_RATE_HORDE: f32 = 10.0;
pub const CREDIT_RATE_ELITE: f32 = 6.0;
pub const CREDIT_RATE_BOSS: f32 = 5.0;
/// Max credits cap (prevents budget stacking across long lulls).
pub const CREDITS_MAX: f32 = 120.0;

/// Probability of upgrading to an elite variant when budget allows.
pub const ELITE_CHANCE: f32 = 0.25;

/// Elite gold tint overlay. Mixed with the enemy's base color at spawn
/// time so the enemy reads as "special".
pub const ELITE_TINT: Color = Color::srgb(0.95, 0.78, 0.12);

/// Beacon color. Bright gold/yellow — distinct from every other enemy.
pub const BEACON_COLOR: Color = Color::srgb(1.0, 0.85, 0.20);

/// Bright white death-puff color.
pub const DEATH_PUFF_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);

// ─── Spawn helper functions ─────────────────────────────────────────────

/// Floor difficulty multiplier: returns (hp_mult, dmg_mult).
pub fn floor_difficulty_mult(floor: u32) -> (f32, f32) {
    let hp = 1.0 + floor as f32 * 0.08;
    let dmg = 1.0 + floor as f32 * 0.05;
    (hp, dmg)
}

/// Apply gold-tinted elite overlay to a base color.
pub fn elite_color_tint(base: Color) -> Color {
    let srgba = base.to_srgba();
    let elite = ELITE_TINT.to_srgba();
    Color::srgb(
        srgba.red * 0.6 + elite.red * 0.4,
        srgba.green * 0.6 + elite.green * 0.4,
        srgba.blue * 0.6 + elite.blue * 0.4,
    )
}

/// Shared spawn-position math: random angle, random distance.
pub fn random_spawn_position(
    settings: &GameSettings,
    player_pos: Vec2,
    room: &Room,
    rng: &mut RunRng,
) -> Vec2 {
    let angle = rng.next_unit() * std::f32::consts::TAU;
    let distance = settings.spawner_min_distance
        + rng.next_unit() * (settings.spawner_max_distance - settings.spawner_min_distance);
    let offset = Vec2::new(angle.cos(), angle.sin()) * distance;
    let raw = player_pos + offset;

    const SPAWN_MARGIN: f32 = 25.0;
    Vec2::new(
        raw.x.clamp(
            room.bounds_min.x + SPAWN_MARGIN,
            room.bounds_max.x - SPAWN_MARGIN,
        ),
        raw.y.clamp(
            room.bounds_min.y + SPAWN_MARGIN,
            room.bounds_max.y - SPAWN_MARGIN,
        ),
    )
}

// ─── Individual spawn functions ─────────────────────────────────────────

pub fn spawn_chaser(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(CHASER_COLOR)
    } else {
        CHASER_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(CHASER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Chaser,
        },
        Collider {
            half_extents: Vec2::splat(CHASER_SIZE * 0.5),
        },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

pub fn spawn_dasher(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.dasher_max_hp * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(DASHER_COLOR)
    } else {
        DASHER_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(DASHER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Dasher,
        },
        Collider {
            half_extents: Vec2::splat(DASHER_SIZE * 0.5),
        },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        DasherState {
            phase: DasherPhase::Approach,
            phase_timer: Timer::from_seconds(settings.dasher_approach_duration_s, TimerMode::Once),
        },
        BaseColor(base_color),
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

pub fn spawn_shooter(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.shooter_max_hp * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(SHOOTER_COLOR)
    } else {
        SHOOTER_COLOR
    };

    let interval = 1.0 / settings.shooter_fire_rate;
    let initial_cooldown = interval * 0.5;

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(SHOOTER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Shooter,
        },
        Collider {
            half_extents: Vec2::splat(SHOOTER_SIZE * 0.5),
        },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        ShooterState {
            fire_timer: Timer::from_seconds(initial_cooldown, TimerMode::Once),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

pub fn spawn_sniper(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.sniper_max_hp * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(SNIPER_COLOR)
    } else {
        SNIPER_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(SNIPER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Sniper,
        },
        Collider {
            half_extents: Vec2::splat(SNIPER_SIZE * 0.5),
        },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        SniperState {
            charge_timer: Timer::from_seconds(
                settings.sniper_post_fire_cooldown_s,
                TimerMode::Once,
            ),
            is_charging: false,
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

pub fn spawn_bomber(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.bomber_max_hp * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(BOMBER_COLOR)
    } else {
        BOMBER_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(BOMBER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Bomber,
        },
        Collider {
            half_extents: Vec2::splat(BOMBER_SIZE * 0.5),
        },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        BomberState {
            explosion_radius: settings.bomber_explosion_radius,
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

pub fn spawn_orbiter(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.orbiter_max_hp * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(ORBITER_COLOR)
    } else {
        ORBITER_COLOR
    };

    let starting_angle =
        (spawn_pos.x * 0.791 + spawn_pos.y * 1.273).fract() * std::f32::consts::TAU;

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(ORBITER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Orbiter,
        },
        Collider {
            half_extents: Vec2::splat(ORBITER_SIZE * 0.5),
        },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        OrbiterState {
            angle: starting_angle,
            orbit_radius: settings.orbiter_orbit_radius,
            angular_speed: settings.orbiter_move_speed / settings.orbiter_orbit_radius,
            fire_timer: Timer::from_seconds(1.0 / settings.orbiter_fire_rate, TimerMode::Repeating),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Spawn a Team::Enemy shooter projectile at enemy_pos aimed at the player.
pub fn spawn_shooter_projectile(
    commands: &mut Commands,
    settings: &GameSettings,
    enemy_pos: Vec2,
    dir: Vec2,
) {
    let muzzle = SHOOTER_SIZE * 0.5 + SHOOTER_PROJECTILE_SIZE.y * 0.5 + 2.0;
    let spawn_pos = enemy_pos + dir * muzzle;
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;
    let speed = settings.shooter_projectile_speed;

    commands.spawn((
        Sprite {
            color: SHOOTER_PROJECTILE_COLOR,
            custom_size: Some(SHOOTER_PROJECTILE_SIZE),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
            .with_rotation(Quat::from_rotation_z(angle)),
        BaseVelocity(dir * speed),
        Velocity(dir * speed),
        CascadeAffected,
        Collider {
            half_extents: Vec2::new(
                SHOOTER_PROJECTILE_SIZE.x * 0.5,
                SHOOTER_PROJECTILE_SIZE.y * 0.5,
            ),
        },
        Projectile {
            damage: settings.shooter_projectile_damage,
            lifetime: Timer::from_seconds(settings.shooter_projectile_lifetime_s, TimerMode::Once),
            team: Team::Enemy,
        },
    ));
}

/// Spawn a Team::Enemy sniper projectile at enemy_pos aimed at the player.
pub fn spawn_sniper_projectile(
    commands: &mut Commands,
    settings: &GameSettings,
    enemy_pos: Vec2,
    dir: Vec2,
) {
    let muzzle = SNIPER_SIZE * 0.5 + settings.sniper_projectile_size_y * 0.5 + 2.0;
    let spawn_pos = enemy_pos + dir * muzzle;
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;
    let speed = settings.sniper_projectile_speed;

    commands.spawn((
        Sprite {
            color: SNIPER_PROJECTILE_COLOR,
            custom_size: Some(Vec2::new(
                settings.sniper_projectile_size_x,
                settings.sniper_projectile_size_y,
            )),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
            .with_rotation(Quat::from_rotation_z(angle)),
        BaseVelocity(dir * speed),
        Velocity(dir * speed),
        CascadeAffected,
        Collider {
            half_extents: Vec2::new(
                settings.sniper_projectile_size_x * 0.5,
                settings.sniper_projectile_size_y * 0.5,
            ),
        },
        Projectile {
            damage: settings.sniper_projectile_damage,
            lifetime: Timer::from_seconds(settings.sniper_projectile_lifetime_s, TimerMode::Once),
            team: Team::Enemy,
        },
    ));
}

/// Spawn a Team::Enemy orbiter projectile at enemy_pos moving in dir.
pub fn spawn_orbiter_projectile(
    commands: &mut Commands,
    settings: &GameSettings,
    enemy_pos: Vec2,
    dir: Vec2,
) {
    let muzzle = ORBITER_SIZE * 0.5 + 3.0 + 2.0;
    let spawn_pos = enemy_pos + dir * muzzle;
    let speed = settings.orbiter_projectile_speed;

    commands.spawn((
        Sprite {
            color: ORBITER_PROJECTILE_COLOR,
            custom_size: Some(Vec2::splat(6.0)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5),
        BaseVelocity(dir * speed),
        Velocity(dir * speed),
        CascadeAffected,
        Collider {
            half_extents: Vec2::splat(3.0),
        },
        Projectile {
            damage: settings.orbiter_projectile_damage,
            lifetime: Timer::from_seconds(settings.orbiter_projectile_lifetime_s, TimerMode::Once),
            team: Team::Enemy,
        },
    ));
}

/// Spawn a Team::Enemy boss projectile at enemy_pos moving in dir.
pub fn spawn_boss_projectile(
    commands: &mut Commands,
    settings: &GameSettings,
    enemy_pos: Vec2,
    dir: Vec2,
) {
    let muzzle = BOSS_SIZE * 0.5 + settings.boss_projectile_size_y * 0.5 + 2.0;
    let spawn_pos = enemy_pos + dir * muzzle;
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;
    let speed = settings.boss_projectile_speed;

    commands.spawn((
        Sprite {
            color: BOSS_PROJECTILE_COLOR,
            custom_size: Some(BOSS_PROJECTILE_SIZE),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.5)
            .with_rotation(Quat::from_rotation_z(angle)),
        BaseVelocity(dir * speed),
        Velocity(dir * speed),
        CascadeAffected,
        Collider {
            half_extents: Vec2::new(
                settings.boss_projectile_size_x * 0.5,
                settings.boss_projectile_size_y * 0.5,
            ),
        },
        Projectile {
            damage: settings.boss_projectile_damage,
            lifetime: Timer::from_seconds(settings.boss_projectile_lifetime_s, TimerMode::Once),
            team: Team::Enemy,
        },
    ));
}

/// Phase 1 mini-boss: spawn the boss at position with full HP.
pub fn spawn_boss(commands: &mut Commands, settings: &GameSettings, floor: &FloorState, position: Vec2) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    commands.spawn((
        Sprite {
            color: BOSS_COLOR,
            custom_size: Some(Vec2::splat(BOSS_SIZE)),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 0.0),
        Boss,
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Boss,
        },
        Collider {
            half_extents: Vec2::splat(BOSS_SIZE * 0.5),
        },
        Health::new(settings.boss_max_hp * hp_mult),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(BOSS_COLOR),
        BossState {
            phase: BossPhase::Phase1,
            attack_timer: Timer::from_seconds(
                settings.boss_phase2_attack_interval_s,
                TimerMode::Once,
            ),
            current_hp: settings.boss_max_hp * hp_mult,
        },
    ));
}

/// Phase 2 P5: spawn a Cascade beacon at position.
pub fn spawn_beacon(commands: &mut Commands, settings: &GameSettings, position: Vec2) {
    commands.spawn((
        Sprite {
            color: BEACON_COLOR,
            custom_size: Some(Vec2::splat(settings.beacon_size)),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 0.0),
        Beacon,
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Beacon,
        },
        Collider {
            half_extents: Vec2::splat(settings.beacon_size * 0.5),
        },
        Health::new(settings.beacon_hp),
        Velocity::default(),
        BaseVelocity::default(),
        BaseColor(BEACON_COLOR),
        MotionTrail,
    ));
}

/// Spawn a DeathPuff entity at position.
pub fn spawn_death_puff(commands: &mut Commands, settings: &GameSettings, position: Vec2) {
    commands.spawn((
        Sprite {
            color: DEATH_PUFF_COLOR,
            custom_size: Some(Vec2::splat(CHASER_SIZE)),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 0.0),
        DeathPuff {
            lifetime: Timer::from_seconds(settings.death_puff_lifetime_s, TimerMode::Once),
        },
    ));
}

/// Tick every DeathPuff: fade alpha from 1.0 to 0.0 over its lifetime,
/// then despawn.
pub fn death_puff_tick_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut DeathPuff, &mut Sprite)>,
) {
    for (entity, mut puff, mut sprite) in &mut query {
        puff.lifetime.tick(time.delta());
        if puff.lifetime.finished() {
            commands.entity(entity).despawn();
            continue;
        }
        let progress = puff.lifetime.elapsed_secs() / puff.lifetime.duration().as_secs_f32();
        let alpha = 1.0 - progress.clamp(0.0, 1.0);
        sprite.color = sprite.color.with_alpha(alpha);
    }
}

/// Brief white pulse on an enemy when hit by a projectile. Inserted by
/// the projectile-damage system; tick the timer and lerp the enemy color
/// toward white, fading back to `BaseColor` when the timer elapses.
pub fn hit_flash_tick_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut HitFlash, &mut Sprite, &BaseColor), With<Enemy>>,
) {
    for (entity, mut flash, mut sprite, base_color) in &mut query {
        flash.timer.tick(time.delta());
        if flash.timer.finished() {
            sprite.color = base_color.0;
            commands.entity(entity).remove::<HitFlash>();
            continue;
        }
        let progress = flash.timer.elapsed_secs() / flash.timer.duration().as_secs_f32();
        let t = 1.0 - progress;
        let base = base_color.0.to_srgba();
        sprite.color = Color::srgba(
            base.red + (1.0 - base.red) * t,
            base.green + (1.0 - base.green) * t,
            base.blue + (1.0 - base.blue) * t,
            base.alpha,
        );
    }
}