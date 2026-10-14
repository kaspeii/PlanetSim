use bevy::prelude::*;

#[derive(Resource, Clone)]
pub struct PlanetConfig {
    pub radius: f32,
    pub subdivisions: u32,
    pub num_plates: usize,
    pub continental_ratio: f64,

    // Параметры тектоники и шума
    pub warp_strength: f32,
    pub edge_threshold: f32,
    pub cont_base_height: f32,
    pub ocean_base_height: f32,
    
    // Параметры сглаживания
    pub diffusion_strength: f32,
}

impl Default for PlanetConfig {
    fn default() -> Self {
        Self {
            radius: 5.0,
            subdivisions: 7,
            num_plates: 15,
            continental_ratio: 0.4,
            warp_strength: 0.15,
            edge_threshold: 0.45,
            cont_base_height: 0.12,
            ocean_base_height: -0.35,
            diffusion_strength: 0.25,
        }
    }
}