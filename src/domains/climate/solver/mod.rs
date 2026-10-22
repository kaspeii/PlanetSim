mod circulation;
mod continentality;
mod thermodynamics;

use super::{
    climate_config::ClimateConfig,
    climate_data::{ClimateLayer, SeasonalSnapshot},
};
use crate::{domains::tectonics::tectonics_data::GeologyLayer, world::topology::WorldTopology};

use circulation::{compute_ocean_currents, compute_season_pressure_and_winds};
use continentality::compute_continentality;
use thermodynamics::{calc_albedo, calc_radiative_sea_temp, elevation_cooling};

pub(super) fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Главная функция: считает океаны 1 раз, затем просчитывает Весну, Лето и Зиму
pub fn simulate_all_seasons(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    climate: &mut ClimateLayer,
    config: &ClimateConfig,
) {
    // 1. Считаем расстояние до океанов (общая география для всех сезонов)
    compute_continentality(topology, geology, climate, config);

    let continentality = climate.continentality.clone();

    // 2. Просчитываем 3 сезона: Весна (0.0), Лето Сев. полушария (+1.0), Зима Сев. полушария (-1.0)
    simulate_single_season(
        0.0,
        topology,
        geology,
        &continentality,
        config,
        &mut climate.spring,
    );
    simulate_single_season(
        1.0,
        topology,
        geology,
        &continentality,
        config,
        &mut climate.summer,
    );
    simulate_single_season(
        -1.0,
        topology,
        geology,
        &continentality,
        config,
        &mut climate.winter,
    );

    // 3. Загружаем выбранный сезон (по умолчанию Spring) в основные буферы отрисовки
    climate.apply_active_season(config.active_season);
}

