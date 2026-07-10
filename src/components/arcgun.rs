//! Arc Gun chain-lightning marker component.

use bevy::prelude::*;

/// Marker component for Arc Gun projectiles.
/// When an Arc Gun projectile hits an enemy, it chains lightning
/// to nearby enemies for reduced damage.
#[derive(Component)]
pub struct ArcGunProjectile;