use bevy::prelude::*;

#[derive(PartialEq, Clone, Copy)]
pub enum PlateType {
    Oceanic,
    Continental,
}

pub struct Plate {
    pub center_dir: Vec3,
    pub plate_type: PlateType,
    pub euler_pole: Vec3,
    pub angular_vel: f32,
    pub base_offset: f32,
}

pub struct Volcano {
    pub center: Vec3,
    pub height: f32,
    pub radius: f32,
}

/// Слой геологических данных: массивы индексируются по `CellId`
#[derive(Resource, Default)]
pub struct GeologyLayer {
    pub plate_ids: Vec<usize>,
    pub elevations: Vec<f32>,
    pub plates: Vec<Plate>,
    pub volcanoes: Vec<Volcano>,
    pub volcano_mask: Vec<bool>,
    pub boundary_dist: Vec<f32>,
    pub boundary_stress: Vec<f32>, // >0 сжатие, <0 растяжение
}

impl GeologyLayer {
    pub fn resize(&mut self, num_cells: usize) {
        self.plate_ids.resize(num_cells, 0);
        self.elevations.resize(num_cells, 0.0);
        self.volcano_mask.resize(num_cells, false);
        self.boundary_dist.resize(num_cells, 0.0);
        self.boundary_stress.resize(num_cells, 0.0);
    }
}
