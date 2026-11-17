//! 2D surfaces: GFX composition, texture painting, UV layout, asset library,
//! extension manager and video preview.

use super::{rgb, section};
use crate::app::RoEditApp;
use crate::command::Action;
use eframe::egui;
use glam::Vec2;
use roedit_core::gfx::BlendMode;
use roedit_image::brush::{stamp, BrushMode, BrushSettings};

const MAX_TEXTURE_PIXELS: f32 = 1400.0;

/// Name, id, version, description and (command id, command label) pairs.
type PackSummary = (String, String, String, String, Vec<(String, String)>);

fn upload(
    app: &mut RoEditApp,
    ctx: &egui::Context,
    name: &str,
    image: &roedit_core::raster::RasterImage,
) {
    if image.is_empty() {
        return;
    }
    let w = image.width as usize;
    let h = image.height as usize;
    let color = egui::ColorImage::from_rgba_unmultiplied([w, h], &image.data);
    let handle = ctx.load_texture(name, color, egui::TextureOptions::LINEAR);
    app.canvas_texture = Some(handle);
}

fn ensure_image_texture(app: &mut RoEditApp, ctx: &egui::Context) {
    let Some(id) = app.ui.selected_image else {
        return;
    };
    let Some(image) = app.doc.project.scene.images.get(&id).cloned() else {
        return;
    };
    let key = format!(
        "img:{}:{}x{}:{}:{:?}",
        id.0,
        image.width,
        image.height,
        image.data.len(),
        image.data.first()
    );
    if app.scratch.canvas_key == key && app.canvas_texture.is_some() && !app.canvas_dirty {
        return;
    }
    upload(app, ctx, "roedit-image", &image);
    app.scratch.canvas_key = key;
    app.canvas_dirty = false;
}

fn ensure_gfx_texture(app: &mut RoEditApp, ctx: &egui::Context) {
    if !app.gfx_dirty && app.gfx_texture.is_some() {
        return;
    }
    let Some(gfx) = app.doc.project.gfx.as_ref() else {
        app.gfx_texture = None;
        return;
    };
    let opts = roedit_image::composite::FlattenOptions::default();
    let image = roedit_image::composite::flatten_doc(gfx, &opts);
    let color = egui::ColorImage::from_rgba_unmultiplied(
        [image.width as usize, image.height as usize],
        &image.data,
    );
    app.gfx_texture = Some(ctx.load_texture("roedit-gfx", color, egui::TextureOptions::LINEAR));
    app.gfx_dirty = false;
}

fn painter_rect(
    ui: &mut egui::Ui,
    tex: &egui::TextureHandle,
    max: f32,
) -> (egui::Rect, egui::Rect) {
    let size = tex.size_vec2();
    let available = ui.available_size();
    let scale = (available.x / size.x.max(1.0))
        .min(available.y / size.y.max(1.0))
        .min(max / size.x.max(size.y))
        .max(0.01);
    let target = egui::vec2(size.x * scale, size.y * scale);
    let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
    let image_rect = egui::Rect::from_center_size(rect.center(), target);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, rgb([18, 20, 25]));
    painter.image(
        tex.id(),
        image_rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
    (rect, image_rect)
}

// ------------------------------------------------------------------ texture

