//! Audio plugin.
//!
//! Listens for game events (DamageEvent, EnemyKilledEvent, PickupCollectedEvent,
//! WeaponFiredEvent, PlayerDashEvent, etc.) and plays corresponding sound effects
//! via bevy's built-in `Audio` resource.
//!
//! All 21 `.ogg` assets are loaded once at startup via `AudioHandles` and
//! then dispatched by event-driven systems.

use bevy::audio::AudioPlayer;
use bevy::prelude::*;

use crate::events::{
    CascadeEndEvent, CascadeStartEvent, DamageEvent, EnemyKilledEvent, PickupCollectedEvent,
    PlayerDashEvent, RoomClearedEvent, WeaponFiredEvent,
};

/// All 21 audio handles, loaded at plugin startup.
#[derive(Resource)]
pub struct AudioHandles {
    pub pistol_fire: Handle<AudioSource>,
    pub shotgun_fire: Handle<AudioSource>,
    pub burst_fire: Handle<AudioSource>,
    pub beam_fire: Handle<AudioSource>,
    pub rocket_fire: Handle<AudioSource>,
    pub sniper_fire: Handle<AudioSource>,
    pub smg_fire: Handle<AudioSource>,
    pub arcgun_fire: Handle<AudioSource>,
    pub enemy_hit: Handle<AudioSource>,
    pub enemy_death: Handle<AudioSource>,
    pub pickup_collect: Handle<AudioSource>,
    pub pickup_heal: Handle<AudioSource>,
    pub pickup_weapon: Handle<AudioSource>,
    pub pickup_gold: Handle<AudioSource>,
    pub dash: Handle<AudioSource>,
    pub cascade_start: Handle<AudioSource>,
    pub cascade_end: Handle<AudioSource>,
    pub room_cleared: Handle<AudioSource>,
    pub extraction: Handle<AudioSource>,
    pub menu_press: Handle<AudioSource>,
    pub crown_purchase: Handle<AudioSource>,
}

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        // Register audio-specific events (shared game events like DamageEvent
        // are registered in lib.rs — we only register the ones we own here).
        app.add_event::<PlayerDashEvent>()
            .add_event::<WeaponFiredEvent>()
            .add_event::<PickupCollectedEvent>();

        // Pre-load all 21 audio assets at startup.
        let asset_server = app.world().resource::<AssetServer>();
        app.insert_resource(AudioHandles {
            pistol_fire: asset_server.load("audio/pistol_fire.ogg"),
            shotgun_fire: asset_server.load("audio/shotgun_fire.ogg"),
            burst_fire: asset_server.load("audio/burst_fire.ogg"),
            beam_fire: asset_server.load("audio/beam_fire.ogg"),
            rocket_fire: asset_server.load("audio/rocket_fire.ogg"),
            sniper_fire: asset_server.load("audio/sniper_fire.ogg"),
            smg_fire: asset_server.load("audio/smg_fire.ogg"),
            arcgun_fire: asset_server.load("audio/arcgun_fire.ogg"),
            enemy_hit: asset_server.load("audio/enemy_hit.ogg"),
            enemy_death: asset_server.load("audio/enemy_death.ogg"),
            pickup_collect: asset_server.load("audio/pickup_collect.ogg"),
            pickup_heal: asset_server.load("audio/pickup_heal.ogg"),
            pickup_weapon: asset_server.load("audio/pickup_weapon.ogg"),
            pickup_gold: asset_server.load("audio/pickup_gold.ogg"),
            dash: asset_server.load("audio/dash.ogg"),
            cascade_start: asset_server.load("audio/cascade_start.ogg"),
            cascade_end: asset_server.load("audio/cascade_end.ogg"),
            room_cleared: asset_server.load("audio/room_cleared.ogg"),
            extraction: asset_server.load("audio/extraction.ogg"),
            menu_press: asset_server.load("audio/menu_press.ogg"),
            crown_purchase: asset_server.load("audio/crown_purchase.ogg"),
        });

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
    mut commands: Commands,
    handles: Res<AudioHandles>,
) {
    for ev in events.read() {
        let (handle, speed) = match ev.archetype {
            crate::components::WeaponArchetype::Pistol => (&handles.pistol_fire, 1.0),
            crate::components::WeaponArchetype::Shotgun => (&handles.shotgun_fire, 0.8),
            crate::components::WeaponArchetype::Burst => (&handles.burst_fire, 1.1),
            crate::components::WeaponArchetype::Beam => (&handles.beam_fire, 1.0),
            crate::components::WeaponArchetype::RocketLauncher => (&handles.rocket_fire, 1.0),
            crate::components::WeaponArchetype::Sniper => (&handles.sniper_fire, 1.0),
            crate::components::WeaponArchetype::SMG => (&handles.smg_fire, 1.0),
            crate::components::WeaponArchetype::ArcGun => (&handles.arcgun_fire, 1.0),
        };
        commands.spawn((
            AudioPlayer::new(handle.clone()),
            PlaybackSettings { speed, ..default() },
        ));
    }
}

