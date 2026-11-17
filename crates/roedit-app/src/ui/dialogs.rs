//! Modal windows: settings, shortcut editor, export/render, import, recovery,
//! about and the transient toast overlay.

use super::{rgb, section};
use crate::app::RoEditApp;
use crate::command::{binding_for, Binding};
use eframe::egui;

pub fn dialogs(app: &mut RoEditApp, ctx: &egui::Context) {
    settings_window(app, ctx);
    shortcuts_window(app, ctx);
    about_window(app, ctx);
    export_window(app, ctx);
    import_window(app, ctx);
    recovery_window(app, ctx);
    toast(app, ctx);
}

fn settings_window(app: &mut RoEditApp, ctx: &egui::Context) {
    if !app.ui.show_settings {
        return;
    }
    let mut open = true;
    egui::Window::new("Settings")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(460.0)
        .show(ctx, |ui| {
            section(ui, "Appearance", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Theme");
                    let current = app.settings.theme.clone();
                    egui::ComboBox::from_id_salt("theme-select")
                        .selected_text(current.clone())
                        .show_ui(ui, |ui| {
                            for theme in roedit_core::settings::Theme::presets() {
                                if ui
                                    .selectable_label(current == theme.name, &theme.name)
                                    .clicked()
                                    && app.settings.theme != theme.name
                                {
                                    app.settings.theme = theme.name.clone();
                                    crate::ui::theme::apply(ctx, &app.settings);
                                }
                            }
                        });
                });
                if ui
                    .add(
                        egui::Slider::new(&mut app.settings.ui_scale, 0.75..=2.0)
                            .text("UI scale")
                            .step_by(0.05),
                    )
                    .changed()
                {
                    ctx.set_pixels_per_point(app.settings.ui_scale);
                }
                ui.checkbox(
                    &mut app.settings.reduce_motion,
                    "Reduce motion (disable animations)",
                );
                ui.checkbox(
                    &mut app.settings.low_memory_mode,
                    "Low memory mode (fewer cached previews)",
                );
            });
            ui.separator();
            section(ui, "Files and recovery", |ui| {
                ui.checkbox(&mut app.settings.autosave_enabled, "Autosave");
                ui.add(
                    egui::DragValue::new(&mut app.settings.autosave_interval_secs)
                        .range(15..=3600)
                        .prefix("every ")
                        .suffix(" s"),
                );
                ui.horizontal(|ui| {
                    ui.label("Undo steps");
                    let mut limit = app.settings.max_undo_steps as u32;
                    if ui
                        .add(egui::DragValue::new(&mut limit).range(16..=2000))
                        .changed()
                    {
                        app.settings.max_undo_steps = limit as usize;
                    }
                    if ui.button("Apply history limit").clicked() {
                        app.doc.set_history_limit(app.settings.max_undo_steps);
                    }
                });
                ui.label(
                    egui::RichText::new(format!(
                        "Autosave file: {}",
                        crate::document::Document::autosave_path().display()
                    ))
                    .small()
                    .color(super::theme::text_dim(&app.theme())),
                );
            });
            ui.separator();
            section(ui, "Snapping and performance", |ui| {
                ui.add(
                    egui::DragValue::new(&mut app.settings.grid_snap)
                        .range(0.01..=10.0)
                        .speed(0.01)
                        .prefix("Grid snap "),
                );
                ui.add(
                    egui::DragValue::new(&mut app.settings.angle_snap_deg)
                        .range(1.0..=90.0)
                        .prefix("Angle snap ")
                        .suffix("°"),
                );
                ui.add(
                    egui::DragValue::new(&mut app.settings.render_threads)
                        .range(0..=256)
                        .prefix("Render threads (0 = auto) "),
                );
            });
            ui.separator();
            section(ui, "Extensions", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.settings.theme)
                        .desired_width(0.0)
                        .interactive(false),
                );
                ui.horizontal(|ui| {
                    ui.label("Plugin folder");
                    ui.add(egui::TextEdit::singleline(&mut app.ui.plugin_dir).desired_width(260.0));
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save settings").clicked() {
                    match app.settings.save() {
                        Ok(()) => {
                            app.ui.log("success", "Settings saved");
                            app.ui.toast("Settings saved");
                        }
                        Err(err) => app
                            .ui
                            .log("error", format!("Could not save settings: {err}")),
                    }
                }
                if ui.button("Reset shortcuts").clicked() {
                    app.settings.reset_shortcuts();
                    app.ui.toast("Shortcuts reset to defaults");
                }
                if ui.button("Close").clicked() {
                    app.ui.show_settings = false;
                }
            });
        });
    if !open {
        app.ui.show_settings = false;
    }
}

