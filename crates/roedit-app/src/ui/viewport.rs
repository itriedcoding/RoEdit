//! 3D viewport: preview image, orbit/pan/zoom, box selection by raycast,
//! transform dragging, sculpting and overlays (grid, wireframe, gizmo, stats).

use crate::app::RoEditApp;
use crate::state::{OrbitCamera, Tool, ViewMode};
use eframe::egui;
use glam::{Mat4, Quat, Vec2, Vec3};
use roedit_core::math::Ray;
use roedit_mesh::sculpt::{sculpt, BrushSettings as SculptSettings};

fn basis(cam: &OrbitCamera) -> (Vec3, Vec3, Vec3, Vec3) {
    let pos = cam.position();
    let forward = (cam.target - pos).normalize_or_zero();
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    (pos, forward, right, up)
}

fn ray_from_pointer(cam: &OrbitCamera, rect: egui::Rect, pointer: egui::Pos2, aspect: f32) -> Ray {
    let (pos, forward, right, up) = basis(cam);
    let ndc_x = ((pointer.x - rect.left()) / rect.width().max(1.0)) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((pointer.y - rect.top()) / rect.height().max(1.0)) * 2.0;
    let tan = (cam.fov_y_deg.to_radians() * 0.5).tan();
    let dir = (forward + right * ndc_x * tan * aspect + up * ndc_y * tan).normalize_or_zero();
    Ray::new(pos, dir)
}

fn project(point: Vec3, view_proj: Mat4, rect: egui::Rect) -> Option<egui::Pos2> {
    let clip = view_proj * point.extend(1.0);
    if clip.w <= 1e-4 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !(-1.2..=1.2).contains(&ndc.x) || !(-1.2..=1.2).contains(&ndc.y) {
        return None;
    }
    Some(egui::pos2(
        rect.left() + (ndc.x * 0.5 + 0.5) * rect.width(),
        rect.top() + (0.5 - ndc.y * 0.5) * rect.height(),
    ))
}

