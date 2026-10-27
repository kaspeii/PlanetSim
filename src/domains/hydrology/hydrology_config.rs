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
        }
    }
}