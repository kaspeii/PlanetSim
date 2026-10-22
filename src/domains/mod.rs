pub mod generator;
pub mod climate;
pub mod tectonics;
// pub mod hydrology;
// pub mod civilization;

use bevy::prelude::*;

use crate::domains::generator::{handle_generation_task, spawn_generation_task};

pub struct DomainsPlugin;

impl Plugin for DomainsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((tectonics::TectonicsPlugin, climate::ClimatePlugin))
            // Запускаем спавн задачи, когда появились в состоянии Generating, а задачи еще нет
            .add_systems(
                Update,
                (
                    spawn_generation_task.run_if(
                        generator::is_generating
                            .and_then(not(any_with_component::<generator::GenerationTask>)),
                    ),
                    handle_generation_task.run_if(generator::is_generating),
                ),
            );
    }
}
