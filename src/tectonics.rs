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
    pub drift_dir: Vec3,
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

        let rand_vec = Vec3::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0));
        let drift_dir = (rand_vec - best_dir * best_dir.dot(rand_vec)).normalize_or_zero();

        plates.push(Plate { center_dir: best_dir, plate_type, drift_dir });
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

        let (p1_idx, p2_idx, dist_1, dist_2) = find_closest_plates(warped_dir, plates);
        cell.plate_id = p1_idx;
        
        let p1 = &plates[p1_idx];
        let p2 = &plates[p2_idx];
        let boundary_dist = dist_2 - dist_1;

        let mut h = calculate_base_elevation(p1, p2, boundary_dist, config);

        if boundary_dist < config.edge_threshold {
            let f = (1.0 - boundary_dist / config.edge_threshold).clamp(0.0, 1.0);
            let boundary_normal = (p2.center_dir - p1.center_dir).normalize_or_zero();
            let convergence = (p1.drift_dir - p2.drift_dir).dot(boundary_normal);

            let mountain_noise = 0.6 + noise.get([warped_dir.x as f64 * 15.0, warped_dir.y as f64 * 15.0, warped_dir.z as f64 * 15.0]) as f32 * 1.2;
            h += calculate_boundary_elevation(p1.plate_type, p2.plate_type, f, convergence, mountain_noise);
        }

        let detail_noise = noise.get([warped_dir.x as f64 * 4.0, warped_dir.y as f64 * 4.0, warped_dir.z as f64 * 4.0]) as f32;
        cell.elevation = (h + detail_noise * 0.35).max(-0.9);
    }

    step_elevation_diffusion(grid, config.diffusion_strength);
}

fn find_closest_plates(dir: Vec3, plates: &[Plate]) -> (usize, usize, f32, f32) {
    let mut dist_1 = f32::MAX;
    let mut dist_2 = f32::MAX;
    let mut p1_idx = 0;
    let mut p2_idx = 0;

    for (i, plate) in plates.iter().enumerate() {
        let d = dir.dot(plate.center_dir).clamp(-1.0, 1.0).acos();
        if d < dist_1 {
            dist_2 = dist_1;
            p2_idx = p1_idx;
            dist_1 = d;
            p1_idx = i;
        } else if d < dist_2 {
            dist_2 = d;
            p2_idx = i;
        }
    }
    (p1_idx, p2_idx, dist_1, dist_2)
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
        (PlateType::Continental, PlateType::Continental) if convergence > -0.1 => {
            let shape = (f * PI * 1.5).sin().powi(2);
            shape * (0.5 + 0.5 * f) * 0.4 * mountain_noise
        }
        (PlateType::Continental, PlateType::Oceanic) if convergence > 0.0 => {
            let m_zone = ((f - 0.2) * 2.0).clamp(0.0, 1.0);
            (m_zone * PI).sin().powi(2) * 0.35 * mountain_noise
        }
        (PlateType::Oceanic, PlateType::Continental) if convergence > 0.0 => {
            -((1.0 - (f - 0.5).abs() * 2.0).clamp(0.0, 1.0)) * 0.25
        }
        (PlateType::Oceanic, PlateType::Oceanic) if convergence < -0.1 => {
            (f * PI).sin() * 0.2 * mountain_noise
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