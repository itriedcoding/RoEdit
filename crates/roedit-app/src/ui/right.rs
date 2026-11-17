//! Right dock: object inspector, material editor, environment and render settings.

use super::{num, rgb, section};
use crate::app::RoEditApp;
use eframe::egui;
use glam::Vec3;

pub fn right_panel(app: &mut RoEditApp, ctx: &egui::Context) {
    let theme = app.theme();
    egui::SidePanel::right("roedit-right")
        .resizable(true)
        .default_width(300.0)
        .width_range(240.0..=460.0)
        .frame(
            egui::Frame::none()
                .fill(rgb(theme.panel))
                .inner_margin(egui::Margin::symmetric(8.0, 6.0)),
        )
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("inspector-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    match app.workspace {
                        crate::state::Workspace::Render => render_settings(app, ui),
                        crate::state::Workspace::Sculpt => sculpt_settings(app, ui),
                        crate::state::Workspace::Uv => uv_settings(app, ui),
                        crate::state::Workspace::Materials => material_editor(app, ui),
                        crate::state::Workspace::Animate => animation_settings(app, ui),
                        _ => {
                            object_inspector(app, ui);
                            ui.separator();
                            material_editor(app, ui);
                        }
                    }
                    ui.separator();
                    environment_editor(app, ui);
                    ui.separator();
                    performance(app, ui);
                });
        });
}

fn object_inspector(app: &mut RoEditApp, ui: &mut egui::Ui) {
    section(ui, "Object", |ui| {
        let Some(id) = app.ui.selected else {
            ui.label("Nothing selected. Click an object in the viewport or hierarchy.");
            return;
        };
        let Some((name, visible, mut transform, mesh_name, tris)) =
            app.doc.project.scene.entity(id).map(|e| {
                (
                    e.name.clone(),
                    e.visible,
                    e.transform.clone(),
                    e.mesh
                        .and_then(|m| app.doc.project.scene.mesh(m))
                        .map(|m| m.name.clone())
                        .unwrap_or_default(),
                    e.mesh
                        .and_then(|m| app.doc.project.scene.mesh(m))
                        .map(|m| m.triangle_count())
                        .unwrap_or(0),
                )
            })
        else {
            return;
        };
        ui.horizontal(|ui| {
            let mut new_name = name.clone();
            if ui
                .add(egui::TextEdit::singleline(&mut new_name).desired_width(150.0))
                .lost_focus()
                && new_name != name
                && !new_name.trim().is_empty()
            {
                app.doc.edit("Rename object", |p| {
                    p.scene.rename(id, new_name.trim());
                });
            }
            let mut vis = visible;
            if ui.checkbox(&mut vis, "Visible").changed() {
                app.doc.edit("Toggle visibility", |p| {
                    if let Some(e) = p.scene.entity_mut(id) {
                        e.visible = vis;
                    }
                });
                app.preview_dirty = true;
            }
        });
        if !mesh_name.is_empty() {
            ui.label(
                egui::RichText::new(format!("Mesh: {mesh_name} · {tris} tris"))
                    .small()
                    .color(super::theme::text_dim(&app.theme())),
            );
        }

        let mut changed = false;
        egui::Grid::new("transform-grid")
            .num_columns(2)
            .spacing([8.0, 3.0])
            .show(ui, |ui| {
                ui.label("Position");
                ui.horizontal(|ui| {
                    for (axis, color) in [
                        (0, egui::Color32::from_rgb(220, 90, 90)),
                        (1, egui::Color32::from_rgb(120, 200, 120)),
                        (2, egui::Color32::from_rgb(110, 150, 240)),
                    ] {
                        ui.label(egui::RichText::new(["X", "Y", "Z"][axis]).color(color));
                        changed |= ui
                            .add(
                                egui::DragValue::new(match axis {
                                    0 => &mut transform.translation.x,
                                    1 => &mut transform.translation.y,
                                    _ => &mut transform.translation.z,
                                })
                                .speed(0.01)
                                .max_decimals(3),
                            )
                            .changed();
                    }
                });
                ui.end_row();
                ui.label("Rotation°");
                let mut euler = transform.rotation_euler_deg();
                ui.horizontal(|ui| {
                    for axis in 0..3 {
                        changed |= ui
                            .add(
                                egui::DragValue::new(match axis {
                                    0 => &mut euler.x,
                                    1 => &mut euler.y,
                                    _ => &mut euler.z,
                                })
                                .speed(0.5)
                                .max_decimals(1),
                            )
                            .changed();
                    }
                });
                if changed {
                    transform.set_rotation_euler_deg(euler);
                }
                ui.end_row();
                ui.label("Scale");
                ui.horizontal(|ui| {
                    for axis in 0..3 {
                        changed |= ui
                            .add(
                                egui::DragValue::new(match axis {
                                    0 => &mut transform.scale.x,
                                    1 => &mut transform.scale.y,
                                    _ => &mut transform.scale.z,
                                })
                                .speed(0.005)
                                .range(0.001..=1000.0)
                                .max_decimals(3),
                            )
                            .changed();
                    }
                });
                ui.end_row();
            });
        if changed {
            app.doc.silent_edit(|p| {
                if let Some(e) = p.scene.entity_mut(id) {
                    e.transform = transform;
                }
            });
            app.preview_dirty = true;
        }
        ui.horizontal(|ui| {
            if ui.button("Reset transform").clicked() {
                app.doc.edit("Reset transform", |p| {
                    if let Some(e) = p.scene.entity_mut(id) {
                        e.transform = roedit_core::scene::Transform::identity();
                    }
                });
                app.preview_dirty = true;
            }
            if ui.button("Frame").clicked() {
                app.dispatch(crate::command::Action::FrameSelection);
            }
            if ui.button("Duplicate").clicked() {
                app.dispatch(crate::command::Action::DuplicateSelected);
            }
        });
        // Quick transforms for precise entry.
        ui.horizontal(|ui| {
            if ui
                .button("Drop to floor")
                .on_hover_text("Move the object's origin to y=0")
                .clicked()
            {
                app.doc.edit("Drop to floor", |p| {
                    if let Some(e) = p.scene.entity_mut(id) {
                        e.transform.translation.y = 0.0;
                    }
                });
                app.preview_dirty = true;
            }
            if ui.button("Zero rotation").clicked() {
                app.doc.edit("Zero rotation", |p| {
                    if let Some(e) = p.scene.entity_mut(id) {
                        e.transform.rotation = glam::Quat::IDENTITY;
                    }
                });
                app.preview_dirty = true;
            }
        });
    });
}

