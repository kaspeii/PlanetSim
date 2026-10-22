// /src/domains/climate/solver/circulation.rs

use bevy::prelude::*;

use super::{smoothstep, SeasonScratch, SolverPrecalc};
use super::super::{climate_config::ClimateConfig, climate_data::SeasonalSnapshot};
use crate::{domains::tectonics::tectonics_data::GeologyLayer, world::topology::WorldTopology};

pub(super) fn compute_season_pressure_and_winds(
    sun_lat: f32,
    topology: &WorldTopology,
    geology: &GeologyLayer,
    continentality: &[f32],
    config: &ClimateConfig,
    precalc: &SolverPrecalc,
    scratch: &mut SeasonScratch,
    snap: &mut SeasonalSnapshot,
) {
    let num_cells = topology.cells.len();
    let avg_hex_rad = precalc.avg_hex_rad;

    // 1. Считаем среднюю температуру ОКЕАНА как эталон для каждой широты.
    for i in 0..num_cells {
        let c = continentality[i]; // -1.0 (океан) .. +1.0 (суша)
        let sea_level_temp = snap.temperatures[i] + precalc.mountain_cooling[i];
        let ocean_ref_temp = precalc.ocean_ref_temp[i];

        let temp_diff = (sea_level_temp - ocean_ref_temp) / 12.0;

        let weight = if c >= 0.0 {
            0.4 + 0.6 * c
        } else {
            0.4 * (1.0 + c * 0.6)
        };
        scratch.thermal_field[i] = (-temp_diff * weight).clamp(-1.2, 1.2);
    }

    // 2. Сглаживаем термическое поле (20 итераций) без аллокаций памяти
    for _ in 0..20 {
        std::mem::swap(&mut scratch.thermal_field, &mut scratch.prev_thermal);
        for i in 0..num_cells {
            let neighbors = &topology.cells[i].neighbors;
            if neighbors.is_empty() {
                continue;
            }
            let sum: f32 = neighbors.iter().map(|&n| scratch.prev_thermal[n]).sum();
            scratch.thermal_field[i] =
                scratch.prev_thermal[i] * 0.3 + (sum / neighbors.len() as f32) * 0.7;
        }
    }

    // 3. Собираем итоговое давление (Широтные пояса + Океанические ядра + Термические центры)
    for i in 0..num_cells {
        let lat = topology.cells[i].lat_lon.x;
        let lon = topology.cells[i].lat_lon.y;
        let c = continentality[i];
        let belt_lat = lat - sun_lat * 0.55;

        let polar_damping = 0.5 + 0.5 * precalc.cos_lat[i];
        let base_belt = -(6.0 * belt_lat).cos() * polar_damping;

        let belt_envelope = if c < 0.0 {
            let ocean_core = smoothstep(-c);
            // sin(2 * lat) = 2 * sin(lat) * cos(lat) — берем из кэша без вызова тригонометрии!
            let sin_2lat_abs = (2.0 * precalc.sin_lat[i] * precalc.cos_lat[i]).abs();
            let rossby_wave =
                1.0 + 0.35 * (3.0 * lon + belt_lat * 2.0).cos() * sin_2lat_abs;
            (0.55 + 0.85 * ocean_core) * rossby_wave
        } else {
            1.0 - 0.65 * smoothstep(c)
        };

        snap.pressure[i] = base_belt * belt_envelope * config.latitudinal_pressure_weight
            + scratch.thermal_field[i] * config.thermal_pressure_weight;
    }

    // Сглаживание итогового давления (8 итераций) через переиспользуемый буфер
    for _ in 0..8 {
        std::mem::swap(&mut snap.pressure, &mut scratch.prev_pressure);
        for i in 0..num_cells {
            let neighbors = &topology.cells[i].neighbors;
            if neighbors.is_empty() {
                continue;
            }
            let sum: f32 = neighbors.iter().map(|&n| scratch.prev_pressure[n]).sum();
            snap.pressure[i] =
                scratch.prev_pressure[i] * 0.3 + (sum / neighbors.len() as f32) * 0.7;
        }
    }

    // 4. Векторы ветров: ЕДИНЫЙ ФИЗИЧНЫЙ ГРАДИЕНТ прямо из карты давления!
    for i in 0..num_cells {
        let cell = &topology.cells[i];
        let pos = cell.center;
        let my_elev = geology.elevations[i].max(0.0);

        let east = precalc.east[i];
        let north = precalc.north[i];

        let my_p = snap.pressure[i];
        let mut grad_3d = Vec3::ZERO;
        let avg_step_dist = avg_hex_rad;

        for &n_id in &cell.neighbors {
            let n_pos = topology.cells[n_id].center;
            let raw_dir = n_pos - pos;

            let tangent_dir = (raw_dir - pos * raw_dir.dot(pos)).normalize_or_zero();
            let safe_dist = raw_dir.length().max(avg_hex_rad * 0.3);
            let diff = (my_p - snap.pressure[n_id]) / safe_dist;
            grad_3d += tangent_dir * diff;
        }

        if !cell.neighbors.is_empty() {
            let count = cell.neighbors.len() as f32;
            grad_3d = (grad_3d / count) * 0.22;
        }

        // Поворот Кориолиса из предрассчитанного синуса широты
        let coriolis_angle = -precalc.sin_lat[i] * config.coriolis_strength;
        let coriolis_rot = Quat::from_axis_angle(pos, coriolis_angle);
        let mut wind_3d = coriolis_rot * grad_3d;

        // Орографический барьер
        let mut mountain_drag = 0.0;
        let mut upslope_grad = Vec3::ZERO;
        let wind_dir = wind_3d.normalize_or_zero();

        for &n_id in &cell.neighbors {
            let n_pos = topology.cells[n_id].center;
            let raw_dir = n_pos - pos;
            let to_neighbor = (raw_dir - pos * raw_dir.dot(pos)).normalize_or_zero();

            let n_elev = geology.elevations[n_id].max(0.0);
            let elev_diff = n_elev - my_elev;

            if elev_diff > 0.0 {
                let slope = elev_diff / avg_step_dist;
                upslope_grad += to_neighbor * slope;

                let alignment = wind_dir.dot(to_neighbor);
                if alignment > 0.0 {
                    mountain_drag += slope * alignment;
                }
            }
        }

        if upslope_grad.length_squared() > 1e-6 {
            let slope_normal = upslope_grad.normalize();
            let into_wall = wind_3d.dot(slope_normal).max(0.0);
            let deflect_strength = (upslope_grad.length() * 0.35).clamp(0.0, 0.75);
            wind_3d -= slope_normal * (into_wall * deflect_strength);
        }

        let plateau_roughness = (my_elev - config.mountain_base_level).max(0.0) * 2.5;
        let total_drag = mountain_drag + plateau_roughness;

        let barrier_factor = 1.0 / (1.0 + total_drag * config.mountain_wind_barrier * 0.15);
        wind_3d = wind_3d * barrier_factor * config.wind_speed_multiplier;

        snap.winds_3d[i] = wind_3d;
        snap.winds[i] = Vec2::new(wind_3d.dot(east), wind_3d.dot(north));
    }
}

