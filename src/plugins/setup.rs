//! Phase 2 setup: window, camera, AppState, default resources, and
//! the title-screen overlay.
//!
//! Phase 0 setup was intentionally minimal. Phase 2 adds:
//! - `app.init_state::<AppState>()` so the state machine is live
//!   from frame 1
//! - `OnEnter(AppState::InRun)` (`enter_in_run_system`) — replaces
//!   the Phase 1 key-gated `restart_system`; fires on every InRun
//!   entry (boot from MainMenu, restart from GameOver)
//! - `OnEnter(AppState::MainMenu)` and `OnExit(...)` scaffolds for
//!   the title screen fade-in / fade-out
//! - The MainMenu overlay entity (dim bg + "PRESS ENTER TO START")
//!   spawned once at startup, toggled by `state_visibility_system`
//! - `menu_input_system`, `in_run_esc_system`, `game_over_input_system`
//!   — per-state key handlers, replacing the Phase 1 `exit_on_esc`
//!   in lib.rs

use bevy::prelude::*;
use log::info;

use crate::components::{
    AimIndicator, AppState, BaseVelocity, Collider, DashCooldown, DeathPuff, Enemy, EnemyArchetype,
    Health, HudElement, HudElementKind, Invulnerable, Knockback, LastMoveDirection, MainCamera,
    Pickup, Player, Projectile, RoomId, RoomScope, Spawner, Velocity, Weapon, WeaponArchetype,
    PLAYER_COLOR,
};
use crate::resources::{BiomeState, CascadeState, GameSettings, RunState, ScreenShake};

/// Color of the aim indicator line — high-contrast white so it stays
/// readable against any background we'll add later.
const AIM_INDICATOR_COLOR: Color = Color::WHITE;

/// Z-offset of the MainMenu overlay, above the cascade telegraph
/// (`HUD_Z + 0.4`) and the game-over overlay (`HUD_Z + 0.2 / + 0.3`),
/// so a transition InRun → MainMenu (e.g. player dies + ESC) cleanly
/// covers whatever was on screen.
const MAIN_MENU_Z: f32 = 0.6;
/// Width/height of the dim background (2× view, same trick as the
/// cascade telegraph — absorbs camera-follow lag without exposing
/// a border).
const MAIN_MENU_BG_SIZE: (f32, f32) = (1280.0 * 2.0, 720.0 * 2.0);
const MAIN_MENU_BG_COLOR: Color = Color::srgba(0.0, 0.0, 0.05, 0.85);
const MAIN_MENU_TEXT: &str = "PRESS ENTER TO START";
const MAIN_MENU_TEXT_COLOR: Color = Color::srgb(0.95, 0.95, 0.95);
const MAIN_MENU_TEXT_SIZE: f32 = 56.0;

/// Bevy plugin that wires up everything needed before a run can start.
pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        // `init_state` registers `AppState` as a Bevy State and
        // seeds it with the #[default] variant (MainMenu). Must
        // come BEFORE any system that uses `in_state(...)` /
        // `Res<State<AppState>>` / `ResMut<NextState<AppState>>`.
        app.init_state::<AppState>()
            .init_resource::<BiomeState>()
            .init_resource::<CascadeState>()
            .init_resource::<RunState>()
            .init_resource::<GameSettings>()
            // Day 13-14: ScreenShake is Default, so a single
            // init_resource call seeds it (intensity=0, no shake).
            .init_resource::<ScreenShake>()
            // Phase 2: the world is built once at startup (player,
            // camera, spawners, all HUD elements including the new
            // MainMenu overlay) and we toggle visibility per state
            // via `state_visibility_system`. This is cheaper than
            // respawning on every state transition and means the
            // MainMenu overlay's "PRESS ENTER" text + the game-over
            // overlay's "GAME OVER" text are both resident, just
            // hidden by default and shown only in their respective
            // states.
            .add_systems(Startup, spawn_world)
            .add_systems(
                Update,
                (
                    menu_input_system,
                    in_run_esc_system,
                    game_over_input_system,
                    state_visibility_system,
                )
                    .chain(),
            )
            .add_systems(OnEnter(AppState::InRun), enter_in_run_system);
    }
}

