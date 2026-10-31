// src/ui/overlays.rs

use bevy::prelude::*;
use bevy_egui::{EguiContext, egui};

use crate::domains::{generator::GenerationTask, save::SaveLoadTask};

/// Отрисовать строки «лога задач» (одинаковы для генерации и сейв/лоада).
macro_rules! draw_task_entries {
    ($ui:expr, $width:expr, $entries:expr) => {{
        for entry in $entries {
            $ui.horizontal(|ui| {
                ui.set_width($width);
                if let Some(ms) = entry.duration_ms {
                    ui.label(
                        egui::RichText::new("✔").color(egui::Color32::from_rgb(80, 210, 120)),
                    );
                    ui.label(
                        egui::RichText::new(&entry.message).color(egui::Color32::from_gray(200)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("{} мс", ms)).weak().monospace());
                    });
                } else {
                    let live_ms = entry.started_at.elapsed().as_millis();
                    ui.spinner();
                    ui.label(
                        egui::RichText::new(&entry.message)
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("{} мс...", live_ms))
                                .color(egui::Color32::from_rgb(240, 200, 80))
                                .monospace(),
                        );
                    });
                }
            });
            $ui.add_space(2.0);
        }
    }};
}

/// Неблокирующий виджет лога генерации.
pub fn generation_progress_overlay(
    mut contexts: Query<&mut EguiContext>,
    tasks: Query<&GenerationTask>,
) -> Result {
    let Some(task) = tasks.iter().next() else {
        return Ok(());
    };
    let Ok(mut egui_ctx) = contexts.single_mut() else {
        return Ok(());
    };

    let snap = task.logger.snapshot();
    let progress = snap.progress();

    let (align, offset) = if task.is_initial {
        (egui::Align2::CENTER_CENTER, [0.0, 0.0])
    } else {
        (egui::Align2::CENTER_TOP, [0.0, 42.0])
    };

    let content_width = 440.0;

    egui::Window::new("generation_status")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(align, offset)
        .fixed_size([content_width, 0.0])
        .show(egui_ctx.get_mut(), |ui| {
            ui.set_width(content_width);
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.set_width(content_width);
                if snap.is_finished {
                    ui.label(
                        egui::RichText::new("✔")
                            .color(egui::Color32::from_rgb(80, 210, 120))
                            .strong(),
                    );
                    ui.strong("Генерация мира завершена!");
                } else {
                    ui.spinner();
                    ui.strong("Генерация мира...");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "Шаг {}/{}  •  {:.2} с",
                            snap.completed_steps.min(snap.total_steps),
                            snap.total_steps,
                            snap.total_elapsed_ms as f32 / 1000.0
                        ))
                        .weak()
                        .monospace(),
                    );
                });
            });

            ui.separator();
            ui.add_space(2.0);

            draw_task_entries!(ui, content_width, &snap.entries);

            ui.add_space(6.0);
            ui.add(
                egui::ProgressBar::new(progress)
                    .show_percentage()
                    .desired_width(content_width),
            );
            ui.add_space(4.0);
        });

    Ok(())
}

/// Неблокирующий оверлей сохранения / загрузки планеты.
pub fn save_load_progress_overlay(
    mut contexts: Query<&mut EguiContext>,
    tasks: Query<&SaveLoadTask>,
) -> Result {
    let Some(task) = tasks.iter().next() else {
        return Ok(());
    };
    let Ok(mut egui_ctx) = contexts.single_mut() else {
        return Ok(());
    };

    let snap = task.logger.snapshot();
    let progress = snap.progress();

    let (align, offset) = if task.is_initial && !task.is_saving {
        (egui::Align2::CENTER_CENTER, [0.0, 0.0])
    } else {
        (egui::Align2::CENTER_TOP, [0.0, 42.0])
    };

    let content_width = 440.0;

    egui::Window::new("save_load_status")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(align, offset)
        .fixed_size([content_width, 0.0])
        .show(egui_ctx.get_mut(), |ui| {
            ui.set_width(content_width);
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.set_width(content_width);
                if let Some(ref err) = snap.error {
                    ui.label(
                        egui::RichText::new("✖")
                            .color(egui::Color32::from_rgb(230, 90, 90))
                            .strong(),
                    );
                    ui.strong(format!("Ошибка: {}", err));
                } else if snap.is_finished {
                    ui.label(
                        egui::RichText::new("✔")
                            .color(egui::Color32::from_rgb(80, 210, 120))
                            .strong(),
                    );
                    let done_text = if task.is_saving {
                        format!("{} — готово!", task.title)
                    } else {
                        format!("{} — завершена!", task.title)
                    };
                    ui.strong(done_text);
                } else {
                    ui.spinner();
                    let icon = if task.is_saving { "💾" } else { "📂" };
                    ui.strong(format!("{} {}...", icon, task.title));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "Шаг {}/{}  •  {:.2} с",
                            snap.completed_steps.min(snap.total_steps),
                            snap.total_steps,
                            snap.total_elapsed_ms as f32 / 1000.0
                        ))
                        .weak()
                        .monospace(),
                    );
                });
            });

            ui.separator();
            ui.add_space(2.0);

            draw_task_entries!(ui, content_width, &snap.entries);

            ui.add_space(6.0);
            ui.add(
                egui::ProgressBar::new(progress)
                    .show_percentage()
                    .desired_width(content_width),
            );
            ui.add_space(4.0);
        });

    Ok(())
}