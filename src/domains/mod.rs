// src/domains/mod.rs

pub mod climate;
pub mod generator;
pub mod hydrology;
pub mod save;
pub mod tectonics;

use bevy::prelude::*;
use generator::{GenerationRequest, handle_generation_task, spawn_generation_task};
use save::SaveUiState;

pub struct DomainsPlugin;

impl Plugin for DomainsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GenerationRequest>()
            .init_resource::<SaveUiState>()
            .add_plugins((
                tectonics::TectonicsPlugin,
                climate::ClimatePlugin,
                hydrology::HydrologyPlugin,
            ))
            .add_systems(Startup, warmup_gizmos_shader)
            .add_systems(Update, (spawn_generation_task, handle_generation_task));
    }
}

/// Прогревает wgpu-пайплайн для 3D-стрелок на старте, чтобы при первом клике не было фриза
fn warmup_gizmos_shader(mut gizmos: Gizmos) {
    gizmos.arrow(Vec3::ZERO, Vec3::Y * 0.0001, Color::NONE);
}