fn shortcuts_window(app: &mut RoEditApp, ctx: &egui::Context) {
    if !app.ui.show_shortcuts {
        return;
    }
    let mut open = true;
    egui::Window::new("Keyboard shortcuts")
        .open(&mut open)
        .resizable(true)
        .default_width(560.0)
        .default_height(460.0)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(
                    "Click a binding and type a new one, for example Ctrl+Shift+K, F9 or Alt+E. Invalid bindings are rejected on focus loss.",
                )
                .small()
                .color(super::theme::text_dim(&app.theme())),
            );
            let rows: Vec<(String, String, String, String)> = app
                .commands
                .iter()
                .map(|c| {
                    (
                        c.id.clone(),
                        c.label.clone(),
                        c.category.to_string(),
                        binding_for(c, &app.settings).unwrap_or("").to_string(),
                    )
                })
                .collect();
            for (id, label, category, current) in rows {
                if !app.scratch.shortcut_buffers.contains_key(&id) {
                    app.scratch
                        .shortcut_buffers
                        .insert(id.clone(), current.clone());
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{category}: {label}")).small());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let buffer = app.scratch.shortcut_buffers.get_mut(&id).unwrap();
                        let response = ui.add(
                            egui::TextEdit::singleline(buffer)
                                .desired_width(120.0)
                                .font(egui::TextStyle::Monospace),
                        );
                        if response.lost_focus() {
                            let text = buffer.trim().to_string();
                            if text.is_empty() {
                                app.settings.shortcuts.remove(&id);
                                app.ui.log("info", format!("Cleared shortcut for {label}"));
                            } else if Binding::parse(&text).is_some() {
                                app.settings.set_binding(&id, &text);
                                app.ui.log("info", format!("{label} → {text}"));
                            } else {
                                app.ui.toast(format!("'{text}' is not a valid shortcut"));
                                app.scratch.shortcut_buffers.insert(id.clone(), current.clone());
                            }
                        }
                    });
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Reset all").clicked() {
                    app.settings.reset_shortcuts();
                    app.scratch.shortcut_buffers.clear();
                }
                if ui.button("Save").clicked() {
                    match app.settings.save() {
                        Ok(()) => app.ui.toast("Shortcuts saved"),
                        Err(err) => app.ui.log("error", format!("{err}")),
                    }
                }
            });
        });
    if !open {
        app.ui.show_shortcuts = false;
    }
}

fn about_window(app: &mut RoEditApp, ctx: &egui::Context) {
    if !app.ui.show_about {
        return;
    }
    let mut open = true;
    egui::Window::new("About RoEdit Studio")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(480.0)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new("RoEdit Studio").strong().size(18.0));
            ui.label(format!(
                "Version {} · native Rust desktop suite",
                env!("CARGO_PKG_VERSION")
            ));
            ui.add_space(6.0);
            ui.label(
                "Modelling, sculpting, UV and texture tools, Roblox R6/R15 rigging, keyframe animation, a CPU path tracer, GFX compositing and a video timeline — all offline, with no AI features.",
            );
            ui.add_space(6.0);
            section(ui, "Honest limitations", |ui| {
                for line in [
                    "Video is exported as image sequences (PNG/JPEG/TIFF) plus a WAV mix; MP4/MOV/MP3 are not bundled.",
                    "FBX and SVG import are not implemented; use OBJ, STL or GLB.",
                    "Extension packs are data-driven JSON macros and cannot load native libraries.",
                    "The 3D viewport preview is a CPU path tracer; the GPU viewport path requires a DirectX 12 capable adapter.",
                ] {
                    ui.label(egui::RichText::new(format!("• {line}")).small());
                }
            });
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!("Project: {}", app.doc.title()))
                    .small()
                    .color(super::theme::text_dim(&app.theme())),
            );
            if ui.button("Close").clicked() {
                app.ui.show_about = false;
            }
        });
    if !open {
        app.ui.show_about = false;
    }
}