fn material_editor(app: &mut RoEditApp, ui: &mut egui::Ui) {
    section(ui, "Material", |ui| {
        let material_id = app.ui.selected_material.or_else(|| {
            app.ui
                .selected
                .and_then(|e| app.doc.project.scene.entity(e))
                .and_then(|e| e.material)
        });
        let Some(mid) = material_id else {
            ui.label("No material selected.");
            if ui.button("Create material").clicked() {
                let mut id = None;
                app.doc.edit("Create material", |p| {
                    id = Some(
                        p.scene
                            .add_material(roedit_core::material::Material::default()),
                    );
                });
                app.ui.selected_material = id;
            }
            return;
        };
        app.ui.selected_material = Some(mid);
        let Some(mut material) = app.doc.project.scene.material(mid).cloned() else {
            return;
        };
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Name");
            changed |= ui
                .add(egui::TextEdit::singleline(&mut material.name).desired_width(150.0))
                .changed();
        });
        ui.horizontal(|ui| {
            ui.label("Base");
            changed |= ui.color_edit_button_rgb(&mut material.base_color).changed();
            if ui
                .small_button("Assign")
                .on_hover_text("Assign to selection")
                .clicked()
            {
                if let Some(entity) = app.ui.selected {
                    app.doc.edit("Assign material", |p| {
                        if let Some(e) = p.scene.entity_mut(entity) {
                            e.material = Some(mid);
                        }
                    });
                    app.preview_dirty = true;
                }
            }
        });
        changed |= ui
            .add(egui::Slider::new(&mut material.metallic, 0.0..=1.0).text("Metallic"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut material.roughness, 0.0..=1.0).text("Roughness"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut material.opacity, 0.0..=1.0).text("Opacity"))
            .changed();
        ui.horizontal(|ui| {
            ui.label("Emission");
            changed |= ui.color_edit_button_rgb(&mut material.emission).changed();
        });
        changed |= ui
            .add(
                egui::Slider::new(&mut material.emission_strength, 0.0..=20.0)
                    .text("Emission strength"),
            )
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut material.ior, 1.0..=2.5).text("IOR"))
            .changed();
        changed |= ui
            .add(
                egui::Slider::new(&mut material.normal_strength, 0.0..=3.0).text("Normal strength"),
            )
            .changed();
        changed |= ui
            .checkbox(&mut material.double_sided, "Double sided")
            .changed();

        ui.add_space(3.0);
        ui.label(egui::RichText::new("Texture slots").small());
        let assign_slot = |ui: &mut egui::Ui,
                           label: &str,
                           slot: &mut Option<roedit_core::ids::ImageId>,
                           selected: Option<roedit_core::ids::ImageId>|
         -> bool {
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label(label);
                let current = slot.and_then(|id| app.doc.project.scene.images.get(&id).map(|_| id));
                ui.label(
                    egui::RichText::new(match current {
                        Some(id) => format!("image #{}", id.0),
                        None => "none".into(),
                    })
                    .small(),
                );
                if ui.small_button("Set").clicked() {
                    *slot = selected;
                    changed = true;
                }
                if ui.small_button("×").clicked() {
                    *slot = None;
                    changed = true;
                }
            });
            changed
        };
        let selected_image = app.ui.selected_image;
        changed |= assign_slot(ui, "Albedo", &mut material.albedo_map, selected_image);
        changed |= assign_slot(ui, "ORM", &mut material.orm_map, selected_image);
        changed |= assign_slot(ui, "Normal", &mut material.normal_map, selected_image);
        changed |= assign_slot(ui, "Emission", &mut material.emission_map, selected_image);
        changed |= assign_slot(ui, "Opacity", &mut material.opacity_map, selected_image);

        if changed {
            app.doc.silent_edit(|p| {
                p.scene.materials.insert(mid, material.clone());
            });
            app.preview_dirty = true;
        }
        ui.horizontal(|ui| {
            if ui.button("Duplicate material").clicked() {
                let mut new_id = None;
                app.doc.edit("Duplicate material", |p| {
                    let mut copy = material.clone();
                    copy.name = format!("{} Copy", material.name);
                    new_id = Some(p.scene.add_material(copy));
                });
                app.ui.selected_material = new_id;
            }
            if ui.button("Presets…").clicked() {
                app.ui.show_settings = false;
                app.ui.palette_open = true;
                app.ui.palette_query = "material".into();
                app.ui.palette_index = 0;
            }
        });
    });
}

