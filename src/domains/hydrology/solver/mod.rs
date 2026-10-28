// src/domains/hydrology/solver/mod.rs

mod biomes;
mod rivers;
mod soils;

use super::{
    hydrology_config::HydrologyConfig,
    hydrology_data::{Biome, HydrologyLayer},
    hydrology_palettes::biome_satellite_rgb,
};
use crate::{
    domains::{
        climate::{ClimateConfig, climate_data::ClimateLayer},
        tectonics::tectonics_data::GeologyLayer,
    },
    world::topology::WorldTopology,
};
use biomes::compute_biomes;
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

    // Этап 2: Поверхностный сток, русла рек и влажность почвы (от дождей и рек)
    compute_rivers(topology, geology, climate, hydrology, config);

    // Этап 3: Биомы (строятся по влажности почвы, климату и рельефу)
    compute_biomes(topology, geology, climate, hydrology, config);
}

/// Записывает обратную связь от биомов, влажности почвы и рек обратно в ClimateLayer
pub fn apply_surface_feedback_to_climate(
    geology: &GeologyLayer,
    climate: &mut ClimateLayer,
    hydrology: &HydrologyLayer,
    c_config: &ClimateConfig,
    h_config: &HydrologyConfig,
) {
    let num_cells = geology.elevations.len();
    climate
        .surface_albedo
        .resize(num_cells, c_config.land_albedo);
    climate
        .surface_wetness
        .resize(num_cells, c_config.land_moisture_recycling);

    let threshold = h_config.river_threshold.max(1e-6);

    for i in 0..num_cells {
        if geology.elevations[i] <= 0.0 {
            climate.surface_albedo[i] = c_config.ocean_albedo;
            climate.surface_wetness[i] = 1.0;
            continue;
        }

        let biome = hydrology.biomes[i];
        let sm = hydrology.soil_moisture[i];
        let flow_ratio = (hydrology.flow_accumulation[i] / threshold).max(0.0);

        // 1. Альбедо поверхности: берем физическое альбедо биома,
        // слегка затемняем влажную почву и открытые водные зеркала крупных рек
        let (_, _, _, biome_albedo) = biome_satellite_rgb(biome);
        let moisture_darkening = sm * 0.035;
        let river_water_darkening = (flow_ratio.sqrt() * 0.015).clamp(0.0, 0.05);
        climate.surface_albedo[i] =
            (biome_albedo - moisture_darkening - river_water_darkening).clamp(0.08, 0.75);

        // 2. Испаряющая способность поверхности (surface_wetness):
        // Складывается из влажности почвы, транспирации растительности (биотический насос) и зеркала рек
        let vegetation_transpiration = match biome {
            Biome::TropicalRainforest | Biome::TemperateRainforest => 0.48 * sm.sqrt(),
            Biome::Wetland => 0.58,
            Biome::RiparianOasis
            | Biome::TropicalSeasonalForest
            | Biome::TemperateDeciduousForest
            | Biome::BorealTaiga => 0.36 * sm.sqrt(),
            Biome::Savanna | Biome::TemperateGrassland | Biome::MediterraneanShrubland => {
                0.24 * sm.sqrt()
            }
            Biome::Tundra => 0.18 * sm.sqrt(),
            _ => 0.08 * sm.sqrt(),
        };

        let river_evap = if flow_ratio >= 0.3 {
            (0.18 * flow_ratio.powf(0.40)).clamp(0.0, 0.55)
        } else {
            0.0
        };

        let raw_wetness = (sm * 0.55 + vegetation_transpiration + river_evap).clamp(0.08, 1.0);
        // Плавно смешиваем с базовым рециклингом конфига (70% биомы/реки + 30% базовый фон),
        // чтобы 2-й проход усиливал зеленые коридоры рек, но не выжигал переходные зоны в пустыню
        climate.surface_wetness[i] = raw_wetness * 0.72 + c_config.land_moisture_recycling * 0.28;
    }
}
