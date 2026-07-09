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
use bevy::sprite::Anchor;
use log::info;

use crate::components::{
    AimIndicator, AppState, BaseVelocity, Collider, DashCooldown, DeathPuff, Enemy, GameOverReason,
    Health, HudElement, HudElementKind, Invulnerable, Knockback, LastMoveDirection, MainCamera,
    MotionTrail, Pickup, Player, PlayerClass, Projectile, Velocity, Weapon, WeaponArchetype,
    WeaponInventory, PLAYER_COLOR,
};
use crate::resources::{CascadeState, CrownSave, FloorState, GameSettings, MetaConfig, MetaUpgradeKind, RunRng, RunState, ScreenShake};

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
            .init_resource::<FloorState>()
            .init_resource::<RunRng>()
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
                    paused_esc_system,
                    game_over_input_system,
                    state_visibility_system,
                )
                    .chain(),
            )
            .add_systems(OnEnter(AppState::InRun), enter_in_run_system);
    }
}

fn spawn_world(mut commands: Commands, settings: Res<GameSettings>, meta: Res<MetaConfig>) {
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
            // Phase 2.4 (P4): multi-slot weapon inventory.
            // The player owns up to 4 weapons (PLAN.md §5).
            // Starts with Pistol in slot 0 and 3 empty
            // slots. The `Weapon` component below is the
            // ACTIVE weapon (slot 0's archetype by
            // default). Picking up a weapon adds it to
            // the first empty slot; the 1-4 hotkeys
            // swap which slot is active.
            WeaponInventory::default(),
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
            // Phase 2 P5.1: Sandevistan-style motion trail during
            // Cascade Mode. The trail system spawns ghost afterimages
            // at this entity's position each frame while Cascade is
            // active, fading from full alpha to transparent.
            MotionTrail,
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

    // Day 8-10: chaser spawner entity — Phase 2.3 removes this
    // and the 3 siblings below. Wave spawning now lives in
    // `setup_rooms_system` (rooms.rs) as one `WaveSpawner`
    // per Fight room, driven by `WaveDefinition` lists. The
    // `Spawner { interval, spawn }` struct is gone (see
    // components.rs).
    //
    // Why move spawners to per-room? Because Phase 2 added
    // rooms: enemies in R0 (Spawn) and R2 (Extract) make no
    // sense, so the 4 "always firing" spawners are wrong by
    // construction. A per-room wave definition lets each room
    // have its own progression (R1: 3 waves; later biomes:
    // 5+ waves with mini-bosses; etc.).
    //
    // The old spawner queries (e.g. `Query<&mut Spawner>` in
    // enter_in_run_system) are removed; the wave spawner is
    // re-armed on room transition (door_transition_system)
    // and on full restart (setup_rooms_system creates a
    // fresh WaveSpawner on every OnEnter InRun).

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

    // Phase 3 meta-progression: shop overlay. Spawned once at startup,
    // visible only on MainMenu (toggled by state_visibility_system).
    // Shows Crown balance + 8 upgrade rows. Each row shows key, name,
    // level/cost. The Crown balance entity updates dynamically via a
    // small update system.
    //
    // The shop text is a single large multi-line Text2d entity.
    // Positioned in the top-left quarter of the screen.
    let shop_text = format!(
        "=== CROWN SHOP ===\n\nCrown: {{}}\n\n1: {:<12} {}/3 (15 Cr)\n2: {:<12} {}/3 (20 Cr)\n3: {:<12} {}/3 (15 Cr)\n4: {:<12} {}/2 (10 Cr)\n5: {:<12} {}/1 (50 Cr)\n6: {:<12} {}/2 (25 Cr)\n7: {:<12} {}/3 (30 Cr)\n8: {:<12} {}/2 (25 Cr)\n\nPress 1-8 to buy  |  ESC to exit",
        meta.upgrades[0].name, meta.upgrades[0].max_level,
        meta.upgrades[1].name, meta.upgrades[1].max_level,
        meta.upgrades[2].name, meta.upgrades[2].max_level,
        meta.upgrades[3].name, meta.upgrades[3].max_level,
        meta.upgrades[4].name, meta.upgrades[4].max_level,
        meta.upgrades[5].name, meta.upgrades[5].max_level,
        meta.upgrades[6].name, meta.upgrades[6].max_level,
        meta.upgrades[7].name, meta.upgrades[7].max_level,
    );
    commands.spawn((
        Text2d::new(shop_text),
        TextFont { font_size: 16.0, ..default() },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        Anchor::TopLeft,
        TextLayout::new_with_justify(JustifyText::Left),
        Transform::from_xyz(-580.0, 320.0, MAIN_MENU_Z + 0.2),
        Visibility::Hidden,
        HudElement { kind: HudElementKind::MetaShop },
    ));

    // Character select text — centered below the title.
    // Shows 4 characters with current selection highlighted.
    // Updated dynamically by character_select_system.
    let char_text = concat!(
        "Select character (1-4):\n\n",
        "1: Marauder  [selected]  — Knife AoE | Grenade (3s)\n",
        "2: Reaver    [press 2]   — Lifesteal | Dash-strike (2s)\n",
        "3: Spectre   [press 3]   — Regen     | Teleport (1.5s)\n",
        "4: Engineer  [press 4]   — +1 HP     | Turret (4s)\n",
    );
    commands.spawn((
        Text2d::new(char_text),
        TextFont { font_size: 14.0, ..default() },
        TextColor(Color::srgb(0.7, 0.7, 0.8)),
        Anchor::Center,
        TextLayout::new_with_justify(JustifyText::Center),
        Transform::from_xyz(0.0, -280.0, MAIN_MENU_Z + 0.15),
        Visibility::Hidden,
        HudElement { kind: HudElementKind::CharacterSelect },
    ));

    info!("RUSTWORN booted — player + chaser/dasher/shooter/sniper spawners + main menu overlay spawned at origin");
}

