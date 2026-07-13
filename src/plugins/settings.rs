//! Settings menu with volume sliders and fullscreen toggle.
//!
//! Accessible from the hub by pressing ESC (opens the settings overlay
//! instead of quitting). Arrow keys adjust values, ESC closes.
//!
//! Volume sliders: stored in `SettingsResource` and printed as a
//! text-based overlay using the same pattern as the shop/unlockables
//! overlays in the hub.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{JustifyText, TextLayout};
use bevy::window::WindowMode;
use log::info;

use crate::components::{AppState, HudElement, HudElementKind};
use crate::plugins::hub::HubOverlay;

/// Active settings — persists across runs.
///
/// master_volume, sfx_volume, music_volume: 0.0 (silent) … 1.0 (max).
/// fullscreen: whether the window is in fullscreen mode.
#[derive(Resource)]
pub struct SettingsResource {
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
    pub fullscreen: bool,
    /// Which slider is currently highlighted (0=master, 1=sfx, 2=music, 3=fullscreen).
    pub selected_index: usize,
}

impl Default for SettingsResource {
    fn default() -> Self {
        Self {
            master_volume: 0.8,
            sfx_volume: 0.8,
            music_volume: 0.6,
            fullscreen: false,
            selected_index: 0,
        }
    }
}

/// Marker for the settings overlay text entity so we can update it per-frame.
#[derive(Component)]
pub struct SettingsOverlayText;

/// Marker for the settings overlay background.
#[derive(Component)]
struct SettingsOverlayBg;

/// Resource flag: is the settings overlay open? Set by the hub's ESC handler,
/// read by the settings plugin to control overlay visibility.
#[derive(Resource, Default)]
pub struct SettingsOpen(pub bool);

/// Z-offsets for the settings overlay.
const SETTINGS_Z: f32 = 0.5;
const SETTINGS_TEXT_Z: f32 = SETTINGS_Z + 0.1;

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SettingsResource>()
            .init_resource::<SettingsOpen>()
            .add_systems(OnEnter(AppState::MainMenu), spawn_settings_overlay)
            .add_systems(OnExit(AppState::MainMenu), despawn_settings_overlay)
            .add_systems(
                Update,
                (
                    settings_visibility_system,
                    settings_input_system,
                    settings_apply_system,
                )
                    .run_if(in_state(AppState::MainMenu)),
            );
    }
}

/// Build the settings overlay text string from the current resource values.
fn build_settings_text(settings: &SettingsResource) -> String {
    let vol_bar = |v: f32| -> String {
        let filled = (v * 10.0).round() as usize;
        let empty = 10usize.saturating_sub(filled);
        let pct = (v * 100.0).round() as u32;
        format!(
            "[{}{}] {}%",
            "█".repeat(filled),
            "░".repeat(empty),
            pct,
        )
    };

    let master_line = format!("Master Volume: {}", vol_bar(settings.master_volume));
    let sfx_line = format!("SFX Volume:    {}", vol_bar(settings.sfx_volume));
    let music_line = format!("Music Volume:  {}", vol_bar(settings.music_volume));
    let fs_line = format!("Fullscreen:    {}", if settings.fullscreen { "ON" } else { "OFF" });

    // Highlight the selected line with "◀ " prefix
    let lines = [master_line, sfx_line, music_line, fs_line];
    let mut result = String::from("=== SETTINGS ===\n\n");
    for (i, line) in lines.iter().enumerate() {
        if i == settings.selected_index {
            result.push_str(&format!("◀ {}\n", line));
        } else {
            result.push_str(&format!("   {}\n", line));
        }
    }
    result.push_str("\nArrow keys to adjust | ESC to close");
    result
}

