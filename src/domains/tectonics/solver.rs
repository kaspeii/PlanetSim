use super::{
    config::TectonicsConfig,
    data::{GeologyLayer, Plate, PlateType, Volcano},
};
use crate::world::topology::WorldTopology;
use bevy::math::{Quat, Vec3};
use noise::NoiseFn;
use rand::RngExt;
use std::collections::VecDeque;
use std::f32::consts::PI;

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn profile_factor(t: f32, smoothness: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let sm = smoothstep(t);
    t * (1.0 - smoothness) + sm * smoothness
}

/// Генерирует центры тектонических плит и определяет их физические параметры (ось и скорость вращения)
pub fn generate_plates(rng: &mut impl RngExt, config: &TectonicsConfig) -> Vec<Plate> {
    let mut plates = Vec::with_capacity(config.num_plates);

    for _ in 0..config.num_plates {
        // 1. Случайным образом определяем тип плиты с учетом заданного в конфиге шанса материка
        let is_cont = rng.random_bool(config.continental_ratio);
        let plate_type = if is_cont {
            PlateType::Continental
        } else {
            PlateType::Oceanic
        };

        let mut best_dir = Vec3::Y;
        let mut best_dist = -1.0;

        // 2. Равномерно распределяем центры плит по сфере
        // Метод выбирает случайные точки и оставляет ту, что находится дальше всего от уже созданных плит
        for _ in 0..config.plate_dir_iterations {
            let z: f32 = rng.random_range(-1.0..1.0);
            let lon: f32 = rng.random_range(-PI..PI);
            let r = (1.0 - z * z).sqrt();
            let candidate = Vec3::new(r * lon.cos(), z, r * lon.sin());

            let min_dist = plates
                .iter()
                .map(|p: &Plate| {
                    let dot_product = candidate.dot(p.center_dir);
                    let clamped_dot = dot_product.clamp(-1.0, 1.0);
                    clamped_dot.acos()
                })
                .fold(f32::MAX, f32::min);

            if min_dist > best_dist {
                best_dist = min_dist;
                best_dir = candidate;
            }
        }

        // 3. Генерируем полюс Эйлера (ось, вокруг которой будет вращаться плита).
        // Это обеспечивает корректную сферическую геометрию движения (в отличие от линейных векторов).
        let rand_vec = Vec3::new(
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
        )
        .normalize_or_zero();

        // Предохранитель на маловероятный случай генерации нулевого вектора
        let euler_pole = if rand_vec == Vec3::ZERO {
            Vec3::Y
        } else {
            rand_vec
        };

        // 4. Задаем случайную скалярную скорость вращения плиты
        let angular_vel = rng.random_range(config.angular_vel_min..config.angular_vel_max);

        plates.push(Plate {
            center_dir: best_dir,
            plate_type,
            euler_pole,
            angular_vel,
        });
    }
    plates
}

