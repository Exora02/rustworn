//! Singleton resources. These are the state that lives across systems
//! but only once per app.
//!
//! For per-run state (current seed, biome, score, time remaining) see
//! `RunState` — it gets reset at the start of every run.

pub use cascade::CascadeState;
pub use effects::{BossRampTimer, Hitstop, ScreenShake};
pub use rng::RunRng;
pub use state::{FloorState, RunState};
pub use meta::{CrownSave, MetaConfig, MetaUpgradeKind, UnlockableDef, UnlockableKind};
pub use tuning::GameSettings;

mod cascade;
mod effects;
mod rng;
mod meta;
mod state;
mod tuning;