fn sculpt_settings(app: &mut RoEditApp, ui: &mut egui::Ui) {
    section(ui, "Sculpt brush", |ui| {
        for brush in roedit_mesh::sculpt::SculptBrush::ALL {
            if ui
                .selectable_label(app.ui.sculpt_brush == brush, brush.label())
                .on_hover_text(brush.label())
                .clicked()
            {
                app.ui.sculpt_brush = brush;
                app.ui.tool = crate::state::Tool::Sculpt;
            }
        }
        ui.add_space(4.0);
        ui.add(
            egui::Slider::new(&mut app.ui.sculpt_radius, 0.02..=2.0)
                .text("Radius")
                .logarithmic(true),
        );
        ui.add(egui::Slider::new(&mut app.ui.sculpt_strength, 0.01..=1.0).text("Strength"));
        ui.label(
            egui::RichText::new("Click and drag on the model surface to sculpt.")
                .small()
                .color(super::theme::text_dim(&app.theme())),
        );
    });
    ui.separator();
    object_inspector(app, ui);
}

fn uv_settings(app: &mut RoEditApp, ui: &mut egui::Ui) {
    section(ui, "UV mapping", |ui| {
        for kind in crate::command::UvKind::ALL {
            if ui.button(kind.label()).clicked() {
                app.dispatch(crate::command::Action::ApplyUv(kind));
            }
        }
        if let Some(mesh_id) = app
            .ui
            .selected
            .and_then(|e| app.doc.project.scene.entity(e))
            .and_then(|e| e.mesh)
        {
            if let Some(mesh) = app.doc.project.scene.mesh(mesh_id) {
                let stats = roedit_mesh::uv::uv_stats(mesh);
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} of {} faces have UVs",
                        stats.faces_with_uvs,
                        mesh.faces.len()
                    ))
                    .small(),
                );
                ui.label(
                    egui::RichText::new("UV islands can be inspected in the UV workspace.")
                        .small()
                        .color(super::theme::text_dim(&app.theme())),
                );
            }
        }
    });
}