// ─── Per-state input systems ──────────────────────────────────────────────

// MainMenu: ENTER starts a new run. ESC quits the binary — the
// title screen is the natural "I'm done" point, and there's no
// "back" hierarchy to fall through to. (If the user wants to
// escape during a run, ESC takes them back to MainMenu via
// `in_run_esc_system` below.)
//
// 1-8 keys buy meta-upgrades when the shop is visible.
fn menu_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    meta: Res<MetaConfig>,
    mut save: ResMut<CrownSave>,
    mut run: ResMut<RunState>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::InRun);
    }
    if keys.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }

    // Meta-upgrade purchases (keys 1-8).
    if !handle_upgrade_purchases(&keys, &meta, &mut save) {
        // Key wasn't a purchase — try character select.
        handle_character_select(&keys, run);
    }
}

/// Returns true if a key was consumed as an upgrade purchase.
fn handle_upgrade_purchases(
    keys: &Res<ButtonInput<KeyCode>>,
    meta: &Res<MetaConfig>,
    save: &mut ResMut<CrownSave>,
) -> bool {
        for (key_idx, key) in [
            KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4,
            KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8,
        ].iter().enumerate() {
            if keys.just_pressed(*key) {
                let def = &meta.upgrades[key_idx];
                let current_level = save.upgrades[key_idx];
                if current_level < def.max_level {
                    let cost = def.base_cost;
                    if save.crown >= cost {
                        save.crown -= cost;
                        save.upgrades[key_idx] = current_level + 1;
                        crate::plugins::meta::write_save(&save);
                        info!(
                            "Bought {} (level {}/{}) for {} Crown",
                            def.name, current_level + 1, def.max_level, cost
                        );
                    } else {
                        info!(
                            "Not enough Crown for {} (need {}, have {})",
                            def.name, cost, save.crown
                        );
                    }
                }
                return true;
            }
        }
        false
    }

    /// Handle character select keys (1-4 on the MainMenu).
    fn handle_character_select(keys: &Res<ButtonInput<KeyCode>>, mut run: ResMut<RunState>) {
        if keys.just_pressed(KeyCode::Digit1) {
            run.selected_class = PlayerClass::Marauder;
            info!("Selected character: Marauder");
        } else if keys.just_pressed(KeyCode::Digit2) {
            run.selected_class = PlayerClass::Reaver;
            info!("Selected character: Reaver");
        } else if keys.just_pressed(KeyCode::Digit3) {
            run.selected_class = PlayerClass::Spectre;
            info!("Selected character: Spectre");
        } else if keys.just_pressed(KeyCode::Digit4) {
            run.selected_class = PlayerClass::Engineer;
            info!("Selected character: Engineer");
        }
    }
// Paused: ESC resumes (→ InRun).
fn paused_esc_system(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::InRun);
    }
}

