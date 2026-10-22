// /src/domains/climate/solver/circulation.rs

use bevy::prelude::*;

use super::{smoothstep, thermodynamics::elevation_cooling};
use super::super::{climate_config::ClimateConfig, climate_data::SeasonalSnapshot};
use crate::{domains::tectonics::tectonics_data::GeologyLayer, world::topology::WorldTopology};

pub(super) fn compute_season_pressure_and_winds(
    sun_lat: f32,
    topology: &WorldTopology,
    geology: &GeologyLayer,
    continentality: &[f32],
    config: &ClimateConfig,
    snap: &mut SeasonalSnapshot,
) {
    let num_cells = topology.cells.len();
    let avg_hex_rad = (4.0 * std::f32::consts::PI / num_cells as f32).sqrt();

    // 1. Считаем среднюю температуру ОКЕАНА как эталон для каждой широты.
    // Именно разница между сушей и водой создает муссоны и барические центры!
    let mut thermal_field = vec![0.0_f32; num_cells];
    for i in 0..num_cells {
        let lat = topology.cells[i].lat_lon.x;
        let c = continentality[i]; // -1.0 (океан) .. +1.0 (суша)
        let elev = geology.elevations[i];

        let mountain_height = elevation_cooling(
            elev,
            config.mountain_base_level,
            config.mountain_blend_width,
        );
        let sea_level_temp = snap.temperatures[i] + mountain_height * config.altitude_lapse_rate;

        // Эталонная температура открытого океана на данной широте (с учетом его термоинерции)
        let annual_lat_insolation = lat.cos().max(0.0);
        let ocean_lat_baseline = config.pole_temp
            + (config.equator_temp - config.pole_temp) * annual_lat_insolation * 0.85;
        let global_avg = (config.equator_temp + config.pole_temp) / 2.0;
        let ocean_ref_temp = ocean_lat_baseline * 0.65 + global_avg * 0.35;

        // Насколько данная клетка теплее или холоднее океана на этой же широте:
        // Даже весной в тропиках суша теплее океана (низкое давление), а в Сибири холоднее (высокое)!
        let temp_diff = (sea_level_temp - ocean_ref_temp) / 12.0;

        // Над сушей (c > 0) термический эффект работает в полную силу,
        // над океаном (c < 0) реагирует только на теплые/холодные ветровые течения и лед (слабее)
        let weight = if c >= 0.0 {
            0.4 + 0.6 * c
        } else {
            0.4 * (1.0 + c * 0.6)
        };
        thermal_field[i] = (-temp_diff * weight).clamp(-1.2, 1.2);
    }

    // 2. Сглаживаем термическое поле (20 итераций), чтобы получить широкие мягкие вихри
    let mut prev_thermal = vec![0.0_f32; num_cells];

    for _ in 0..20 {
        std::mem::swap(&mut thermal_field, &mut prev_thermal);
        for i in 0..num_cells {
            let neighbors = &topology.cells[i].neighbors;
            if neighbors.is_empty() {
                continue;
            }
            let sum: f32 = neighbors.iter().map(|&n| prev_thermal[n]).sum();
            thermal_field[i] = prev_thermal[i] * 0.3 + (sum / neighbors.len() as f32) * 0.7;
        }
    }

    // 3. Собираем итоговое давление (Широтные пояса + Океанические ядра + Термические центры)
    for i in 0..num_cells {
        let lat = topology.cells[i].lat_lon.x;
        let lon = topology.cells[i].lat_lon.y;
        let c = continentality[i];
        let belt_lat = lat - sun_lat * 0.55;

        let polar_damping = 0.5 + 0.5 * lat.cos();
        let base_belt = -(6.0 * belt_lat).cos() * polar_damping;

        // В океане (c < 0) фокусируем пояса в замкнутые круглые ядра вдали от берегов
        // и добавляем планетарную волну Россби (3 волны на круг по долготе),
        // чтобы в огромном океане рождались отдельные циклоны и антициклоны!
        let belt_envelope = if c < 0.0 {
            let ocean_core = smoothstep(-c); // от 0.0 у берега до 1.0 в центре океана
            let rossby_wave =
                1.0 + 0.35 * (3.0 * lon + belt_lat * 2.0).cos() * (lat * 2.0).sin().abs();
            (0.55 + 0.85 * ocean_core) * rossby_wave
        } else {
            // На суше зональные пояса быстро уступают место термическим муссонам
            1.0 - 0.65 * smoothstep(c)
        };

        snap.pressure[i] = base_belt * belt_envelope * config.latitudinal_pressure_weight
            + thermal_field[i] * config.thermal_pressure_weight;
    }

    let mut prev_pressure = vec![0.0_f32; num_cells];

    for _ in 0..8 {
        std::mem::swap(&mut snap.pressure, &mut prev_pressure);
        for i in 0..num_cells {
            let neighbors = &topology.cells[i].neighbors;
            if neighbors.is_empty() {
                continue;
            }
            let sum: f32 = neighbors.iter().map(|&n| prev_pressure[n]).sum();
            snap.pressure[i] = prev_pressure[i] * 0.3 + (sum / neighbors.len() as f32) * 0.7;
        }
    }

    // 4. Векторы ветров: ЕДИНЫЙ ФИЗИЧНЫЙ ГРАДИЕНТ прямо из карты давления!
    for i in 0..num_cells {
        let cell = &topology.cells[i];
        let pos = cell.center;
        let lat = cell.lat_lon.x;
        let my_elev = geology.elevations[i].max(0.0);

        let east = pos.cross(Vec3::Y).normalize_or_zero();
        let north = pos.cross(east).normalize_or_zero();

        let my_p = snap.pressure[i];
        let mut grad_3d = Vec3::ZERO;
        let avg_step_dist = avg_hex_rad;

        for &n_id in &cell.neighbors {
            let n_pos = topology.cells[n_id].center;
            let raw_dir = n_pos - pos;

            let tangent_dir = (raw_dir - pos * raw_dir.dot(pos)).normalize_or_zero();

            // ВАЖНО: умножаем на (avg_hex_rad / dist) вместо деления на микроскопический dist!
            // Это гарантирует, что стрелки НИКОГДА не взорвутся и не улетят в космос!
            let safe_dist = raw_dir.length().max(avg_hex_rad * 0.3);
            let diff = (my_p - snap.pressure[n_id]) / safe_dist;
            grad_3d += tangent_dir * diff;
        }

        if !cell.neighbors.is_empty() {
            let count = cell.neighbors.len() as f32;
            grad_3d = (grad_3d / count) * 0.22;
        }

        // Поворот Кориолиса (вблизи полюсов плавно ограничиваем, чтобы не крутило "бублик")
        let coriolis_angle = -lat.sin() * config.coriolis_strength;
        let coriolis_rot = Quat::from_axis_angle(pos, coriolis_angle);
        let mut wind_3d = coriolis_rot * grad_3d;

        // Орографический барьер (горы тормозят ветер и заставляют его огибать склоны)
        let mut mountain_drag = 0.0;
        let mut upslope_grad = Vec3::ZERO;
        let wind_dir = wind_3d.normalize_or_zero();

        for &n_id in &cell.neighbors {
            let n_pos = topology.cells[n_id].center;
            let raw_dir = n_pos - pos;
            let to_neighbor = (raw_dir - pos * raw_dir.dot(pos)).normalize_or_zero();

            let n_elev = geology.elevations[n_id].max(0.0);
            let elev_diff = n_elev - my_elev;

            // Градиент подъема рельефа в касательной плоскости
            if elev_diff > 0.0 {
                let slope = elev_diff / avg_step_dist;
                upslope_grad += to_neighbor * slope;

                let alignment = wind_dir.dot(to_neighbor);
                if alignment > 0.0 {
                    mountain_drag += slope * alignment;
                }
            }
        }

        // А) Огибание хребта: вычитаем часть компоненты ветра, бьющей прямо в крутой склон
        if upslope_grad.length_squared() > 1e-6 {
            let slope_normal = upslope_grad.normalize();
            let into_wall = wind_3d.dot(slope_normal).max(0.0);
            let deflect_strength = (upslope_grad.length() * 0.35).clamp(0.0, 0.75);
            wind_3d -= slope_normal * (into_wall * deflect_strength);
        }

        // Б) Базовое трение о высокогорье (чтобы на плоских плато ветер не разгонялся как в океане)
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
    snap: &mut SeasonalSnapshot,
) {
    let num_cells = topology.cells.len();

    for i in 0..num_cells {
        // Течения существуют только в воде
        if geology.elevations[i] > 0.0 {
            snap.ocean_currents[i] = Vec3::ZERO;
            continue;
        }

        let cell = &topology.cells[i];
        let pos = cell.center;
        let lat = cell.lat_lon.x;

        // 1. Течение подхватывается ветром
        let mut current_3d = snap.winds_3d[i] * config.ocean_current_speed_multiplier;

        // 2. Спираль Экмана: из-за плотности воды Кориолис отклоняет течения сильнее ветра (до ~45 градусов)
        let ekman_angle = -lat.sin() * 0.75;
        let ekman_rot = Quat::from_axis_angle(pos, ekman_angle);
        current_3d = ekman_rot * current_3d;

        // 3. Граничные течения (Отклонение от берегов)
        let mut coast_normal = Vec3::ZERO;
        for &n_id in &cell.neighbors {
            if geology.elevations[n_id] > 0.0 {
                let raw_dir = pos - topology.cells[n_id].center;
                // Проецируем нормаль берега строго в касательную плоскость сферы
                let tangent_from_coast = (raw_dir - pos * raw_dir.dot(pos)).normalize_or_zero();
                coast_normal += tangent_from_coast;
            }
        }

        let orig_speed = current_3d.length();
        if coast_normal.length_squared() > 1e-6 && orig_speed > 1e-5 {
            coast_normal = coast_normal.normalize();
            let into_coast = current_3d.dot(-coast_normal); // проекция удара в берег

            if into_coast > 0.0 {
                let impact_ratio = (into_coast / orig_speed).clamp(0.0, 1.0);
                // Убираем компоненту, бьющую в сушу
                let mut along_coast = current_3d + coast_normal * into_coast;
                
                // Восстанавливаем скорость потока (вода не исчезает при ударе в берег, а поворачивает вдоль него)
                if along_coast.length_squared() > 1e-8 {
                    along_coast = along_coast.normalize() * orig_speed;
                }
                
                // Плавное ускорение пограничного течения пропорционально углу натекания
                let boost = 1.0 + (config.boundary_current_multiplier - 1.0) * impact_ratio;
                current_3d = along_coast * boost;
            }
        } else {
            // Мягкая поддержка скорости струи в прибрежной зоне (на 2-3 гекса от берега),
            // где continentality близка к 0.0
            let coastal_proximity = (1.0 - continentality[i].abs() * 3.0).clamp(0.0, 1.0);
            current_3d *= 1.0 + (config.boundary_current_multiplier - 1.0) * 0.35 * coastal_proximity;
        }

        snap.ocean_currents[i] = current_3d;
    }

    // 4. Лёгкое сглаживание течений по соседям (4 итерации).
    // Убирает "зубчатость" на границах после Экмана и отскока от берега,
    // делает потоки визуально струйными.
    let mut prev = snap.ocean_currents.clone();
    for _ in 0..4 {
        std::mem::swap(&mut snap.ocean_currents, &mut prev);
        for i in 0..num_cells {
            if geology.elevations[i] > 0.0 {
                snap.ocean_currents[i] = Vec3::ZERO;
                continue;
            }
            let neighbors = &topology.cells[i].neighbors;
            if neighbors.is_empty() {
                snap.ocean_currents[i] = prev[i];
                continue;
            }
            let mut sum = Vec3::ZERO;
            let mut cnt = 0.0_f32;
            for &n in neighbors {
                // если сосед — суша, считаем, что там скорость та же (граничное условие)
                if geology.elevations[n] > 0.0 {
                    sum += prev[i];
                } else {
                    sum += prev[n];
                }
                cnt += 1.0;
            }
            snap.ocean_currents[i] = prev[i] * 0.5 + (sum / cnt) * 0.5;
        }
    }
}