pub fn paint_canvas(app: &mut RoEditApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Texture Paint").strong());
        let images: Vec<(roedit_core::ids::ImageId, u32, u32)> = app
            .doc
            .project
            .scene
            .images
            .iter()
            .map(|(id, img)| (*id, img.width, img.height))
            .collect();
        egui::ComboBox::from_id_salt("paint-image")
            .selected_text(match app.ui.selected_image {
                Some(id) => format!("Image #{id}"),
                None => "Select image".into(),
            })
            .show_ui(ui, |ui| {
                for (id, w, h) in images {
                    if ui
                        .selectable_label(
                            app.ui.selected_image == Some(id),
                            format!("Image #{} ({w}x{h})", id.0),
                        )
                        .clicked()
                    {
                        app.ui.selected_image = Some(id);
                        app.canvas_dirty = true;
                    }
                }
            });
        if ui.button("Import image…").clicked() {
            app.dispatch(Action::ShowImport);
        }
        ui.separator();
        ui.label("Colour");
        if ui
            .color_edit_button_rgba_unmultiplied(&mut app.ui.paint_color)
            .changed()
        {
            app.ui.brush.color = app.ui.paint_color;
        }
        egui::ComboBox::from_id_salt("brush-mode")
            .selected_text(mode_label(app.ui.brush.mode))
            .show_ui(ui, |ui| {
                for mode in BrushMode::ALL {
                    if ui
                        .selectable_label(app.ui.brush.mode == mode, mode_label(mode))
                        .clicked()
                    {
                        app.ui.brush.mode = mode;
                    }
                }
            });
        ui.add(egui::Slider::new(&mut app.ui.brush.size, 1.0..=256.0).text("Size"));
        ui.add(egui::Slider::new(&mut app.ui.brush.hardness, 0.0..=1.0).text("Hardness"));
        ui.add(egui::Slider::new(&mut app.ui.brush.opacity, 0.0..=1.0).text("Opacity"));
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Adjust:").small());
        for op in crate::command::ImageOpKind::ALL {
            if ui.small_button(op.label()).clicked() {
                app.dispatch(Action::ImageOp(op));
                app.canvas_dirty = true;
            }
        }
    });
    ensure_image_texture(app, ui.ctx());
    let Some(texture) = app.canvas_texture.clone() else {
        ui.add_space(20.0);
        ui.label("Import a PNG/JPEG/TIFF/EXR image, or choose an existing project image above.");
        ui.label(
            egui::RichText::new(
                "Painting edits the project copy inside the document; use File → Export to write it back to disk.",
            )
            .small()
            .color(super::theme::text_dim(&app.theme())),
        );
        return;
    };
    let (_rect, image_rect) = painter_rect(ui, &texture, MAX_TEXTURE_PIXELS);
    let response = ui.interact(
        image_rect,
        ui.id().with("paint-canvas"),
        egui::Sense::click_and_drag(),
    );
    if let Some(pointer) = response.interact_pointer_pos() {
        if response.dragged() || response.clicked() {
            if response.drag_started() {
                app.doc.edit("Paint stroke", |_| {});
            }
            let Some(id) = app.ui.selected_image else {
                return;
            };
            let rel = egui::vec2(
                (pointer.x - image_rect.left()) / image_rect.width().max(1.0),
                (pointer.y - image_rect.top()) / image_rect.height().max(1.0),
            );
            let settings = BrushSettings {
                color: app.ui.paint_color,
                ..app.ui.brush.clone()
            };
            let mut painted = 0;
            app.doc.silent_edit(|p| {
                if let Some(image) = p.scene.image_mut(id) {
                    let center = Vec2::new(rel.x * image.width as f32, rel.y * image.height as f32);
                    painted = stamp(image, center, &settings);
                }
            });
            if painted > 0 {
                app.canvas_dirty = true;
            }
        }
    }
    ui.label(
        egui::RichText::new(
            "Drag on the image to paint. Erase and Blur modes are in the brush menu.",
        )
        .small()
        .color(super::theme::text_dim(&app.theme())),
    );
}

fn mode_label(mode: BrushMode) -> &'static str {
    match mode {
        BrushMode::Normal => "Normal",
        BrushMode::Erase => "Erase",
        BrushMode::Multiply => "Multiply",
        BrushMode::Add => "Add",
        BrushMode::Blur => "Blur",
        BrushMode::Clone => "Clone",
    }
}

// ---------------------------------------------------------------------- GFX

