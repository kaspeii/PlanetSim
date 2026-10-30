// src/domains/tectonics/earth_preset.rs

use bevy::math::{Quat, Vec3};
use noise::{Fbm, NoiseFn, Perlin};
use std::{
    collections::VecDeque,
    f32::consts::{FRAC_PI_2, PI},
    fs,
};

use super::{
    tectonics_config::TectonicsConfig,
    tectonics_data::{GeologyLayer, Plate, PlateType, RockType, Volcano},
    tectonics_solver::step_elevation_diffusion,
};
use crate::world::topology::WorldTopology;

pub const EARTH_ASSET_PATH: &str = "assets/earth/earth_data.bin";

#[inline]
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn profile_factor(t: f32, smoothness: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let sm = smoothstep(t);
    t * (1.0 - smoothness) + sm * smoothness
}

/// Переводит градусы широты и долготы в 3D-вектор правосторонней сферы Bevy (+Y — север)
#[inline]
fn lat_lon_deg_to_vec3(lat_deg: f32, lon_deg: f32) -> Vec3 {
    let lat = lat_deg.to_radians();
    let lon = -lon_deg.to_radians();
    Vec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin()).normalize()
}

/// Кусочно-нелинейная калибровка высот суши Земли (в метрах) под физику planetSim
#[inline]
fn earth_land_meters_to_sim(meters: f32) -> f32 {
    if meters <= 550.0 {
        // Низменности и равнины (0 .. 550 м) -> [0.004 .. 0.095] (ниже mountain_base_level = 0.10)
        let t = (meters / 550.0).clamp(0.0, 1.0);
        0.004 + t.powf(0.85) * 0.091
    } else {
        // Предгорья, нагорья и горные хребты (550 .. 8848 м) -> [0.10 .. 0.68]
        let t = ((meters - 550.0) / (8848.0 - 550.0)).clamp(0.0, 1.0);
        0.10 + t.powf(0.80) * 0.58
    }
}