fn spawn_world(mut commands: Commands, settings: Res<GameSettings>) {
    commands.spawn((Camera2d, MainCamera));

    commands
        .spawn((
            // Visual: a 40x40 orange square. Real sprite lands in Phase 1.
            Sprite {
                color: PLAYER_COLOR,
                custom_size: Some(Vec2::splat(40.0)),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 0.0),
            // ECS data: same as a real player will have on day one.
            Player,
            Health::new(4.0),
            Velocity::default(),
            BaseVelocity::default(),
            // Initial movement direction is up — the dash
            // system reads this if the player dashes before
            // pressing WASD. Default (`Vec2::ZERO`) is fine
            // too because the dash has a fallback chain
            // (last_move → aim → up), but starting at +Y
            // means a reflex dash at game start goes up the
            // screen (away from spawn-ward enemies) rather
            // than falling through to the aim fallback.
            LastMoveDirection(Vec2::Y),
            // Day 8-10: 40x40 sprite → 20x20 half-extents for AABB.
            Collider {
                half_extents: Vec2::splat(20.0),
            },
            // Day 5-7: starting pistol. Timer is set to a zero-duration
            // finished timer so the first click fires immediately.
            // Phase 1 (shotgun): `pellet_count` + `spread_rad` round
            // out the Weapon fields. Pistol = 1 pellet, no spread.
            // Phase 1 (burst): `burst_rounds` + `burst_rounds_remaining`
            // round out the Weapon state. Pistol starts with
            // burst_rounds = 1 (single-shot) and remaining = 0
            // (no burst in progress). The burst_archetype gets
            // 3 rounds the first time the player switches to it.
            Weapon {
                archetype: WeaponArchetype::Pistol,
                damage: settings.weapon_damage,
                fire_rate: settings.weapon_fire_rate,
                projectile_speed: settings.weapon_projectile_speed,
                pellet_count: 1,
                spread_rad: 0.0,
                burst_rounds: 1,
                fire_timer: Timer::from_seconds(0.0, TimerMode::Once),
                burst_rounds_remaining: 0,
                // Phase 1 (beam): start with the beam unheld.
                // The repeating timer is initialized to the
                // configured tick interval so the first damage
                // tick is exactly `beam_tick_interval_s` after
                // the press, not immediate.
                beam_active: false,
                beam_tick_timer: Timer::from_seconds(
                    settings.beam_tick_interval_s,
                    TimerMode::Repeating,
                ),
            },
        ))
        .with_children(|parent| {
            // White line sticking out the top of the player; rotated each
            // frame to match the current aim direction. This is the
            // placeholder for a future gun sprite / hand attachment.
            parent.spawn((
                Sprite {
                    color: AIM_INDICATOR_COLOR,
                    custom_size: Some(Vec2::new(6.0, 30.0)),
                    ..default()
                },
                // +Y offset so the line sits ahead of the player body,
                // not centered on it. Z slightly forward to avoid z-fighting.
                Transform::from_xyz(0.0, 20.0, 1.0),
                AimIndicator,
            ));
        });

    // Day 8-10: chaser spawner entity — persists for the run, ticks
    // every `spawner_interval_s` and creates a chaser at a random
    // angle from the player, at a random distance in [min_distance,
    // max_distance].
    //
    // Phase 2: `RoomScope { room_id: RoomId(1) }` scopes this
    // spawner to the Fight room (Room 1). The spawner entity
    // exists from startup through the whole run; the
    // `spawner_tick_system` in enemy.rs filters by
    // `BiomeState.current_room` and only ticks spawners in the
    // current room. The Spawn / Extract rooms have no spawners
    // (R0 is the player's safe spawn, R2 is the extract endpoint
    // for Patch 6).
    commands.spawn((
        Spawner {
            interval: Timer::from_seconds(settings.spawner_interval_s, TimerMode::Repeating),
            spawn: EnemyArchetype::Chaser,
        },
        RoomScope { room_id: RoomId(1) },
    ));

    // Phase 1: dasher spawner. Same interval, separate entity — the
    // two spawners tick independently, so the player sees a chaser
    // every 3s and a dasher every 3s in alternating (well, parallel)
    // waves. Phase 2 will fold this into a single wave-spawner
    // entity that picks archetypes from a wave definition.
    commands.spawn((
        Spawner {
            interval: Timer::from_seconds(settings.spawner_interval_s, TimerMode::Repeating),
            spawn: EnemyArchetype::Dasher,
        },
        RoomScope { room_id: RoomId(1) },
    ));

    // Phase 1: shooter spawner. Same parallel-spawner pattern as
    // dasher. A fresh wave of shooters will land every 3s, spread
    // their first shot over the fire interval (so they don't
    // triple-tap on the first frame — see `spawn_shooter`), and
    // start plinking at the player from 250 units away. With
    // chaser + dasher + shooter all spawning every 3s, the player
    // sees 3 archetypes in parallel waves.
    commands.spawn((
        Spawner {
            interval: Timer::from_seconds(settings.spawner_interval_s, TimerMode::Repeating),
            spawn: EnemyArchetype::Shooter,
        },
        RoomScope { room_id: RoomId(1) },
    ));

    // Phase 1: sniper spawner. Same parallel pattern. A fresh
    // sniper spawns in the post-fire cooldown state (not
    // charging) so the wave doesn't all open fire on the same
    // frame — see `spawn_sniper`. Sniper max range is 400 vs
    // shooter's 250, so the two ranged archetypes naturally
    // space out and the player faces pressure from multiple
    // distances.
    commands.spawn((
        Spawner {
            interval: Timer::from_seconds(settings.spawner_interval_s, TimerMode::Repeating),
            spawn: EnemyArchetype::Sniper,
        },
        RoomScope { room_id: RoomId(1) },
    ));

    // Phase 2: title-screen overlay. Spawned once at startup and
    // toggled visible by `state_visibility_system` when state is
    // `MainMenu`. Hidden in InRun / GameOver. Two entities: a dim
    // full-screen bg + a centered "PRESS ENTER TO START" text.
    // z = MAIN_MENU_Z (above the game-over overlay) so a
    // MainMenu re-entry cleanly covers whatever was on screen.
    commands.spawn((
        Sprite {
            color: MAIN_MENU_BG_COLOR,
            custom_size: Some(Vec2::new(MAIN_MENU_BG_SIZE.0, MAIN_MENU_BG_SIZE.1)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, MAIN_MENU_Z),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::MainMenu,
        },
    ));
    commands.spawn((
        Text2d::new(MAIN_MENU_TEXT),
        TextFont {
            font_size: MAIN_MENU_TEXT_SIZE,
            ..default()
        },
        TextColor(MAIN_MENU_TEXT_COLOR),
        // +0.1 over the bg so the text renders on top within the
        // same 2D phase (matches the §12 sprite+text z-offset rule).
        Transform::from_xyz(0.0, 0.0, MAIN_MENU_Z + 0.1),
        Visibility::Hidden,
        HudElement {
            kind: HudElementKind::MainMenu,
        },
    ));

    info!("RUSTWORN booted — player + chaser/dasher/shooter/sniper spawners + main menu overlay spawned at origin");
}

