use crate::{config::PlanetConfig, grid::WorldGrid};
use bevy::prelude::*;
use noise::NoiseFn;
use rand::RngExt;
use std::f32::consts::PI;

#[derive(PartialEq, Clone, Copy)]
pub enum PlateType { Oceanic, Continental }

pub struct Plate {
    pub center_dir: Vec3,
    pub plate_type: PlateType,
    pub euler_pole: Vec3,
    pub angular_vel: f32,
}

pub fn generate_plates(rng: &mut impl RngExt, num_plates: usize, continental_ratio: f64) -> Vec<Plate> {
    let mut plates = Vec::with_capacity(num_plates);
    for _ in 0..num_plates {
        let plate_type = if rng.random_bool(continental_ratio) { PlateType::Continental } else { PlateType::Oceanic };
        let mut best_dir = Vec3::Y;
        let mut best_dist = -1.0;

        for _ in 0..15 {
            let z: f32 = rng.random_range(-1.0..1.0);
            let lon: f32 = rng.random_range(-PI..PI);
            let r = (1.0 - z * z).sqrt();
            let candidate = Vec3::new(r * lon.cos(), z, r * lon.sin());

            let min_dist = plates.iter().map(|p: &Plate | candidate.dot(p.center_dir).clamp(-1.0, 1.0).acos()).fold(f32::MAX, f32::min);
            if min_dist > best_dist {
                best_dist = min_dist;
                best_dir = candidate;
            }
        }

        // Генерируем случайную ось вращения (полюс Эйлера) для плиты
        let rand_vec = Vec3::new(
            rng.random_range(-1.0..1.0), 
            rng.random_range(-1.0..1.0), 
            rng.random_range(-1.0..1.0)
        ).normalize_or_zero();
        
        // Защита от нулевого вектора
        let euler_pole = if rand_vec == Vec3::ZERO { Vec3::Y } else { rand_vec };
        
        // Задаем случайную скорость вращения плиты
        let angular_vel = rng.random_range(0.5..2.0); 

        plates.push(Plate { 
            center_dir: best_dir, 
            plate_type, 
            euler_pole, 
            angular_vel 
        });
    }
    plates
}

pub fn simulate_tectonics(grid: &mut WorldGrid, plates: &[Plate], noise: &impl NoiseFn<f64, 3>, config: &PlanetConfig) {
    for cell in &mut grid.cells {
        let dir = cell.center;
        let p = dir.as_dvec3() * 1.5;
        let warp_noise = Vec3::new(
            noise.get([p.x + 12.3, p.y + 45.6, p.z + 78.9]) as f32,
            noise.get([p.x + 98.7, p.y + 65.4, p.z + 32.1]) as f32,
            noise.get([p.x + 55.5, p.y + 11.1, p.z + 99.9]) as f32,
        ) * config.warp_strength;

        let warped_dir = (dir + warp_noise).normalize();

        // Теперь получаем 3 ближайшие плиты
        let (p1_idx, p2_idx, p3_idx, dist_1, dist_2, dist_3) = find_closest_plates(warped_dir, plates);
        cell.plate_id = p1_idx;
        
        let p1 = &plates[p1_idx];
        let p2 = &plates[p2_idx];
        let p3 = &plates[p3_idx];
        
        let bd2 = dist_2 - dist_1;
        let bd3 = dist_3 - dist_1;

        // Вычисляем веса обратного расстояния. 
        // 0.0001 защищает от деления на ноль.
        let w2 = 1.0 / (bd2.powi(2) + 0.0001);
        let w3 = 1.0 / (bd3.powi(2) + 0.0001);
        let w_sum = w2 + w3;
        
        // Нормализуем веса (чтобы в сумме они давали 1.0)
        let weight2 = w2 / w_sum;
        let weight3 = w3 / w_sum;

        // Рассчитываем базовую высоту для двух возможных границ
        let mut h2 = calculate_base_elevation(p1, p2, bd2, config);
        let mut h3 = calculate_base_elevation(p1, p3, bd3, config);

        let v1 = p1.euler_pole.cross(dir) * p1.angular_vel;

        // Добавляем горы/рифты для границы 1-2
        if bd2 < config.edge_threshold {
            let f2 = (1.0 - bd2 / config.edge_threshold).clamp(0.0, 1.0);
            let normal2 = (p2.center_dir - p1.center_dir).normalize_or_zero();
            let v2 = p2.euler_pole.cross(dir) * p2.angular_vel;
            let convergence2 = (v1 - v2).dot(normal2);
            
            let m_noise = 0.6 + noise.get([warped_dir.x as f64 * 15.0, warped_dir.y as f64 * 15.0, warped_dir.z as f64 * 15.0]) as f32 * 1.2;
            h2 += calculate_boundary_elevation(p1.plate_type, p2.plate_type, f2, convergence2, m_noise);
        }

        // Добавляем горы/рифты для границы 1-3
        if bd3 < config.edge_threshold {
            let f3 = (1.0 - bd3 / config.edge_threshold).clamp(0.0, 1.0);
            let normal3 = (p3.center_dir - p1.center_dir).normalize_or_zero();
            let v3 = p3.euler_pole.cross(dir) * p3.angular_vel;
            let convergence3 = (v1 - v3).dot(normal3);
            
            let m_noise = 0.6 + noise.get([warped_dir.x as f64 * 15.0, warped_dir.y as f64 * 15.0, warped_dir.z as f64 * 15.0]) as f32 * 1.2;
            h3 += calculate_boundary_elevation(p1.plate_type, p3.plate_type, f3, convergence3, m_noise);
        }

        // Итоговая высота — это плавное взвешенное среднее двух границ
        let h = h2 * weight2 + h3 * weight3;

        let detail_noise = noise.get([warped_dir.x as f64 * 4.0, warped_dir.y as f64 * 4.0, warped_dir.z as f64 * 4.0]) as f32;
        cell.elevation = (h + detail_noise * 0.35).max(-0.9);
    }

    step_elevation_diffusion(grid, config.diffusion_strength);
}

