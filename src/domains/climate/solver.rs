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