fn export_window(app: &mut RoEditApp, ctx: &egui::Context) {
    if !app.ui.show_export_dialog {
        return;
    }
    let mut open = true;
    egui::Window::new("Export and render")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(520.0)
        .show(ctx, |ui| {
            section(ui, "Image", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Path");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.ui.render.output)
                            .desired_width(300.0)
                            .hint_text("renders/still.png"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Format");
                    egui::ComboBox::from_id_salt("export-image-format")
                        .selected_text(format!("{:?}", app.ui.render.format))
                        .show_ui(ui, |ui| {
                            for f in [
                                roedit_io::SaveFormat::Png,
                                roedit_io::SaveFormat::Jpeg,
                                roedit_io::SaveFormat::Tiff,
                                roedit_io::SaveFormat::Exr,
                            ] {
                                ui.selectable_value(&mut app.ui.render.format, f, format!("{f:?}"));
                            }
                        });
                    if app.ui.render.format == roedit_io::SaveFormat::Jpeg {
                        ui.add(
                            egui::DragValue::new(&mut app.ui.render.jpeg_quality)
                                .range(10..=100)
                                .prefix("quality "),
                        );
                    }
                });
                let settings = app.doc.project.render.clone();
                ui.label(
                    egui::RichText::new(format!(
                        "{}×{} · {} samples · {} bounces · {:?}",
                        settings.width, settings.height, settings.samples, settings.max_bounces, settings.mode
                    ))
                    .small()
                    .color(super::theme::text_dim(&app.theme())),
                );
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(app.job.is_none(), egui::Button::new("Render still"))
                        .clicked()
                    {
                        app.dispatch(crate::command::Action::RenderStill);
                    }
                    if ui.button("Render preview (F5)").clicked() {
                        app.dispatch(crate::command::Action::QuickPreview);
                    }
                    if ui.button("Flatten GFX to PNG").clicked() {
                        app.dispatch(crate::command::Action::GfxFlatten);
                    }
                });
            });
            ui.separator();
            section(ui, "Mesh", |ui| {
                ui.label(
                    egui::RichText::new(
                        "The selected object's mesh is written out. OBJ includes a matching MTL file.",
                    )
                    .small()
                    .color(super::theme::text_dim(&app.theme())),
                );
                ui.horizontal(|ui| {
                    ui.label("Base path");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.ui.io_path)
                            .desired_width(280.0)
                            .hint_text("exports/model"),
                    );
                });
                ui.horizontal(|ui| {
                    for ext in ["obj", "stl", "glb"] {
                        if ui.small_button(ext.to_uppercase()).clicked() {
                            let mut path = app.ui.io_path.trim().to_string();
                            if path.is_empty() {
                                path = "exports/model".into();
                            }
                            let path = std::path::Path::new(&path).with_extension(ext);
                            app.ui.io_path = path.to_string_lossy().to_string();
                            app.action_export_mesh();
                        }
                    }
                });
            });
            ui.separator();
            section(ui, "Video sequence", |ui| {
                egui::Grid::new("seq-grid").num_columns(2).show(ui, |ui| {
                    ui.label("Directory");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.ui.export.directory)
                            .desired_width(260.0),
                    );
                    ui.end_row();
                    ui.label("Prefix");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.ui.export.prefix).desired_width(260.0),
                    );
                    ui.end_row();
                    ui.label("Format");
                    egui::ComboBox::from_id_salt("seq-format")
                        .selected_text(format!("{:?}", app.ui.export.format))
                        .show_ui(ui, |ui| {
                            for f in [
                                roedit_io::SaveFormat::Png,
                                roedit_io::SaveFormat::Jpeg,
                                roedit_io::SaveFormat::Tiff,
                            ] {
                                ui.selectable_value(&mut app.ui.export.format, f, format!("{f:?}"));
                            }
                        });
                    ui.end_row();
                    ui.label("Frame range");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut app.ui.export.start_frame).range(0..=100000));
                        ui.label("to");
                        ui.add(egui::DragValue::new(&mut app.ui.export.end_frame).range(0..=100000));
                    });
                    ui.end_row();
                });
                ui.checkbox(&mut app.ui.export.write_audio, "Write mix.wav from timeline audio");
                if ui
                    .add_enabled(app.job.is_none(), egui::Button::new("Export frame sequence"))
                    .clicked()
                {
                    app.dispatch(crate::command::Action::ExportSequence);
                }
                ui.label(
                    egui::RichText::new(
                        "No MP4 encoder is bundled: combine the frames with ffmpeg externally if a video file is required.",
                    )
                    .small()
                    .color(super::theme::warning(&app.theme())),
                );
            });
            ui.separator();
            if let Some(job) = &app.job {
                ui.add(egui::ProgressBar::new(job.progress_fraction()).show_percentage());
                ui.label(
                    egui::RichText::new(format!(
                        "{} · {:.1}s elapsed",
                        job.label,
                        job.elapsed_secs()
                    ))
                    .small(),
                );
            }
        });
    if !open {
        app.ui.show_export_dialog = false;
    }
}

