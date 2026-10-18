// src/ui/mod.rs

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

use crate::{
    domains::{
        climate::{config::ClimateConfig, data::ClimateLayer, ui::draw_climate_tab}, tectonics::{config::TectonicsConfig, ui::draw_tectonics_tab},
    }, viewport::MapOverlay, world::pipeline::WorldPipeline,
};

#[derive(Resource, Default, PartialEq)]
pub enum ActiveTab {
    #[default]
    Tectonics,
    Climate,
    Hydrology,
    Civilization,
}

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }

        app.init_resource::<ActiveTab>()
            .add_systems(EguiPrimaryContextPass, render_ui_shell);
    }
}

fn render_ui_shell(
    mut contexts: EguiContexts,
    mut active_tab: ResMut<ActiveTab>,
    mut tectonics_config: ResMut<TectonicsConfig>,
    mut climate_config: ResMut<ClimateConfig>,
    mut climate: ResMut<ClimateLayer>, 
    mut pipeline: ResMut<WorldPipeline>,
    mut overlay: ResMut<MapOverlay>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    let rect = ctx.content_rect();

    egui::Area::new(egui::Id::new("root_ui_shell"))
        .fixed_pos(rect.min)
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());

            // === НИЖНЯЯ ПАНЕЛЬ (Вкладки / Taskbar) ===
            egui::Panel::bottom("bottom_panel")
                .exact_size(40.0)
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.selectable_value(&mut *active_tab, ActiveTab::Tectonics, "Тектоника");
                        ui.selectable_value(&mut *active_tab, ActiveTab::Climate, "Климат");
                        ui.selectable_value(&mut *active_tab, ActiveTab::Hydrology, "Гидрология");
                        ui.selectable_value(
                            &mut *active_tab,
                            ActiveTab::Civilization,
                            "Цивилизации",
                        );
                    });
                });

            // === ЛЕВАЯ ПАНЕЛЬ (Режимы отображения) ===
            egui::Panel::left("left_panel")
                .default_size(200.0)
                .show(ui, |ui| {
                    ui.heading("Отображение");
                    ui.separator();

                    ui.label("Слои карты:");
                    let mut changed = false;

                    changed |= ui
                        .radio_value(&mut *overlay, MapOverlay::SpaceView, "Базовый")
                        .changed();
                    changed |= ui
                        .radio_value(&mut *overlay, MapOverlay::Elevation, "Рельеф")
                        .changed();
                    changed |= ui
                        .radio_value(
                            &mut *overlay,
                            MapOverlay::TectonicPlates,
                            "Тектонические плиты",
                        )
                        .changed();
                    changed |= ui
                        .radio_value(&mut *overlay, MapOverlay::Temperature, "Температура")
                        .changed();

                    changed |= ui
                        .radio_value(&mut *overlay, MapOverlay::Albedo, "Альбедо")
                        .changed();

                    changed |= ui
                        .radio_value(
                            &mut *overlay,
                            MapOverlay::Continentality,
                            "Континентальность",
                        )
                        .changed();

                    changed |= ui
                        .radio_value(&mut *overlay, MapOverlay::Pressure, "Ячейки давления")
                        .changed();

                    changed |= ui
                        .radio_value(&mut *overlay, MapOverlay::Winds, "Ветра")
                        .changed();

                    if changed {
                        pipeline.recolor_mesh = true;
                    }
                });

            // === ПРАВАЯ ПАНЕЛЬ (Настройки генерации) ===
            egui::Panel::right("right_panel")
                .default_size(300.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| match *active_tab {
                        ActiveTab::Tectonics => {
                            draw_tectonics_tab(ui, &mut tectonics_config, &mut pipeline)
                        }
                        ActiveTab::Climate => {
                            draw_climate_tab(ui, &mut climate_config, &mut climate, &mut pipeline)
                        }
                        ActiveTab::Hydrology => {
                            ui.label("Настройки рек (в разработке)");
                        }
                        ActiveTab::Civilization => {
                            ui.label("Настройки цивилизаций (в разработке)");
                        }
                    });
                });
        });

    Ok(())
}
