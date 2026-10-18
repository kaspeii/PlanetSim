use super::{
    config::ClimateConfig,
    data::{ClimateLayer, SeasonalSnapshot},
};
use crate::{domains::tectonics::data::GeologyLayer, world::topology::WorldTopology};
use bevy::prelude::*;
use std::collections::VecDeque;

fn smoothstep(t: f32) -> f32 {
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

        let mut temp_est =
            config.pole_temp + (config.equator_temp - config.pole_temp) * base_insolation;
        if elev > 0.0 {
            temp_est -= elev * config.altitude_lapse_rate;
        }

        snap.albedo[i] = calc_albedo(temp_est, elev, config);
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
        let mountain_height = (elev - 0.08).max(0.0);
        snap.temperatures[i] = rad_sea_temp - mountain_height * config.altitude_lapse_rate;
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
            snap,
        );

        // Б) Дальнобойный перенос тепла ветрами (25 шагов чистой адвекции без гашения!).
        // Это позволяет ветрам с материков и теплых морей реально пробивать вглубь океана и ломать круг льда!
        let mut advected_temps = sea_level_temps.clone();
        for _ in 0..25 {
            let prev = advected_temps.clone();
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
                        // Возводим в квадрат для направленного струйного переноса без бокового размытия
                        let w = alignment * alignment;
                        upwind_sum += prev[n_id] * w;
                        weight_sum += w;
                    }
                }

                if weight_sum > 0.0 {
                    let incoming = upwind_sum / weight_sum;
                    let step_rate = (wind_speed * 0.06).clamp(0.0, 0.55);
                    advected_temps[i] = prev[i] * (1.0 - step_rate) + incoming * step_rate;
                }
            }
        }

        // В) Радиационный баланс: смешиваем принесенный ветром воздух с локальным нагревом Солнца
        let blend = (config.heat_advection_strength * 0.55).clamp(0.0, 0.85);
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
            sea_level_temps[i] = rad_temp * (1.0 - blend) + advected_temps[i] * blend;

            let elev = geology.elevations[i];
            let mountain_height = (elev - 0.08).max(0.0);
            let final_temp = sea_level_temps[i] - mountain_height * config.altitude_lapse_rate;
            snap.temperatures[i] = final_temp;
            snap.albedo[i] = calc_albedo(final_temp, elev, config);
        }
    }

    // Финальная синхронизация давления и ветров
    compute_season_pressure_and_winds(sun_lat, topology, geology, continentality, config, snap);
}

fn calc_radiative_sea_temp(
    i: usize,
    albedo: f32,
    sun_lat: f32,
    topology: &WorldTopology,
    continentality: &[f32],
    config: &ClimateConfig,
    geology: &GeologyLayer,
) -> f32 {
    let lat = topology.cells[i].lat_lon.x;

    // Инсоляция с учетом смещения зенита Солнца в текущем сезоне
    let base_insolation = (lat - sun_lat).cos().max(0.0);
    let albedo_modifier = (1.0 - albedo) / (1.0 - config.land_albedo);
    let effective_insolation = base_insolation * albedo_modifier;

    let raw_temp =
        config.pole_temp + (config.equator_temp - config.pole_temp) * effective_insolation;

    let c = continentality[i]; // -1..+1
    // Океан (c < 0): moderation выше базовой (открытый океан стабильнее)
    // Суша (c > 0): moderation падает до 0
    let moderation = if c < 0.0 {
        // c ∈ [-1, 0]: открытый океан = base, берег/маргинальное море = base * (1 - reduction)
        let reduction = config.marginal_sea_moderation_reduction;
        config.base_ocean_moderation * (1.0 - reduction * (1.0 - c.abs()))
    } else {
        // c ∈ [0, 1]: берег = base, глубина континента = 0
        config.base_ocean_moderation * (1.0 - c)
    };

    // ВАЖНО: Океан обладает термоинерцией и сопротивляется резким сезонным скачкам,
    // но на самой широте остается теплым на экваторе и холодным у полюсов (годовая норма широты),
    // а не стягивает всю планету к +1°C!
    let annual_lat_insolation = lat.cos().max(0.0);
    let ocean_lat_baseline = config.pole_temp
        + (config.equator_temp - config.pole_temp) * annual_lat_insolation * 0.85;
    let global_avg = (config.equator_temp + config.pole_temp) / 2.0;
    let ocean_equilibrium = ocean_lat_baseline * 0.65 + global_avg * 0.35;

    let mut final_sea_temp = raw_temp * (1.0 - moderation) + ocean_equilibrium * moderation;

    // ЭФФЕКТ БАТИМЕТРИИ (Шельф против Глубокого океана):
    // На мелководном шельфе (h от 0.0 до -0.12) тонкий слой воды быстрее вымерзает в полярных широтах.
    // Над глубоководными впадинами и за бровкой шельфа (h < -0.18) огромная масса воды удерживает тепло,
    // останавливая кромку льда вдоль континентального склона!
    let elev = geology.elevations[i];
    if elev <= 0.0 && lat.abs() > 0.6 {
        // 0.0 на мелком шельфе (-0.04), 1.0 в глубоком океане (-0.30)
        let deep_water_factor = smoothstep((elev - -0.06) / (0.22 - -0.06));
        // Мелкий шельф у полюсов теряет до -3.5°C (обрастает льдом), а глубокая вода получает до +3.5°C защиты
        let bathymetry_shift = (deep_water_factor - 0.35) * 6.0;
        final_sea_temp += bathymetry_shift;
    }

    final_sea_temp
}

