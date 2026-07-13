//! Spawn functions and constants.
//!
//! Color constants, size constants, cost constants, and all spawn
//! functions for every enemy archetype.

use bevy::prelude::*;

use crate::components::{
    AiBehavior, BaseColor, BaseVelocity, Beacon, Biome, BomberState, Boss, BossPhase, BossState,
    CascadeAffected, CinderWispState, Collider, CorruptionOrbState, DasherPhase, DasherState,
    DeathPuff, Elite, Enemy, EnemyAI, FireMageState, GhostState, Health,
    HitFlash, IceGolemState, LavaElemental, MotionTrail, OrbiterState, PenguinBomberState,
    Projectile, Room, ShadowCloneState, ShooterState, SkeletonArcherState, SkullSwarmState,
    SniperState, SnowDrifterState, SporePodState, Team, TreantState,
    Velocity, VineCreeperState, VoidWalkerState,
};
use crate::resources::{FloorState, GameSettings, RunRng};

// ─── Pixel art sprite resource ─────────────────────────────────────────

/// Resource holding handles to all pixel art sprites.
/// Loaded once at Startup, cloned at spawn time.
#[derive(Resource)]
pub struct GameSprites {
    pub player: Handle<Image>,
    pub enemy_chaser: Handle<Image>,
    pub enemy_dasher: Handle<Image>,
    pub ui_heart: Handle<Image>,
    pub weapon_pistol: Handle<Image>,
    pub weapon_shotgun: Handle<Image>,
    pub weapon_burst: Handle<Image>,
    pub weapon_beam: Handle<Image>,
    pub weapon_rocket: Handle<Image>,
    pub boss_lich: Handle<Image>,
    pub boss_yeti: Handle<Image>,
    pub boss_inferno: Handle<Image>,
    pub boss_treant: Handle<Image>,
    pub boss_voidwalker: Handle<Image>,
}

/// Load all pixel art sprites at startup. Called as a Startup system.
pub fn load_sprites(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(GameSprites {
        player: asset_server.load("art/player.png"),
        enemy_chaser: asset_server.load("art/enemy_chaser.png"),
        enemy_dasher: asset_server.load("art/enemy_dasher.png"),
        ui_heart: asset_server.load("art/ui_heart.png"),
        weapon_pistol: asset_server.load("art/weapon_pistol.png"),
        weapon_shotgun: asset_server.load("art/weapon_shotgun.png"),
        weapon_burst: asset_server.load("art/weapon_burst.png"),
        weapon_beam: asset_server.load("art/weapon_beam.png"),
        weapon_rocket: asset_server.load("art/weapon_rocket.png"),
        boss_lich: asset_server.load("art/boss_lich.png"),
        boss_yeti: asset_server.load("art/boss_yeti.png"),
        boss_inferno: asset_server.load("art/boss_inferno.png"),
        boss_treant: asset_server.load("art/boss_treant.png"),
        boss_voidwalker: asset_server.load("art/boss_voidwalker.png"),
    });
}

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

// ─── Per-biome boss constants ─────────────────────────────────────────

/// Lich (Catacombs) boss constants.
pub const LICH_COLOR: Color = Color::srgb(0.55, 0.20, 0.55); // purple
pub const LICH_SIZE: f32 = 60.0;
pub const LICH_HP: f32 = 30.0;
pub const LICH_DAMAGE: f32 = 1.0;

/// Yeti (Frost) boss constants.
pub const YETI_COLOR: Color = Color::srgb(0.30, 0.65, 0.90); // ice blue
pub const YETI_SIZE: f32 = 80.0;
pub const YETI_HP: f32 = 40.0;
pub const YETI_DAMAGE: f32 = 2.0;

/// Inferno (Magma) boss constants.
pub const INFERNO_COLOR: Color = Color::srgb(0.95, 0.55, 0.10); // orange
pub const INFERNO_SIZE: f32 = 70.0;
pub const INFERNO_HP: f32 = 35.0;
pub const INFERNO_DAMAGE: f32 = 1.5;

