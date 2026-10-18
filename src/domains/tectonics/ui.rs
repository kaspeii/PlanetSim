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
        .enumerate()
        .map(|(i, &plate_id)| {
            if geology.volcano_mask[i] {
                Color::srgb(0.1, 0.1, 0.1).to_linear().to_f32_array()
            } else {
                let hue = (plate_id as f32 / total_plates) * 360.0;
                Color::hsl(hue, 0.65, 0.5).to_linear().to_f32_array()
            }
        })
        .collect()
}

pub fn elevation_to_color(h: f32) -> Color {
    match h {
        x if x <= -0.44 => Color::srgb(0.05, 0.02, 0.01), // почти чёрный кофе
        x if x <= -0.16 => Color::srgb(0.15, 0.08, 0.04), // тёмный шоколад
        x if x < -0.02  => Color::srgb(0.30, 0.17, 0.09), // горький шоколад
        x if x < 0.01   => Color::srgb(0.50, 0.32, 0.17), // карамель (было 0.035)
        x if x < 0.12   => Color::srgb(0.46, 0.29, 0.15), // средний коричневый — равнина (светлее)
        x if x < 0.32   => Color::srgb(0.62, 0.40, 0.20), // тёплый орех (было 0.18)
        x if x < 0.50   => Color::srgb(0.78, 0.46, 0.19), // охра (было 0.4)
        x if x < 0.64   => Color::srgb(0.87, 0.33, 0.12), // кирпич (было 0.55)
        _               => Color::srgb(0.95, 0.18, 0.10), // раскалённый красный (было 0.7)
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