fn calc_albedo(temp: f32, elevation: f32, config: &ClimateConfig) -> f32 {
    let base_albedo = if elevation <= 0.0 {
        config.ocean_albedo
    } else {
        config.land_albedo
    };
    let ice_blend = smoothstep((temp - config.freezing_temp - 4.0) / (config.freezing_temp + 2.0 - config.freezing_temp - 4.0));
    config.ice_albedo * (1.0 - ice_blend) + base_albedo * ice_blend
}

fn compute_season_pressure_and_winds(
    sun_lat: f32,
    topology: &WorldTopology,
    geology: &GeologyLayer,
    continentality: &[f32],
    config: &ClimateConfig,
    snap: &mut SeasonalSnapshot,
) {
    let num_cells = topology.cells.len();

    // 1. Термическая аномалия (и над сушей, и над океаном, где ветра нагнали тепло/холод!)
    let mut thermal_field = vec![0.0_f32; num_cells];
    for i in 0..num_cells {
        let lat = topology.cells[i].lat_lon.x;
        let elev = geology.elevations[i].max(0.0);
        let sea_level_temp = snap.temperatures[i] + elev * config.altitude_lapse_rate;

        // Сравниваем температуру клетки со средней нормой на этой широте
        let annual_insolation = lat.cos().max(0.0);
        let expected_lat_temp =
            config.pole_temp + (config.equator_temp - config.pole_temp) * annual_insolation;

        let temp_anomaly = (sea_level_temp - expected_lat_temp) / 14.0;
        // Над сушей термические барические центры сильнее (1.0), над океаном мягче (0.35), но они ЕСТЬ!
        // Это создает прибрежные барические ложбины, которые закручивают ветра у кромки льда!
        let c = continentality[i];
        let surface_mult = 0.35 + 0.65 * smoothstep((c + 0.3) / 1.3);
        thermal_field[i] = (-temp_anomaly * surface_mult).clamp(-1.5, 1.5);
    }

    // 2. Сглаживание термического поля
    for _ in 0..15 {
        let prev = thermal_field.clone();
        for i in 0..num_cells {
            let neighbors = &topology.cells[i].neighbors;
            let sum: f32 = neighbors.iter().map(|&n| prev[n]).sum();
            thermal_field[i] = prev[i] * 0.3 + (sum / neighbors.len() as f32) * 0.7;
        }
    }

    // 3. Итоговое давление
    for i in 0..num_cells {
        let lat = topology.cells[i].lat_lon.x;
        let belt_lat = lat - sun_lat * 0.55;
        let belt_pressure = -(6.0 * belt_lat).cos();

        snap.pressure[i] = belt_pressure * config.latitudinal_pressure_weight
            + thermal_field[i] * config.thermal_pressure_weight;
    }

    // 4. Векторы ветров
    for i in 0..num_cells {
        let cell = &topology.cells[i];
        let pos = cell.center;
        let lat = cell.lat_lon.x;
        let my_elev = geology.elevations[i].max(0.0);

        let east = Vec3::Y.cross(pos).normalize_or_zero();
        let north = pos.cross(east).normalize_or_zero();

        let belt_lat = lat - sun_lat * 0.55;
        let belt_force_north = -(6.0 * belt_lat).sin() * config.latitudinal_pressure_weight * 0.35;
        let belt_grad_3d = north * belt_force_north;

        let my_therm = thermal_field[i];
        let mut therm_grad_3d = Vec3::ZERO;
        let mut avg_step_dist = 0.001_f32;

        for &n_id in &cell.neighbors {
            let n_pos = topology.cells[n_id].center;
            let raw_dir = n_pos - pos;
            let dist = raw_dir.length().max(0.0001);
            avg_step_dist += dist;

            let tangent_dir = (raw_dir - pos * raw_dir.dot(pos)).normalize_or_zero();
            let diff = (my_therm - thermal_field[n_id]) / dist;
            therm_grad_3d += tangent_dir * diff;
        }

        if !cell.neighbors.is_empty() {
            let count = cell.neighbors.len() as f32;
            therm_grad_3d = (therm_grad_3d / count) * config.thermal_pressure_weight * 0.12;
            avg_step_dist /= count;
        }

        let total_grad_3d = belt_grad_3d + therm_grad_3d;

        let coriolis_angle = -lat.sin() * config.coriolis_strength;
        let coriolis_rot = Quat::from_axis_angle(pos, coriolis_angle);
        let mut wind_3d = coriolis_rot * total_grad_3d;

        let mut mountain_drag = 0.0;
        for &n_id in &cell.neighbors {
            let n_pos = topology.cells[n_id].center;
            let to_neighbor = (n_pos - pos).normalize_or_zero();
            let alignment = wind_3d.normalize_or_zero().dot(to_neighbor);
            if alignment > 0.0 {
                let n_elev = geology.elevations[n_id].max(0.0);
                let slope = ((n_elev - my_elev).max(0.0)) / avg_step_dist;
                mountain_drag += slope * alignment;
            }
        }

        let barrier_factor = 1.0 / (1.0 + mountain_drag * config.mountain_wind_barrier * 0.15);
        wind_3d = wind_3d * barrier_factor * config.wind_speed_multiplier;

        snap.winds_3d[i] = wind_3d;
        snap.winds[i] = Vec2::new(wind_3d.dot(east), wind_3d.dot(north));
    }
}