fn import_window(app: &mut RoEditApp, ctx: &egui::Context) {
    if !app.scratch.show_import_dialog {
        return;
    }
    let mut open = true;
    egui::Window::new("Import file")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .show(ctx, |ui| {
            ui.label("Images: PNG, JPEG, TIFF, BMP, WebP, EXR");
            ui.label("Meshes: OBJ, STL (binary/ASCII), GLB");
            ui.horizontal(|ui| {
                ui.label("Path");
                ui.add(
                    egui::TextEdit::singleline(&mut app.scratch.import_buffer)
                        .desired_width(300.0)
                        .hint_text("C:/models/chair.obj"),
                );
            });
            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    let path = app.scratch.import_buffer.clone();
                    app.scratch.show_import_dialog = false;
                    app.apply_pending_import(&path);
                }
                if ui.button("Cancel").clicked() {
                    app.scratch.show_import_dialog = false;
                }
            });
        });
    if !open {
        app.scratch.show_import_dialog = false;
    }
}

fn recovery_window(app: &mut RoEditApp, ctx: &egui::Context) {
    if !app.ui.show_recovery {
        return;
    }
    egui::Window::new("Recover autosave")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label("An autosaved project was found from a previous session.");
            ui.label(
                egui::RichText::new(
                    crate::document::Document::autosave_path()
                        .display()
                        .to_string(),
                )
                .small()
                .color(super::theme::text_dim(&app.theme())),
            );
            ui.horizontal(|ui| {
                if ui.button("Recover it").clicked() {
                    app.accept_recovery();
                }
                if ui.button("Discard").clicked() {
                    app.discard_recovery();
                }
            });
        });
}

fn toast(app: &mut RoEditApp, ctx: &egui::Context) {
    let Some(message) = app.ui.active_toast().map(|s| s.to_string()) else {
        return;
    };
    egui::Area::new(egui::Id::new("roedit-toast"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -70.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::window(ui.style())
                .fill(rgb(app.theme().panel_alt))
                .inner_margin(egui::Margin::symmetric(12.0, 6.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(message).size(12.0));
                });
        });
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
}
