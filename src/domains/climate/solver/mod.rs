// src/domains/climate/solver/mod.rs

use bevy::math::Vec3;

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

/// Предрассчитанные константы сетки и рельефа (считаются 1 раз вместо ~300 раз за генерацию)
pub(super) struct SolverPrecalc {
    pub avg_hex_rad: f32,
    pub sin_lat: Vec<f32>,
    pub cos_lat: Vec<f32>,
    pub east: Vec<Vec3>,
    pub north: Vec<Vec3>,
    pub mountain_cooling: Vec<f32>,
    pub ocean_equilibrium: Vec<f32>,
    pub ocean_ref_temp: Vec<f32>,
    pub moderation: Vec<f32>,
    pub bathymetry_shift: Vec<f32>,
}

impl SolverPrecalc {
    fn new(
        topology: &WorldTopology,
        geology: &GeologyLayer,
        continentality: &[f32],
        config: &ClimateConfig,
    ) -> Self {
        let num_cells = topology.cells.len();
        let avg_hex_rad = (4.0 * std::f32::consts::PI / num_cells as f32).sqrt();
        let global_avg = (config.equator_temp + config.pole_temp) / 2.0;
        let r = config.marginal_sea_moderation_reduction;

        let mut sin_lat = Vec::with_capacity(num_cells);
        let mut cos_lat = Vec::with_capacity(num_cells);
        let mut east = Vec::with_capacity(num_cells);
        let mut north = Vec::with_capacity(num_cells);
        let mut mountain_cooling = Vec::with_capacity(num_cells);
        let mut ocean_equilibrium = Vec::with_capacity(num_cells);
        let mut ocean_ref_temp = Vec::with_capacity(num_cells);
        let mut moderation = Vec::with_capacity(num_cells);
        let mut bathymetry_shift = Vec::with_capacity(num_cells);

        for i in 0..num_cells {
            let cell = &topology.cells[i];
            let pos = cell.center;
            let lat = cell.lat_lon.x;
            let elev = geology.elevations[i];
            let c = continentality[i];

            let (s_lat, c_lat) = lat.sin_cos();
            sin_lat.push(s_lat);
            cos_lat.push(c_lat);

            let e = pos.cross(Vec3::Y).normalize_or_zero();
            let n = pos.cross(e).normalize_or_zero();
            east.push(e);
            north.push(n);

            let m_height = elevation_cooling(
                elev,
                config.mountain_base_level,
                config.mountain_blend_width,
            );
            mountain_cooling.push(m_height * config.altitude_lapse_rate);

            let annual_lat_insolation = c_lat.max(0.0);
            let ocean_lat_baseline = config.pole_temp
                + (config.equator_temp - config.pole_temp) * annual_lat_insolation * 0.85;

            ocean_equilibrium.push(ocean_lat_baseline * 0.82 + global_avg * 0.18);
            ocean_ref_temp.push(ocean_lat_baseline * 0.65 + global_avg * 0.35);

            let mod_val = if c < 0.0 {
                config.base_ocean_moderation * (1.0 - r * (1.0 - c.abs()))
            } else {
                config.base_ocean_moderation * (1.0 - r) * (1.0 - c)
            };
            moderation.push(mod_val);

            let b_shift = if elev <= 0.0 && lat.abs() > 1.0 {
                let deep_water_factor = smoothstep((elev - (-0.06)) / (-0.22 - (-0.06)));
                (deep_water_factor - 0.35) * 6.0
            } else {
                0.0
            };
            bathymetry_shift.push(b_shift);
        }

        Self {
            avg_hex_rad,
            sin_lat,
            cos_lat,
            east,
            north,
            mountain_cooling,
            ocean_equilibrium,
            ocean_ref_temp,
            moderation,
            bathymetry_shift,
        }
    }
}

