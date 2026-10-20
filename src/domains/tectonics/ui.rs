use super::{config::TectonicsConfig, data::GeologyLayer, data::PlateType};
use crate::world::pipeline::WorldPipeline;
use bevy::prelude::*;
use bevy_egui::egui;

/// Генерирует массив цветов по высотам рельефа
pub fn build_elevation_colors(geology: &GeologyLayer) -> Vec<[f32; 4]> {
    geology
        .elevations
        .iter()
        .map(|&h| elevation_to_color(h).to_linear().to_f32_array())
        .collect()
}

/// Генерирует массив цветов для режима просмотра тектонических плит с картой высот и напряжений
pub fn build_plate_colors(geology: &GeologyLayer) -> Vec<[f32; 4]> {
    // В идеале эти значения нужно брать из вашего TectonicsConfig.
    // Используем стандартные значения из вашего конфига для расчетов.
    let base_cont_h = 0.03;   //[cite: 7]
    let base_ocean_h = -0.06; //[cite: 7]

    geology
        .plate_ids
        .iter()
        .enumerate()
        .map(|(i, &plate_id)| {
            // Вулканы всегда горят ярким оранжевым поверх всего
            if geology.volcano_mask[i] {
                return Color::srgb(1.0, 0.3, 0.0).to_linear().to_f32_array();
            }

            let Some(plate) = geology.plates.get(plate_id) else {
                return Color::srgb(1.0, 1.0, 1.0).to_linear().to_f32_array();
            };

            let dist = geology.boundary_dist[i];
            let stress = geology.boundary_stress[i];
            let cell_elev = geology.elevations[i];

            // 1. ЦВЕТ ПЛИТЫ ЗАВИСИТ ОТ ЕЁ УНИКАЛЬНОГО СМЕЩЕНИЯ (base_offset)
            let (hue, sat, base_light, expected_h) = match plate.plate_type {
                PlateType::Continental => {
                    // Переводим смещение (-0.05..0.08) в нормализованное t от 0.0 до 1.0
                    let t = ((plate.base_offset + 0.05) / 0.13).clamp(0.0, 1.0);
                    // Градиент: от Зеленого (120.0) для низменностей до Оранжевого (30.0) для плато
                    let h = 120.0 - t * 90.0;
                    (h, 0.65, 0.45, base_cont_h + plate.base_offset)
                }
                PlateType::Oceanic => {
                    // Переводим смещение (-0.01..0.01) в t от 0.0 до 1.0
                    let t = ((plate.base_offset + 0.01) / 0.02).clamp(0.0, 1.0);
                    // Градиент: от Темно-синего (240.0) до Голубого/Циан (190.0)
                    let h = 240.0 - t * 50.0;
                    (h, 0.60, 0.35, base_ocean_h + plate.base_offset)
                }
            };

            // 2. РЕЛЬЕФНЫЙ ШЕЙДИНГ (Светотень)
            // Смотрим, насколько этот конкретный гекс выше или ниже "нормальной" высоты своей плиты
            let elev_diff = cell_elev - expected_h;
            // Множитель 4.0 делает горы явно светлее, а впадины темнее
            let relief_mod = (elev_diff * 4.0).clamp(-0.35, 0.45); 
            
            // Применяем шейдинг к базовой яркости
            let mut light = (base_light + relief_mod).clamp(0.05, 0.95);

            // Мягко затемняем самые центры кратонов для глобального объема
            let center_darkness = (dist / 0.5).clamp(0.0, 1.0);
            light *= 1.0 - (center_darkness * 0.15);

            let plate_color = Color::hsl(hue, sat, light).to_linear().to_f32_array();

            // 3. ОТРИСОВКА ГРАНИЦ И ГРАДИЕНТА (Красный, Голубой, Белый)
            let boundary_color = if stress > 0.02 {
                Color::srgb(1.0, 0.15, 0.15).to_linear().to_f32_array() // Сжатие - Красный
            } else if stress < -0.02 {
                Color::srgb(0.0, 0.8, 1.0).to_linear().to_f32_array() // Растяжение - Голубой
            } else {
                Color::srgb(0.9, 0.9, 0.9).to_linear().to_f32_array() // Сдвиг - Белый
            };

            let edge_thickness = 0.12; 
            let t_dist = (dist / edge_thickness).clamp(0.0, 1.0);
            let mix_factor = (1.0 - t_dist).powi(2);

            let r = plate_color[0] * (1.0 - mix_factor) + boundary_color[0] * mix_factor;
            let g = plate_color[1] * (1.0 - mix_factor) + boundary_color[1] * mix_factor;
            let b = plate_color[2] * (1.0 - mix_factor) + boundary_color[2] * mix_factor;

            [r, g, b, 1.0]
        })
        .collect()
}

pub fn elevation_to_color(h: f32) -> Color {
    match h {
        x if x <= -0.44 => Color::srgb(0.05, 0.02, 0.01), // почти чёрный кофе
        x if x <= -0.16 => Color::srgb(0.15, 0.08, 0.04), // тёмный шоколад
        x if x < -0.02 => Color::srgb(0.30, 0.17, 0.09),  // горький шоколад
        x if x < 0.01 => Color::srgb(0.50, 0.32, 0.17),   // карамель (было 0.035)
        x if x < 0.12 => Color::srgb(0.46, 0.29, 0.15),   // средний коричневый — равнина (светлее)
        x if x < 0.32 => Color::srgb(0.62, 0.40, 0.20),   // тёплый орех (было 0.18)
        x if x < 0.50 => Color::srgb(0.78, 0.46, 0.19),   // охра (было 0.4)
        x if x < 0.64 => Color::srgb(0.87, 0.33, 0.12),   // кирпич (было 0.55)
        _ => Color::srgb(0.95, 0.18, 0.10),               // раскалённый красный (было 0.7)
    }
}

pub fn draw_tectonics_tab(
    ui: &mut egui::Ui,
    config: &mut TectonicsConfig,
    pipeline: &mut WorldPipeline,
) {
    ui.heading("Настройки Тектоники");
    ui.separator();

    // Кнопка регенерации (выделенная цветом)
    let regen_btn = ui.add_sized(
        [ui.available_width(), 40.0],
        egui::Button::new("Сгенерировать ландшафт"),
    );
    if regen_btn.clicked() {
        pipeline.regenerate_tectonics = true;
    }

    ui.separator();
    ui.label("Базовые параметры:");

    // Ползунки напрямую меняют значения в config
    ui.add(egui::Slider::new(&mut config.num_plates, 5..=50).text("Кол-во плит"));
    ui.add(egui::Slider::new(&mut config.continental_ratio, 0.0..=1.0).text("Доля континентов"));

    ui.separator();
    ui.label("Высоты и океаны:");
    ui.add(egui::Slider::new(&mut config.cont_base_height, 0.0..=0.5).text("Высота суши"));
    ui.add(egui::Slider::new(&mut config.ocean_base_height, -1.0..=0.0).text("Глубина океана"));

    ui.separator();
    ui.label("Шум и искажения:");
    ui.add(egui::Slider::new(&mut config.warp_strength, 0.0..=1.0).text("Сила искажения границ"));
    ui.add(egui::Slider::new(&mut config.boundary_noise_amp, 0.0..=3.0).text("Амплитуда гор"));
    ui.add(
        egui::Slider::new(&mut config.detail_noise_amp, 0.0..=1.0)
            .text("Мелкий шум (шероховатость)"),
    );
}
