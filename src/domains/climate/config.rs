use bevy::prelude::*;

#[derive(Resource, Clone)]
pub struct ClimateConfig {
    /// Базовая температура на экваторе (в градусах Цельсия)
    pub equator_temp: f32,
    /// Базовая температура на полюсах
    pub pole_temp: f32,
    /// Насколько сильно падает температура с высотой (Lapse Rate)
    pub altitude_lapse_rate: f32,

    /// Минимальный размер водоема (в долях от общего числа гексов планеты 0.0..1.0),
    pub min_ocean_size_ratio: f32,
    /// Базовое смягчение температур над самим океаном (dist = 0)
    pub base_ocean_moderation: f32,
    /// Как быстро падает влияние океана при продвижении вглубь материка
    pub continentality_factor: f32,

    /// Отражательная способность воды (поглощает много тепла)
    pub ocean_albedo: f32,
    /// Отражательная способность обычной суши
    pub land_albedo: f32,
    /// Отражательная способность льда (отражает тепло в космос)
    pub ice_albedo: f32,
    /// Температура, при которой земля покрывается льдом (меняя альбедо)
    pub freezing_temp: f32,
}

impl Default for ClimateConfig {
    fn default() -> Self {
        Self {
            equator_temp: 32.0,
            pole_temp: -30.0,
            altitude_lapse_rate: 70.0,

            min_ocean_size_ratio: 0.01, // 1% от площади планеты
            base_ocean_moderation: 0.6,
            continentality_factor: 0.1, // При 0.1 на расстоянии 10 гексов от моря эффект упадет в 2 раза

            ocean_albedo: 0.06,
            land_albedo: 0.3,
            ice_albedo: 0.8,
            freezing_temp: -5.0, // Лед формируется при устойчивом минусе
        }
    }
}