pub fn gfx_canvas(app: &mut RoEditApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("GFX Studio").strong());
        ui.add(
            egui::TextEdit::singleline(&mut app.ui.gfx_text)
                .hint_text("Text content")
                .desired_width(180.0),
        );
        ui.add(
            egui::DragValue::new(&mut app.ui.gfx_font_size)
                .range(8.0..=400.0)
                .prefix("size "),
        );
        if ui.button("Add text").clicked() {
            app.dispatch(Action::GfxAddText);
        }
        if ui.button("Add group").clicked() {
            app.dispatch(Action::GfxAddGroup);
        }
        if ui.button("Flatten → PNG").clicked() {
            app.dispatch(Action::GfxFlatten);
        }
    });
    ui.horizontal(|ui| {
        let presets = [
            ("1920×1080", 1920, 1080),
            ("1080×1080", 1080, 1080),
            ("1080×1920", 1080, 1920),
            ("1280×720", 1280, 720),
            ("2560×1440", 2560, 1440),
        ];
        for (label, w, h) in presets {
            if ui.small_button(label).clicked() {
                app.doc.edit("Set canvas size", |p| {
                    let gfx = p.gfx_document(w, h);
                    gfx.width = w;
                    gfx.height = h;
                });
                app.gfx_dirty = true;
            }
        }
        ui.separator();
        ui.label(
            egui::RichText::new("Blend mode applies to the selected layer")
                .small()
                .color(super::theme::text_dim(&app.theme())),
        );
    });

    let Some(gfx) = app.doc.project.gfx.clone() else {
        ui.add_space(20.0);
        ui.label("No GFX document yet. Add a text layer or a group to begin a design.");
        ui.label(
            egui::RichText::new(
                "GFX uses resolution-independent text and shape layers with 19 blend modes; flattening exports PNG.",
            )
            .small()
            .color(super::theme::text_dim(&app.theme())),
        );
        return;
    };

    // Layer list.
    let active = gfx.active;
    let mut selected_layer = None;
    let mut visibility_change = None;
    let mut opacity_change = None;
    let mut blend_change = None;
    section(ui, &format!("Layers ({})", gfx.layers.len()), |ui| {
        egui::ScrollArea::vertical()
            .id_salt("layer-list")
            .max_height(120.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for layer in gfx.layers.iter().rev() {
                    ui.horizontal(|ui| {
                        let mut visible = layer.visible;
                        if ui.checkbox(&mut visible, "").changed() {
                            visibility_change = Some((layer.id, visible));
                        }
                        let kind = if layer.is_group {
                            "▤"
                        } else if layer.text.is_some() {
                            "T"
                        } else {
                            "▣"
                        };
                        if ui
                            .selectable_label(
                                active == Some(layer.id),
                                format!("{kind} {}", layer.name),
                            )
                            .clicked()
                        {
                            selected_layer = Some(layer.id);
                        }
                        let mut opacity = layer.opacity;
                        if ui
                            .add(
                                egui::DragValue::new(&mut opacity)
                                    .speed(0.01)
                                    .range(0.0..=1.0),
                            )
                            .changed()
                        {
                            opacity_change = Some((layer.id, opacity));
                        }
                        ui.label(
                            egui::RichText::new(format!("{:?}", layer.blend))
                                .small()
                                .color(super::theme::text_dim(&app.theme())),
                        );
                    });
                }
            });
        let current_blend = gfx
            .layer(active.unwrap_or(roedit_core::ids::LayerId(0)))
            .map(|l| l.blend)
            .unwrap_or(BlendMode::Normal);
        egui::ComboBox::from_id_salt("gfx-blend")
            .selected_text(format!("{current_blend:?}"))
            .show_ui(ui, |ui| {
                for mode in BlendMode::ALL {
                    if ui
                        .selectable_label(current_blend == mode, format!("{mode:?}"))
                        .clicked()
                    {
                        blend_change = Some(mode);
                    }
                }
            });
    });
    if let Some(id) = selected_layer {
        app.doc.silent_edit(|p| {
            if let Some(gfx) = p.gfx.as_mut() {
                gfx.active = Some(id);
            }
        });
        app.ui.selected_layer = Some(id);
        app.gfx_dirty = true;
    }
    if let Some((id, visible)) = visibility_change {
        app.doc.edit("Toggle layer", |p| {
            if let Some(gfx) = p.gfx.as_mut() {
                if let Some(layer) = gfx.layers.iter_mut().find(|l| l.id == id) {
                    layer.visible = visible;
                }
            }
        });
        app.gfx_dirty = true;
    }
    if let Some((id, opacity)) = opacity_change {
        app.doc.silent_edit(|p| {
            if let Some(gfx) = p.gfx.as_mut() {
                if let Some(layer) = gfx.layers.iter_mut().find(|l| l.id == id) {
                    layer.opacity = opacity;
                }
            }
        });
        app.gfx_dirty = true;
    }
    if let Some(mode) = blend_change {
        app.dispatch(Action::GfxSetBlend(mode));
        app.gfx_dirty = true;
    }

    // Editable text of the active layer.
    if let Some(id) = app.doc.project.gfx.as_ref().and_then(|g| g.active) {
        if let Some(mut spec) = app
            .doc
            .project
            .gfx
            .as_ref()
            .and_then(|g| g.layer(id))
            .and_then(|l| l.text.clone())
        {
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("Text");
                changed |= ui
                    .add(egui::TextEdit::singleline(&mut spec.text).desired_width(220.0))
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut spec.position.x)
                            .speed(1.0)
                            .prefix("x "),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut spec.position.y)
                            .speed(1.0)
                            .prefix("y "),
                    )
                    .changed();
                ui.label("Colour");
                changed |= ui
                    .color_edit_button_rgba_unmultiplied(&mut spec.color)
                    .changed();
            });
            if changed {
                app.doc.silent_edit(|p| {
                    if let Some(gfx) = p.gfx.as_mut() {
                        if let Some(layer) = gfx.layers.iter_mut().find(|l| l.id == id) {
                            layer.text = Some(spec.clone());
                            layer.name = spec.text.clone();
                        }
                    }
                });
                app.gfx_dirty = true;
            }
        }
    }

    ensure_gfx_texture(app, ui.ctx());
    if let Some(texture) = app.gfx_texture.clone() {
        let (rect, image_rect) = painter_rect(ui, &texture, MAX_TEXTURE_PIXELS);
        // Checkerboard behind transparent regions.
        let painter = ui.painter_at(image_rect);
        let cell: f32 = 12.0;
        let mut y = image_rect.top();
        let mut row = 0;
        while y < image_rect.bottom() {
            let mut x = image_rect.left();
            let mut col = 0;
            while x < image_rect.right() {
                if (row + col) % 2 == 0 {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(x, y),
                            egui::vec2(
                                cell.min(image_rect.right() - x),
                                cell.min(image_rect.bottom() - y),
                            ),
                        ),
                        0.0,
                        egui::Color32::from_rgb(38, 40, 46),
                    );
                }
                x += cell;
                col += 1;
            }
            y += cell;
            row += 1;
        }
        painter.image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        // Click to place text with the Text tool.
        let response = ui.interact(image_rect, ui.id().with("gfx-canvas"), egui::Sense::click());
        if response.clicked() && app.ui.tool == crate::state::Tool::Text {
            if let Some(pointer) = response.interact_pointer_pos() {
                let rel = egui::vec2(
                    (pointer.x - image_rect.left()) / image_rect.width().max(1.0),
                    (pointer.y - image_rect.top()) / image_rect.height().max(1.0),
                );
                let text = app.ui.gfx_text.clone();
                let size = app.ui.gfx_font_size;
                app.doc.edit("Place text layer", |p| {
                    let gfx = p.gfx_document(1920, 1080);
                    let mut spec = crate::command::default_text_spec(&text, size);
                    spec.position = Vec2::new(gfx.width as f32 * rel.x, gfx.height as f32 * rel.y);
                    gfx.add_text_layer(text.clone(), spec);
                });
                app.gfx_dirty = true;
            }
        }
        let _ = rect;
    }
}

