use super::{config::ClimateConfig, data::ClimateLayer};
use crate::{domains::tectonics::data::GeologyLayer, world::topology::WorldTopology};
use bevy::prelude::*;

pub fn simulate_temperature(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    climate: &mut ClimateLayer,
    config: &ClimateConfig,
) {
    let num_cells = topology.cells.len();

    for i in 0..num_cells {
        let cell = &topology.cells[i];
        let elevation = geology.elevations[i];

        // 1. Базовая температура по широте
        // lat_lon.x - долгота, lat_lon.x - широта (от -PI/2 до PI/2)
        let lat = cell.lat_lon.x;

        // cos(lat) дает 1.0 на экваторе и 0.0 на полюсах
        let solar_insolation = lat.cos();

        let mut temp =
            config.pole_temp + (config.equator_temp - config.pole_temp) * solar_insolation;

        // 2. Влияние высоты (остывание в горах)
        if elevation > 0.0 {
            temp -= elevation * config.altitude_lapse_rate;
        }

        // 3. Термоинерция океанов (вода медленнее греется и остывает, смягчая экстремумы)
        if elevation <= 0.0 {
            let global_avg = (config.equator_temp + config.pole_temp) / 2.0;
            temp = temp * (1.0 - config.ocean_moderation) + global_avg * config.ocean_moderation;
        }

        climate.temperatures[i] = temp;
    }
}

// В будущем добавим:
// pub fn simulate_humidity(...) {}
