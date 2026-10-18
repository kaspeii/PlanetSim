use super::{config::ClimateConfig, data::ClimateLayer};
use crate::{domains::tectonics::data::GeologyLayer, world::topology::WorldTopology};
use bevy::prelude::*;
use std::collections::VecDeque;

pub fn simulate_temperature(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    climate: &mut ClimateLayer,
    config: &ClimateConfig,
) {
    let num_cells = topology.cells.len();

    // 1. ПРОХОД: Поиск настоящих океанов (Flood Fill для подсчета площади водоемов)
    let min_ocean_cells = (num_cells as f32 * config.min_ocean_size_ratio) as usize;
    let mut is_ocean = vec![false; num_cells];
    let mut visited = vec![false; num_cells];

    for i in 0..num_cells {
        if geology.elevations[i] <= 0.0 && !visited[i] {
            let mut current_body = Vec::new();
            let mut stack = vec![i];
            visited[i] = true;

            // Заливка для определения границ конкретного водоема
            while let Some(cell_id) = stack.pop() {
                current_body.push(cell_id);
                for &neighbor_id in &topology.cells[cell_id].neighbors {
                    if geology.elevations[neighbor_id] <= 0.0 && !visited[neighbor_id] {
                        visited[neighbor_id] = true;
                        stack.push(neighbor_id);
                    }
                }
            }

            // Если водоем достаточно большой, помечаем все его гексы как океанические
            if current_body.len() >= min_ocean_cells {
                for &cell_id in &current_body {
                    is_ocean[cell_id] = true;
                }
            }
        }
    }

    // 2. ПРОХОД: Поиск в ширину (BFS) для расчета расстояния только от океанов
    let mut queue = VecDeque::new();
    for (i, &ocean) in is_ocean.iter().enumerate().take(num_cells) {
        if ocean {
            climate.distance_to_ocean[i] = 0;
            queue.push_back(i);
        } else {
            climate.distance_to_ocean[i] = u32::MAX;
        }
    }

    while let Some(cell_id) = queue.pop_front() {
        let current_dist = climate.distance_to_ocean[cell_id];
        for &neighbor_id in &topology.cells[cell_id].neighbors {
            if climate.distance_to_ocean[neighbor_id] > current_dist + 1 {
                climate.distance_to_ocean[neighbor_id] = current_dist + 1;
                queue.push_back(neighbor_id);
            }
        }
    }

    // 3. ПРОХОД: Основной расчет температуры и альбедо
    for i in 0..num_cells {
        let cell = &topology.cells[i];
        let elevation = geology.elevations[i];
        let dist = climate.distance_to_ocean[i];

        // Широта и базовая инсоляция (от 0.0 на полюсах до 1.0 на экваторе)
        let lat = cell.lat_lon.x;
        let base_insolation = lat.cos().max(0.0);

        // Оценка температуры только от широты и высоты (чтобы понять, есть ли условия для льда)
        let mut temp_estimate =
            config.pole_temp + (config.equator_temp - config.pole_temp) * base_insolation;
        if elevation > 0.0 {
            temp_estimate -= elevation * config.altitude_lapse_rate;
        }

        // Вычисляем альбедо текущего гекса
        let albedo = if temp_estimate < config.freezing_temp {
            config.ice_albedo // Ледяная шапка
        } else if elevation <= 0.0 {
            config.ocean_albedo // Океан
        } else {
            config.land_albedo // Обычная суша
        };
        climate.albedo[i] = albedo;

        // Нормализуем инсоляцию относительно базовой суши.
        // Суша (0.3) поглощает 70%. Океан (0.06) поглощает 94% (нагрев сильнее). Лед (0.8) поглощает 20%.
        // Это нужно, чтобы equator_temp из конфига все еще выдавал ожидаемые 32 градуса на суше.
        let albedo_modifier = (1.0 - albedo) / (1.0 - config.land_albedo);
        let effective_insolation = base_insolation * albedo_modifier;

        // Считаем реальную температуру с учетом поглощения тепла
        let mut temp =
            config.pole_temp + (config.equator_temp - config.pole_temp) * effective_insolation;

        // Влияние высоты (остывание в горах)
        if elevation > 0.0 {
            temp -= elevation * config.altitude_lapse_rate;
        }

        // Термоинерция океанов (Континентальность)
        // На побережье (dist=0) -> множитель 1.0, в глубине Сибири -> стремится к 0.0
        let moderation_factor = 1.0 / (1.0 + dist as f32 * config.continentality_factor);
        let moderation = config.base_ocean_moderation * moderation_factor;

        // Чем ближе к морю, тем сильнее температура стягивается к глобальной средней
        let global_avg = (config.equator_temp + config.pole_temp) / 2.0;
        temp = temp * (1.0 - moderation) + global_avg * moderation;

        climate.temperatures[i] = temp;
    }
}