pub(super) fn compute_ocean_currents(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    continentality: &[f32],
    config: &ClimateConfig,
    precalc: &SolverPrecalc,
    scratch: &mut SeasonScratch,
    snap: &mut SeasonalSnapshot,
) {
    let num_cells = topology.cells.len();
    let avg_hex_rad = precalc.avg_hex_rad;

    for i in 0..num_cells {
        if geology.elevations[i] > 0.0 {
            snap.ocean_currents[i] = Vec3::ZERO;
            continue;
        }

        let cell = &topology.cells[i];
        let pos = cell.center;
        let sin_lat = precalc.sin_lat[i];

        // 1. Ветровое течение
        let mut wind_current = snap.winds_3d[i] * config.ocean_current_speed_multiplier;

        if snap.temperatures[i] <= 0.0 {
            wind_current *= 0.1;
        }

        // 2. Спираль Экмана
        let ekman_angle = -sin_lat * 0.75;
        let ekman_rot = Quat::from_axis_angle(pos, ekman_angle);
        wind_current = ekman_rot * wind_current;

        // 3. Геострофическое течение
        let my_p = snap.pressure[i];
        let mut grad_p = Vec3::ZERO;
        for &n_id in &cell.neighbors {
            if geology.elevations[n_id] <= 0.0 {
                let n_pos = topology.cells[n_id].center;
                let raw_dir = n_pos - pos;
                let tangent_dir = (raw_dir - pos * raw_dir.dot(pos)).normalize_or_zero();
                let safe_dist = raw_dir.length().max(avg_hex_rad * 0.3);
                let diff = (my_p - snap.pressure[n_id]) / safe_dist;
                grad_p += tangent_dir * diff;
            }
        }
        if !cell.neighbors.is_empty() {
            grad_p = (grad_p / cell.neighbors.len() as f32) * 0.5;
        }

        let geo_rot = Quat::from_axis_angle(pos, -sin_lat.signum() * std::f32::consts::FRAC_PI_2);
        let geostrophic_current = geo_rot * grad_p * config.geostrophic_multiplier;

        // 4. Смешиваем их
        let mut current_3d = wind_current * 0.4 + geostrophic_current * 0.6;

        // 5. Граничные течения (Отклонение от берегов)
        let mut coast_normal = Vec3::ZERO;
        for &n_id in &cell.neighbors {
            if geology.elevations[n_id] > 0.0 {
                let raw_dir = pos - topology.cells[n_id].center;
                let tangent_from_coast = (raw_dir - pos * raw_dir.dot(pos)).normalize_or_zero();
                coast_normal += tangent_from_coast;
            }
        }

        let orig_speed = current_3d.length();
        if coast_normal.length_squared() > 1e-6 && orig_speed > 1e-5 {
            coast_normal = coast_normal.normalize();
            let into_coast = current_3d.dot(-coast_normal);

            if into_coast > 0.0 {
                let mut along_coast = current_3d + coast_normal * into_coast;

                if along_coast.length_squared() > 1e-8 {
                    along_coast = along_coast.normalize() * orig_speed;
                }

                let east = precalc.east[i];
                let western_alignment = (-coast_normal).dot(east);

                let boost = config.boundary_current_multiplier
                    * (1.0 + 1.5 * smoothstep((western_alignment + 0.5) / 1.0));

                current_3d = along_coast * boost;
            }
        } else {
            let coastal_proximity = (1.0 - continentality[i].abs() * 3.0).clamp(0.0, 1.0);
            current_3d *=
                1.0 + (config.boundary_current_multiplier - 1.0) * 0.35 * coastal_proximity;
        }

        snap.ocean_currents[i] = current_3d;
    }

    // 6. Лёгкое сглаживание течений по соседям без аллокации вектора
    for _ in 0..2 {
        std::mem::swap(&mut snap.ocean_currents, &mut scratch.prev_currents);
        for i in 0..num_cells {
            if geology.elevations[i] > 0.0 {
                snap.ocean_currents[i] = Vec3::ZERO;
                continue;
            }
            let neighbors = &topology.cells[i].neighbors;
            if neighbors.is_empty() {
                snap.ocean_currents[i] = scratch.prev_currents[i];
                continue;
            }
            let mut sum = Vec3::ZERO;
            let mut cnt = 0.0_f32;
            for &n in neighbors {
                if geology.elevations[n] > 0.0 {
                    sum += scratch.prev_currents[i];
                } else {
                    sum += scratch.prev_currents[n];
                }
                cnt += 1.0;
            }
            snap.ocean_currents[i] = scratch.prev_currents[i] * 0.5 + (sum / cnt) * 0.5;
        }
    }
}