/// Переиспользуемые буферы памяти на один сезон (0 аллокаций внутри цикла равновесия!)
pub(super) struct SeasonScratch {
    pub sea_level_temps: Vec<f32>,
    pub ocean_temps: Vec<f32>,
    pub prev_ocean: Vec<f32>,
    pub advected_temps: Vec<f32>,
    pub prev_temps: Vec<f32>,
    pub thermal_field: Vec<f32>,
    pub prev_thermal: Vec<f32>,
    pub prev_pressure: Vec<f32>,
    pub prev_currents: Vec<Vec3>,
}

impl SeasonScratch {
    fn new(num_cells: usize) -> Self {
        Self {
            sea_level_temps: vec![0.0; num_cells],
            ocean_temps: vec![0.0; num_cells],
            prev_ocean: vec![0.0; num_cells],
            advected_temps: vec![0.0; num_cells],
            prev_temps: vec![0.0; num_cells],
            thermal_field: vec![0.0; num_cells],
            prev_thermal: vec![0.0; num_cells],
            prev_pressure: vec![0.0; num_cells],
            prev_currents: vec![Vec3::ZERO; num_cells],
        }
    }
}

/// Главная функция: считает океаны 1 раз, затем ПАРАЛЛЕЛЬНО просчитывает Весну, Лето и Зиму
pub fn simulate_all_seasons(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    climate: &mut ClimateLayer,
    config: &ClimateConfig,
) {
    // 1. Считаем расстояние до океанов (общая география для всех сезонов)
    compute_continentality(topology, geology, climate, config);

    // 2. Предрасчет тригонометрии, локальных осей и базовых констант океана (1 раз!)
    let precalc = SolverPrecalc::new(topology, geology, &climate.continentality, config);

    let continentality = &climate.continentality;
    let spring = &mut climate.spring;
    let summer = &mut climate.summer;
    let winter = &mut climate.winter;

    // 3. Параллельно просчитываем 3 сезона на разных ядрах CPU через rayon::scope
    rayon::scope(|s| {
        s.spawn(|_| {
            simulate_single_season(0.0, topology, geology, continentality, config, &precalc, spring);
        });
        s.spawn(|_| {
            simulate_single_season(1.0, topology, geology, continentality, config, &precalc, summer);
        });
        s.spawn(|_| {
            simulate_single_season(-1.0, topology, geology, continentality, config, &precalc, winter);
        });
    });

    // 4. Формируем кэш 3D-стрелок по выборке Фибоначчи (1 раз в конце)
    let sampled_cells = sample_cells_on_sphere(topology, 900);

    for snap in [&mut climate.spring, &mut climate.summer, &mut climate.winter] {
        snap.wind_arrows_cache = build_arrows_from_samples(&sampled_cells, &snap.winds_3d, None);
        snap.current_arrows_cache = build_arrows_from_samples(
            &sampled_cells,
            &snap.ocean_currents,
            Some(&geology.elevations),
        );
    }

    // 5. Загружаем выбранный сезон в основные буферы отрисовки
    climate.apply_active_season(config.active_season);
}

