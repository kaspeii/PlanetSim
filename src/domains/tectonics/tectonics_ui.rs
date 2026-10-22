// /src/domains/tectonics/tectonics_ui.rs

use bevy::prelude::*;
use bevy_egui::egui;
use crate::{world::pipeline::PipelineStage, domains::generator::GenerationRequest};


// Импортируем приватный конфиг только внутри своего модуля tectonics!
use super::tectonics_config::TectonicsConfig;

pub fn draw_tectonics_tab_system(
    InMut(ui): InMut<'_, egui::Ui>,
    mut config: ResMut<TectonicsConfig>,
    mut gen_req: ResMut<GenerationRequest>,
) {
 ui.heading("Настройки Тектоники");
    ui.separator();

    // Кнопка регенерации (выделенная цветом)
    let regen_btn = ui.add_sized(
        [ui.available_width(), 40.0],
        egui::Button::new("Сгенерировать ландшафт"),
    );
    if regen_btn.clicked() {
        gen_req.request(PipelineStage::Tectonics);
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