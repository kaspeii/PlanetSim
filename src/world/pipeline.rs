// src/world/pipeline.rs

use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum PipelineSet {
    Input,
    Simulation,
    Render,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum PipelineStage {
    Tectonics,
    Climate,
    // В будущем: Hydrology, Biomes
}

#[derive(Resource, Default)]
pub struct WorldPipeline {
    /// Полная пересборка 3D-вершин (после смены тектоники)
    pub rebuild_mesh: bool,
    /// Сплющивание/возврат океана (после галочки "Плоский океан")
    pub reposition_mesh: bool,
    /// Быстрая перекраска меша (после смены климата, сезона или слоя карты)
    pub recolor_mesh: bool,
}