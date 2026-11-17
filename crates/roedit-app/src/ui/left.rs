//! Left dock: scene hierarchy, lights, materials and recent actions.

use super::{rgb, section};
use crate::app::RoEditApp;
use crate::command::{Action, PrimitiveKind};
use eframe::egui;
use roedit_core::ids::EntityId;

fn hierarchy_node(
    app: &mut RoEditApp,
    ui: &mut egui::Ui,
    id: EntityId,
    depth: usize,
    filter: &str,
) {
    let Some(entity) = app.doc.project.scene.entity(id) else {
        return;
    };
    let name = entity.name.clone();
    let children = entity.children.clone();
    let visible = entity.visible;
    let has_mesh = entity.mesh.is_some();
    let has_light = entity.light.is_some();
    let matches = filter.is_empty() || name.to_lowercase().contains(&filter.to_lowercase());
    let has_matching_child = children.iter().any(|c| {
        app.doc
            .project
            .scene
            .entity(*c)
            .map(|e| e.name.to_lowercase().contains(&filter.to_lowercase()))
            .unwrap_or(false)
    });
    if !matches && !has_matching_child && !filter.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 12.0);
        let icon = if has_light {
            "☀"
        } else if has_mesh {
            "▣"
        } else {
            "○"
        };
        let selected = app.ui.selected == Some(id);
        if ui
            .selectable_label(selected, format!("{icon} {name}"))
            .clicked()
        {
            app.ui.selected = Some(id);
            app.preview_dirty = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button(if visible { "◉" } else { "◌" })
                .on_hover_text("Toggle visibility")
                .clicked()
            {
                app.doc.edit("Toggle visibility", |p| {
                    if let Some(e) = p.scene.entity_mut(id) {
                        e.visible = !e.visible;
                    }
                });
                app.preview_dirty = true;
            }
        });
    });
    for child in children {
        hierarchy_node(app, ui, child, depth + 1, filter);
    }
}

pub fn left_panel(app: &mut RoEditApp, ctx: &egui::Context) {
    let theme = app.theme();
    egui::SidePanel::left("roedit-left")
        .resizable(true)
        .default_width(250.0)
        .width_range(190.0..=380.0)
        .frame(
            egui::Frame::none()
                .fill(rgb(theme.panel))
                .inner_margin(egui::Margin::symmetric(8.0, 6.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Scene").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button("+ Add", |ui| {
                        for p in PrimitiveKind::ALL {
                            if ui.button(p.label()).clicked() {
                                app.dispatch(Action::AddPrimitive(p));
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        for rig in roedit_mesh::roblox::RigType::ALL {
                            if ui.button(format!("{} rig", rig.label())).clicked() {
                                app.dispatch(Action::BuildRig(rig));
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
            ui.add(
                egui::TextEdit::singleline(&mut app.ui.hierarchy_filter)
                    .hint_text("Filter objects…")
                    .desired_width(f32::INFINITY),
            );
            let roots = app.doc.project.scene.roots.clone();
            egui::ScrollArea::vertical()
                .id_salt("hierarchy-scroll")
                .max_height(ui.available_height() * 0.42)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if roots.is_empty() {
                        ui.label(
                            egui::RichText::new("Empty scene — add a primitive to begin")
                                .small()
                                .color(super::theme::text_dim(&app.theme())),
                        );
                    }
                    let filter = app.ui.hierarchy_filter.clone();
                    for root in roots {
                        hierarchy_node(app, ui, root, 0, &filter);
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .button("Focus")
                    .on_hover_text("Frame the selection (F)")
                    .clicked()
                {
                    app.dispatch(Action::FrameSelection);
                }
                if ui.button("Duplicate").clicked() {
                    app.dispatch(Action::DuplicateSelected);
                }
                if ui
                    .button(egui::RichText::new("Delete").color(super::theme::error(&app.theme())))
                    .clicked()
                {
                    app.dispatch(Action::DeleteSelected);
                }
            });

            ui.separator();
            section(ui, "Lights", |ui| {
                let lights: Vec<_> = app
                    .doc
                    .project
                    .scene
                    .lights
                    .iter()
                    .map(|(id, l)| (*id, l.name.clone(), l.kind.label(), l.enabled, l.intensity))
                    .collect();
                if lights.is_empty() {
                    ui.label("No lights yet.");
                }
                for (id, name, kind, mut enabled, intensity) in lights {
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut enabled, "").changed() {
                            app.doc.edit("Toggle light", |p| {
                                if let Some(l) = p.scene.light_mut(id) {
                                    l.enabled = enabled;
                                }
                            });
                            app.preview_dirty = true;
                        }
                        if ui
                            .selectable_label(
                                app.ui.selected_light == Some(id),
                                format!("☀ {name}"),
                            )
                            .clicked()
                        {
                            app.ui.selected_light = Some(id);
                        }
                        ui.label(
                            egui::RichText::new(format!("{kind} · {intensity:.1}"))
                                .small()
                                .color(super::theme::text_dim(&app.theme())),
                        );
                    });
                }
            });

            ui.separator();
            section(ui, "Materials", |ui| {
                let materials: Vec<_> = app
                    .doc
                    .project
                    .scene
                    .materials
                    .iter()
                    .map(|(id, m)| (*id, m.name.clone(), m.base_color))
                    .collect();
                for (id, name, color) in materials {
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, super::rgb_f(color));
                        if ui
                            .selectable_label(app.ui.selected_material == Some(id), name)
                            .clicked()
                        {
                            app.ui.selected_material = Some(id);
                        }
                        if ui
                            .small_button("Assign")
                            .on_hover_text("Assign to the selected object")
                            .clicked()
                        {
                            if let Some(entity) = app.ui.selected {
                                app.doc.edit("Assign material", |p| {
                                    if let Some(e) = p.scene.entity_mut(entity) {
                                        e.material = Some(id);
                                    }
                                });
                                app.preview_dirty = true;
                            }
                        }
                    });
                }
            });

            ui.separator();
            ui.label(
                egui::RichText::new(app.doc.stats_line())
                    .small()
                    .color(super::theme::text_dim(&app.theme())),
            );
            if let Some(label) = app.doc.undo_label() {
                ui.label(
                    egui::RichText::new(format!("Undo available: {label}"))
                        .small()
                        .color(super::theme::text_dim(&app.theme())),
                );
            }
        });
}