/// Treant (Verdant) boss constants.
pub const TREANT_COLOR: Color = Color::srgb(0.20, 0.75, 0.20); // green
pub const TREANT_SIZE: f32 = 90.0;
pub const TREANT_HP: f32 = 50.0;
pub const TREANT_DAMAGE: f32 = 1.0;

/// Void Walker (Void) boss constants.
pub const VOID_WALKER_COLOR: Color = Color::srgb(0.08, 0.02, 0.15); // near-black with purple
pub const VOID_WALKER_SIZE: f32 = 75.0;
pub const VOID_WALKER_HP: f32 = 45.0;
pub const VOID_WALKER_DAMAGE: f32 = 2.0;

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
    sprites: &GameSprites,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(CHASER_COLOR)
    } else {
        CHASER_COLOR
    };

    // Use pixel art sprite, colored with base_color (tint preserves it)
    let mut spawn = commands.spawn((
        Sprite {
            image: sprites.enemy_chaser.clone(),
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
    sprites: &GameSprites,
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
            image: sprites.enemy_dasher.clone(),
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

// ─── Biome-specific enemy colors ──────────────────────────────────────────

/// Catacombs biome colors
pub const SKELETON_ARCHER_COLOR: Color = Color::srgb(0.75, 0.70, 0.60); // bone white
pub const GHOST_COLOR: Color = Color::srgb(0.50, 0.40, 0.65); // translucent purple
pub const SKULL_SWARM_COLOR: Color = Color::srgb(0.65, 0.55, 0.45); // skull tan

/// Frost biome colors
pub const ICE_GOLEM_COLOR: Color = Color::srgb(0.35, 0.70, 0.90); // ice blue
pub const SNOW_DRIFTER_COLOR: Color = Color::srgb(0.80, 0.85, 0.95); // white-blue
pub const PENGUIN_BOMBER_COLOR: Color = Color::srgb(0.10, 0.10, 0.30); // dark navy

/// Magma biome colors
pub const FIRE_MAGE_COLOR: Color = Color::srgb(0.90, 0.40, 0.10); // orange-red
pub const LAVA_ELEMENTAL_COLOR: Color = Color::srgb(0.95, 0.60, 0.05); // bright orange
pub const CINDER_WISP_COLOR: Color = Color::srgb(1.0, 0.70, 0.15); // gold/orange

/// Verdant biome colors
pub const TREANT_ENEMY_COLOR: Color = Color::srgb(0.25, 0.55, 0.15); // deep green
pub const VINE_CREEPER_COLOR: Color = Color::srgb(0.30, 0.70, 0.20); // bright green
pub const SPORE_POD_COLOR: Color = Color::srgb(0.60, 0.75, 0.20); // yellow-green

/// Void biome colors
pub const VOID_WALKER_ENEMY_COLOR: Color = Color::srgb(0.10, 0.02, 0.20); // near-black purple
pub const CORRUPTION_ORB_COLOR: Color = Color::srgb(0.50, 0.10, 0.50); // deep purple
pub const SHADOW_CLONE_COLOR: Color = Color::srgb(0.08, 0.05, 0.12); // near-black

// ─── Biome-specific enemy sizes ───────────────────────────────────────────

pub const SKELETON_ARCHER_SIZE: f32 = 28.0;
pub const GHOST_SIZE: f32 = 26.0;
pub const SKULL_SWARM_SIZE: f32 = 12.0; // each skull is small
pub const ICE_GOLEM_SIZE: f32 = 36.0;
pub const SNOW_DRIFTER_SIZE: f32 = 22.0;
pub const PENGUIN_BOMBER_SIZE: f32 = 18.0;
pub const FIRE_MAGE_SIZE: f32 = 28.0;
pub const LAVA_ELEMENTAL_SIZE: f32 = 30.0;
pub const CINDER_WISP_SIZE: f32 = 14.0;
pub const TREANT_ENEMY_SIZE: f32 = 32.0;
pub const VINE_CREEPER_SIZE: f32 = 20.0;
pub const SPORE_POD_SIZE: f32 = 22.0;
pub const VOID_WALKER_ENEMY_SIZE: f32 = 28.0;
pub const CORRUPTION_ORB_SIZE: f32 = 24.0;
pub const SHADOW_CLONE_SIZE: f32 = 26.0;

// ─── Biome-specific enemy costs ───────────────────────────────────────────

pub const ENEMY_COST_SKELETON_ARCHER: f32 = 9.0;
pub const ENEMY_COST_GHOST: f32 = 10.0;
pub const ENEMY_COST_SKULL_SWARM: f32 = 12.0;
pub const ENEMY_COST_ICE_GOLEM: f32 = 14.0;
pub const ENEMY_COST_SNOW_DRIFTER: f32 = 8.0;
pub const ENEMY_COST_PENGUIN_BOMBER: f32 = 8.0;
pub const ENEMY_COST_FIRE_MAGE: f32 = 11.0;
pub const ENEMY_COST_LAVA_ELEMENTAL: f32 = 12.0;
pub const ENEMY_COST_CINDER_WISP: f32 = 7.0;
pub const ENEMY_COST_TREANT: f32 = 13.0;
pub const ENEMY_COST_VINE_CREEPER: f32 = 9.0;
pub const ENEMY_COST_SPORE_POD: f32 = 6.0;
pub const ENEMY_COST_VOID_WALKER: f32 = 12.0;
pub const ENEMY_COST_CORRUPTION_ORB: f32 = 10.0;
pub const ENEMY_COST_SHADOW_CLONE: f32 = 10.0;

// ─── Biome-specific enemy spawn functions ─────────────────────────────────

/// Catacombs: SkeletonArcher — stands at mid-range, shoots bone arrows.
pub fn spawn_skeleton_archer(
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
        elite_color_tint(SKELETON_ARCHER_COLOR)
    } else {
        SKELETON_ARCHER_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(SKELETON_ARCHER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::SkeletonArcher },
        Collider { half_extents: Vec2::splat(SKELETON_ARCHER_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        SkeletonArcherState {
            fire_timer: Timer::from_seconds(1.5, TimerMode::Repeating),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Catacombs: Ghost — phases through walls (periodic semi-transparent).
pub fn spawn_ghost(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * 0.75 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(GHOST_COLOR)
    } else {
        GHOST_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(GHOST_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::Ghost },
        Collider { half_extents: Vec2::splat(GHOST_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        GhostState {
            phase_timer: Timer::from_seconds(2.0, TimerMode::Repeating),
            is_phasing: false,
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Catacombs: SkullSwarm — small orbiting skull that lunges.
pub fn spawn_skull_swarm(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = 3.0 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(SKULL_SWARM_COLOR)
    } else {
        SKULL_SWARM_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(SKULL_SWARM_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::SkullSwarm },
        Collider { half_extents: Vec2::splat(SKULL_SWARM_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        SkullSwarmState {
            orbit_angle: spawn_pos.x * 0.5 + spawn_pos.y * 0.3, // deterministic offset
            orbit_radius: 40.0,
            lunge_timer: Timer::from_seconds(2.5, TimerMode::Once),
            is_lunging: false,
            lunge_target: Vec2::ZERO,
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Frost: IceGolem — slow, high HP, AOE slam every 3s.
pub fn spawn_ice_golem(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * 3.0 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(ICE_GOLEM_COLOR)
    } else {
        ICE_GOLEM_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(ICE_GOLEM_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::IceGolem },
        Collider { half_extents: Vec2::splat(ICE_GOLEM_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        IceGolemState {
            slam_timer: Timer::from_seconds(3.0, TimerMode::Repeating),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Frost: SnowDrifter — fast zigzag, slowing trail.
pub fn spawn_snow_drifter(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * 0.6 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(SNOW_DRIFTER_COLOR)
    } else {
        SNOW_DRIFTER_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(SNOW_DRIFTER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::SnowDrifter },
        Collider { half_extents: Vec2::splat(SNOW_DRIFTER_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        SnowDrifterState {
            zigzag_timer: Timer::from_seconds(0.4, TimerMode::Repeating),
            zig_direction: 1.0,
            trail_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Frost: PenguinBomber — fast, explosion on death.
pub fn spawn_penguin_bomber(
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
        elite_color_tint(PENGUIN_BOMBER_COLOR)
    } else {
        PENGUIN_BOMBER_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(PENGUIN_BOMBER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::PenguinBomber },
        Collider { half_extents: Vec2::splat(PENGUIN_BOMBER_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        PenguinBomberState {
            explosion_radius: settings.bomber_explosion_radius,
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Magma: FireMage — teleports every 2s, shoots 3 fireballs in a spread.
pub fn spawn_fire_mage(
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
        elite_color_tint(FIRE_MAGE_COLOR)
    } else {
        FIRE_MAGE_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(FIRE_MAGE_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::FireMage },
        Collider { half_extents: Vec2::splat(FIRE_MAGE_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        FireMageState {
            teleport_timer: Timer::from_seconds(2.0, TimerMode::Once),
            fire_timer: Timer::from_seconds(1.0, TimerMode::Once),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Magma: LavaElemental — splits into 2 smaller versions on death.
pub fn spawn_lava_elemental(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * 2.0 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(LAVA_ELEMENTAL_COLOR)
    } else {
        LAVA_ELEMENTAL_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(LAVA_ELEMENTAL_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::LavaElemental },
        Collider { half_extents: Vec2::splat(LAVA_ELEMENTAL_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        LavaElemental { split_count: 2 },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Magma: CinderWisp — fast, leaves fire trail.
pub fn spawn_cinder_wisp(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * 0.5 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(CINDER_WISP_COLOR)
    } else {
        CINDER_WISP_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(CINDER_WISP_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::CinderWisp },
        Collider { half_extents: Vec2::splat(CINDER_WISP_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        CinderWispState {
            trail_timer: Timer::from_seconds(0.3, TimerMode::Repeating),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Verdant: Treant — slow, roots player (slow zone).
pub fn spawn_treant_enemy(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * 2.5 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(TREANT_ENEMY_COLOR)
    } else {
        TREANT_ENEMY_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(TREANT_ENEMY_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::Treant },
        Collider { half_extents: Vec2::splat(TREANT_ENEMY_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        TreantState {
            root_timer: Timer::from_seconds(2.5, TimerMode::Repeating),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Verdant: VineCreeper — wall-edge movement, dive attack.
pub fn spawn_vine_creeper(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * 0.8 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(VINE_CREEPER_COLOR)
    } else {
        VINE_CREEPER_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(VINE_CREEPER_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::VineCreeper },
        Collider { half_extents: Vec2::splat(VINE_CREEPER_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        VineCreeperState {
            is_diving: false,
            dive_timer: Timer::from_seconds(0.5, TimerMode::Once),
            dive_cooldown: Timer::from_seconds(3.0, TimerMode::Once),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Verdant: SporePod — stationary, poison clouds.
pub fn spawn_spore_pod(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * 1.5 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(SPORE_POD_COLOR)
    } else {
        SPORE_POD_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(SPORE_POD_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::SporePod },
        Collider { half_extents: Vec2::splat(SPORE_POD_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        SporePodState {
            emit_timer: Timer::from_seconds(2.0, TimerMode::Repeating),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Void: VoidWalker — teleports behind player, melee slash.
pub fn spawn_void_walker_enemy(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * 1.5 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(VOID_WALKER_ENEMY_COLOR)
    } else {
        VOID_WALKER_ENEMY_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(VOID_WALKER_ENEMY_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::VoidWalker },
        Collider { half_extents: Vec2::splat(VOID_WALKER_ENEMY_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        VoidWalkerState {
            teleport_timer: Timer::from_seconds(2.5, TimerMode::Once),
            attack_timer: Timer::from_seconds(0.3, TimerMode::Once),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Void: CorruptionOrb — floating, homing projectiles.
pub fn spawn_corruption_orb(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.shooter_max_hp * 0.8 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(CORRUPTION_ORB_COLOR)
    } else {
        CORRUPTION_ORB_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(CORRUPTION_ORB_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::CorruptionOrb },
        Collider { half_extents: Vec2::splat(CORRUPTION_ORB_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        CorruptionOrbState {
            fire_timer: Timer::from_seconds(2.0, TimerMode::Repeating),
        },
    ));
    if is_elite {
        spawn.insert(Elite);
    }
}

/// Void: ShadowClone — mirrors player position.
pub fn spawn_shadow_clone(
    commands: &mut Commands,
    settings: &GameSettings,
    _room: &Room,
    spawn_pos: Vec2,
    floor: &FloorState,
    is_elite: bool,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);
    let hp = settings.enemy_max_hp * 0.7 * hp_mult * if is_elite { 2.0 } else { 1.0 };
    let base_color = if is_elite {
        elite_color_tint(SHADOW_CLONE_COLOR)
    } else {
        SHADOW_CLONE_COLOR
    };

    let mut spawn = commands.spawn((
        Sprite {
            color: base_color,
            custom_size: Some(Vec2::splat(SHADOW_CLONE_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.0),
        Enemy,
        EnemyAI { behavior: AiBehavior::ShadowClone },
        Collider { half_extents: Vec2::splat(SHADOW_CLONE_SIZE * 0.5) },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(base_color),
        ShadowCloneState {
            mirror_offset: spawn_pos,
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

/// Phase 1 mini-boss: spawn a biome-specific boss at position with full HP.
pub fn spawn_boss(
    commands: &mut Commands,
    settings: &GameSettings,
    floor: &FloorState,
    position: Vec2,
    biome: Biome,
    sprites: &GameSprites,
) {
    let (hp_mult, _dmg_mult) = floor_difficulty_mult(floor.floor_number);

    // Select boss stats + sprite per biome.
    let (boss_color, boss_size, base_hp, _damage, sprite_handle) = match biome {
        Biome::Catacombs => (LICH_COLOR, LICH_SIZE, LICH_HP, LICH_DAMAGE, &sprites.boss_lich),
        Biome::Frost => (YETI_COLOR, YETI_SIZE, YETI_HP, YETI_DAMAGE, &sprites.boss_yeti),
        Biome::Magma => (INFERNO_COLOR, INFERNO_SIZE, INFERNO_HP, INFERNO_DAMAGE, &sprites.boss_inferno),
        Biome::Verdant => (TREANT_COLOR, TREANT_SIZE, TREANT_HP, TREANT_DAMAGE, &sprites.boss_treant),
        Biome::Void => (VOID_WALKER_COLOR, VOID_WALKER_SIZE, VOID_WALKER_HP, VOID_WALKER_DAMAGE, &sprites.boss_voidwalker),
    };

    let hp = base_hp * hp_mult;

    commands.spawn((
        Sprite {
            image: sprite_handle.clone(),
            color: boss_color,
            custom_size: Some(Vec2::splat(boss_size)),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 0.0),
        Boss,
        Enemy,
        EnemyAI {
            behavior: AiBehavior::Boss,
        },
        Collider {
            half_extents: Vec2::splat(boss_size * 0.5),
        },
        Health::new(hp),
        Velocity::default(),
        BaseVelocity::default(),
        CascadeAffected,
        BaseColor(boss_color),
        BossState {
            phase: BossPhase::Phase1,
            attack_timer: Timer::from_seconds(
                settings.boss_phase2_attack_interval_s,
                TimerMode::Once,
            ),
            phase2_attack_timer: Timer::from_seconds(1.0, TimerMode::Once),
            current_hp: hp,
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