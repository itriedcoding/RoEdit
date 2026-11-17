//! Fuzzy command palette (Ctrl+K): every registered command, searchable and
//! executable with the keyboard.

use crate::app::RoEditApp;
use crate::command::{binding_for, search_commands, Action};
use eframe::egui;

pub fn palette(app: &mut RoEditApp, ctx: &egui::Context) {
    if !app.ui.palette_open {
        return;
    }
    let mut open = true;
    let mut chosen: Option<Action> = None;
    let mut close = false;

    egui::Window::new("Command palette")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 90.0))
        .default_width(560.0)
        .show(ctx, |ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut app.ui.palette_query)
                    .hint_text("Type a command — bevel, rig, export, render…")
                    .desired_width(f32::INFINITY),
            );
            if !response.has_focus() && !ui.memory(|m| m.focused().is_some()) {
                response.request_focus();
            }
            let results = search_commands(&app.commands, &app.ui.palette_query);
            let count = results.len();
            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                app.ui.palette_index = (app.ui.palette_index + 1) % count.max(1);
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                app.ui.palette_index = app
                    .ui
                    .palette_index
                    .checked_sub(1)
                    .unwrap_or(count.saturating_sub(1));
            }
            if count == 0 {
                ui.label("No matching command.");
            }
            let rows: Vec<(String, String, String, &'static str, Action)> = results
                .iter()
                .map(|c| {
                    (
                        c.label.clone(),
                        c.category.to_string(),
                        binding_for(c, &app.settings).unwrap_or("").to_string(),
                        c.category,
                        c.action.clone(),
                    )
                })
                .collect();
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            egui::ScrollArea::vertical()
                .max_height(380.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (i, (label, category, key, _, action)) in rows.iter().enumerate() {
                        let selected = i == app.ui.palette_index % rows.len().max(1);
                        let text = format!("{label}   ·   {category}");
                        let response = ui.selectable_label(selected, text);
                        if selected {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }
                        if response.clicked() {
                            chosen = Some(action.clone());
                            close = true;
                        }
                        if !key.is_empty() {
                            ui.label(
                                egui::RichText::new(key)
                                    .small()
                                    .monospace()
                                    .color(super::theme::text_dim(&app.theme())),
                            );
                        }
                        if selected && enter {
                            chosen = Some(action.clone());
                            close = true;
                        }
                    }
                });
            ui.label(
                egui::RichText::new(format!(
                    "{count} command(s) · ↑↓ to move · Enter to run · Esc to close"
                ))
                .small()
                .color(super::theme::text_dim(&app.theme())),
            );
        });

    if let Some(action) = chosen {
        app.ui.palette_open = false;
        app.dispatch(action);
    } else if !open || close {
        app.ui.palette_open = false;
    }
}