// ----------------------------------------------------------------------- UV

pub fn uv_canvas(app: &mut RoEditApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("UV Editing").strong());
        for kind in crate::command::UvKind::ALL {
            if ui.button(kind.label()).clicked() {
                app.dispatch(Action::ApplyUv(kind));
            }
        }
    });
    let mesh = app
        .ui
        .selected
        .and_then(|e| app.doc.project.scene.entity(e))
        .and_then(|e| e.mesh)
        .and_then(|m| app.doc.project.scene.mesh(m).cloned());
    let Some(mesh) = mesh else {
        ui.label("Select an object with a mesh to view and edit its UVs.");
        return;
    };
    let side = ui.available_width().min(ui.available_height()).max(200.0) - 8.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let theme = app.theme();
    painter.rect_filled(rect, 2.0, rgb(theme.viewport));
    let inner = rect.shrink(10.0);
    painter.rect_stroke(inner, 0.0, egui::Stroke::new(1.0_f32, rgb(theme.grid)));
    for i in 0..=10 {
        let f = i as f32 / 10.0;
        let x = inner.left() + inner.width() * f;
        let y = inner.top() + inner.height() * f;
        let stroke = egui::Stroke::new(0.5_f32, rgb(theme.grid).gamma_multiply(0.6));
        painter.line_segment(
            [egui::pos2(x, inner.top()), egui::pos2(x, inner.bottom())],
            stroke,
        );
        painter.line_segment(
            [egui::pos2(inner.left(), y), egui::pos2(inner.right(), y)],
            stroke,
        );
    }
    let mut drawn = 0;
    for face in &mesh.faces {
        if face.uvs.len() < 2 || drawn > 6000 {
            break;
        }
        let points: Vec<egui::Pos2> = face
            .uvs
            .iter()
            .map(|uv| {
                egui::pos2(
                    inner.left() + uv.x.rem_euclid(1.0) * inner.width(),
                    inner.bottom() - uv.y.rem_euclid(1.0) * inner.height(),
                )
            })
            .collect();
        for i in 0..points.len() {
            painter.line_segment(
                [points[i], points[(i + 1) % points.len()]],
                egui::Stroke::new(
                    0.8_f32,
                    egui::Color32::from_rgba_unmultiplied(150, 200, 255, 140),
                ),
            );
        }
        drawn += 1;
    }
    painter.text(
        egui::pos2(inner.left() + 4.0, inner.top() + 4.0),
        egui::Align2::LEFT_TOP,
        format!("{} · {} faces with UVs drawn", mesh.name, drawn),
        egui::FontId::proportional(10.0),
        super::theme::text_dim(&theme),
    );
    ui.label(
        egui::RichText::new(
            "Projections overwrite existing UVs. Unwrap and Pack builds islands per face.",
        )
        .small()
        .color(super::theme::text_dim(&theme)),
    );
}

