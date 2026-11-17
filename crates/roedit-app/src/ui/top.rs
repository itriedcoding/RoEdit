//! Menu bar, workspace tabs, tool strip and transport.

use super::{num, rgb};
use crate::app::RoEditApp;
use crate::command::{binding_for, Action};
use crate::state::{Tool, Workspace};
use eframe::egui;
use roedit_core::material::Material;

fn shortcut_text(app: &RoEditApp, action: &Action) -> String {
    app.commands
        .iter()
        .find(|c| &c.action == action)
        .and_then(|c| binding_for(c, &app.settings))
        .map(|s| format!("  ({s})"))
        .unwrap_or_default()
}

fn menu_action(ui: &mut egui::Ui, app: &mut RoEditApp, label: &str, action: Action) {
    let hint = shortcut_text(app, &action);
    if ui.button(format!("{label}{hint}")).clicked() {
        app.dispatch(action);
        ui.close_menu();
    }
}

pub fn top_bar(app: &mut RoEditApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("roedit-top")
        .frame(
            egui::Frame::none()
                .fill(rgb(app.theme().window))
                .inner_margin(egui::Margin::symmetric(6.0, 4.0)),
        )
        .show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    menu_action(ui, app, "New Project", Action::NewProject);
                    menu_action(ui, app, "Open Project…", Action::OpenProject);
                    ui.separator();
                    menu_action(ui, app, "Save", Action::Save);
                    menu_action(ui, app, "Save As…", Action::SaveAs);
                    ui.separator();
                    menu_action(ui, app, "Import File…", Action::ShowImport);
                    menu_action(ui, app, "Export…", Action::ShowExport);
                    ui.separator();
                    menu_action(ui, app, "Settings", Action::ShowSettings);
                    menu_action(ui, app, "Quit", Action::Quit);
                });
                ui.menu_button("Edit", |ui| {
                    let undo = app
                        .doc
                        .undo_label()
                        .map(|l| format!("Undo {l}"))
                        .unwrap_or_else(|| "Undo".into());
                    menu_action(ui, app, &undo, Action::Undo);
                    let redo = app
                        .doc
                        .history
                        .redo_label()
                        .map(|l| format!("Redo {l}"))
                        .unwrap_or_else(|| "Redo".into());
                    menu_action(ui, app, &redo, Action::Redo);
                    ui.separator();
                    menu_action(ui, app, "Duplicate", Action::DuplicateSelected);
                    menu_action(ui, app, "Delete", Action::DeleteSelected);
                    ui.separator();
                    menu_action(ui, app, "Command Palette", Action::TogglePalette);
                });
                ui.menu_button("Create", |ui| {
                    ui.menu_button("Primitives", |ui| {
                        for p in crate::command::PrimitiveKind::ALL {
                            if ui.button(p.label()).clicked() {
                                app.dispatch(Action::AddPrimitive(p));
                                ui.close_menu();
                            }
                        }
                    });
                    ui.menu_button("Roblox Rig", |ui| {
                        for rig in roedit_mesh::roblox::RigType::ALL {
                            if ui
                                .button(format!("{} ({} parts)", rig.label(), rig.part_count()))
                                .clicked()
                            {
                                app.dispatch(Action::BuildRig(rig));
                                ui.close_menu();
                            }
                        }
                    });
                    ui.menu_button("Light", |ui| {
                        for kind in [
                            roedit_core::scene::LightKind::Point,
                            roedit_core::scene::LightKind::Spot,
                            roedit_core::scene::LightKind::Directional,
                        ] {
                            if ui.button(kind.label()).clicked() {
                                app.dispatch(Action::AddLight(kind));
                                ui.close_menu();
                            }
                        }
                    });
                    ui.menu_button("Material Preset", |ui| {
                        for (i, (name, m)) in Material::presets().into_iter().enumerate() {
                            let color = ui.style().visuals.widgets.inactive.bg_fill;
                            let _ = color;
                            if ui
                                .horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(10.0, 10.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter()
                                        .rect_filled(rect, 2.0, super::rgb_f(m.base_color));
                                    ui.button(name).clicked()
                                })
                                .inner
                            {
                                app.dispatch(Action::AddPreset(i));
                                ui.close_menu();
                            }
                        }
                    });
                });
                ui.menu_button("Mesh", |ui| {
                    for op in crate::command::MeshOpKind::ALL {
                        let hint = if op.unit().is_empty() {
                            String::new()
                        } else {
                            format!("  ({} {})", num(op.default_amount()), op.unit())
                        };
                        if ui.button(format!("{}{hint}", op.label())).clicked() {
                            app.dispatch(Action::MeshOp(op));
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("UV", |ui| {
                    for uv in crate::command::UvKind::ALL {
                        if ui.button(uv.label()).clicked() {
                            app.dispatch(Action::ApplyUv(uv));
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("Render", |ui| {
                    menu_action(ui, app, "Render Still", Action::RenderStill);
                    menu_action(ui, app, "Refresh Preview", Action::QuickPreview);
                    ui.separator();
                    for mode in roedit_core::render_settings::RenderMode::ALL {
                        menu_action(ui, app, mode.label(), Action::SetRenderMode(mode));
                    }
                    ui.separator();
                    for q in roedit_core::render_settings::RenderQuality::ALL {
                        menu_action(ui, app, q.label(), Action::SetQuality(q));
                    }
                });
                ui.menu_button("Video", |ui| {
                    menu_action(ui, app, "Add Colour Clip", Action::TimelineAddColor);
                    menu_action(ui, app, "Add Text Clip", Action::TimelineAddText);
                    menu_action(ui, app, "Split Clip", Action::TimelineSplit);
                    menu_action(ui, app, "Delete Clip", Action::TimelineDeleteClip);
                    menu_action(ui, app, "Add Marker", Action::TimelineMarker);
                    ui.separator();
                    menu_action(ui, app, "Export Sequence…", Action::ExportSequence);
                });
                ui.menu_button("View", |ui| {
                    menu_action(ui, app, "Frame Selection", Action::FrameSelection);
                    menu_action(ui, app, "Reset Camera", Action::ResetCamera);
                    menu_action(ui, app, "Toggle Grid", Action::ToggleGrid);
                    menu_action(ui, app, "Toggle Wireframe", Action::ToggleWireframe);
                    menu_action(ui, app, "Toggle Stats", Action::ToggleStats);
                });
                ui.menu_button("Help", |ui| {
                    menu_action(ui, app, "Keyboard Shortcuts", Action::ShowShortcuts);
                    menu_action(ui, app, "About RoEdit Studio", Action::ShowAbout);
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(job) = &app.job {
                        ui.add(
                            egui::ProgressBar::new(job.progress_fraction())
                                .desired_width(150.0)
                                .show_percentage(),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · {:.1}s",
                                job.label,
                                job.elapsed_secs()
                            ))
                            .small()
                            .color(super::theme::text_dim(&app.theme())),
                        );
                    } else if app.preview.busy() {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Preview")
                                .small()
                                .color(super::theme::text_dim(&app.theme())),
                        );
                    }
                    if ui
                        .button("⌘ Palette")
                        .on_hover_text("Search every command (Ctrl+K)")
                        .clicked()
                    {
                        app.dispatch(Action::TogglePalette);
                    }
                    if app.doc.dirty {
                        ui.label(
                            egui::RichText::new("● unsaved")
                                .small()
                                .color(super::theme::warning(&app.theme())),
                        );
                    }
                });
            });

            ui.add_space(3.0);
            ui.horizontal(|ui| {
                for w in Workspace::ALL {
                    let selected = app.workspace == w;
                    if ui
                        .selectable_label(selected, egui::RichText::new(w.short_label()).size(12.0))
                        .on_hover_text(format!("{} — {}", w.label(), w.description()))
                        .clicked()
                    {
                        app.set_workspace(w);
                    }
                }
                ui.separator();
                for t in Tool::ALL {
                    if ui
                        .selectable_label(app.ui.tool == t, t.label())
                        .on_hover_text(t.hint())
                        .clicked()
                    {
                        app.dispatch(Action::Tool(t));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut app.ui.show_grid, "Grid");
                    ui.checkbox(&mut app.ui.show_wireframe, "Wireframe");
                    ui.checkbox(&mut app.ui.show_stats, "Stats");
                });
            });
        });
}