fn simulate_single_season(
    season_mult: f32, // 0.0 = Весна, 1.0 = Лето, -1.0 = Зима
    topology: &WorldTopology,
    geology: &GeologyLayer,
    continentality: &[f32],
    config: &ClimateConfig,
    precalc: &SolverPrecalc,
    snap: &mut SeasonalSnapshot,
) {
    let num_cells = topology.cells.len();
    let sun_lat = config.axial_tilt * season_mult;
    let (sin_sun, cos_sun) = sun_lat.sin_cos();

    let mut scratch = SeasonScratch::new(num_cells);

    // Первичная оценка температуры от Солнца
    for i in 0..num_cells {
        let elev = geology.elevations[i];
        // Быстрое косинусное тождество: cos(lat - sun_lat) = cos(lat)*cos(sun) + sin(lat)*sin(sun)
        let base_insolation = (precalc.cos_lat[i] * cos_sun + precalc.sin_lat[i] * sin_sun).max(0.0);

        let temp_est =
            config.pole_temp + (config.equator_temp - config.pole_temp) * base_insolation;
        let temp_with_height = temp_est - precalc.mountain_cooling[i];

        snap.albedo[i] = calc_albedo(temp_with_height, elev, config);
        let rad_sea_temp = calc_radiative_sea_temp(
            i,
            snap.albedo[i],
            base_insolation,
            precalc,
            config,
            geology,
        );

        scratch.sea_level_temps[i] = rad_sea_temp;
        snap.temperatures[i] = rad_sea_temp - precalc.mountain_cooling[i];
    }

    // Цикл поиска равновесия (Температура <-> Давление/Ветра <-> Альбедо льда)
    let iterations = config.equilibrium_iterations.max(1);
    for _ in 0..iterations {
        // А) Считаем давление и ветра для текущего распределения температур
        compute_season_pressure_and_winds(
            sun_lat,
            topology,
            geology,
            continentality,
            config,
            precalc,
            &mut scratch,
            snap,
        );

        // Б) Генерируем океанические течения на основе новых ветров
        compute_ocean_currents(
            topology,
            geology,
            continentality,
            config,
            precalc,
            &mut scratch,
            snap,
        );

        // В) Океаническая адвекция: течения взламывают льды
        scratch.ocean_temps.copy_from_slice(&scratch.sea_level_temps);
        for _ in 0..15 {
            std::mem::swap(&mut scratch.ocean_temps, &mut scratch.prev_ocean);
            for i in 0..num_cells {
                if geology.elevations[i] > 0.0 {
                    scratch.ocean_temps[i] = scratch.prev_ocean[i];
                    continue;
                }

                let current = snap.ocean_currents[i];
                let speed = current.length();
                if speed < 0.001 {
                    scratch.ocean_temps[i] = scratch.prev_ocean[i];
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
                        upwind_sum += scratch.prev_ocean[n_id] * w;
                        weight_sum += w;
                    }
                }

                if weight_sum > 0.0 {
                    let incoming = upwind_sum / weight_sum;
                    let ref_current_speed =
                        (1.5 * config.ocean_current_speed_multiplier).max(0.05);
                    let norm_speed = (speed / ref_current_speed).clamp(0.0, 1.0);
                    let step_rate = norm_speed * config.ocean_advection_rate.min(0.85);
                    scratch.ocean_temps[i] =
                        scratch.prev_ocean[i] * (1.0 - step_rate) + incoming * step_rate;
                } else {
                    scratch.ocean_temps[i] = scratch.prev_ocean[i];
                }
            }
        }

        // Фиксируем тепло океана в базовой температуре перед воздушной адвекцией
        for i in 0..num_cells {
            if geology.elevations[i] <= 0.0 {
                scratch.sea_level_temps[i] =
                    scratch.sea_level_temps[i] * 0.25 + scratch.ocean_temps[i] * 0.75;
            }
        }

        // Г) Дальнобойный перенос тепла ветрами (воздух над сушей и морем)
        scratch.advected_temps.copy_from_slice(&scratch.sea_level_temps);
        for _ in 0..25 {
            std::mem::swap(&mut scratch.advected_temps, &mut scratch.prev_temps);
            for i in 0..num_cells {
                let cell = &topology.cells[i];
                let wind = snap.winds_3d[i];
                let wind_speed = wind.length();

                if wind_speed < 0.001 {
                    scratch.advected_temps[i] = scratch.prev_temps[i];
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
                        upwind_sum += scratch.prev_temps[n_id] * w;
                        weight_sum += w;
                    }
                }

                if weight_sum > 0.0 {
                    let incoming = upwind_sum / weight_sum;
                    let step_rate = (wind_speed * 0.06).clamp(0.0, 0.55);
                    scratch.advected_temps[i] =
                        scratch.prev_temps[i] * (1.0 - step_rate) + incoming * step_rate;
                } else {
                    scratch.advected_temps[i] = scratch.prev_temps[i];
                }
            }
        }

        // Д) Радиационный баланс: смешиваем принесенный ветром воздух с локальным нагревом Солнца
        let air_blend = (config.heat_advection_strength * 0.55).clamp(0.0, 0.85);
        let ocean_blend = config.ocean_advection_rate.clamp(0.0, 0.85);

        for i in 0..num_cells {
            let base_insolation =
                (precalc.cos_lat[i] * cos_sun + precalc.sin_lat[i] * sin_sun).max(0.0);
            let rad_temp = calc_radiative_sea_temp(
                i,
                snap.albedo[i],
                base_insolation,
                precalc,
                config,
                geology,
            );
            let elev = geology.elevations[i];

            if elev <= 0.0 {
                let transported_water_temp =
                    scratch.ocean_temps[i] * 0.8 + scratch.advected_temps[i] * 0.2;
                scratch.sea_level_temps[i] =
                    rad_temp * (1.0 - ocean_blend) + transported_water_temp * ocean_blend;
            } else {
                scratch.sea_level_temps[i] =
                    rad_temp * (1.0 - air_blend) + scratch.advected_temps[i] * air_blend;
            }

            let final_temp = scratch.sea_level_temps[i] - precalc.mountain_cooling[i];
            snap.temperatures[i] = final_temp;
            snap.albedo[i] = calc_albedo(final_temp, elev, config);
        }
    }

    // Финальная синхронизация давления, ветров и течений
    compute_season_pressure_and_winds(
        sun_lat,
        topology,
        geology,
        continentality,
        config,
        precalc,
        &mut scratch,
        snap,
    );
    compute_ocean_currents(
        topology,
        geology,
        continentality,
        config,
        precalc,
        &mut scratch,
        snap,
    );
}