fn find_closest_plates(dir: Vec3, plates: &[Plate]) -> (usize, usize, usize, f32, f32, f32) {
    let mut dist_1 = f32::MAX;
    let mut dist_2 = f32::MAX;
    let mut dist_3 = f32::MAX;
    let mut p1_idx = 0;
    let mut p2_idx = 0;
    let mut p3_idx = 0;

    for (i, plate) in plates.iter().enumerate() {
        let d = dir.dot(plate.center_dir).clamp(-1.0, 1.0).acos();
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

fn calculate_base_elevation(p1: &Plate, p2: &Plate, boundary_dist: f32, config: &PlanetConfig) -> f32 {
    let h_base = match p1.plate_type {
        PlateType::Continental => config.cont_base_height,
        PlateType::Oceanic => config.ocean_base_height,
    };

    match (p1.plate_type, p2.plate_type) {
        (PlateType::Continental, PlateType::Oceanic) => {
            let factor = (boundary_dist / 0.2).clamp(0.0, 1.0);
            h_base * factor - (1.0 - factor) * 0.05
        }
        (PlateType::Oceanic, PlateType::Continental) => {
            let factor = (boundary_dist / 0.25).clamp(0.0, 1.0);
            h_base * factor - (1.0 - factor) * 0.1
        }
        (PlateType::Continental, PlateType::Continental) => {
            let factor = (boundary_dist / 0.10).clamp(0.0, 1.0);
            h_base * factor + (1.0 - factor) * 0.3
        }
        (PlateType::Oceanic, PlateType::Oceanic) => {
            let factor = (boundary_dist / 0.25).clamp(0.0, 1.0);
            h_base * factor + (1.0 - factor) * 0.01
        }
    }
}

fn calculate_boundary_elevation(t1: PlateType, t2: PlateType, f: f32, convergence: f32, mountain_noise: f32) -> f32 {
    match (t1, t2) {
        // Коллизия (Гималаи)
        (PlateType::Continental, PlateType::Continental) if convergence > 0.0 => {
            let shape = (f * PI * 1.5).sin().powi(2);
            shape * (0.5 + 0.5 * f) * 0.4 * mountain_noise
        }
        // Континентальный рифтогенез (Восточно-Африканская рифтовая долина)
        (PlateType::Continental, PlateType::Continental) if convergence < 0.0 => {
            // f.powi(4) дает широкую долину в центре, а sin поднимает плечи рифта по краям
            let graben = -f.powi(4) * 0.6; 
            let horsts = (f * PI).sin() * 0.3; 
            (horsts + graben) * mountain_noise
        }
        // Субдукция, сторона континента (Анды)
        (PlateType::Continental, PlateType::Oceanic) if convergence > 0.0 => {
            let m_zone = ((f - 0.2) * 2.0).clamp(0.0, 1.0);
            (m_zone * PI).sin().powi(2) * 0.35 * mountain_noise
        }
        // Переходная зона с разломом континентальной плиты
        (PlateType::Continental, PlateType::Oceanic) if convergence < 0.0 => {
            let m_zone = ((f - 0.2) * 2.0).clamp(0.0, 1.0);
            -(m_zone * PI).sin().powi(2) * 0.35 * mountain_noise
        }
        // Субдукция, сторона океана (Океанический желоб)
        (PlateType::Oceanic, PlateType::Continental) if convergence > 0.0 => {
            -((1.0 - (f - 0.5).abs() * 2.0).clamp(0.0, 1.0)) * 0.25
        }
        // Субдукция, два океана (Островные дуги)
        (PlateType::Oceanic, PlateType::Oceanic) if convergence > 0.0 => {
            (f * PI).sin() * 0.2 * mountain_noise
        }
        // Спрединг, два океана (Срединно-океанический хребет)
        (PlateType::Oceanic, PlateType::Oceanic) if convergence < 0.0 => {
            // f.powi(12) дает очень узкую, резкую трещину ровно при f -> 1.0
            let axial_rift = -f.powi(12) * 0.4;
            // sin дает пологие гряды, которые плавно затухают в 0.0 при удалении (f -> 0.0)
            let ridges = (f * PI).sin() * 0.2;
            
            (ridges + axial_rift) * (0.5 + 0.5 * mountain_noise)
        }
        _ => 0.0,
    }
}

pub fn step_elevation_diffusion(grid: &mut WorldGrid, strength: f32) {
    let old_elevations: Vec<f32> = grid.cells.iter().map(|c| c.elevation).collect();
    for (i, cell) in grid.cells.iter_mut().enumerate() {
        let avg_neighbor: f32 = cell.neighbors.iter().map(|&n_id| old_elevations[n_id]).sum::<f32>() / cell.neighbors.len() as f32;
        cell.elevation = old_elevations[i] * (1.0 - strength) + avg_neighbor * strength;
    }
}