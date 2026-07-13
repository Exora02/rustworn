//! Bevy plugins. Each module exports a `*Plugin` struct that wires up
//! its systems, resources, and events into the `App`.
//!
//! Convention: a plugin owns one feature area and exposes a single
//! `Plugin::build()` that registers everything it needs. Long-term
//! modules (see PLAN.md §8) live alongside the minimal Phase 0 set.

pub mod audio;
pub mod combat;
pub mod enemy;
pub mod fx;
pub mod hub;
pub mod hud;
pub mod meta;
pub mod pickups;
pub mod player;
pub mod rooms;
pub mod settings;
pub mod setup;

pub use audio::AudioPlugin;
pub use combat::CombatPlugin;
pub use enemy::EnemyPlugin;
pub use fx::FxPlugin;
pub use hub::HubPlugin;
pub use hud::HudPlugin;
pub use meta::MetaPlugin;
pub use pickups::PickupsPlugin;
pub use player::PlayerPlugin;
pub use rooms::RoomsPlugin;
pub use settings::SettingsPlugin;
pub use setup::SetupPlugin;
