// /src/world/states.rs

use bevy::prelude::*;
use super::pipeline::PipelineStage;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    MainMenu,
    Generating(PipelineStage),
    InGame,
}