// ------------------------------------------------------------------- assets

pub fn assets_panel(app: &mut RoEditApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            section(ui, "Images", |ui| {
                let images: Vec<(roedit_core::ids::ImageId, u32, u32, usize)> = app
                    .doc
                    .project
                    .scene
                    .images
                    .iter()
                    .map(|(id, img)| (*id, img.width, img.height, img.data.len()))
                    .collect();
                if images.is_empty() {
                    ui.label("No images in this project yet.");
                }
                for (id, w, h, bytes) in images {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                app.ui.selected_image == Some(id),
                                format!("Image #{}", id.0),
                            )
                            .clicked()
                        {
                            app.ui.selected_image = Some(id);
                            app.canvas_dirty = true;
                        }
                        ui.label(
                            egui::RichText::new(format!(
                                "{w}×{h} · {:.1} KB",
                                bytes as f32 / 1024.0
                            ))
                            .small()
                            .color(super::theme::text_dim(&app.theme())),
                        );
                        if ui.small_button("Assign to material").clicked() {
                            if let Some(mid) = app.ui.selected_material {
                                app.doc.edit("Assign albedo map", |p| {
                                    if let Some(m) = p.scene.material_mut(mid) {
                                        m.albedo_map = Some(id);
                                    }
                                });
                                app.preview_dirty = true;
                            } else {
                                app.ui.toast("Select a material in the left dock first");
                            }
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut app.ui.io_path)
                            .hint_text("path/to/image.png")
                            .desired_width(220.0),
                    );
                    if ui.button("Import").clicked() {
                        app.dispatch(Action::ShowImport);
                    }
                    if ui.button("Export copy").clicked() {
                        app.export_selected_image();
                    }
                });
            });
            ui.separator();
            section(ui, "Meshes", |ui| {
                let meshes: Vec<(String, usize, usize)> = app
                    .doc
                    .project
                    .scene
                    .meshes
                    .values()
                    .map(|m| (m.name.clone(), m.vertex_count(), m.triangle_count()))
                    .collect();
                if meshes.is_empty() {
                    ui.label("No meshes yet.");
                }
                for (name, verts, tris) in meshes {
                    ui.label(format!("{name} · {verts} verts · {tris} tris"));
                }
            });
            ui.separator();
            section(ui, "File format support", |ui| {
                egui::Grid::new("formats")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for support in roedit_io::list_formats() {
                            ui.label(support.kind.label());
                            ui.label(
                                egui::RichText::new(if support.supported {
                                    "supported"
                                } else {
                                    "not supported"
                                })
                                .small()
                                .color(if support.supported {
                                    super::theme::success(&app.theme())
                                } else {
                                    super::theme::error(&app.theme())
                                }),
                            );
                            ui.end_row();
                            ui.label("");
                            ui.label(
                                egui::RichText::new(&support.message)
                                    .small()
                                    .color(super::theme::text_dim(&app.theme())),
                            );
                            ui.end_row();
                        }
                    });
            });
        });
}