pub fn viewport(app: &mut RoEditApp, ui: &mut egui::Ui) {
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
    let theme = app.theme();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, super::theme::viewport_bg(&theme));

    // Preview image (letterboxed to its aspect ratio).
    let mut image_rect = rect;
    if let Some(texture) = &app.preview_texture {
        let size = texture.size_vec2();
        let scale = (rect.width() / size.x.max(1.0)).min(rect.height() / size.y.max(1.0));
        let target = egui::vec2(size.x * scale, size.y * scale);
        image_rect = egui::Rect::from_center_size(rect.center(), target);
        painter.image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            if app.preview.busy() {
                "Rendering preview…"
            } else {
                "No preview yet — add geometry or press F5"
            },
            egui::FontId::proportional(14.0),
            super::theme::text_dim(&theme),
        );
    }
    if let Some(err) = &app.preview.error {
        painter.text(
            egui::pos2(rect.left() + 12.0, rect.top() + 12.0),
            egui::Align2::LEFT_TOP,
            format!("Preview error: {err}"),
            egui::FontId::proportional(12.0),
            super::theme::error(&theme),
        );
    }

    let aspect = image_rect.width() / image_rect.height().max(1.0);
    let camera = app.ui.camera.camera(aspect);
    let view = Mat4::look_at_rh(camera.position, camera.target, Vec3::Y);
    let proj = Mat4::perspective_rh(
        camera.fov_y_deg.to_radians(),
        aspect,
        camera.near,
        camera.far,
    );
    let view_proj = proj * view;

    // Grid.
    if app.ui.show_grid {
        let grid_pen = egui::Stroke::new(1.0_f32, super::rgb(theme.grid).gamma_multiply(0.55));
        for i in -16..=16 {
            let f = i as f32 * 0.5;
            if let (Some(a), Some(b)) = (
                project(Vec3::new(f, 0.0, -8.0), view_proj, image_rect),
                project(Vec3::new(f, 0.0, 8.0), view_proj, image_rect),
            ) {
                painter.line_segment([a, b], grid_pen);
            }
            if let (Some(a), Some(b)) = (
                project(Vec3::new(-8.0, 0.0, f), view_proj, image_rect),
                project(Vec3::new(8.0, 0.0, f), view_proj, image_rect),
            ) {
                painter.line_segment([a, b], grid_pen);
            }
        }
        if let (Some(a), Some(b)) = (
            project(Vec3::new(0.0, 0.0, -8.0), view_proj, image_rect),
            project(Vec3::new(0.0, 0.0, 8.0), view_proj, image_rect),
        ) {
            painter.line_segment(
                [a, b],
                egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(80, 110, 190)),
            );
        }
        if let (Some(a), Some(b)) = (
            project(Vec3::new(-8.0, 0.0, 0.0), view_proj, image_rect),
            project(Vec3::new(8.0, 0.0, 0.0), view_proj, image_rect),
        ) {
            painter.line_segment(
                [a, b],
                egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(180, 80, 80)),
            );
        }
    }

    // Selection box outline.
    if let Some((center, radius)) = app.selected_entity_bounds() {
        let accent = super::theme::accent(&theme);
        let r = radius.max(0.05);
        let corners = [
            Vec3::new(-r, -r, -r),
            Vec3::new(r, -r, -r),
            Vec3::new(r, r, -r),
            Vec3::new(-r, r, -r),
            Vec3::new(-r, -r, r),
            Vec3::new(r, -r, r),
            Vec3::new(r, r, r),
            Vec3::new(-r, r, r),
        ];
        let projected: Vec<Option<egui::Pos2>> = corners
            .iter()
            .map(|c| project(center + *c, view_proj, image_rect))
            .collect();
        for (a, b) in [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ] {
            if let (Some(pa), Some(pb)) = (projected[a], projected[b]) {
                painter.line_segment(
                    [pa, pb],
                    egui::Stroke::new(1.0_f32, accent.gamma_multiply(0.8)),
                );
            }
        }
    }

    // Wireframe overlay.
    if app.ui.show_wireframe || app.ui.view_mode == ViewMode::Geometry {
        let pen = egui::Stroke::new(
            0.7_f32,
            egui::Color32::from_rgba_unmultiplied(200, 220, 255, 120),
        );
        let mut budget = 4000;
        for (_entity, world, mesh_id, _mat) in app.doc.project.scene.visible_mesh_instances() {
            let Some(mesh) = app.doc.project.scene.mesh(mesh_id) else {
                continue;
            };
            for face in &mesh.faces {
                let n = face.verts.len();
                for i in 0..n {
                    if budget == 0 {
                        break;
                    }
                    let a = mesh.positions[face.verts[i] as usize];
                    let b = mesh.positions[face.verts[(i + 1) % n] as usize];
                    if let (Some(pa), Some(pb)) = (
                        project(world.transform_point3(a), view_proj, image_rect),
                        project(world.transform_point3(b), view_proj, image_rect),
                    ) {
                        painter.line_segment([pa, pb], pen);
                        budget -= 1;
                    }
                }
            }
        }
    }

    // Interaction.
    let pointer = response.interact_pointer_pos();
    let shift = ui.input(|i| i.modifiers.shift);
    let alt = ui.input(|i| i.modifiers.alt);
    let middle = response.dragged_by(egui::PointerButton::Middle);
    let delta = response.drag_delta();

    if response.drag_started() {
        if matches!(app.ui.tool, Tool::Move | Tool::Rotate | Tool::Scale)
            && app.ui.selected.is_some()
        {
            app.doc.edit("Transform object", |_| {});
        }
        if app.ui.tool == Tool::Sculpt && app.ui.selected.is_some() {
            app.doc.edit("Sculpt stroke", |_| {});
        }
    }

    if response.dragged() && delta.length() > 0.0 {
        let sculpting = app.ui.tool == Tool::Sculpt;
        let transforming = matches!(app.ui.tool, Tool::Move | Tool::Rotate | Tool::Scale)
            && app.ui.selected.is_some()
            && !alt
            && !middle;
        if middle || shift || app.ui.tool == Tool::Hand {
            app.ui.camera.pan(glam::Vec2::new(delta.x, delta.y));
            app.preview_dirty = true;
        } else if app.ui.tool == Tool::Select || alt || sculpting {
            app.ui.camera.orbit(glam::Vec2::new(delta.x, delta.y));
            app.preview_dirty = true;
        }
        if transforming {
            if let Some(entity) = app.ui.selected {
                let (_, forward, right, _up) = basis(&app.ui.camera);
                let (up_v, _) = (forward, right);
                let scale_factor = app.ui.camera.distance * 0.0016;
                app.doc.silent_edit(|p| {
                    if let Some(e) = p.scene.entity_mut(entity) {
                        match app.ui.tool {
                            Tool::Move => {
                                let world_delta =
                                    right * delta.x * scale_factor + up_v * -delta.y * scale_factor;
                                e.transform.translation += world_delta;
                            }
                            Tool::Rotate => {
                                e.transform.rotation =
                                    Quat::from_rotation_y(delta.x * 0.01) * e.transform.rotation;
                            }
                            Tool::Scale => {
                                let f = (1.0 + delta.y * 0.01).clamp(0.2, 5.0);
                                e.transform.scale *= f;
                            }
                            _ => {}
                        }
                    }
                });
                app.preview_dirty = true;
            }
        }
        if sculpting && !alt {
            if let Some(pos) = pointer {
                if let Some((entity, hit)) = pick(app, &app.ui.camera, image_rect, pos) {
                    apply_sculpt(app, entity, hit);
                }
            }
        }
    }

    if response.hovered() {
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll.abs() > 0.0 {
            app.ui.camera.zoom(scroll);
            app.preview_dirty = true;
        }
    }

    if response.clicked() {
        if let Some(pos) = pointer {
            if let Some((entity, hit)) = pick(app, &app.ui.camera, image_rect, pos) {
                match app.ui.tool {
                    Tool::Sculpt => apply_sculpt(app, entity, hit),
                    Tool::Select => {
                        app.ui.selected = Some(entity);
                        app.ui.selected_material = app
                            .doc
                            .project
                            .scene
                            .entity(entity)
                            .and_then(|e| e.material);
                        app.ui.log(
                            "info",
                            format!(
                                "Selected {}",
                                app.doc
                                    .project
                                    .scene
                                    .entity(entity)
                                    .map(|e| e.name.clone())
                                    .unwrap_or_default()
                            ),
                        );
                    }
                    _ => app.ui.selected = Some(entity),
                }
            } else if app.ui.tool == Tool::Select {
                app.ui.selected = None;
            }
        }
    }

    // Stats overlay.
    if app.ui.show_stats {
        let mut lines = vec![
            format!(
                "{} · {} · {}",
                app.workspace.label(),
                app.ui.tool.label(),
                app.ui.view_mode.label()
            ),
            format!(
                "UI {:.1} ms · preview {:.0} ms · {} fps",
                app.ui.perf.average_ms,
                app.ui.perf.last_preview_ms,
                app.ui.perf.fps()
            ),
            format!(
                "Cam dist {:.2} · yaw {:.0}° · pitch {:.0}°",
                app.ui.camera.distance,
                app.ui.camera.yaw.to_degrees(),
                app.ui.camera.pitch.to_degrees()
            ),
        ];
        if let Some(entity) = app.ui.selected {
            if let Some(e) = app.doc.project.scene.entity(entity) {
                let p = e.transform.translation;
                lines.push(format!("{}  ({:.2}, {:.2}, {:.2})", e.name, p.x, p.y, p.z));
            }
        }
        if let Some(hit) = pointer.and_then(|p| {
            let ray = ray_from_pointer(&app.ui.camera, image_rect, p, aspect);
            app.doc.project.scene.raycast(ray)
        }) {
            lines.push(format!(
                "Hit {} at {:.2} m (face {})",
                app.doc
                    .project
                    .scene
                    .entity(hit.entity)
                    .map(|e| e.name.clone())
                    .unwrap_or_default(),
                hit.distance,
                hit.face
            ));
        }
        let mut y = rect.top() + 10.0;
        for line in lines {
            painter.text(
                egui::pos2(rect.left() + 12.0, y),
                egui::Align2::LEFT_TOP,
                line,
                egui::FontId::monospace(11.0),
                egui::Color32::from_rgba_unmultiplied(210, 225, 245, 200),
            );
            y += 14.0;
        }
    }

    // Axis gizmo.
    let origin = egui::pos2(rect.left() + 40.0, rect.bottom() - 40.0);
    for (dir, color, label) in [
        (Vec3::X, egui::Color32::from_rgb(220, 90, 90), "X"),
        (Vec3::Y, egui::Color32::from_rgb(120, 200, 120), "Y"),
        (Vec3::Z, egui::Color32::from_rgb(110, 150, 240), "Z"),
    ] {
        let (_, forward, right, up) = basis(&app.ui.camera);
        let screen = Vec2::new(dir.dot(right), -dir.dot(up)) * 24.0;
        let end = origin + egui::vec2(screen.x, screen.y);
        painter.line_segment([origin, end], egui::Stroke::new(1.6_f32, color));
        painter.text(
            end,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(9.0),
            color,
        );
        let _ = forward;
    }

    // Workspace hints.
    let hint = match app.workspace {
        crate::state::Workspace::Sculpt => {
            format!(
                "{} brush · radius {:.2} · strength {:.2} — drag on the surface",
                app.ui.sculpt_brush.label(),
                app.ui.sculpt_radius,
                app.ui.sculpt_strength
            )
        }
        crate::state::Workspace::Rig => {
            "Rig workspace: build R6/R15 rigs from the Create menu, then animate bones.".into()
        }
        crate::state::Workspace::Animate => format!(
            "Animation at {:.2}s · {} track(s)",
            app.ui.anim_time,
            app.doc.project.animation.tracks.len()
        ),
        crate::state::Workspace::Materials => {
            "Select a material in the left dock to edit PBR values.".into()
        }
        crate::state::Workspace::Render => {
            "Render workspace: adjust samples and press F12 for a full still.".into()
        }
        _ => "Drag to orbit · Shift/Middle-drag to pan · wheel to zoom · click to select".into(),
    };
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 8.0),
        egui::Align2::CENTER_BOTTOM,
        hint,
        egui::FontId::proportional(10.0),
        super::theme::text_dim(&theme),
    );
}

