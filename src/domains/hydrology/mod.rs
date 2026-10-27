// src/domains/hydrology/mod.rs

mod hydrology_config;
pub mod hydrology_data;
pub mod hydrology_palettes;
pub mod hydrology_ui;
mod solver;

pub use hydrology_config::HydrologyConfig;
pub use hydrology_data::HydrologyLayer;
pub use solver::{apply_surface_feedback_to_climate, simulate_hydrology};

use bevy::prelude::*;

pub struct HydrologyPlugin;

impl Plugin for HydrologyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HydrologyConfig>()
            .init_resource::<HydrologyLayer>();
    }
}