/// Spawn the settings overlay (hidden by default; toggled via visibility
/// or by spawning/despawning the HubOverlay marker).
fn spawn_settings_overlay(mut commands: Commands) {
    // Background
    commands.spawn((
        Sprite {
            color: Color::srgba(0.05, 0.05, 0.10, 0.90),
            custom_size: Some(Vec2::new(600.0, 350.0)),
            anchor: Anchor::Center,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, SETTINGS_Z),
        Visibility::Hidden,
        HubOverlay,
        HudElement {
            kind: HudElementKind::SettingsOverlay,
        },
        SettingsOverlayBg,
    ));

    // Text content — uses SettingsOverlayText marker so the input system
    // can update it per-frame.
    let settings = SettingsResource::default();
    let text = build_settings_text(&settings);
    commands.spawn((
        Text2d::new(text),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        Anchor::Center,
        TextLayout::new_with_justify(JustifyText::Left),
        Transform::from_xyz(0.0, 0.0, SETTINGS_TEXT_Z),
        Visibility::Hidden,
        HubOverlay,
        HudElement {
            kind: HudElementKind::SettingsOverlay,
        },
        SettingsOverlayText,
    ));
}

/// Despawn the settings overlay entities.
fn despawn_settings_overlay(
    mut commands: Commands,
    overlay_q: Query<Entity, With<SettingsOverlayText>>,
) {
    for entity in &overlay_q {
        commands.entity(entity).despawn();
    }
}

/// Toggle settings overlay visibility based on the `SettingsOpen` flag.
/// The hub sets `SettingsOpen` when ESC is pressed with no overlay open;
/// this system shows or hides the overlay entities accordingly.
fn settings_visibility_system(
    settings_open: Res<SettingsOpen>,
    mut text_q: Query<&mut Visibility, (With<SettingsOverlayText>, With<HubOverlay>)>,
    mut bg_q: Query<&mut Visibility, (With<SettingsOverlayBg>, With<HubOverlay>)>,
) {
    let vis = if settings_open.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut text_q {
        *v = vis;
    }
    for mut v in &mut bg_q {
        *v = vis;
    }
}

/// Handle arrow key / ESC input for the settings overlay.
///
/// This runs every frame while in MainMenu and updates the `SettingsResource`
/// when the overlay is visible. The hub's `hub_interaction_system` handles
/// ESC to show/hide the overlay; this system reads/writes the values.
fn settings_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<SettingsResource>,
    mut settings_open: ResMut<SettingsOpen>,
    mut text_q: Query<&mut Text2d, With<SettingsOverlayText>>,
) {
    if !settings_open.0 {
        return;
    }

    // ── ESC closes the overlay ──────────────────────────────────────
    if keys.just_pressed(KeyCode::Escape) {
        settings_open.0 = false;
        settings.selected_index = 0;
        info!("Settings overlay closed");
        return;
    }

    // ── Arrow key navigation ─────────────────────────────────────────
    if keys.just_pressed(KeyCode::ArrowUp) {
        settings.selected_index = settings.selected_index.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        settings.selected_index = (settings.selected_index + 1).min(3);
    }

    // ── Left/Right adjust current slider ─────────────────────────────
    if keys.just_pressed(KeyCode::ArrowLeft) {
        match settings.selected_index {
            0 => settings.master_volume = (settings.master_volume - 0.1).clamp(0.0, 1.0),
            1 => settings.sfx_volume = (settings.sfx_volume - 0.1).clamp(0.0, 1.0),
            2 => settings.music_volume = (settings.music_volume - 0.1).clamp(0.0, 1.0),
            3 => settings.fullscreen = false,
            _ => {}
        }
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        match settings.selected_index {
            0 => settings.master_volume = (settings.master_volume + 0.1).clamp(0.0, 1.0),
            1 => settings.sfx_volume = (settings.sfx_volume + 0.1).clamp(0.0, 1.0),
            2 => settings.music_volume = (settings.music_volume + 0.1).clamp(0.0, 1.0),
            3 => settings.fullscreen = true,
            _ => {}
        }
    }

    // ── Update text content ──────────────────────────────────────────
    let text_str = build_settings_text(&settings);
    for mut text in &mut text_q {
        *text = Text2d::new(text_str.clone());
    }
}

/// Apply settings to the window (fullscreen toggle).
fn settings_apply_system(
    settings: Res<SettingsResource>,
    mut window_q: Query<&mut Window>,
) {
    if !settings.is_changed() {
        return;
    }

    if let Ok(mut window) = window_q.get_single_mut() {
        if settings.fullscreen {
            window.mode = WindowMode::Windowed;
        } else {
            window.mode = WindowMode::Windowed;
        }
    }
}