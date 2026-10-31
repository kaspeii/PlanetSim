// src/domains/hydrology/hydrology_data.rs

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SoilType {
    #[default]
    Ocean, // Морское дно
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
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Biome {
    #[default]
    DeepOcean, // Глубокий океан
    ShallowOcean, // Шельфовое мелководье и рифы
    SeaIce,       // Многолетний морской лед

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

    Wetland,       // Болота, топи и речные плавни (на Gleysol)
    RiparianOasis, // Пойменный галерейный лес и речные оазисы в засушливых зонах
}

/// Ключевые ископаемые ресурсы и минералы (15 типов)
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum MineralResource {
    #[default]
    None,
    Clay,           // Глина (керамика, кирпич, строительные растворы)
    StoneLimestone, // Строительный камень, мрамор и известняк (крепости, цемент/бетон, флюс)
    SilicaQuartz,   // Кварцевый песок и кремний (стекло, оптика, микроэлектроника)
    Salt,           // Каменная и морская соль (консервация еды, снабжение, базовая химия)
    Copper,         // Медь (бронзовый век, электротехника)
    Tin,            // Олово (редкий компонент бронзы, ранняя дальняя торговля)
    Iron,           // Железная руда (оружие, инструменты, сталь, индустриализация)
    PreciousMetals, // Золото и серебро (валюта, торговля, электроника)
    SulfurNitre,    // Сера, селитра и фосфаты (порох, взрывчатка, минеральные удобрения)
    Coal,           // Каменный уголь и торф (металлургия, паровые машины, ТЭЦ)
    Hydrocarbons,   // Нефть и природный газ (ДВС, авиация, полимеры, нефтехимия)
    Bauxite,        // Бокситы / Алюминий (авиация, цветная металлургия, ЛЭП)
    AlloyMetals,    // Титан, никель, хром, кобальт (спецстали, турбины, броня)
    Uranium,        // Урановые руды (ядерная энергетика и вооружение)
    RareEarths,     // Редкоземельные металлы и литий (высокие технологии, чипы, батареи)
}

/// Стратегические ресурсы флоры и фауны (9 типов)
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum BioResource {
    #[default]
    None,
    NavalTimber, // Корабельный и строевой лес (флот, большепролетные балки, крепи шахт)
    TimberFuel,  // Обычная древесина и дрова (жилье, древесный уголь)
    FiberCrops,  // Волокнистые культуры: хлопок, лён, конопля (ткани, парусина, канаты)
    ResinRubber, // Смола, дёготь и каучук (конопатка флота, шины, изоляция, гидроизоляция)
    LuxuryFlora, // Пряности, чай, кофе, какао, красители (сверхприбыльная торговля)
    Horses,      // Лошади (степная кавалерия, быстрый транспорт, почта, тягло)
    Camels,      // Верблюды (караванная торговля через пустыни)
    ElephantsDraft, // Слоны, буйволы и горные вьючные (тяжелая тяга, высокогорная логистика)
    FurIvory,    // Пушнина, меха и моржовая/слоновая кость (северная экспансия и престиж)
}

#[derive(Resource, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
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

    // --- ПРОДОВОЛЬСТВЕННЫЙ ПОТЕНЦИАЛ (4 СТОЛПА ЕДЫ) [0.0 .. 1.0] ---
    /// Пахотное земледелие (злаки, рис, корнеплоды, бобовые)
    pub arable_potential: Vec<f32>,
    /// Пастбищное скотоводство (мясо, молоко, шерсть, кожи)
    pub pastoral_potential: Vec<f32>,
    /// Собирательство, садоводство и охота (фрукты, ягоды, орехи, виноград, лесная дичь)
    pub foraging_potential: Vec<f32>,
    /// Рыболовство и морской промысел (шельфы, апвеллинг, крупные реки)
    pub fishery_potential: Vec<f32>,

    // --- ИСКОПАЕМЫЕ И БИОРЕСУРСЫ ---
    pub minerals: Vec<MineralResource>,
    pub mineral_richness: Vec<f32>,
    pub bio_resources: Vec<BioResource>,
    pub bio_richness: Vec<f32>,
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

        self.arable_potential.resize(num_cells, 0.0);
        self.pastoral_potential.resize(num_cells, 0.0);
        self.foraging_potential.resize(num_cells, 0.0);
        self.fishery_potential.resize(num_cells, 0.0);

        self.minerals.resize(num_cells, MineralResource::None);
        self.mineral_richness.resize(num_cells, 0.0);
        self.bio_resources.resize(num_cells, BioResource::None);
        self.bio_richness.resize(num_cells, 0.0);
    }
}
