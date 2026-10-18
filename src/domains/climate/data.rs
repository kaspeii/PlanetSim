use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct ClimateLayer {
    pub temperatures: Vec<f32>,
    pub humidity: Vec<f32>,
    pub albedo: Vec<f32>,
    pub distance_to_ocean: Vec<u32>,

    /// Атмосферное давление (в условных единицах отклонения от нормы: < 0 — циклон/низкое, > 0 — антициклон/высокое)
    pub pressure: Vec<f32>,
    /// Локальный 2D-вектор ветра: X — с запада на восток, Y — с юга на север
    pub winds: Vec<Vec2>,
    /// 3D-вектор ветра, лежащий в касательной плоскости к сфере (удобно для 3D-стрелок и адвекции)
    pub winds_3d: Vec<Vec3>,
}

impl ClimateLayer {
    pub fn resize(&mut self, num_cells: usize) {
        self.temperatures.resize(num_cells, 0.0);
        self.humidity.resize(num_cells, 0.0);
        self.albedo.resize(num_cells, 0.0);
        self.distance_to_ocean.resize(num_cells, 0);
        self.pressure.resize(num_cells, 0.0);
        self.winds.resize(num_cells, Vec2::ZERO);
        self.winds_3d.resize(num_cells, Vec3::ZERO);
    }
}
