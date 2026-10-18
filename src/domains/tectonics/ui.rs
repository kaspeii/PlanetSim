use super::{config::TectonicsConfig, data::GeologyLayer};
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

/// Генерирует массив цветов для режима просмотра тектонических плит (переключение по Tab)
pub fn build_plate_colors(geology: &GeologyLayer) -> Vec<[f32; 4]> {
    let total_plates = geology.plates.len().max(1) as f32;
    geology
        .plate_ids
        .iter()
        .map(|&plate_id| {
            let hue = (plate_id as f32 / total_plates) * 360.0;
            Color::hsl(hue, 0.65, 0.5).to_linear().to_f32_array()
        })
        .collect()
}

pub fn elevation_to_color(h: f32) -> Color {
    match h {
        x if x <= -0.45 => Color::srgb(0.0, 0.03, 0.12),
        x if x <= -0.18 => Color::srgb(0.01, 0.1, 0.3),
        x if x < 0.0 => Color::srgb(0.05, 0.25, 0.5),
        x if x < 0.035 => Color::srgb(0.85, 0.75, 0.5),
        x if x < 0.18 => Color::srgb(0.2, 0.45, 0.15),
        x if x < 0.4 => Color::srgb(0.4, 0.35, 0.3),
        x if x < 0.6 => Color::srgb(0.3, 0.25, 0.2),
        _ => Color::srgb(0.95, 0.95, 1.0),
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

    ui.separator();
    ui.label("Сглаживание:");
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut config.diffusion_strength, 0.0..=1.0).text("Сила диффузии"));
        if ui.button("Шаг сглаживания").clicked() {
            pipeline.step_tectonic_diffusion = true;
        }
    });
}