/// Главная функция симуляции: искажает границы плит, находит зоны контакта и рассчитывает рельеф
pub fn simulate_tectonics(
    topology: &WorldTopology,
    geology: &mut GeologyLayer,
    noise: &impl NoiseFn<f64, 3>,
    config: &TectonicsConfig,
) {
    let plates = &geology.plates;
    let volcanoes = &geology.volcanoes;
    let mut ridge_cells = vec![false; topology.cells.len()];

    for (cell_id, cell) in topology.cells.iter().enumerate() {
        let dir = cell.center;

        // 1. Искажение координат (Domain Warping).
        // Применяем 3D-шум к вектору, чтобы разломы между плитами выглядели рваными, а не прямыми линиями.
        let p = dir.as_dvec3() * config.warp_scale;
        let offsets = &config.warp_offsets;

        let warp_x = noise.get([
            p.x + offsets[0][0],
            p.y + offsets[0][1],
            p.z + offsets[0][2],
        ]) as f32;
        let warp_y = noise.get([
            p.x + offsets[1][0],
            p.y + offsets[1][1],
            p.z + offsets[1][2],
        ]) as f32;
        let warp_z = noise.get([
            p.x + offsets[2][0],
            p.y + offsets[2][1],
            p.z + offsets[2][2],
        ]) as f32;

        let warp_noise = Vec3::new(warp_x, warp_y, warp_z) * config.warp_strength;
        let warped_dir = (dir + warp_noise).normalize();

        // 2. Находим 3 ближайшие плиты для текущей ячейки (триада).
        // Это необходимо, чтобы избежать багов с резкими обрывами в точках схождения трех плит (Тройных стыках).
        let (p1_idx, p2_idx, p3_idx, dist_1, dist_2, dist_3) =
            find_closest_plates(warped_dir, plates);
        geology.plate_ids[cell_id] = p1_idx; // Ячейка принадлежит самой близкой (первой) плите

        let p1 = &plates[p1_idx];
        let p2 = &plates[p2_idx];
        let p3 = &plates[p3_idx];

        // Дистанция от ячейки до границ с плитой 2 и плитой 3
        let bd2 = dist_2 - dist_1;
        let bd3 = dist_3 - dist_1;

        // 3. Вычисляем веса методом обратного расстояния (IDW).
        // Чем ближе мы ко второй или третьей плите, тем сильнее их влияние на итоговую высоту в этой точке.
        let eps = config.weight_epsilon;
        let w2 = 1.0 / (bd2.powi(2) + eps);
        let w3 = 1.0 / (bd3.powi(2) + eps);
        let w_sum = w2 + w3;

        // Нормализуем веса, чтобы в сумме они давали 1.0
        let weight2 = w2 / w_sum;
        let weight3 = w3 / w_sum;

        // 4. Базовая высота (пляжи, шельфы) для двух потенциальных границ (1-2 и 1-3)
        let mut h2 = calculate_base_elevation(p1, p2, bd2, config);
        let mut h3 = calculate_base_elevation(p1, p3, bd3, config);

        // Расчет линейной скорости коры в этой точке для 1-й плиты (векторное произведение оси вращения на позицию)
        let v1 = p1.euler_pole.cross(dir) * p1.angular_vel;

        // 5. Генерируем макро-шум, из которого будут лепиться горы и неровности
        let m_scale = config.boundary_noise_scale;
        let m_coords = [
            warped_dir.x as f64 * m_scale,
            warped_dir.y as f64 * m_scale,
            warped_dir.z as f64 * m_scale,
        ];
        let raw_m_noise = noise.get(m_coords) as f32;
        let m_noise = config.boundary_noise_base + raw_m_noise * config.boundary_noise_amp;

        // 6. Граница с плитой 2
        if bd2 < config.edge_threshold {
            let f2 = (1.0 - bd2 / config.edge_threshold).clamp(0.0, 1.0);
            let normal2 = (p2.center_dir - p1.center_dir).normalize_or_zero();
            let v2 = p2.euler_pole.cross(dir) * p2.angular_vel;
            let convergence2 = (v1 - v2).dot(normal2) / config.convergence_ref_speed;

            if p1.plate_type == PlateType::Oceanic
                && p2.plate_type == PlateType::Oceanic
                && convergence2 < 0.0
            {
                ridge_cells[cell_id] = true;
            }

            h2 += calculate_boundary_elevation(
                p1.plate_type,
                p2.plate_type,
                f2,
                convergence2,
                m_noise,
                config,
            );
        }

        // 7. Граница с плитой 3 — аналогично
        if bd3 < config.edge_threshold {
            let f3 = (1.0 - bd3 / config.edge_threshold).clamp(0.0, 1.0);
            let normal3 = (p3.center_dir - p1.center_dir).normalize_or_zero();
            let v3 = p3.euler_pole.cross(dir) * p3.angular_vel;
            let convergence3 = (v1 - v3).dot(normal3) / config.convergence_ref_speed;

            if p1.plate_type == PlateType::Oceanic
                && p3.plate_type == PlateType::Oceanic
                && convergence3 < 0.0
            {
                ridge_cells[cell_id] = true;
            }

            h3 += calculate_boundary_elevation(
                p1.plate_type,
                p3.plate_type,
                f3,
                convergence3,
                m_noise,
                config,
            );
        }

        // 8. Смешиваем высоты от двух границ для получения гладкого тройного стыка
        let h1 = h2 * weight2 + h3 * weight3;

        // 9. Наложение горячих точек (мантийных плюмов)
        let mut max_volcano_h: f32 = 0.0;
        let mut is_volcano = false;

        let v_scale = config.volcano_noise_scale;
        let v_coords = [
            warped_dir.x as f64 * v_scale,
            warped_dir.y as f64 * v_scale,
            warped_dir.z as f64 * v_scale,
        ];
        let raw_v_noise = noise.get(v_coords) as f32;
        let v_noise_mod = config.volcano_noise_base + config.volcano_noise_mult * raw_v_noise;

        for v in volcanoes {
            let dot_prod = warped_dir.dot(v.center).clamp(-1.0, 1.0);
            let d = dot_prod.acos();

            // Если ячейка находится внутри радиуса вулкана
            if d < v.radius {
                let f = 1.0 - (d / v.radius);
                // Профиль (powi) задает форму конуса (вогнутые склоны)
                let profile = f.powi(config.volcano_profile_pow);
                let local_h = v.height * profile * v_noise_mod;
                max_volcano_h = max_volcano_h.max(local_h);

                if d < v.radius * 0.70 {
                    is_volcano = true;
                }
            }
        }

        geology.volcano_mask[cell_id] = is_volcano;

        // Вулканы "пробивают" базовый рельеф материка/океана
        let h = h1.max(h1 + max_volcano_h);

        // 10. Финальный проход: накладываем мелкую текстуру (детализацию) на всю планету
        let d_scale = config.detail_noise_scale;
        let detail_coords = [
            warped_dir.x as f64 * d_scale,
            warped_dir.y as f64 * d_scale,
            warped_dir.z as f64 * d_scale,
        ];
        let detail_noise = noise.get(detail_coords) as f32;

        geology.elevations[cell_id] =
            (h + detail_noise * config.detail_noise_amp).max(config.elevation_min_clamp);
    }

    apply_oceanic_crust_cooling(topology, &mut geology.elevations, &ridge_cells, config);

    // Запускаем клеточный автомат для размытия острых ступенек пикселей
    step_elevation_diffusion(
        topology,
        &mut geology.elevations,
        config.diffusion_strength,
        config.slope_erosion_factor,
        config.diffusion_max_strength,
    );
}

