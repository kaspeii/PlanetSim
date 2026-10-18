use super::data::ClimateSeason;
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

    /// Сила базовых широтных поясов (ячейки Хэдли, Феррела, Полярная)
    pub latitudinal_pressure_weight: f32,
    /// Насколько сильно прогрев/охлаждение материков искажает пояса давления
    pub thermal_pressure_weight: f32,
    /// Множитель скорости ветра от перепада давлений
    pub wind_speed_multiplier: f32,
    /// Сила Кориолиса (максимальный угол отклонения ветра в радианах, ~1.1 рад = ~63°)
    pub coriolis_strength: f32,
    /// Насколько сильно горные хребты тормозят ветер при подъеме
    pub mountain_wind_barrier: f32,
    /// Отрисовывать ли 3D-стрелки направления ветра поверх глобуса
    pub show_wind_arrows: bool,

    /// Сила переноса тепла ветрами (0.0 — ветра не влияют на температуру, 1.0 — максимум)
    pub heat_advection_strength: f32,
    /// Количество итераций поиска равновесия (Температура <-> Давление/Ветра)
    pub equilibrium_iterations: usize,

    /// Наклон оси планеты в радианах (0.41 рад ≈ 23.5°)
    pub axial_tilt: f32,
    /// Какой из трех предрассчитанных сезонов сейчас показывать на глобусе
    pub active_season: ClimateSeason,
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

            latitudinal_pressure_weight: 0.65,
            thermal_pressure_weight: 0.45,
            wind_speed_multiplier: 12.0,
            coriolis_strength: 1.05, // Отклонение ~60 градусов дает реалистичные пассаты и западные ветры
            mountain_wind_barrier: 8.0,
            show_wind_arrows: false,

            heat_advection_strength: 0.75,
            equilibrium_iterations: 4,

            axial_tilt: 0.41,
            active_season: ClimateSeason::Spring, // По умолчанию отображаем Весну
        }
    }
}