/// Plays a hit sound on every DamageEvent (enemy hit, player hit, etc.)
fn damage_audio_system(
    mut events: EventReader<DamageEvent>,
    mut commands: Commands,
    handles: Res<AudioHandles>,
) {
    for _ev in events.read() {
        commands.spawn(AudioPlayer::new(handles.enemy_hit.clone()));
    }
}

/// Plays a death sound on every EnemyKilledEvent
fn enemy_killed_audio_system(
    mut events: EventReader<EnemyKilledEvent>,
    mut commands: Commands,
    handles: Res<AudioHandles>,
) {
    for _ev in events.read() {
        commands.spawn(AudioPlayer::new(handles.enemy_death.clone()));
    }
}

/// Plays a pickup sound, choosing between generic "collect" and
/// special sounds for Heal, Weapon, and Gold.
fn pickup_collected_audio_system(
    mut events: EventReader<PickupCollectedEvent>,
    mut commands: Commands,
    handles: Res<AudioHandles>,
) {
    for ev in events.read() {
        let handle = match ev.kind {
            crate::components::PickupKind::Heal => &handles.pickup_heal,
            crate::components::PickupKind::Weapon(_) => &handles.pickup_weapon,
            crate::components::PickupKind::Gold(_) => &handles.pickup_gold,
            _ => &handles.pickup_collect,
        };
        commands.spawn(AudioPlayer::new(handle.clone()));
    }
}

/// Plays a whoosh / wind sound when the player dashes
fn dash_audio_system(
    mut events: EventReader<PlayerDashEvent>,
    mut commands: Commands,
    handles: Res<AudioHandles>,
) {
    for _ev in events.read() {
        commands.spawn(AudioPlayer::new(handles.dash.clone()));
    }
}

/// Plays a cascade-activation sting (speed-up / time-bend effect)
fn cascade_start_audio_system(
    mut events: EventReader<CascadeStartEvent>,
    mut commands: Commands,
    handles: Res<AudioHandles>,
) {
    for _ev in events.read() {
        commands.spawn(AudioPlayer::new(handles.cascade_start.clone()));
    }
}

/// Plays a cascade-deactivation sting (return to normal speed)
fn cascade_end_audio_system(
    mut events: EventReader<CascadeEndEvent>,
    mut commands: Commands,
    handles: Res<AudioHandles>,
) {
    for _ev in events.read() {
        commands.spawn(AudioPlayer::new(handles.cascade_end.clone()));
    }
}

/// Plays a fanfare / success sound when a room is cleared
fn room_cleared_audio_system(
    mut events: EventReader<RoomClearedEvent>,
    mut commands: Commands,
    handles: Res<AudioHandles>,
) {
    for _ev in events.read() {
        commands.spawn(AudioPlayer::new(handles.room_cleared.clone()));
    }
}