/// Равномерно распределённые точки на сфере (golden spiral).
fn sample_cells_on_sphere(topology: &WorldTopology, count: usize) -> Vec<usize> {
    let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
    let mut result = Vec::with_capacity(count);
    let denom = (count.saturating_sub(1)).max(1) as f32;
    let mut used = std::collections::HashSet::with_capacity(count);

    for k in 0..count {
        let y = 1.0 - (k as f32 / denom) * 2.0;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let theta = golden * k as f32;
        let sample = bevy::math::Vec3::new(theta.cos() * r, y, theta.sin() * r);

        let mut best_i = 0usize;
        let mut best_d = f32::MAX;
        for (i, cell) in topology.cells.iter().enumerate() {
            let d = sample.distance_squared(cell.center);
            if d < best_d {
                best_d = d;
                best_i = i;
            }
        }
        if used.insert(best_i) {
            result.push(best_i);
        }
    }
    result
}

/// Строит готовый кэш стрелок (индекс клетки, нормированная скорость) по 95-му процентилю
fn build_arrows_from_samples(
    samples: &[usize],
    vectors: &[bevy::math::Vec3],
    elevations: Option<&[f32]>,
) -> Vec<(usize, f32)> {
    let mut speeds: Vec<f32> = vectors.iter().map(|v| v.length()).collect();
    speeds.sort_by(|a, b| a.total_cmp(b));
    let idx = ((speeds.len() as f32) * 0.95) as usize;
    let max_speed = speeds.get(idx).copied().unwrap_or(0.0001).max(0.0001);

    let mut arrows = Vec::with_capacity(samples.len());
    for &i in samples {
        if let Some(elevs) = elevations {
            if elevs[i] > 0.0 {
                continue;
            }
        }
        let speed = vectors[i].length();
        if speed < max_speed * 0.05 {
            continue;
        }
        let s = (speed / max_speed).clamp(0.0, 1.0).sqrt();
        arrows.push((i, s));
    }
    arrows
}