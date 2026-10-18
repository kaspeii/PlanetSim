use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct ClimateLayer {
    pub temperatures: Vec<f32>,
    pub humidity: Vec<f32>,
    pub albedo: Vec<f32>,
    pub distance_to_ocean: Vec<u32>,
    // В будущем: pub winds: Vec<Vec2>, pub pressure: Vec<f32>
}

impl ClimateLayer {
    pub fn resize(&mut self, num_cells: usize) {
        self.temperatures.resize(num_cells, 0.0);
        self.humidity.resize(num_cells, 0.0);
        self.albedo.resize(num_cells, 0.0);
        self.distance_to_ocean.resize(num_cells, 0);
    }
}