fn animation_settings(app: &mut RoEditApp, ui: &mut egui::Ui) {
    section(ui, "Animation", |ui| {
        ui.add(
            egui::Slider::new(&mut app.ui.anim_time, 0.0..=20.0)
                .text("Time (s)")
                .max_decimals(2),
        );
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("key-mode")
                .selected_text(["Translation", "Rotation", "Scale"][app.ui.anim_key_mode.min(2)])
                .show_ui(ui, |ui| {
                    for (i, label) in ["Translation", "Rotation", "Scale"].iter().enumerate() {
                        ui.selectable_value(&mut app.ui.anim_key_mode, i, *label);
                    }
                });
            if ui.button("Key").clicked() {
                app.dispatch(crate::command::Action::AddKeyframe);
            }
            if ui.button("Delete key").clicked() {
                app.dispatch(crate::command::Action::DeleteKeyframe);
            }
        });
        ui.horizontal(|ui| {
            if ui
                .button(if app.ui.playing { "Pause" } else { "Play" })
                .clicked()
            {
                app.dispatch(crate::command::Action::PlayPause);
            }
            if ui.button("Apply at time").clicked() {
                let t = app.ui.anim_time;
                app.apply_animation_at(t);
                app.preview_dirty = true;
            }
        });
        ui.label(
            egui::RichText::new(format!(
                "{} track(s)",
                app.doc.project.animation.tracks.len()
            ))
            .small()
            .color(super::theme::text_dim(&app.theme())),
        );
    });
    object_inspector(app, ui);
}

fn render_settings(app: &mut RoEditApp, ui: &mut egui::Ui) {
    section(ui, "Render", |ui| {
        let mut settings = app.doc.project.render.clone();
        let mut changed = false;
        egui::Grid::new("render-grid")
            .num_columns(2)
            .spacing([8.0, 3.0])
            .show(ui, |ui| {
                ui.label("Width");
                changed |= ui
                    .add(egui::DragValue::new(&mut settings.width).range(16..=8192))
                    .changed();
                ui.end_row();
                ui.label("Height");
                changed |= ui
                    .add(egui::DragValue::new(&mut settings.height).range(16..=8192))
                    .changed();
                ui.end_row();
                ui.label("Samples");
                changed |= ui
                    .add(egui::DragValue::new(&mut settings.samples).range(1..=4096))
                    .changed();
                ui.end_row();
                ui.label("Bounces");
                changed |= ui
                    .add(egui::DragValue::new(&mut settings.max_bounces).range(1..=32))
                    .changed();
                ui.end_row();
            });
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("render-mode")
                .selected_text(settings.mode.label())
                .show_ui(ui, |ui| {
                    for mode in roedit_core::render_settings::RenderMode::ALL {
                        changed |= ui
                            .selectable_value(&mut settings.mode, mode, mode.label())
                            .changed();
                    }
                });
            egui::ComboBox::from_id_salt("quality")
                .selected_text("Preset")
                .show_ui(ui, |ui| {
                    for q in roedit_core::render_settings::RenderQuality::ALL {
                        if ui.selectable_label(false, q.label()).clicked() {
                            settings.apply_quality_preset(q);
                            changed = true;
                        }
                    }
                });
        });
        if changed {
            app.doc.silent_edit(|p| p.render = settings.clone());
            app.preview_dirty = true;
        }
        ui.horizontal(|ui| {
            ui.label("Output");
            ui.add(
                egui::TextEdit::singleline(&mut app.ui.render.output)
                    .desired_width(150.0)
                    .hint_text("renders/still.png"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Format");
            egui::ComboBox::from_id_salt("render-format")
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
        });
        ui.checkbox(
            &mut app.ui.render.use_settings_resolution,
            "Use project resolution for stills",
        );
        if !app.ui.render.use_settings_resolution {
            ui.label(
                egui::RichText::new("Stills render at 1280×720 when disabled.")
                    .small()
                    .color(super::theme::text_dim(&app.theme())),
            );
        }
        ui.add_space(4.0);
        let busy = app.job.is_some();
        if ui
            .add_enabled(!busy, egui::Button::new("Render still (F12)"))
            .clicked()
        {
            app.dispatch(crate::command::Action::RenderStill);
        }
        if busy {
            ui.label(
                egui::RichText::new("Rendering…")
                    .small()
                    .color(super::theme::warning(&app.theme())),
            );
        }
        ui.label(
            egui::RichText::new(format!(
                "Last render: {:.0} ms · {} samples configured",
                app.ui.perf.last_render_ms, settings.samples
            ))
            .small()
            .color(super::theme::text_dim(&app.theme())),
        );
    });
    ui.separator();
    environment_editor(app, ui);
}

