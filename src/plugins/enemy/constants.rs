//! Enemy spawn difficulty constants and helpers.
//!
//! Color/size constants were previously defined here but moved into
//! individual spawn functions in spawn.rs. Only the difficulty helper
//! remains.

use bevy::prelude::*;

pub const CREDIT_RATE_FIGHT: f32 = 8.0;
pub const CREDIT_RATE_HORDE: f32 = 10.0;
pub const CREDIT_RATE_ELITE: f32 = 6.0;
pub const CREDIT_RATE_BOSS: f32 = 5.0;
pub const CREDITS_MAX: f32 = 120.0;

pub fn floor_difficulty_mult(floor: u32) -> (f32, f32) {
    let hp_mult = 1.0 + (floor as f32).min(10.0) * 0.05;
    let dmg_mult = 1.0 + (floor as f32).min(10.0) * 0.03;
    (hp_mult, dmg_mult)
}