/// Собирает эталонный GeologyLayer Земли под текущую сетку WorldTopology
pub fn build_earth_geology(
    topology: &WorldTopology,
    config: &TectonicsConfig,
) -> Result<GeologyLayer, String> {
    let bytes = fs::read(EARTH_ASSET_PATH).map_err(|e| {
        format!(
            "Не найден файл {} ({}). Сначала запустите: python .\\scripts\\build_earth_assets.py",
            EARTH_ASSET_PATH, e
        )
    })?;

    if bytes.len() < 12 {
        return Err("Поврежденный заголовок earth_data.bin".to_string());
    }

    let magic = &bytes[0..4];
    let (width, height, header_size) = if magic == b"EAR1" {
        let w = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let h = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        (w, h, 12usize)
    } else if magic == b"EAR2" {
        let w = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let h = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        (w, h, 16usize)
    } else {
        return Err("Неизвестный формат earth_data.bin".to_string());
    };

    let num_pixels = width * height;
    let elev_offset = header_size;
    let plate_offset = elev_offset + num_pixels * 2;
    let expected_len = plate_offset + num_pixels;

    if bytes.len() < expected_len {
        return Err("Файл earth_data.bin неполный. Перезапустите build_earth_assets.py".to_string());
    }

    let elev_slice = &bytes[elev_offset..plate_offset];
    let plate_slice = &bytes[plate_offset..expected_len];

    let read_elev_m = |x: usize, y: usize| -> f32 {
        let idx = (y.min(height - 1) * width + x.min(width - 1)) * 2;
        i16::from_le_bytes([elev_slice[idx], elev_slice[idx + 1]]) as f32
    };

    let num_cells = topology.cells.len();
    let mut geology = GeologyLayer::default();
    geology.resize(num_cells);
    geology.plates = build_real_earth_plates();
    // Генерируем ТОЛЬКО цепочки вулканов от мантийных плюмов (горячих точек)!
    geology.volcanoes = build_plume_hotspot_chains(&geology.plates, config);

    let num_plates = geology.plates.len();
    let mut is_ocean = vec![false; num_cells];

    // 1. Сэмплируем сушу/океан и ID тектонических плит
    for (i, cell) in topology.cells.iter().enumerate() {
        let lat = cell.lat_lon.x;
        let lon = cell.lat_lon.y;

        let u = ((PI - lon) / (2.0 * PI)).rem_euclid(1.0);
        let v = ((FRAC_PI_2 - lat) / PI).clamp(0.0, 0.999999);

        let fx = u * (width as f32 - 1.0);
        let fy = v * (height as f32 - 1.0);

        let x0 = fx.floor() as usize;
        let y0 = fy.floor() as usize;
        let x1 = (x0 + 1) % width;
        let y1 = (y0 + 1).min(height - 1);

        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;

        let h00 = read_elev_m(x0, y0);
        let h10 = read_elev_m(x1, y0);
        let h01 = read_elev_m(x0, y1);
        let h11 = read_elev_m(x1, y1);

        let meters = h00 * (1.0 - tx) * (1.0 - ty)
            + h10 * tx * (1.0 - ty)
            + h01 * (1.0 - tx) * ty
            + h11 * tx * ty;

        if meters <= 0.0 {
            is_ocean[i] = true;
            geology.elevations[i] = config.ocean_base_height;
        } else {
            is_ocean[i] = false;
            geology.elevations[i] = earth_land_meters_to_sim(meters);
        }

        let px = (u * width as f32) as usize;
        let py = (v * height as f32) as usize;
        let p_id = plate_slice[py.min(height - 1) * width + px.min(width - 1)] as usize;
        geology.plate_ids[i] = p_id.min(num_plates - 1);
    }

    // 2. Строим физическую батиметрию океана, цепочки плюмов и литологию прямо на икосфере
    synthesize_ocean_bathymetry_and_lithology(topology, &mut geology, &is_ocean, config);

    Ok(geology)
}

/// 14 главных тектонических плит Земли с полюсами Эйлера
fn build_real_earth_plates() -> Vec<Plate> {
    let specs = [
        // 0: Тихоокеанская
        (-10.0, -150.0, PlateType::Oceanic, -60.0, 107.0, 1.85),
        // 1: Евразийская
        (52.0, 65.0, PlateType::Continental, 50.0, -112.0, 0.45),
        // 2: Северо-Американская
        (45.0, -100.0, PlateType::Continental, -4.0, -75.0, 0.55),
        // 3: Южно-Американская
        (-20.0, -60.0, PlateType::Continental, -25.0, -124.0, 0.50),
        // 4: Африканская
        (5.0, 20.0, PlateType::Continental, 50.0, -74.0, 0.58),
        // 5: Австралийская
        (-25.0, 135.0, PlateType::Continental, 33.0, 37.0, 1.65),
        // 6: Индийская
        (20.0, 78.0, PlateType::Continental, 33.0, 37.0, 1.70),
        // 7: Антарктическая
        (-82.0, 0.0, PlateType::Continental, 60.0, -120.0, 0.38),
        // 8: Наска
        (-20.0, -92.0, PlateType::Oceanic, 55.0, -94.0, 1.60),
        // 9: Кокос / Хуан-де-Фука
        (12.0, -102.0, PlateType::Oceanic, 28.0, -120.0, 1.75),
        // 10: Карибская
        (15.0, -75.0, PlateType::Continental, 50.0, -65.0, 0.50),
        // 11: Аравийская
        (24.0, 45.0, PlateType::Continental, 50.0, 20.0, 1.15),
        // 12: Филиппинская
        (18.0, 135.0, PlateType::Oceanic, -48.0, 155.0, 1.50),
        // 13: Скотия
        (-56.0, -45.0, PlateType::Oceanic, -20.0, -80.0, 0.65),
    ];

    specs
        .into_iter()
        .map(|(c_lat, c_lon, plate_type, e_lat, e_lon, angular_vel)| Plate {
            center_dir: lat_lon_deg_to_vec3(c_lat, c_lon),
            plate_type,
            euler_pole: lat_lon_deg_to_vec3(e_lat, e_lon),
            angular_vel,
        })
        .collect()
}