fn environment_editor(app: &mut RoEditApp, ui: &mut egui::Ui) {
    section(ui, "Environment", |ui| {
        let mut env = app.doc.project.scene.environment.clone();
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Sky");
            egui::ComboBox::from_id_salt("sky-mode")
                .selected_text(format!("{:?}", env.sky))
                .show_ui(ui, |ui| {
                    for mode in [
                        roedit_core::material::SkyMode::Solid,
                        roedit_core::material::SkyMode::Gradient,
                        roedit_core::material::SkyMode::Physical,
                    ] {
                        changed |= ui
                            .selectable_value(&mut env.sky, mode, format!("{mode:?}"))
                            .changed();
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Top");
            changed |= ui.color_edit_button_rgb(&mut env.sky_top).changed();
            ui.label("Horizon");
            changed |= ui.color_edit_button_rgb(&mut env.sky_horizon).changed();
        });
        changed |= ui
            .add(egui::Slider::new(&mut env.sun_intensity, 0.0..=12.0).text("Sun"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut env.ambient_intensity, 0.0..=2.0).text("Ambient"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut env.exposure, 0.1..=4.0).text("Exposure"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut env.fog_density, 0.0..=0.5).text("Fog"))
            .changed();
        ui.horizontal(|ui| {
            ui.label("Sun dir");
            changed |= ui
                .add(egui::DragValue::new(&mut env.sun_direction.x).speed(0.01))
                .changed();
            changed |= ui
                .add(egui::DragValue::new(&mut env.sun_direction.y).speed(0.01))
                .changed();
            changed |= ui
                .add(egui::DragValue::new(&mut env.sun_direction.z).speed(0.01))
                .changed();
            if env.sun_direction.length() > 1e-4 {
                env.sun_direction = env.sun_direction.normalize();
            }
        });
        if changed {
            app.doc.silent_edit(|p| p.scene.environment = env.clone());
            app.preview_dirty = true;
        }
        ui.horizontal(|ui| {
            if ui.button("Reset environment").clicked() {
                app.doc.edit("Reset environment", |p| {
                    p.scene.environment = roedit_core::material::Environment::default();
                });
                app.preview_dirty = true;
            }
        });
        let g = env.sky_gradient_image(64, 24);
        let _ = g;
    });
}

fn performance(app: &mut RoEditApp, ui: &mut egui::Ui) {
    let stats = app.doc.project.scene.statistics();
    section(ui, "Statistics", |ui| {
        egui::Grid::new("stats-grid").num_columns(2).show(ui, |ui| {
            for (k, v) in [
                ("Objects", stats.entities.to_string()),
                ("Visible", stats.visible_entities.to_string()),
                ("Meshes", stats.unique_meshes.to_string()),
                ("Instances", stats.mesh_instances.to_string()),
                ("Vertices", stats.vertices.to_string()),
                ("Triangles", stats.triangles.to_string()),
                ("Materials", stats.materials.to_string()),
                (
                    "Images",
                    format!(
                        "{} ({:.1} KB)",
                        stats.images,
                        stats.image_bytes as f32 / 1024.0
                    ),
                ),
                ("Lights", stats.lights.to_string()),
            ] {
                ui.label(k);
                ui.label(egui::RichText::new(v).monospace());
                ui.end_row();
            }
        });
        ui.add_space(3.0);
        ui.label(
            egui::RichText::new(format!(
                "UI {:.1} ms ({:.0} fps) · preview {:.0} ms",
                app.ui.perf.average_ms,
                app.ui.perf.fps(),
                app.ui.perf.last_preview_ms
            ))
            .small()
            .color(super::theme::text_dim(&app.theme())),
        );
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 3.0, rgb(app.theme().viewport));
        let samples = &app.ui.perf.samples;
        if samples.len() > 1 {
            let max = samples.iter().cloned().fold(1.0f32, f32::max);
            let points: Vec<egui::Pos2> = samples
                .iter()
                .enumerate()
                .map(|(i, ms)| {
                    egui::pos2(
                        rect.left() + rect.width() * i as f32 / (samples.len() - 1) as f32,
                        rect.bottom() - rect.height() * (ms / max).clamp(0.0, 1.0),
                    )
                })
                .collect();
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(1.0_f32, super::theme::accent(&app.theme())),
            ));
        }
        let _ = num(0.0);
        let _ = Vec3::ZERO;
    });
}
