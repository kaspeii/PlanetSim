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

/// Высотное охлаждение с мягким порогом.
/// До base_level — 0, после (base_level + blend_width) — линейно.
fn elevation_cooling(elev: f32, base_level: f32, blend_width: f32) -> f32 {
    let x = (elev - base_level).max(0.0);
    let k = blend_width.max(1e-6);
    if x < k {
        x * x / (2.0 * k)
    } else {
        x - k * 0.5
    }
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
    let albedo_modifier = if geology.elevations[i] <= 0.0 && albedo < config.land_albedo {
        1.0 // Вся дополнительная поглощенная водой энергия уходит на испарение (будущую влажность), а не в кипяток!
    } else {
        (1.0 - albedo) / (1.0 - config.land_albedo)
    };
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
        let deep_water_factor = smoothstep((elev - (-0.06)) / (-0.22 - (-0.06)));
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
    let t = (temp - (config.freezing_temp - 4.0)) / 6.0;
    let ice_blend = smoothstep(t);
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
        let weight = if c >= 0.0 { 0.4 + 0.6 * c } else { 0.4 * (1.0 + c * 0.6) };
        thermal_field[i] = (-temp_diff * weight).clamp(-1.2, 1.2);
    }

    // 2. Сглаживаем термическое поле (20 итераций), чтобы получить широкие мягкие вихри
    let mut prev_thermal = vec![0.0_f32; num_cells];

    for _ in 0..20 {
        std::mem::swap(&mut thermal_field, &mut prev_thermal);
        for i in 0..num_cells {
            let neighbors = &topology.cells[i].neighbors;
            if neighbors.is_empty() { continue; }
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
            let rossby_wave = 1.0 + 0.35 * (3.0 * lon + belt_lat * 2.0).cos() * lat.sin().abs();
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
            if neighbors.is_empty() { continue; }
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

        let east = Vec3::Y.cross(pos).normalize_or_zero();
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
        let mut mutupslope_grad = Vec3::ZERO;
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
                mutupslope_grad += to_neighbor * slope;

                let alignment = wind_dir.dot(to_neighbor);
                if alignment > 0.0 {
                    mountain_drag += slope * alignment;
                }
            }
        }

        // А) Огибание хребта: вычитаем часть компоненты ветра, бьющей прямо в крутой склон
        if mutupslope_grad.length_squared() > 1e-6 {
            let slope_normal = mutupslope_grad.normalize();
            let into_wall = wind_3d.dot(slope_normal).max(0.0);
            let deflect_strength = (mutupslope_grad.length() * 0.35).clamp(0.0, 0.75);
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

fn compute_continentality(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    climate: &mut ClimateLayer,
    config: &ClimateConfig,
) {
    let num_cells = topology.cells.len();
    let min_ocean_cells = (num_cells as f32 * config.min_ocean_size_ratio) as usize;
    // Порог для суши: игнорируем мелкие острова (всё, что меньше 0.5% площади планеты)
    let min_land_cells = (num_cells as f32 * (config.min_ocean_size_ratio * 0.5)) as usize;

    let mut is_ocean = vec![false; num_cells];
    let mut is_major_land = vec![false; num_cells];
    let mut visited = vec![false; num_cells];

    // 1. Flood Fill: находим настоящие океаны (отсеиваем мелкие озера)
    // и настоящие континенты (отсеиваем мелкие океанические острова)
    for i in 0..num_cells {
        if visited[i] {
            continue;
        }

        let is_water = geology.elevations[i] <= 0.0;
        let mut current_body = Vec::new();
        let mut stack = vec![i];
        visited[i] = true;

        while let Some(cell_id) = stack.pop() {
            current_body.push(cell_id);
            for &neighbor_id in &topology.cells[cell_id].neighbors {
                let neighbor_is_water = geology.elevations[neighbor_id] <= 0.0;
                if neighbor_is_water == is_water && !visited[neighbor_id] {
                    visited[neighbor_id] = true;
                    stack.push(neighbor_id);
                }
            }
        }

        if is_water {
            if current_body.len() >= min_ocean_cells {
                for &cell_id in &current_body {
                    is_ocean[cell_id] = true;
                }
            }
        } else if current_body.len() >= min_land_cells {
            for &cell_id in &current_body {
                is_major_land[cell_id] = true;
            }
        }
    }

    // BFS 1: расстояние до океана (для всей суши и внутренних озер)
    let mut dist_to_ocean = vec![u32::MAX; num_cells];
    let mut queue = VecDeque::new();
    for i in 0..num_cells {
        if is_ocean[i] {
            dist_to_ocean[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(cell_id) = queue.pop_front() {
        let current_dist = dist_to_ocean[cell_id];
        for &neighbor_id in &topology.cells[cell_id].neighbors {
            if dist_to_ocean[neighbor_id] > current_dist + 1 {
                dist_to_ocean[neighbor_id] = current_dist + 1;
                queue.push_back(neighbor_id);
            }
        }
    }

    // BFS 2: расстояние до КРУПНОЙ суши (для океана, игнорируя мелкие острова!)
    let mut dist_to_land = vec![u32::MAX; num_cells];
    queue.clear();
    for i in 0..num_cells {
        if is_major_land[i] {
            dist_to_land[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(cell_id) = queue.pop_front() {
        let current_dist = dist_to_land[cell_id];
        for &neighbor_id in &topology.cells[cell_id].neighbors {
            if dist_to_land[neighbor_id] > current_dist + 1 {
                dist_to_land[neighbor_id] = current_dist + 1;
                queue.push_back(neighbor_id);
            }
        }
    }

    let avg_hex_rad = (4.0 * std::f32::consts::PI / num_cells as f32).sqrt();

    // Знаковая континентальность: -1 = открытый океан, 0 = берег, +1 = глубь континента
    for i in 0..num_cells {
        climate.continentality[i] = if is_ocean[i] {
            let physical_dist = dist_to_land[i] as f32 * avg_hex_rad;
            let norm = (physical_dist / config.ocean_continentality_scale).min(1.0);
            -smoothstep(norm) // -1 далеко от материков, 0 у берега материка
        } else {
            let physical_dist = dist_to_ocean[i] as f32 * avg_hex_rad;
            let norm = (physical_dist / config.land_continentality_scale).min(1.0);
            smoothstep(norm)  // 0 на островах и пляжах, +1 в глубине материка
        };
    }
}