// ─── Per-state input systems ──────────────────────────────────────────────

// MainMenu: ENTER starts a new run. ESC quits the binary — the
// title screen is the natural "I'm done" point, and there's no
// "back" hierarchy to fall through to. (If the user wants to
// escape during a run, ESC takes them back to MainMenu via
// `in_run_esc_system` below.)
fn menu_input_system(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::InRun);
    }
    if keys.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
}

// InRun: ESC returns to the title. R-restart during a run is
// intentionally NOT bound here — restart is a GameOver-only
// affordance (you can only "restart" if you're dead). If the
// player wants to abandon a run, the path is ESC → MainMenu.
fn in_run_esc_system(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::MainMenu);
    }
}

// GameOver: R restarts (→ InRun, fires `enter_in_run_system`),
// ESC returns to the title (→ MainMenu).
fn game_over_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        next_state.set(AppState::InRun);
    }
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::MainMenu);
    }
}

// ─── State-driven visibility (Phase 2) ───────────────────────────────────

// Per-frame visibility update driven by the current `AppState`.
// Cheaper than respawning entities on every state transition:
// the player, AIM indicator, all HUD bars, and the title overlay
// are spawned once at startup and just toggled. The cascade
// telegraph update is gated separately in hud.rs.
//
// Why a single dedicated system (not a per-state OnEnter/OnExit):
// a future patch might add fade-in/out animations, and having
// the visibility logic in one place makes that a localized
// change. The cost is one query per frame — negligible at our
// entity counts.
//
// B0001 hygiene (per skill §2, §14): player_q has `&mut Visibility`
// + `With<Player>`, aim_q has `&mut Visibility` + `With<AimIndicator>`,
// and hud_q has `&mut Visibility, &HudElement` + `Without<AimIndicator>`.
// The complementary `Without<AimIndicator>` on player_q and
// `Without<Player>` on hud_q would also satisfy the disjointness
// check structurally, but the AIM marker is on a child of the
// player, so the simpler partition is "player owns its visibility
// (and children inherit), HUD owns its own, AIM owns its own".
fn state_visibility_system(
    state: Res<State<AppState>>,
    mut player_q: Query<&mut Visibility, (With<Player>, Without<AimIndicator>)>,
    mut aim_q: Query<&mut Visibility, With<AimIndicator>>,
    mut hud_q: Query<(&mut Visibility, &HudElement), (Without<Player>, Without<AimIndicator>)>,
) {
    // Player + AIM are visible in InRun and GameOver, hidden in
    // MainMenu + Paused. Reading `state.get()` returns the
    // current AppState; comparing it once keeps the per-entity
    // branches cheap.
    let show_player = matches!(state.get(), AppState::InRun | AppState::GameOver);
    let player_vis = if show_player {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut player_q {
        *v = player_vis;
    }
    for mut v in &mut aim_q {
        *v = player_vis;
    }

    // HUD elements are toggled by their HudElementKind. The MainMenu
    // overlay is visible only in MainMenu; the game-over overlay
    // is visible only in GameOver; everything else (HP bar, fire
    // rate, kills, time, cascade, boss HP) is visible in InRun
    // and GameOver.
    for (mut v, hud) in &mut hud_q {
        let show = match hud.kind {
            HudElementKind::MainMenu => *state.get() == AppState::MainMenu,
            HudElementKind::GameOver | HudElementKind::RestartPrompt => {
                *state.get() == AppState::GameOver
            }
            // HP / fire-rate / kill / time / cascade / boss HP:
            // visible whenever the run is "active or showing
            // the result" (InRun + GameOver). Hidden on the
            // title screen so the menu is clean.
            _ => matches!(*state.get(), AppState::InRun | AppState::GameOver),
        };
        *v = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

// ─── OnEnter(AppState::InRun) ─────────────────────────────────────────────

// Replaces the Phase 1 key-gated `restart_system`. Fires on
// every InRun entry — first boot from MainMenu, or restart
// from GameOver (or from any future state that transitions
// back to InRun). The body is the same as the Phase 1
// restart: despawn transient entities, reset player HP /
// position / weapon state, reset spawner timers, reset the
// camera, zero screen shake, reset Cascade cycle, and zero
// RunState counters.
//
// The signature is wider than the Phase 1 version because we
// no longer need the `keys` parameter (the R-key check is
// gone — state transitions are handled by `game_over_input_system`).
// The trade-off: a 13-arg system is a clippy `too_many_arguments`
// warning, but each arg is a different resource / query the
// reset genuinely needs, and refactoring them into a struct
// would scatter the reset logic across more files than it's
// worth. Same warning the Phase 1 restart_system already had.
#[allow(clippy::too_many_arguments)]
fn enter_in_run_system(
    settings: Res<GameSettings>,
    mut run: ResMut<RunState>,
    mut player_query: Query<(Entity, &mut Transform, &mut Health, &mut Weapon), With<Player>>,
    mut spawner_query: Query<&mut Spawner>,
    mut camera_query: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
    mut shake: ResMut<ScreenShake>,
    mut cascade: ResMut<CascadeState>,
    mut commands: Commands,
    enemies: Query<Entity, With<Enemy>>,
    projectiles: Query<Entity, With<Projectile>>,
    puffs: Query<Entity, With<DeathPuff>>,
    pickups: Query<Entity, With<Pickup>>,
) {
    for e in &enemies {
        commands.entity(e).despawn();
    }
    for e in &projectiles {
        commands.entity(e).despawn();
    }
    for e in &puffs {
        commands.entity(e).despawn();
    }
    for e in &pickups {
        commands.entity(e).despawn();
    }

    if let Ok((entity, mut tf, mut h, mut weapon)) = player_query.single_mut() {
        tf.translation = Vec3::ZERO;
        h.current = h.max;
        // Reset fire timer to a zero-duration finished timer so the
        // first click on the fresh run fires immediately (same as
        // the initial spawn in setup.rs).
        weapon.fire_timer = Timer::from_seconds(0.0, TimerMode::Once);
        // Phase 1 (burst): also reset the in-flight burst counter.
        // If the player died mid-burst (e.g. on round 2 of a 3-round
        // burst), the next run shouldn't inherit a half-spent
        // burst — the new run should boot with a fresh weapon
        // ready to fire on click.
        weapon.burst_rounds_remaining = 0;
        // Drop any leftover dash / hit / knockback state from the
        // previous run. The player boots clean, no i-frames, no
        // knockback direction, no cooldown.
        commands
            .entity(entity)
            .remove::<(Invulnerable, Knockback, DashCooldown, Collider)>();
        // Collider is re-added immediately so the fresh player is
        // collidable the instant gameplay resumes — removing and
        // re-adding in the same frame is a no-op visually but
        // ensures we get a fresh, default-sized collider.
        commands.entity(entity).insert(Collider {
            half_extents: Vec2::splat(20.0),
        });
    }

    for mut spawner in &mut spawner_query {
        spawner.interval = Timer::from_seconds(settings.spawner_interval_s, TimerMode::Repeating);
    }

    if let Ok(mut cam_tf) = camera_query.single_mut() {
        cam_tf.translation = Vec3::ZERO;
    }

    shake.intensity = 0.0;
    shake.duration = 0.0;
    shake.remaining = 0.0;

    // Phase 1 cascade: reset the cycle on each new run so a fresh
    // run gets the standard warmup before the first Cascade window.
    // Without this, a Cascade that opened just before the previous
    // run's death would persist (still active, but with no enemies
    // to slow) into the new run, and the player would see the HUD
    // bar empty immediately.
    cascade.active = false;
    cascade.time_remaining = 0.0;
    cascade.kills_during = 0;
    cascade.chain_extends = 0;
    cascade.interval_timer = Timer::from_seconds(30.0, TimerMode::Once);
    cascade.duration_timer = Timer::from_seconds(0.0, TimerMode::Once);

    // Phase 1 mini-boss: reset the boss spawn gate. The boss
    // itself is despawned via the `enemies` loop above
    // (carries the `Enemy` marker), but the gate flag
    // needs to flip back to false so the next run gets a
    // fresh boss spawn at the new `time_elapsed == 0 +
    // boss_spawn_time_s` crossing. Also reset the cascade
    // beacon to false — the flag flips to true on boss
    // death (the Phase 1 cascade hook) and the new run
    // starts without a beacon pending.
    run.boss_spawned = false;
    run.cascade_beacon_alive = false;

    run.time_elapsed = 0.0;
    run.enemies_killed = 0;
    run.score = 0;
    // Phase 1 pickups: reset the run-long stat stacks so the
    // fresh run boots at baseline. The pickups themselves
    // are despawned by the `enemies` / `projectiles` /
    // `puffs` loops above (pickups have no `Projectile`
    // marker, so a separate `pickups` loop is needed —
    // handled below).
    run.damage_stacks = 0;
    run.fire_rate_stacks = 0;
    run.speed_stacks = 0;
    run.regen_per_sec = 0.0;

    info!("RUSTWORN — InRun entered: world reset, run-long stats cleared");
}