pub fn simulate_pressure_and_winds(
    topology: &WorldTopology,
    geology: &GeologyLayer,
    climate: &mut ClimateLayer,
    config: &ClimateConfig,
) {
    let num_cells = topology.cells.len();

    // 1. Считаем континентальную термическую аномалию:
    // Вместо того чтобы считать океан "аномазией", мы генерируем термическое давление
    // ТОЛЬКО над сушей (прогретые или вымороженные континенты), а над океаном ставим 0.0 (база).
    let mut thermal_field = vec![0.0_f32; num_cells];
    for i in 0..num_cells {
        // Если это океан (dist == 0), он является базовой нормой и не создает континентальных аномалий
        if climate.distance_to_ocean[i] == 0 {
            thermal_field[i] = 0.0;
            continue;
        }

        let lat = topology.cells[i].lat_lon.x;
        let elev = geology.elevations[i].max(0.0);

        // Приводим температуру суши к уровню моря
        let sea_level_temp = climate.temperatures[i] + elev * config.altitude_lapse_rate;

        // Сравниваем температуру в глубине материка с мягкой температурой побережья на той же широте
        let base_insolation = lat.cos().max(0.0);
        let raw_lat_temp =
            config.pole_temp + (config.equator_temp - config.pole_temp) * base_insolation;
        let global_avg = (config.equator_temp + config.pole_temp) / 2.0;
        let coastal_temp = raw_lat_temp * (1.0 - config.base_ocean_moderation)
            + global_avg * config.base_ocean_moderation;

        // В глубине жаркого материка sea_level_temp > coastal_temp -> давление падает (термическая депрессия).
        // В глубине холодного материка (Сибирь) sea_level_temp < coastal_temp -> давление растет (антициклон).
        let temp_anomaly = (sea_level_temp - coastal_temp) / 15.0;
        thermal_field[i] = -temp_anomaly.clamp(-1.5, 1.5);
    }

    // 2. Сильное размытие термического поля (15 итераций).
    // Это превращает резкие береговые линии ("бризы") в огромные плавные континентальные циклоны и антициклоны!
    for _ in 0..15 {
        let prev = thermal_field.clone();
        for i in 0..num_cells {
            let neighbors = &topology.cells[i].neighbors;
            let sum: f32 = neighbors.iter().map(|&n| prev[n]).sum();
            thermal_field[i] = prev[i] * 0.3 + (sum / neighbors.len() as f32) * 0.7;
        }
    }

    // 3. Собираем итоговое давление (Пояса + размытые материковые центры)
    for i in 0..num_cells {
        let lat = topology.cells[i].lat_lon.x;
        let belt_pressure = -(6.0 * lat).cos();

        climate.pressure[i] = belt_pressure * config.latitudinal_pressure_weight
            + thermal_field[i] * config.thermal_pressure_weight;
    }

    // 4. Расчет ветров: Глобальная аналитическая тяга поясов + Градиент от материков
    for i in 0..num_cells {
        let cell = &topology.cells[i];
        let pos = cell.center;
        let lat = cell.lat_lon.x;
        let my_elev = geology.elevations[i].max(0.0);

        // Локальные оси координат на сфере
        let east = Vec3::Y.cross(pos).normalize_or_zero();
        let north = pos.cross(east).normalize_or_zero();

        // А) Аналитический градиент широтных поясов (дает идеально ровные, широкие пояса ветров!)
        // Производная от -(6 * lat).cos() по широте равна 6 * sin(6 * lat).
        // Ветер дует против градиента (от High к Low), поэтому берем со знаком минус:
        let belt_force_north = -(6.0 * lat).sin() * config.latitudinal_pressure_weight * 0.35;
        let belt_grad_3d = north * belt_force_north;

        // Б) Ячеечный градиент от термических аномалий материков
        let my_therm = thermal_field[i];
        let mut therm_grad_3d = Vec3::ZERO;
        let mut avg_step_dist = 0.001_f32;

        for &n_id in &cell.neighbors {
            let n_pos = topology.cells[n_id].center;
            let raw_dir = n_pos - pos;
            let dist = raw_dir.length().max(0.0001);
            avg_step_dist += dist;

            let tangent_dir = (raw_dir - pos * raw_dir.dot(pos)).normalize_or_zero();
            // Делим на dist, чтобы сила градиента не зависела от разрешения сетки!
            let diff = (my_therm - thermal_field[n_id]) / dist;
            therm_grad_3d += tangent_dir * diff;
        }

        if !cell.neighbors.is_empty() {
            let count = cell.neighbors.len() as f32;
            therm_grad_3d = (therm_grad_3d / count) * config.thermal_pressure_weight * 0.08;
            avg_step_dist /= count;
        }

        // Суммируем глобальную тягу поясов и локальную тягу материков
        let total_grad_3d = belt_grad_3d + therm_grad_3d;

        // В) Поворот Кориолиса
        let coriolis_angle = -lat.sin() * config.coriolis_strength;
        let coriolis_rot = Quat::from_axis_angle(pos, coriolis_angle);
        let mut wind_3d = coriolis_rot * total_grad_3d;

        // Г) Орографический барьер (горы тормозят и отклоняют уже сформированный ветер)
        let mut mountain_drag = 0.0;
        for &n_id in &cell.neighbors {
            let n_pos = topology.cells[n_id].center;
            let to_neighbor = (n_pos - pos).normalize_or_zero();
            // Проверяем только тех соседей, КУДА дует ветер
            let alignment = wind_3d.normalize_or_zero().dot(to_neighbor);
            if alignment > 0.0 {
                let n_elev = geology.elevations[n_id].max(0.0);
                let slope = ((n_elev - my_elev).max(0.0)) / avg_step_dist;
                mountain_drag += slope * alignment;
            }
        }

        let barrier_factor = 1.0 / (1.0 + mountain_drag * config.mountain_wind_barrier * 0.15);
        wind_3d = wind_3d * barrier_factor * config.wind_speed_multiplier;

        climate.winds_3d[i] = wind_3d;
        climate.winds[i] = Vec2::new(wind_3d.dot(east), wind_3d.dot(north));
    }
}