fn pick(
    app: &RoEditApp,
    camera: &OrbitCamera,
    image_rect: egui::Rect,
    pointer: egui::Pos2,
) -> Option<(roedit_core::ids::EntityId, roedit_core::scene::SceneHit)> {
    let aspect = image_rect.width() / image_rect.height().max(1.0);
    let ray = ray_from_pointer(camera, image_rect, pointer, aspect);
    app.doc
        .project
        .scene
        .raycast(ray)
        .map(|hit| (hit.entity, hit))
}

fn apply_sculpt(
    app: &mut RoEditApp,
    entity: roedit_core::ids::EntityId,
    hit: roedit_core::scene::SceneHit,
) {
    let Some(mesh_id) = app.doc.project.scene.entity(entity).and_then(|e| e.mesh) else {
        return;
    };
    let world = app.doc.project.scene.world_matrix(entity);
    let inv = world.inverse();
    let center = inv.transform_point3(hit.point);
    let scale = world.to_scale_rotation_translation().0;
    let avg_scale = ((scale.x.abs() + scale.y.abs() + scale.z.abs()) / 3.0).max(0.01);
    let radius = app.ui.sculpt_radius / avg_scale;
    let settings = SculptSettings {
        radius,
        strength: app.ui.sculpt_strength,
        ..Default::default()
    };
    let brush = app.ui.sculpt_brush;
    let mut moved = 0;
    app.doc.silent_edit(|p| {
        if let Some(mesh) = p.scene.mesh_mut(mesh_id) {
            moved = sculpt(mesh, brush, center, &settings);
        }
    });
    if moved > 0 {
        app.preview_dirty = true;
    }
}
