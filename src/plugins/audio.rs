//! Placeholder audio plugin.
//!
//! Listens for game events (DamageEvent, EnemyKilledEvent, PickupCollectedEvent,
//! WeaponFiredEvent, PlayerDashEvent, etc.) and plays corresponding sound effects
//! via bevy's built-in `Audio` resource.
//!
//! Currently in **placeholder mode**: the systems log every sound that would play,
//! because no .ogg / .wav files are in `assets/audio/` yet. To activate real audio:
//!
//!   1. Drop `.ogg` files into `assets/audio/` (e.g., `pistol.ogg`, `shotgun.ogg`,
//!      `burst.ogg`, `beam.ogg`, `dash.ogg`, `enemy_hit.ogg`, `enemy_death.ogg`,
//!      `pickup.ogg`, `cascade_start.ogg`, `cascade_end.ogg`, `extraction.ogg`,
//!      `menu_press.ogg`, `crown_purchase.ogg`).
//!   2. Remove the `// STUB:` comments and `log::info!` calls from each sound
//!      handler.
//!   3. Load each asset once at startup (see `AudioPlugin::build`) and store
//!      handles, then call `audio.play(handle)` with optional pitch / speed.
//!
//! Bevy's `Audio` resource supports:
//!   - `audio.play(handle)` — one-shot
//!   - `audio.play_with_settings(handle, PlaybackSettings { speed, volume, .. })`
//!     — pitched playback (different pitch per weapon archetype, for example)
//!
//! For placeholder testing, every sound is prefixed with "🕹️" so the dev log is
//! easy to grep (e.g., `cargo run 2>&1 | grep '🕹️'`).

use bevy::prelude::*;

use crate::events::{
    CascadeEndEvent, CascadeStartEvent, DamageEvent, EnemyKilledEvent, PickupCollectedEvent,
    PlayerDashEvent, RoomClearedEvent, WeaponFiredEvent,
};

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        // Register audio-specific events (shared game events like DamageEvent
        // are registered in lib.rs — we only register the ones we own here).
        app.add_event::<PlayerDashEvent>()
            .add_event::<WeaponFiredEvent>()
            .add_event::<PickupCollectedEvent>();

        // Placeholder: in the future, pre-load audio handles here:
        //
        //   let asset_server = app.world().resource::<AssetServer>();
        //   app.insert_resource(AudioHandles {
        //       pistol:        asset_server.load("audio/pistol.ogg"),
        //       shotgun:       asset_server.load("audio/shotgun.ogg"),
        //       burst:         asset_server.load("audio/burst.ogg"),
        //       beam:          asset_server.load("audio/beam.ogg"),
        //       dash:          asset_server.load("audio/dash.ogg"),
        //       enemy_hit:     asset_server.load("audio/enemy_hit.ogg"),
        //       enemy_death:   asset_server.load("audio/enemy_death.ogg"),
        //       pickup:        asset_server.load("audio/pickup.ogg"),
        //       cascade_start: asset_server.load("audio/cascade_start.ogg"),
        //       cascade_end:   asset_server.load("audio/cascade_end.ogg"),
        //       extraction:    asset_server.load("audio/extraction.ogg"),
        //       menu_press:    asset_server.load("audio/menu_press.ogg"),
        //       crown_purchase: asset_server.load("audio/crown_purchase.ogg"),
        //   });

        // Register event-listener systems.
        // All run every frame but early-return if no events are queued.
        app.add_systems(
            Update,
            (
                weapon_fire_audio_system,
                damage_audio_system,
                enemy_killed_audio_system,
                pickup_collected_audio_system,
                dash_audio_system,
                cascade_start_audio_system,
                cascade_end_audio_system,
                room_cleared_audio_system,
            ),
        );
    }
}

// ─── Sound handler systems ────────────────────────────────────────────

/// Plays a weapon-specific sound when the player fires.
/// Different weapons get different pitches / sounds:
///   Pistol  → default pitch
///   Shotgun → lower pitch (bigger boom)
///   Burst   → slightly higher pitch (snappier)
///   Beam    → sustained / higher (energy weapon)
fn weapon_fire_audio_system(
    mut events: EventReader<WeaponFiredEvent>,
    // STUB: `audio: Res<Audio>` when real assets are loaded
) {
    for ev in events.read() {
        let sound_name = match ev.archetype {
            crate::components::WeaponArchetype::Pistol => "pistol_fire",
            crate::components::WeaponArchetype::Shotgun => "shotgun_fire",
            crate::components::WeaponArchetype::Burst => "burst_fire",
            crate::components::WeaponArchetype::Beam => "beam_fire",
            crate::components::WeaponArchetype::RocketLauncher => "rocket_fire",
            crate::components::WeaponArchetype::Sniper => "sniper_fire",
            crate::components::WeaponArchetype::SMG => "smg_fire",
            crate::components::WeaponArchetype::ArcGun => "arcgun_fire",
        };
        // STUB: audio.play_with_settings(
        //     handles.pistol.clone(),
        //     PlaybackSettings { speed: pitch, .. },
        // );
        log::info!("🕹️ AUDIO: {sound_name}");
    }
}

/// Plays a hit sound on every DamageEvent (enemy hit, player hit, etc.)
fn damage_audio_system(mut events: EventReader<DamageEvent>) {
    for _ev in events.read() {
        // STUB: audio.play(handles.enemy_hit.clone());
        log::info!("🕹️ AUDIO: enemy_hit");
    }
}

/// Plays a death sound on every EnemyKilledEvent
fn enemy_killed_audio_system(mut events: EventReader<EnemyKilledEvent>) {
    for _ev in events.read() {
        // STUB: audio.play(handles.enemy_death.clone());
        log::info!("🕹️ AUDIO: enemy_death");
    }
}

/// Plays a pickup sound, choosing between generic "collect" and
/// special sounds for Heal, Weapon, and Gold.
fn pickup_collected_audio_system(mut events: EventReader<PickupCollectedEvent>) {
    for ev in events.read() {
        let sound_name = match ev.kind {
            crate::components::PickupKind::Heal => "pickup_heal",
            crate::components::PickupKind::Weapon(_) => "pickup_weapon",
            crate::components::PickupKind::Gold(_) => "pickup_gold",
            _ => "pickup_collect",
        };
        // STUB: audio.play(handles.pickup.clone());
        log::info!("🕹️ AUDIO: {sound_name}");
    }
}

/// Plays a whoosh / wind sound when the player dashes
fn dash_audio_system(mut events: EventReader<PlayerDashEvent>) {
    for _ev in events.read() {
        // STUB: audio.play(handles.dash.clone());
        log::info!("🕹️ AUDIO: dash");
    }
}

/// Plays a cascade-activation sting (speed-up / time-bend effect)
fn cascade_start_audio_system(mut events: EventReader<CascadeStartEvent>) {
    for _ev in events.read() {
        // STUB: audio.play(handles.cascade_start.clone());
        log::info!("🕹️ AUDIO: cascade_start");
    }
}

/// Plays a cascade-deactivation sting (return to normal speed)
fn cascade_end_audio_system(mut events: EventReader<CascadeEndEvent>) {
    for _ev in events.read() {
        // STUB: audio.play(handles.cascade_end.clone());
        log::info!("🕹️ AUDIO: cascade_end");
    }
}

/// Plays a fanfare / success sound when a room is cleared
fn room_cleared_audio_system(mut events: EventReader<RoomClearedEvent>) {
    for _ev in events.read() {
        // STUB: audio.play(handles.extraction.clone());
        log::info!("🕹️ AUDIO: room_cleared");
    }
}