/// Генерирует цепочки вулканов ТОЛЬКО от 38 реальных мантийных плюмов Земли (каталог Courtillot/Morgan/Sleep)
/// по тому же кинематическому закону вращения плит, что и generate_hotspots в tectonics_solver.rs
fn build_plume_hotspot_chains(plates: &[Plate], config: &TectonicsConfig) -> Vec<Volcano> {
    // (lat плюма, lon плюма, plate_id, длина цепочки, базовая высота)
    let plumes: &[(f32, f32, usize, usize, f32)] = &[
        // --- Тихоокеанская плита (мощные длинные цепи) ---
        (19.4, -155.3, 0, 18, 0.55),  // 1. Гавайи — Императорский хребет
        (-50.9, -138.1, 0, 16, 0.48), // 2. Луисвилл
        (-29.0, -140.3, 0, 13, 0.45), // 3. Макдональд — Тубуаи
        (-17.8, -148.1, 0, 11, 0.46), // 4. Таити — Острова Общества
        (-25.0, -130.1, 0, 11, 0.42), // 5. Питкэрн — Туамоту
        (-10.5, -139.0, 0, 10, 0.44), // 6. Маркизские о-ва
        (-14.5, -168.2, 0, 11, 0.45), // 7. Самоа
        (5.0, 164.0, 0, 11, 0.42),    // 8. Каролинская цепь
        (53.0, -136.0, 0, 11, 0.42),  // 9. Боуи — Кодьяк (Аляскинский залив)
        (46.0, -130.0, 0, 10, 0.40),  // 10. Кобб — Хуан-де-Фука
        (29.0, -118.3, 0, 8, 0.38),   // 11. Гуадалупе
        (18.8, -111.0, 0, 8, 0.38),   // 12. Сокорро
        // --- Плиты Наска и Кокос ---
        (-0.4, -91.6, 8, 12, 0.50),   // 13. Галапагосы — хребет Карнеги
        (-26.4, -105.3, 8, 14, 0.46), // 14. Остров Пасхи — Сала-и-Гомес
        (-33.6, -78.8, 8, 10, 0.42),  // 15. Хуан-Фернандес
        (-26.3, -80.1, 8, 9, 0.40),   // 16. Сан-Феликс
        // --- Северо-Американская и Южно-Американская плиты ---
        (44.4, -110.6, 2, 12, 0.48),  // 17. Йеллоустоун — Снейк-Ривер
        (36.8, -104.4, 2, 8, 0.36),   // 18. Ратон (Нью-Мексико)
        (-3.8, -32.4, 3, 10, 0.42),   // 19. Фернанду-ди-Норонья
        (-20.5, -29.3, 3, 11, 0.42),  // 20. Триндади — Мартин-Вас
        // --- Атлантика, Евразия и Африканская плита ---
        (64.4, -17.3, 1, 13, 0.52),   // 21. Исландия — Гренландско-Фарерский порог
        (71.1, -8.2, 1, 8, 0.40),     // 22. Ян-Майен
        (38.5, -28.4, 1, 10, 0.44),   // 23. Азорские о-ва
        (50.2, 6.8, 1, 7, 0.35),      // 24. Эйфель (Германия)
        (51.5, 101.0, 1, 8, 0.38),    // 25. Байкальский / Хангайский плюм
        (28.3, -16.6, 4, 11, 0.46),   // 26. Канарские о-ва
        (14.9, -24.3, 4, 10, 0.44),   // 27. Кабо-Верде (Острова Зеленого Мыса)
        (-37.1, -12.3, 4, 15, 0.48),  // 28. Тристан-да-Кунья — Китовый хребет (Уолвис)
        (-40.3, -9.9, 4, 11, 0.42),   // 29. Гоф / Дискавери
        (-54.4, 3.4, 4, 9, 0.40),     // 30. Буве
        (23.3, 5.6, 4, 8, 0.38),      // 31. Ахаггар (Сахара)
        (20.8, 17.5, 4, 8, 0.40),     // 32. Тибести
        (13.0, 24.2, 4, 7, 0.36),     // 33. Дарфур
        (11.8, 41.8, 4, 12, 0.50),    // 34. Афар — Эфиопский плюм
        (-3.1, 35.5, 4, 10, 0.48),    // 35. Восточно-Африканский плюм (Килиманджаро/Кения)
        // --- Индийский океан, Австралия и Антарктида ---
        (-21.2, 55.7, 4, 16, 0.52),   // 36. Реюньон — Маскаренско-Деканский трек
        (-11.8, 43.3, 4, 9, 0.42),    // 37. Коморские о-ва
        (-46.9, 37.7, 4, 10, 0.42),   // 38. Марион — Принс-Эдуард
        (-46.4, 51.7, 7, 10, 0.42),   // 39. Крозе
        (-49.2, 69.2, 7, 16, 0.52),   // 40. Кергелен — Восточно-Индийский хребет (Ninetyeast)
        (-37.8, 145.0, 5, 11, 0.40),  // 41. Восточно-Австралийский / Тасманов плюм
        (-77.5, 167.2, 7, 8, 0.44),   // 42. Эребус / Баллени (Антарктида)
    ];

    let time_step = config.hotspot_time_step;
    let mut volcanoes = Vec::with_capacity(plumes.len() * 12);

    for &(lat, lon, plate_idx, chain_length, height_base) in plumes {
        let plume_pos = lat_lon_deg_to_vec3(lat, lon);
        let plate = &plates[plate_idx.min(plates.len() - 1)];

        for step in 0..chain_length {
            let angle = plate.angular_vel * (step as f32) * time_step;
            let rotation = Quat::from_axis_angle(plate.euler_pole, angle);
            let volcano_pos = (rotation * plume_pos).normalize();

            let age_factor = 1.0 - (step as f32 / chain_length as f32);
            let height = height_base * age_factor.powf(config.hotspot_erosion_pow);
            let radius = (config.hotspot_radius_base
                + height * config.volcano_radius_per_height
                + config.hotspot_radius_growth * (1.0 - age_factor))
                .max(config.volcano_radius_min);

            volcanoes.push(Volcano {
                center: volcano_pos,
                height,
                radius,
            });
        }
    }

    volcanoes
}

