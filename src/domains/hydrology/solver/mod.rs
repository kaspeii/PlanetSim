// src/domains/hydrology/solver/mod.rs

mod rivers;
mod soils;

use crate::{
    domains::{climate::climate_data::ClimateLayer, tectonics::tectonics_data::GeologyLayer},
    world::topology::WorldTopology,
};
use super::{hydrology_config::HydrologyConfig, hydrology_data::HydrologyLayer};
use rivers::compute_rivers;
use soils::compute_soils;

pub fn simulate_hydrology(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    climate: &ClimateLayer,
    hydrology: &mut HydrologyLayer,
    config: &HydrologyConfig,
) {
    hydrology.resize(topology.cells.len());

    // Этап 1: Почвы и педотрансферные функции (CLORPT)
    compute_soils(topology, geology, climate, hydrology, config);

    // Этап 2: Поверхностный сток, заполнение впадин и речная сеть
    compute_rivers(topology, geology, climate, hydrology, config);
}