// ---------------------------------------------------------------- extensions

pub fn plugins_panel(app: &mut RoEditApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        section(ui, "Extension folder", |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.ui.plugin_dir)
                        .desired_width(320.0)
                        .hint_text("folder containing JSON packs"),
                );
                if ui.button("Reload packs").clicked() {
                    app.dispatch(Action::ReloadPlugins);
                }
            });
            ui.label(
                egui::RichText::new(
                    "Packs are JSON documents with macros, material presets and tips. They cannot execute native code; every step is validated before it runs.",
                )
                .small()
                .color(super::theme::text_dim(&app.theme())),
            );
        });
        ui.separator();
        let packs: Vec<PackSummary> = app
            .plugins
            .packs()
            .iter()
            .map(|p| {
                (
                    p.manifest.name.clone(),
                    p.manifest.id.clone(),
                    p.manifest.version.clone(),
                    p.manifest.description.clone(),
                    p.manifest
                        .commands
                        .iter()
                        .map(|c| (c.id.clone(), c.label.clone()))
                        .collect(),
                )
            })
            .collect();
        section(ui, &format!("Installed packs ({})", packs.len()), |ui| {
            if packs.is_empty() {
                ui.label("No packs loaded. Point the folder above at a directory of .json packs and press Reload packs.");
            }
            for (name, id, version, description, commands) in packs {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&name).strong());
                    ui.label(
                        egui::RichText::new(format!("{id} v{version}"))
                            .small()
                            .color(super::theme::text_dim(&app.theme())),
                    );
                });
                if !description.is_empty() {
                    ui.label(egui::RichText::new(description).small());
                }
                for (command_id, label) in commands {
                    ui.horizontal(|ui| {
                        ui.label(format!("• {label}"));
                        if ui.small_button("Run").clicked() {
                            app.run_plugin_command(&command_id);
                        }
                    });
                }
            }
        });
        ui.separator();
        section(ui, "Tips and documentation", |ui| {
            ui.label(
                egui::RichText::new(
                    "Packs may carry author tips and material presets. A pack lives in one JSON file; see docs/plugin-development.md for the schema and the validated step list.",
                )
                .small(),
            );
        });
    });
}

// -------------------------------------------------------------------- video

pub fn video_preview(app: &mut RoEditApp, ui: &mut egui::Ui) {
    let timeline_info = app
        .doc
        .project
        .timeline
        .as_ref()
        .map(|t| (t.playhead, t.fps, t.duration, t.clips.len()));
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Program monitor").strong());
        if let Some((playhead, fps, duration, clips)) = timeline_info {
            ui.label(
                egui::RichText::new(format!(
                    "t {playhead:.2}s · frame {} · {clips} clip(s) · {duration:.1}s",
                    (playhead * fps).round() as u64
                ))
                .small()
                .color(super::theme::text_dim(&app.theme())),
            );
        }
        if ui
            .button(if app.ui.playing { "Pause" } else { "Play" })
            .clicked()
        {
            app.dispatch(Action::PlayPause);
        }
        if ui.button("Render frame").clicked() {
            if let Some(timeline) = app.doc.project.timeline.as_ref() {
                let t = timeline.playhead;
                app.preview
                    .request_video(app.doc.project.clone(), t, 640, 360);
            }
        }
        if ui.button("Export sequence…").clicked() {
            app.dispatch(Action::ExportSequence);
        }
    });
    let texture = app
        .video_texture
        .clone()
        .or_else(|| app.preview_texture.clone());
    match texture {
        Some(texture) => {
            let size = texture.size_vec2();
            let available = ui.available_size();
            let scale = (available.x / size.x.max(1.0))
                .min(available.y / size.y.max(1.0))
                .min(3.0);
            let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
            let image_rect = egui::Rect::from_center_size(rect.center(), size * scale);
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, egui::Color32::BLACK);
            painter.image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        None => {
            ui.add_space(30.0);
            ui.label("The program monitor shows the composited timeline frame.");
            ui.label(
                egui::RichText::new(
                    "Add a colour or text clip below, then press Play or Render frame. Exports write PNG/JPEG/TIFF sequences plus a WAV mix.",
                )
                .small()
                .color(super::theme::text_dim(&app.theme())),
            );
        }
    }
}