/// Строит полный подводный рельеф океана (шельфы, СОХ, желоба, остывание коры, вулканические цепи)
/// и карту пород RockType напрямую на икосфере WorldTopology
fn synthesize_ocean_bathymetry_and_lithology(
    topology: &WorldTopology,
    geology: &mut GeologyLayer,
    is_ocean: &[bool],
    config: &TectonicsConfig,
) {
    let num_cells = topology.cells.len();
    let noise = Fbm::<Perlin>::new(42_2026);

    // 1. Геодезический BFS расстояния от берега вглубь океана (в радианах)
    let mut dist_to_coast = vec![f32::MAX; num_cells];
    let mut queue = VecDeque::new();
    for i in 0..num_cells {
        if !is_ocean[i] {
            dist_to_coast[i] = 0.0;
            queue.push_back(i);
        }
    }
    let max_coast_rad = (config.base_oc_dist * 2.2).max(0.28);
    while let Some(curr) = queue.pop_front() {
        let d_curr = dist_to_coast[curr];
        if d_curr >= max_coast_rad {
            continue;
        }
        let pos_curr = topology.cells[curr].center;
        for &n_id in &topology.cells[curr].neighbors {
            let step = pos_curr.dot(topology.cells[n_id].center).clamp(-1.0, 1.0).acos();
            let next_d = d_curr + step;
            if next_d < dist_to_coast[n_id] {
                dist_to_coast[n_id] = next_d;
                queue.push_back(n_id);
            }
        }
    }

    // 2. Геодезический BFS расстояния до реальных границ плит PB2002 (в радианах)
    let mut dist_to_boundary = vec![f32::MAX; num_cells];
    let mut other_plate = geology.plate_ids.clone();
    let mut boundary_dir = vec![Vec3::ZERO; num_cells];
    queue.clear();

    for i in 0..num_cells {
        let my_p = geology.plate_ids[i];
        let my_pos = topology.cells[i].center;
        for &n_id in &topology.cells[i].neighbors {
            let n_p = geology.plate_ids[n_id];
            if n_p != my_p {
                dist_to_boundary[i] = 0.0;
                other_plate[i] = n_p;
                boundary_dir[i] = (topology.cells[n_id].center - my_pos).normalize_or_zero();
                queue.push_back(i);
                break;
            }
        }
    }

    let edge_thresh = config.edge_threshold.max(0.32);
    while let Some(curr) = queue.pop_front() {
        let d_curr = dist_to_boundary[curr];
        if d_curr >= edge_thresh {
            continue;
        }
        let pos_curr = topology.cells[curr].center;
        for &n_id in &topology.cells[curr].neighbors {
            let step = pos_curr.dot(topology.cells[n_id].center).clamp(-1.0, 1.0).acos();
            let next_d = d_curr + step;
            if next_d < dist_to_boundary[n_id] {
                dist_to_boundary[n_id] = next_d;
                other_plate[n_id] = other_plate[curr];
                boundary_dir[n_id] = boundary_dir[curr];
                queue.push_back(n_id);
            }
        }
    }

    let mut ridge_cells = vec![false; num_cells];

    // 3. Расчет рельефа океана (шельфы, СОХ, желоба, плюмы, шум) и базовой литологии
    for i in 0..num_cells {
        let cell = &topology.cells[i];
        let dir = cell.center;
        let d_vec = dir.as_dvec3();

        let p1_id = geology.plate_ids[i];
        let p2_id = other_plate[i];
        let p1 = &geology.plates[p1_id];
        let p2 = &geology.plates[p2_id];

        let bd = dist_to_boundary[i];
        let f = (1.0 - bd / edge_thresh).clamp(0.0, 1.0);

        let v1 = p1.euler_pole.cross(dir) * p1.angular_vel;
        let v2 = p2.euler_pole.cross(dir) * p2.angular_vel;
        let normal = if boundary_dir[i] != Vec3::ZERO {
            boundary_dir[i]
        } else {
            (p2.center_dir - p1.center_dir).normalize_or_zero()
        };
        let convergence = (v1 - v2).dot(normal) / config.convergence_ref_speed.max(0.1);

        let m_scale = config.boundary_noise_scale;
        let raw_m_noise = noise.get([d_vec.x * m_scale, d_vec.y * m_scale, d_vec.z * m_scale]) as f32;
        let m_noise = config.boundary_noise_base + raw_m_noise * config.boundary_noise_amp;

        let d_scale = config.detail_noise_scale;
        let detail_noise = noise.get([d_vec.x * d_scale, d_vec.y * d_scale, d_vec.z * d_scale]) as f32;

        // --- А) Рельеф морского дна для ячеек океана ---
        if is_ocean[i] {
            let coast_d = dist_to_coast[i];
            // В Арктике шельф шире (Баренцево, Карское, Восточно-Сибирское моря)
            let arctic_bonus = ((cell.lat_lon.x - 0.95).max(0.0) * 1.4).clamp(0.0, 0.9);
            let oc_dist = config.base_oc_dist.max(0.09) * (1.0 + arctic_bonus);

            // Плавный профиль от берега (шельфа) к базовой глубине океана (как в calculate_base_elevation)
            let factor = profile_factor(coast_d / oc_dist, config.base_profile_smoothness);
            let shelf_offset = (config.base_co_offset + config.base_oc_offset) * 0.5;
            let mut h = config.ocean_base_height * factor - (1.0 - factor) * shelf_offset.max(0.015);

            // Тектонические границы в океане (на удалении от береговой кромки)
            let offshore_blend = smoothstep((coast_d - 0.015) / 0.05);
            if f > 0.0 && offshore_blend > 0.0 {
                if convergence < 0.0 {
                    // Расхождение в океане -> Срединно-океанический хребет с осевым рифтом!
                    ridge_cells[i] = true;
                    let conv_factor = (-convergence).clamp(0.0, 1.0).exp();
                    let axial_rift = -f.powi(config.oo_rift_axial_pow) * config.oo_rift_axial_amp;
                    let ridges = (f * PI).sin() * config.oo_rift_ridge_amp;
                    let noise_mod =
                        config.oo_rift_noise_base + config.oo_rift_noise_mult * m_noise;
                    let dh = (ridges + axial_rift) * config.oo_rift_total_amp * noise_mod * conv_factor;
                    h += dh * offshore_blend;
                } else {
                    // Схождение в океане -> Глубоководный жёлоб (Марианский, Перуанский, Курильский и др.)
                    let conv_factor = convergence.clamp(0.0, 1.0).exp();
                    let offset_dist = (f - config.oc_col_center).abs();
                    let trench_prof = (1.0 - offset_dist * config.oc_col_scale).clamp(0.0, 1.0);
                    let dh = -trench_prof * config.oc_col_amp * conv_factor;
                    h += dh * offshore_blend;
                }
            }

            geology.elevations[i] = h;
        }

        // --- Б) Наложение цепочек горячих точек (мантийных плюмов) ---
        let v_scale = config.volcano_noise_scale;
        let raw_v_noise = noise.get([d_vec.x * v_scale, d_vec.y * v_scale, d_vec.z * v_scale]) as f32;
        let v_noise_mod = config.volcano_noise_base + config.volcano_noise_mult * raw_v_noise;

        let mut max_volcano_h = 0.0_f32;
        let mut is_volcano = false;
        for v in &geology.volcanoes {
            let cos_d = dir.dot(v.center).clamp(-1.0, 1.0);
            if cos_d > 0.992 {
                let d = cos_d.acos();
                if d < v.radius {
                    let vf = 1.0 - (d / v.radius);
                    let profile = vf.powi(config.volcano_profile_pow);
                    let local_h = v.height * profile * v_noise_mod;
                    max_volcano_h = max_volcano_h.max(local_h);

                    if d < v.radius * 0.70 {
                        is_volcano = true;
                    }
                }
            }
        }
        geology.volcano_mask[i] = is_volcano;

        if is_ocean[i] {
            // В океане цепи плюмов формируют выраженные подводные хребты и гайоты (но не пробивают 0.0, если там нет острова)
            let h_with_plume = (geology.elevations[i] + max_volcano_h * 0.75).min(-0.006);
            geology.elevations[i] = (h_with_plume + detail_noise * config.detail_noise_amp)
                .clamp(config.elevation_min_clamp, -0.003);
        } else if max_volcano_h > 0.0 {
            // На суше мягко акцентируем купола горячих точек
            geology.elevations[i] += max_volcano_h * 0.22;
        }

        // --- В) Первичная литология (RockType) ---
        let t1 = if is_ocean[i] {
            PlateType::Oceanic
        } else {
            PlateType::Continental
        };
        let t2 = p2.plate_type;

        geology.lithology[i] = if t1 == PlateType::Oceanic {
            if geology.elevations[i] < -0.45 && f > 0.25 && convergence > 0.0 {
                RockType::PelagicSediment
            } else if f > 0.40 && convergence > 0.15 && geology.elevations[i] > -0.18 {
                RockType::Andesite
            } else {
                RockType::OceanicBasalt
            }
        } else if geology.elevations[i] > 0.24 {
            if t2 == PlateType::Oceanic && convergence > 0.0 && raw_m_noise > -0.15 {
                RockType::Andesite
            } else if detail_noise > -0.10 {
                RockType::Granite
            } else {
                RockType::GneissSchist
            }
        } else if geology.elevations[i] > 0.11 {
            if raw_m_noise + detail_noise * 0.4 > 0.35 {
                RockType::Granite
            } else if raw_m_noise > -0.25 {
                RockType::GneissSchist
            } else {
                RockType::Sandstone
            }
        } else {
            let shield_val = raw_m_noise + detail_noise * 0.35;
            if shield_val > 0.55 {
                RockType::Granite
            } else if shield_val > 0.28 {
                RockType::GneissSchist
            } else {
                RockType::Sandstone
            }
        };
    }

    // 4. Закон Парсонса-Склейтера: охлаждение и проседание океанической коры по мере удаления от СОХ
    apply_earth_oceanic_cooling(topology, &mut geology.elevations, &ridge_cells, config);

    // 5. Легкая диффузия только для дна океана (сглаживает ступеньки без размытия берегов суши)
    let land_backup = geology.elevations.clone();
    step_elevation_diffusion(
        topology,
        &mut geology.elevations,
        config.diffusion_strength * 0.6,
        config.slope_erosion_factor * 0.5,
        config.diffusion_max_strength,
    );
    for i in 0..num_cells {
        if !is_ocean[i] {
            geology.elevations[i] = land_backup[i];
        } else {
            geology.elevations[i] = geology.elevations[i].min(-0.003);
        }
    }

    // 6. Поверхностные наложения пород (траппы, вулканы, известняки, осадочные бассейны)
    for i in 0..num_cells {
        let elev = geology.elevations[i];
        let lat_abs = topology.cells[i].lat_lon.x.abs();

        if geology.volcano_mask[i] {
            geology.lithology[i] = if elev <= 0.0 || elev < 0.22 {
                RockType::FloodBasalt
            } else {
                RockType::Andesite
            };
            continue;
        }

        let mut max_slope = 0.0_f32;
        for &n_id in &topology.cells[i].neighbors {
            let diff = (elev - geology.elevations[n_id]).abs();
            if diff > max_slope {
                max_slope = diff;
            }
        }

        let dir = topology.cells[i].center.as_dvec3();
        let litho_noise =
            noise.get([dir.x * 6.5 + 31.7, dir.y * 6.5 - 19.3, dir.z * 6.5 + 47.1]) as f32;

        if elev <= 0.0 {
            if elev > -0.09 + litho_noise * 0.015 && lat_abs < 0.62 && !ridge_cells[i] {
                geology.lithology[i] = RockType::Limestone;
            } else if elev < config.ocean_base_height - 0.05 && !ridge_cells[i] {
                geology.lithology[i] = RockType::PelagicSediment;
            }
            continue;
        }

        if elev < 0.085
            && max_slope < 0.018
            && lat_abs < 0.60 + litho_noise * 0.05
            && litho_noise > 0.14
        {
            geology.lithology[i] = RockType::Limestone;
            continue;
        }

        if elev < 0.095 && max_slope < 0.014 {
            if max_slope < 0.006 && elev < 0.055 {
                geology.lithology[i] = RockType::ShaleClay;
            } else if matches!(
                geology.lithology[i],
                RockType::Granite | RockType::GneissSchist
            ) {
                geology.lithology[i] = RockType::Sandstone;
            }
        }
    }
}

fn apply_earth_oceanic_cooling(
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