/// Находит ровно 3 ближайшие плиты к данной точке. Это необходимо для правильной интерполяции
/// тройных узлов, где 4-е плиты нестабильны и не встречаются в природе.
fn find_closest_plates(dir: Vec3, plates: &[Plate]) -> (usize, usize, usize, f32, f32, f32) {
    let mut dist_1 = f32::MAX;
    let mut dist_2 = f32::MAX;
    let mut dist_3 = f32::MAX;

    let mut p1_idx = 0;
    let mut p2_idx = 0;
    let mut p3_idx = 0;

    for (i, plate) in plates.iter().enumerate() {
        let dot_prod = dir.dot(plate.center_dir).clamp(-1.0, 1.0);
        let d = dot_prod.acos();

        if d < dist_1 {
            dist_3 = dist_2;
            p3_idx = p2_idx;
            dist_2 = dist_1;
            p2_idx = p1_idx;
            dist_1 = d;
            p1_idx = i;
        } else if d < dist_2 {
            dist_3 = dist_2;
            p3_idx = p2_idx;
            dist_2 = d;
            p2_idx = i;
        } else if d < dist_3 {
            dist_3 = d;
            p3_idx = i;
        }
    }
    (p1_idx, p2_idx, p3_idx, dist_1, dist_2, dist_3)
}

/// Вычисляет плавный переход высот между равнинами и океанами (шельфы, побережья)
fn calculate_base_elevation(
    p1: &Plate,
    p2: &Plate,
    boundary_dist: f32,
    config: &TectonicsConfig,
) -> f32 {
    let h_base = match p1.plate_type {
        PlateType::Continental => config.cont_base_height,
        PlateType::Oceanic => config.ocean_base_height,
    };
    let sm = config.base_profile_smoothness;

    match (p1.plate_type, p2.plate_type) {
        (PlateType::Continental, PlateType::Oceanic) => {
            let factor = profile_factor(boundary_dist / config.base_co_dist, sm);
            let offset = (config.base_co_offset + config.base_oc_offset) * 0.5;
            h_base * factor - (1.0 - factor) * offset
        }
        (PlateType::Oceanic, PlateType::Continental) => {
            let factor = profile_factor(boundary_dist / config.base_oc_dist, sm);
            let offset = (config.base_co_offset + config.base_oc_offset) * 0.5;
            h_base * factor - (1.0 - factor) * offset
        }
        (PlateType::Continental, PlateType::Continental) => {
            let factor = profile_factor(boundary_dist / config.base_cc_dist, sm);
            h_base * factor + (1.0 - factor) * config.base_cc_offset
        }
        (PlateType::Oceanic, PlateType::Oceanic) => {
            let factor = profile_factor(boundary_dist / config.base_oo_dist, sm);
            h_base * factor + (1.0 - factor) * config.base_oo_offset
        }
    }
}

