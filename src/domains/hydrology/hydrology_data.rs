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

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Biome {
    #[default]
    DeepOcean,                // Глубокий океан
    ShallowOcean,             // Шельфовое мелководье и рифы
    SeaIce,                   // Многолетний морской лед

    IceSheet,                 // Полярные ледниковые щиты и вечные снега гор
    AlpineBare,               // Высокогорные скалы, осыпи и гольцы (на Leptosol)
    Tundra,                   // Полярная и субарктическая тундра (на Gelisol)
    BorealTaiga,              // Тайга (бореальные хвойные леса на Podzol)
    TemperateDeciduousForest, // Умеренный широколиственный и смешанный лес
    TemperateRainforest,      // Умеренный влажный прибрежный лес
    MediterraneanShrubland,   // Средиземноморские жестколистные леса и кустарники
    TemperateGrassland,       // Степь и прерия (на Chernozem)
    ColdDesert,               // Холодная континентальная пустыня и полупустыня
    HotDesert,                // Жаркая субтропическая пустыня (на Aridisol)
    XericShrubland,           // Засушливая кустарниковая полупустыня (скраб)
    Savanna,                  // Тропическая саванна и редколесье
    TropicalSeasonalForest,   // Тропический муссонный (сезонно-влажный) лес
    TropicalRainforest,       // Влажный экваториальный лес (джунгли)

    Wetland,                  // Болота, топи и речные плавни (на Gleysol)
    RiparianOasis,            // Пойменный галерейный лес и речные оазисы в засушливых зонах
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
    pub runoff: Vec<f32>,
    pub flow_dir: Vec<Option<u8>>,
    pub flow_accumulation: Vec<f32>,

    /// Фактическая влажность почвы (от осадков, талых вод и фильтрации из рек) [0.0 .. 1.0]
    pub soil_moisture: Vec<f32>,
   
    // --- БИОМЫ ---
    pub biomes: Vec<Biome>,
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

        self.soil_moisture.resize(num_cells, 0.0);
        
        self.biomes.resize(num_cells, Biome::DeepOcean);
    }
}