fn simulate_single_season(
    season_mult: f32, // 0.0 = Весна, 1.0 = Лето, -1.0 = Зима
    topology: &WorldTopology,
    geology: &GeologyLayer,
    continentality: &[f32],
    config: &ClimateConfig,
    snap: &mut SeasonalSnapshot,
) {
    let num_cells = topology.cells.len();
    let sun_lat = config.axial_tilt * season_mult;

    // Первичная оценка температуры от Солнца
    let mut sea_level_temps = vec![0.0_f32; num_cells];
    for i in 0..num_cells {
        let elev = geology.elevations[i];
        let lat = topology.cells[i].lat_lon.x;
        let base_insolation = (lat - sun_lat).cos().max(0.0);

        let temp_est =
            config.pole_temp + (config.equator_temp - config.pole_temp) * base_insolation;

        let mountain_height = elevation_cooling(
            elev,
            config.mountain_base_level,
            config.mountain_blend_width,
        );
        let temp_with_height = temp_est - mountain_height * config.altitude_lapse_rate;

        snap.albedo[i] = calc_albedo(temp_with_height, elev, config);
        let rad_sea_temp = calc_radiative_sea_temp(
            i,
            snap.albedo[i],
            sun_lat,
            topology,
            continentality,
            config,
            geology,
        );

        sea_level_temps[i] = rad_sea_temp;
        snap.temperatures[i] = rad_sea_temp - mountain_height * config.altitude_lapse_rate;
    }

    // Цикл поиска равновесия (Температура <-> Давление/Ветра <-> Альбедо льда)
    let iterations = config.equilibrium_iterations.max(1);
    for _ in 0..iterations {
        // А) Считаем давление и ветра для текущего распределения температур
        compute_season_pressure_and_winds(sun_lat, topology, geology, continentality, config, snap);

        // Б) Генерируем океанические течения на основе новых ветров
        compute_ocean_currents(topology, geology, continentality, config, snap);

        // В) Океаническая адвекция: течения взламывают льды
        let mut ocean_temps = sea_level_temps.clone();
        let mut prev_ocean = vec![0.0_f32; num_cells];
        for _ in 0..15 {
            std::mem::swap(&mut ocean_temps, &mut prev_ocean);
            for i in 0..num_cells {
                if geology.elevations[i] > 0.0 {
                    ocean_temps[i] = prev_ocean[i];
                    continue;
                }

                let current = snap.ocean_currents[i];
                let speed = current.length();
                if speed < 0.001 {
                    ocean_temps[i] = prev_ocean[i];
                    continue;
                }

                let dir = current / speed;
                let mut upwind_sum = 0.0;
                let mut weight_sum = 0.0;

                for &n_id in &topology.cells[i].neighbors {
                    if geology.elevations[n_id] > 0.0 {
                        continue;
                    }

                    let from_neighbor = (topology.cells[i].center - topology.cells[n_id].center)
                        .normalize_or_zero();
                    let alignment = from_neighbor.dot(dir);
                    if alignment > 0.0 {
                        let w = alignment * alignment;
                        upwind_sum += prev_ocean[n_id] * w;
                        weight_sum += w;
                    }
                }

                if weight_sum > 0.0 {
                    let incoming = upwind_sum / weight_sum;
                    let ref_current_speed =
                        (1.5 * config.ocean_current_speed_multiplier).max(0.05);
                    let norm_speed = (speed / ref_current_speed).clamp(0.0, 1.0);
                    let step_rate = norm_speed * config.ocean_advection_rate.min(0.85);
                    ocean_temps[i] = prev_ocean[i] * (1.0 - step_rate) + incoming * step_rate;
                } else {
                    ocean_temps[i] = prev_ocean[i];
                }
            }
        }

        // Фиксируем тепло океана в базовой температуре перед воздушной адвекцией
        for i in 0..num_cells {
            if geology.elevations[i] <= 0.0 {
                sea_level_temps[i] = sea_level_temps[i] * 0.25 + ocean_temps[i] * 0.75;
            }
        }

        // Г) Дальнобойный перенос тепла ветрами (воздух над сушей и морем)
        let mut advected_temps = sea_level_temps.clone();
        let mut prev_temps = vec![0.0_f32; num_cells];
        for _ in 0..25 {
            std::mem::swap(&mut advected_temps, &mut prev_temps);
            for i in 0..num_cells {
                let cell = &topology.cells[i];
                let wind = snap.winds_3d[i];
                let wind_speed = wind.length();

                if wind_speed < 0.001 {
                    continue;
                }

                let wind_dir = wind / wind_speed;
                let mut upwind_sum = 0.0;
                let mut weight_sum = 0.0;

                for &n_id in &cell.neighbors {
                    let from_neighbor =
                        (cell.center - topology.cells[n_id].center).normalize_or_zero();
                    let alignment = from_neighbor.dot(wind_dir);
                    if alignment > 0.0 {
                        let w = alignment * alignment;
                        upwind_sum += prev_temps[n_id] * w;
                        weight_sum += w;
                    }
                }

                if weight_sum > 0.0 {
                    let incoming = upwind_sum / weight_sum;
                    let step_rate = (wind_speed * 0.06).clamp(0.0, 0.55);
                    advected_temps[i] = prev_temps[i] * (1.0 - step_rate) + incoming * step_rate;
                }
            }
        }

        // Д) Радиационный баланс: смешиваем принесенный ветром воздух с локальным нагревом Солнца
        let air_blend = (config.heat_advection_strength * 0.55).clamp(0.0, 0.85);
        let ocean_blend = config.ocean_advection_rate.clamp(0.0, 0.85);

        for i in 0..num_cells {
            let rad_temp = calc_radiative_sea_temp(
                i,
                snap.albedo[i],
                sun_lat,
                topology,
                continentality,
                config,
                geology,
            );
            let elev = geology.elevations[i];

            if elev <= 0.0 {
                let transported_water_temp = ocean_temps[i] * 0.8 + advected_temps[i] * 0.2;
                sea_level_temps[i] =
                    rad_temp * (1.0 - ocean_blend) + transported_water_temp * ocean_blend;
            } else {
                sea_level_temps[i] = rad_temp * (1.0 - air_blend) + advected_temps[i] * air_blend;
            }

            let mountain_height = elevation_cooling(
                elev,
                config.mountain_base_level,
                config.mountain_blend_width,
            );
            let final_temp = sea_level_temps[i] - mountain_height * config.altitude_lapse_rate;
            snap.temperatures[i] = final_temp;
            snap.albedo[i] = calc_albedo(final_temp, elev, config);
        }
    }

    // Финальная синхронизация давления и ветров
    compute_season_pressure_and_winds(sun_lat, topology, geology, continentality, config, snap);
    // Финальный проход течений для отрисовки
    compute_ocean_currents(topology, geology, continentality, config, snap);
}