/// Выполняет расчет рельефа, возникающего непосредственно от тектонического столкновения или растяжения
fn calculate_boundary_elevation(
    t1: PlateType,
    t2: PlateType,
    f: f32,
    convergence: f32,
    mountain_noise: f32,
    config: &TectonicsConfig,
) -> f32 {
    // Экспонента усиливает влияние экстремально быстрых столкновений
    // и плавно гасит слабые трения плит
    let conv_factor = convergence.clamp(0.0, 1.0).exp();

    match (t1, t2) {
        // --- 1. СЖАТИЕ (Конвергенция) ---
        // Коллизия материков (Например: Гималаи, Тибет) - высокие складчатые горы и плато
        (PlateType::Continental, PlateType::Continental) if convergence > 0.0 => {
            let angle = f * PI * config.cc_col_freq;
            let shape = angle.sin().powi(2);
            let blend = config.cc_col_base + config.cc_col_f_mult * f;

            shape * blend * config.cc_col_amp * mountain_noise * conv_factor
        }

        // Субдукция: сторона Континента (Например: Анды) - сдвиг гор вглубь суши от линии разлома
        (PlateType::Continental, PlateType::Oceanic) if convergence > 0.0 => {
            let offset_zone = f - config.co_col_offset;
            let m_zone = (offset_zone * config.co_col_scale).clamp(0.0, 1.0);

            (m_zone * PI).sin().powi(2) * config.co_col_amp * mountain_noise * conv_factor
        }

        // Субдукция: сторона Океана - глубоководный желоб, в который прогибается заныривающая плита
        (PlateType::Oceanic, PlateType::Continental) if convergence > 0.0 => {
            let offset_dist = (f - config.oc_col_center).abs();
            let profile = (1.0 - offset_dist * config.oc_col_scale).clamp(0.0, 1.0);

            -profile * config.oc_col_amp * conv_factor
        }

        // Субдукция двух океанов - вулканические островные дуги
        (PlateType::Oceanic, PlateType::Oceanic) if convergence > 0.0 => {
            (f * PI).sin() * config.oo_col_amp * mountain_noise * conv_factor
        }

        // --- 2. РАСТЯЖЕНИЕ (Дивергенция / Спрединг) ---
        // Рифтогенез континентов (Например: Восточная Африка) - глубокая долина в центре и поднятия по бокам
        (PlateType::Continental, PlateType::Continental) if convergence < 0.0 => {
            let graben = -f.powi(config.cc_rift_graben_pow) * config.cc_rift_graben_amp;
            let horsts = (f * PI).sin() * config.cc_rift_horst_amp;

            (horsts + graben) * config.cc_rift_total_amp * mountain_noise * conv_factor
        }

        // Переходная зона (разлом континента и океана) - плавный спуск к воде
        (PlateType::Continental, PlateType::Oceanic) if convergence < 0.0 => {
            let offset_zone = f - config.co_rift_offset;
            let m_zone = (offset_zone * config.co_rift_scale).clamp(0.0, 1.0);

            -(m_zone * PI).sin().powi(2) * config.co_rift_amp * mountain_noise * conv_factor
        }

        // Спрединг океанов (Срединно-океанический хребет) - осевая щель и подводные валы
        (PlateType::Oceanic, PlateType::Oceanic) if convergence < 0.0 => {
            let axial_rift = -f.powi(config.oo_rift_axial_pow) * config.oo_rift_axial_amp;
            let ridges = (f * PI).sin() * config.oo_rift_ridge_amp;
            let noise_mod = config.oo_rift_noise_base + config.oo_rift_noise_mult * mountain_noise;

            (ridges + axial_rift) * config.oo_rift_total_amp * noise_mod * conv_factor
        }

        // Трансформный разлом (скольжение) - не образует выраженного рельефа
        _ => 0.0,
    }
}

