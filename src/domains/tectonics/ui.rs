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

/// Генерирует массив цветов для режима просмотра тектонических плит (переключение по Tab)
pub fn build_plate_colors(geology: &GeologyLayer) -> Vec<[f32; 4]> {
    // сколько всего континентальных и океанических
    let (mut n_cont, mut n_ocean) = (0usize, 0usize);
    for p in &geology.plates {
        match p.plate_type {
            PlateType::Continental => n_cont += 1,
            PlateType::Oceanic => n_ocean += 1,
        }
    }
    let n_cont = n_cont.max(1) as f32;
    let n_ocean = n_ocean.max(1) as f32;

    // индекс плиты внутри своего типа
    let mut cont_rank = vec![0usize; geology.plates.len()];
    let mut ocean_rank = vec![0usize; geology.plates.len()];
    let (mut ci, mut oi) = (0usize, 0usize);
    for (i, p) in geology.plates.iter().enumerate() {
        match p.plate_type {
            PlateType::Continental => { cont_rank[i] = ci; ci += 1; }
            PlateType::Oceanic => { ocean_rank[i] = oi; oi += 1; }
        }
    }

    geology
        .plate_ids
        .iter()
        .enumerate()
        .map(|(i, &plate_id)| {
            if geology.volcano_mask[i] {
                return Color::srgb(0.1, 0.1, 0.1).to_linear().to_f32_array()
            }

            let Some(plate) = geology.plates.get(plate_id) else {
                return Color::srgb(1.0, 1.0, 1.0).to_linear().to_f32_array();
            };

            let (hue, sat, light) = match plate.plate_type {
                PlateType::Continental => {
                    let t = cont_rank[plate_id] as f32 / n_cont;
                    (10.0 + t * 85.0, 0.75, 0.40)
                }
                PlateType::Oceanic => {
                    let t = ocean_rank[plate_id] as f32 / n_ocean;
                    (185.0 + t * 85.0, 0.55, 0.20)
                }
            };

            Color::hsl(hue, sat, light).to_linear().to_f32_array()
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
