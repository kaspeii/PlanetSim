// src/domains/hydrology/hydrology_data.rs

use bevy::prelude::*;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SoilType {
    #[default]
    Ocean,     // Морское дно
    Leptosol,  // Горные скелетные почвы и скальные выходы на крутых склонах
    Gelisol,   // Мерзлотные тундровые почвы (вечная мерзлота, водоупор)
    Podzol,    // Кислотные подзолистые почвы холодных лесов (на песках и гранитах)
    Cambisol,  // Бурые и серые лесные почвы умеренных широт
    Chernozem, // Плодородные степные черноземы и каштановые почвы
    Aridisol,  // Сухие пустынные и полупустынные песчано-солончаковые почвы
    Laterite,  // Красно-желтые ферраллитные почвы жарких влажных тропиков
    Andisol,   // Темные высокоплодородные вулканические почвы
    Gleysol,   // Переувлажненные глинисто-болотные и пойменные почвы низин
}

#[derive(Resource, Default, Clone)]
pub struct HydrologyLayer {
    pub soils: Vec<SoilType>,
    /// Текстура почвы по треугольнику USDA: `[sand, silt, clay]`, сумма = 1.0
    pub soil_texture: Vec<[f32; 3]>,
    /// Относительная мощность (глубина) почвенного профиля [0.05 .. 1.0]
    pub soil_depth: Vec<f32>,
    /// Влагоемкость корнеобитаемого слоя (WHC) [0.0 .. 1.0]
    pub water_capacity: Vec<f32>,
    /// Водопроницаемость (инфильтрация) [0.0 .. 1.0]
    pub permeability: Vec<f32>,

    // --- ГИДРОЛОГИЯ И РЕКИ ---
    /// Локальный поверхностный и грунтовый сток клетки за год
    pub runoff: Vec<f32>,
    /// Индекс грани/соседа `k` (в `cell.neighbors[k]`), куда вытекает вода из клетки (`None` для океана)
    pub flow_dir: Vec<Option<u8>>,
    /// Суммарный накопленный поток воды (площадь водосбора × сток, инвариантно к разрешению сетки)
    pub flow_accumulation: Vec<f32>,
}

impl HydrologyLayer {
    pub fn resize(&mut self, num_cells: usize) {
        self.soils.resize(num_cells, SoilType::Ocean);
        self.soil_texture.resize(num_cells, [0.33, 0.34, 0.33]);
        self.soil_depth.resize(num_cells, 0.0);
        self.water_capacity.resize(num_cells, 0.0);
        self.permeability.resize(num_cells, 0.0);

        self.runoff.resize(num_cells, 0.0);
        self.flow_dir.resize(num_cells, None);
        self.flow_accumulation.resize(num_cells, 0.0);
    }
}