fn compute_continentality(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    climate: &mut ClimateLayer,
    config: &ClimateConfig,
) {
    let num_cells = topology.cells.len();
    let min_ocean_cells = (num_cells as f32 * config.min_ocean_size_ratio) as usize;
    let mut is_ocean = vec![false; num_cells];
    let mut visited = vec![false; num_cells];

    for i in 0..num_cells {
        if geology.elevations[i] <= 0.0 && !visited[i] {
            let mut current_body = Vec::new();
            let mut stack = vec![i];
            visited[i] = true;

            while let Some(cell_id) = stack.pop() {
                current_body.push(cell_id);
                for &neighbor_id in &topology.cells[cell_id].neighbors {
                    if geology.elevations[neighbor_id] <= 0.0 && !visited[neighbor_id] {
                        visited[neighbor_id] = true;
                        stack.push(neighbor_id);
                    }
                }
            }

            if current_body.len() >= min_ocean_cells {
                for &cell_id in &current_body {
                    is_ocean[cell_id] = true;
                }
            }
        }
    }
    // BFS 1: расстояние до океана (для суши)
    let mut dist_to_ocean = vec![u32::MAX; num_cells];
    let mut queue = VecDeque::new();
    for i in 0..num_cells {
        if is_ocean[i] {
            dist_to_ocean[i] = 0;
            queue.push_back(i);
        }
    }
    // ... стандартный BFS ...

    // BFS 2: расстояние до суши (для океана)
    let mut dist_to_land = vec![u32::MAX; num_cells];
    queue.clear();
    for i in 0..num_cells {
        if !is_ocean[i] {
            dist_to_land[i] = 0;
            queue.push_back(i);
        }
    }
    // ... стандартный BFS ...

    // Знаковая континентальность: -1 = открытый океан, 0 = берег, +1 = глубь континента
    for i in 0..num_cells {
        climate.continentality[i] = if is_ocean[i] {
            let norm = (dist_to_land[i] as f32 / config.ocean_continentality_scale).min(1.0);
            -smoothstep(norm)  // -1 далеко от суши, 0 у берега
        } else {
            let norm = (dist_to_ocean[i] as f32 / config.land_continentality_scale).min(1.0);
            smoothstep(norm)   // 0 у берега, +1 в глуби континента
        };
    }
}