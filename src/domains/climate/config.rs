use bevy::prelude::*;

#[derive(Resource, Clone)]
pub struct ClimateConfig {
    /// Базовая температура на экваторе (в градусах Цельсия)
    pub equator_temp: f32,
    /// Базовая температура на полюсах
    pub pole_temp: f32,
    /// Насколько сильно падает температура с высотой (Lapse Rate)
    pub altitude_lapse_rate: f32,
    /// Смягчение температур над океаном (термоинерция воды)
    pub ocean_moderation: f32,
}

impl Default for ClimateConfig {
    fn default() -> Self {
        Self {
            equator_temp: 32.0,
            pole_temp: -30.0,
            altitude_lapse_rate: 20.0, // Чем выше гора, тем холоднее
            ocean_moderation: 0.4,     // Океаны делают климат мягче
        }
    }
}