// player wants to abandon a run, the path is ESC → MainMenu.
fn in_run_esc_system(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Paused);
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
    run: Option<Res<RunState>>,
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
            HudElementKind::GameOver => {
                *state.get() == AppState::GameOver
                    && run.as_ref().map(|r| r.game_over_reason == GameOverReason::Died).unwrap_or(true)
            }
            HudElementKind::GameOverExtracted => {
                *state.get() == AppState::GameOver
                    && run.as_ref().map(|r| r.game_over_reason == GameOverReason::Extracted).unwrap_or(false)
            }
            HudElementKind::PauseOverlay => *state.get() == AppState::Paused,
            HudElementKind::RestartPrompt => {
                *state.get() == AppState::GameOver
            }
            // Meta shop & character select: visible only in MainMenu.
            HudElementKind::MetaShop | HudElementKind::CharacterSelect => *state.get() == AppState::MainMenu,
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
    mut settings: ResMut<GameSettings>,
    _meta: Res<MetaConfig>,
    save: Res<CrownSave>,
    mut run: ResMut<RunState>,
    mut rng: ResMut<RunRng>,
    mut player_query: Query<
        (
            Entity,
            &mut Transform,
            &mut Health,
            &mut Weapon,
            &mut WeaponInventory,
        ),
        With<Player>,
    >,
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

    if let Ok((entity, mut tf, mut h, mut weapon, mut inventory)) = player_query.single_mut() {
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
        // Phase 2.4 (P4): reset the weapon inventory to the
        // default (Pistol in slot 0, 3 empty slots). The
        // player should boot into a fresh run with no
        // carryover weapons from a previous attempt.
        *inventory = WeaponInventory::default();
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

        // Apply meta-progression upgrades.
        // Max HP
        let max_hp_level = save.upgrades[MetaUpgradeKind::MaxHp as usize];
        h.max += max_hp_level as f32;
        h.current = h.max;

        // Starting gold
        let start_gold = save.upgrades[MetaUpgradeKind::StartingGold as usize];
        run.gold = start_gold * 10;

        // Extra slot: expand inventory to 5 if at level 1
        let extra_slot = save.upgrades[MetaUpgradeKind::ExtraSlot as usize];
        if extra_slot > 0 && inventory.slots.len() < 5 {
            // Tried to add a 5th slot — but slots is [Option<WeaponArchetype>; 4]
            // This requires changing the type. For now: skip — we handle this
            // separately in a future refactor if needed.
        }

        // Apply character-specific starting stats.
        match run.selected_class {
            PlayerClass::Marauder => {
                // Default stats — everything stays as-is.
            }
            PlayerClass::Reaver => {
                h.max = 3.0;
                h.current = 3.0;
                run.regen_per_sec += 0.1; // Minor lifesteal supplement
            }
            PlayerClass::Spectre => {
                h.max = 3.0;
                h.current = 3.0;
                run.regen_per_sec += 0.5; // Better baseline regen
            }
            PlayerClass::Engineer => {
                h.max = 5.0;
                h.current = 5.0;
            }
        }
    }

    // Apply meta-progression upgrades (game-level, not player-entity-level).
    // Move speed
    let spd_level = save.upgrades[MetaUpgradeKind::MoveSpeed as usize];
    if spd_level > 0 {
        // Apply to GameSettings — player_move_speed is the base
        // and the run stat uses this value
        settings.player_move_speed *= 1.0 + spd_level as f32 * 0.10;
    }

    // Dash cooldown
    let dash_cd = save.upgrades[MetaUpgradeKind::DashCooldown as usize];
    if dash_cd > 0 {
        settings.player_dash_cooldown_s -= dash_cd as f32 * 0.2;
    }

    // Cascade duration
    let cas_level = save.upgrades[MetaUpgradeKind::CascadeDuration as usize];
    if cas_level > 0 {
        settings.cascade_duration_s += cas_level as f32 * 3.0;
    }

    // Damage: store as a base multiplier in RunState
    let dmg_level = save.upgrades[MetaUpgradeKind::Damage as usize];
    if dmg_level > 0 {
        run.damage_stacks += dmg_level;
    }

    // Regen
    let regen_level = save.upgrades[MetaUpgradeKind::Regen as usize];
    if regen_level > 0 {
        run.regen_per_sec += regen_level as f32 * 0.5;
    }

    // Apply character-specific speed modifiers.
    match run.selected_class {
        PlayerClass::Reaver => {
            settings.player_move_speed *= 1.15;
        }
        PlayerClass::Engineer => {
            settings.player_move_speed *= 0.85;
        }
        _ => {}
    }

    // Phase 2.3: seed the run's RNG from system time. Phase 3+
    // will let the player pick a seed (daily-challenge mode)
    // and the seed will come from the menu screen instead of
    // the clock. For Patch 3, "fresh run = different enemies
    // each time" is the right behavior.
    //
    // The seed goes into both `run.seed` (a record of the seed
    // for the run summary screen — Phase 6 will read it) and
    // `rng.state` (the actual xorshift state, advanced by
    // every random call this run).
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xDEADBEEF);
    run.seed = seed;
    *rng = RunRng::new(seed);

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
    run.xp = 0;
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

    // Reset ability cooldowns so they're ready on fresh run.
    run.marauder_ability_cooldown = Timer::from_seconds(0.0, TimerMode::Once);

    info!("RUSTWORN — InRun entered: world reset, run-long stats cleared");
}