/// Клеточный автомат для диффузии (размытия) высот, избавляет от пиксельных ступенек на сетке
pub fn step_elevation_diffusion(
    topology: &WorldTopology,
    elevations: &mut [f32],
    strength: f32,
    slope_erosion_factor: f32,
    max_strength: f32,
) {
    let old_elevations = elevations.to_vec();

    for (i, cell) in topology.cells.iter().enumerate() {
        if cell.neighbors.is_empty() {
            continue;
        }
        let neighbor_sum: f32 = cell
            .neighbors
            .iter()
            .map(|&n_id| old_elevations[n_id])
            .sum();
        let avg_neighbor = neighbor_sum / cell.neighbors.len() as f32;

        let mut max_slope = 0.0_f32;
        for &n_id in &cell.neighbors {
            let diff = (old_elevations[i] - old_elevations[n_id]).abs();
            if diff > max_slope {
                max_slope = diff;
            }
        }

        let local_strength = (strength + max_slope * slope_erosion_factor).min(max_strength);

        elevations[i] = old_elevations[i] * (1.0 - local_strength) + avg_neighbor * local_strength;
    }
}

fn apply_oceanic_crust_cooling(
    topology: &WorldTopology,
    elevations: &mut [f32],
    ridge_cells: &[bool],
    config: &TectonicsConfig,
) {
    let num_cells = topology.cells.len();
    let mut dist_from_ridge = vec![u32::MAX; num_cells];
    let mut queue = VecDeque::new();

    for i in 0..num_cells {
        if ridge_cells[i] {
            dist_from_ridge[i] = 0;
            queue.push_back(i);
        }
    }

    while let Some(cell_id) = queue.pop_front() {
        let current_dist = dist_from_ridge[cell_id];
        for &neighbor_id in &topology.cells[cell_id].neighbors {
            if elevations[neighbor_id] <= 0.0 && dist_from_ridge[neighbor_id] > current_dist + 1 {
                dist_from_ridge[neighbor_id] = current_dist + 1;
                queue.push_back(neighbor_id);
            }
        }
    }

    let max_dist = config.oceanic_age_max_distance.max(1.0);
    for i in 0..num_cells {
        if elevations[i] > 0.0 || dist_from_ridge[i] == u32::MAX {
            continue;
        }
        let age_factor = smoothstep((dist_from_ridge[i] as f32) / max_dist);
        elevations[i] -= age_factor * config.oceanic_age_depth_gain;
    }
}

/// Создает цепочки вулканов, вращая статичные мантийные плюмы (горячие точки) по дуге
/// вдоль направления движения плиты, создавая эффект "Гавайских островов" или супервулканов.
pub fn generate_hotspots(
    rng: &mut impl RngExt,
    plates: &[Plate],
    config: &TectonicsConfig,
) -> Vec<Volcano> {
    let mut volcanoes = Vec::new();

    for _ in 0..config.hotspot_count {
        // Выбираем статичную точку в мантии планеты
        let plume_pos = Vec3::new(
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
        )
        .normalize_or_zero();

        if plume_pos == Vec3::ZERO {
            continue;
        }

        // Находим, какая из тектонических плит сейчас проплывает над этим плюмом
        let mut best_dist = f32::MAX;
        let mut plate_idx = 0;

        for (i, plate) in plates.iter().enumerate() {
            let dot_prod = plume_pos.dot(plate.center_dir).clamp(-1.0, 1.0);
            let d = dot_prod.acos();
            if d < best_dist {
                best_dist = d;
                plate_idx = i;
            }
        }

        let plate = &plates[plate_idx];
        let chain_length = rng.random_range(config.hotspot_chain_min..config.hotspot_chain_max);

        // Фиксированный шаг по времени (гарантирует, что на быстрых плитах цепочка островов
        // будет вытянута, а на медленных они сольются в один крупный массив).
        let time_step = config.hotspot_time_step;

        for step in 0..chain_length {
            // Вращаем точку плюма назад по вектору движения плиты
            let angle = plate.angular_vel * (step as f32) * time_step;
            let rotation = Quat::from_axis_angle(plate.euler_pole, angle);
            let volcano_pos = rotation * plume_pos;

            // Расчет эрозии: старые вулканы (большой step) становятся ниже и расползаются вширь
            let age_factor = 1.0 - (step as f32 / chain_length as f32);
            let height_base =
                rng.random_range(config.hotspot_height_min..config.hotspot_height_max);

            let height = height_base * age_factor.powf(config.hotspot_erosion_pow);
            let radius =
                config.hotspot_radius_base + config.hotspot_radius_growth * (1.0 - age_factor);

            volcanoes.push(Volcano {
                center: volcano_pos,
                height,
                radius,
            });
        }
    }
    volcanoes
}
