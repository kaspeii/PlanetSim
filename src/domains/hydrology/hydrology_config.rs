// src/domains/hydrology/hydrology_config.rs

use bevy::prelude::*;

#[derive(Resource, Clone)]
pub struct HydrologyConfig {
    /// Интенсивность химического выветривания (переход песка/ила в глину в теплом влажном климате)
    pub chemical_weathering_rate: f32,
    /// Чувствительность смыва почвы к крутизне склона (оголение скал в горах)
    pub slope_erosion_sensitivity: f32,
    /// Порог уклона для формирования горных скелетных почв (Leptosol)
    pub mountain_slope_threshold: f32,
    /// Среднегодовая температура вечной мерзлоты (°C) для формирования Gelisol
    pub permafrost_temp_threshold: f32,
    /// Коэффициент потенциального испарения (баланс сухости/влажности климата)
    pub aridity_evap_scale: f32,

    // --- ПАРАМЕТРЫ РЕК ---
    /// Множитель полноводности стока
    pub runoff_multiplier: f32,
    /// Минимальный порог накопленного потока для образования реки (в физических единицах сферы)
    pub river_threshold: f32,
    /// Насколько охотно реки выбирают мягкие осадочные породы вместо твердых гранитов
    pub lithology_erosion_bias: f32,
    /// Масштаб ширины 3D-лент рек на глобусе
    pub river_width_scale: f32,
    
    /// Сила увлажнения почвы реками (в самой клетке русла и фильтрация в соседние клетки долины)
    pub river_groundwater_seepage: f32,
    /// Включить двухпроходную обратную связь (Биомы и Реки меняют Альбедо, Влажность воздуха и Осадки)
    pub coupled_climate_feedback: bool,

    // --- ПАРАМЕТРЫ БИОМОВ ---
    /// Сила влияния текстуры и влагоемкости почвы (песок/глина) на сдвиг биомов Уиттекера
    pub soil_biome_influence: f32,
    /// Во сколько раз поток реки должен превышать `river_threshold`, чтобы создать оазис/пойменный лес
    pub riparian_river_multiplier: f32,
    /// Максимальный уклон для образования болот (Wetland) на глинистых почвах
    pub wetland_max_slope: f32,
}

impl Default for HydrologyConfig {
    fn default() -> Self {
        Self {
            chemical_weathering_rate: 0.45,
            slope_erosion_sensitivity: 18.0,
            mountain_slope_threshold: 0.042,
            permafrost_temp_threshold: -3.5,
            aridity_evap_scale: 1.0,

            runoff_multiplier: 1.0,
            river_threshold: 0.00035,
            lithology_erosion_bias: 0.015,
            river_width_scale: 3.0,

            river_groundwater_seepage: 1.0,
            coupled_climate_feedback: true,

            soil_biome_influence: 0.55,
            riparian_river_multiplier: 2.2,
            wetland_max_slope: 0.009,
        }
    }
}
