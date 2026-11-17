//! Bottom dock: video timeline, animation dope sheet, log and performance.

use super::rgb;
use crate::app::RoEditApp;
use eframe::egui;

const TABS: [&str; 4] = ["Timeline", "Dope Sheet", "Log", "Performance"];

pub fn bottom_panel(app: &mut RoEditApp, ctx: &egui::Context) {
    let theme = app.theme();
    egui::TopBottomPanel::bottom("roedit-bottom")
        .resizable(true)
        .default_height(220.0)
        .height_range(110.0..=460.0)
        .frame(
            egui::Frame::none()
                .fill(rgb(theme.panel))
                .inner_margin(egui::Margin::symmetric(8.0, 4.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, tab) in TABS.iter().enumerate() {
                    let label = if *tab == "Timeline" {
                        format!(
                            "{tab} ({})",
                            app.doc
                                .project
                                .timeline
                                .as_ref()
                                .map(|t| t.clips.len())
                                .unwrap_or(0)
                        )
                    } else {
                        tab.to_string()
                    };
                    if ui
                        .selectable_label(
                            app.scratch.bottom_tab == i,
                            egui::RichText::new(label).size(12.0),
                        )
                        .clicked()
                    {
                        app.scratch.bottom_tab = i;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(&app.ui.autosave_status)
                            .small()
                            .color(super::theme::text_dim(&app.theme())),
                    );
                });
            });
            ui.separator();
            match app.scratch.bottom_tab {
                0 => timeline(app, ui),
                1 => dope_sheet(app, ui),
                2 => log_view(app, ui),
                _ => perf_view(app, ui),
            }
        });
}

fn timeline(app: &mut RoEditApp, ui: &mut egui::Ui) {
    let Some(mut timeline) = app.doc.project.timeline.clone() else {
        ui.horizontal(|ui| {
            ui.label("This project has no timeline yet.");
            if ui.button("Create timeline").clicked() {
                app.dispatch(crate::command::Action::TimelineAddColor);
            }
            if ui.button("Add text clip").clicked() {
                app.dispatch(crate::command::Action::TimelineAddText);
            }
        });
        ui.label(
            egui::RichText::new(
                "Video exports are image sequences (PNG/JPEG/TIFF) plus a WAV mix; MP4 encoding is not bundled.",
            )
            .small()
            .color(super::theme::text_dim(&app.theme())),
        );
        return;
    };
    let mut playhead = timeline.playhead;
    let fps = timeline.fps.max(1.0);
    let duration = timeline.duration.max(1.0);
    ui.horizontal(|ui| {
        if ui
            .button(if app.ui.playing { "⏸" } else { "▶" })
            .on_hover_text("Play / pause (Space)")
            .clicked()
        {
            app.dispatch(crate::command::Action::PlayPause);
        }
        if ui.button("⏮").on_hover_text("Go to start").clicked() {
            playhead = 0.0;
        }
        ui.add(
            egui::DragValue::new(&mut playhead)
                .speed(0.01)
                .range(0.0..=duration)
                .max_decimals(2)
                .prefix("t "),
        );
        ui.label(
            egui::RichText::new(format!(
                "frame {} / {:.1}s @ {fps:.0} fps",
                (playhead * fps).round() as u64,
                duration
            ))
            .small()
            .color(super::theme::text_dim(&app.theme())),
        );
        ui.separator();
        if ui.button("Add colour").clicked() {
            app.dispatch(crate::command::Action::TimelineAddColor);
        }
        if ui.button("Add text").clicked() {
            app.dispatch(crate::command::Action::TimelineAddText);
        }
        if ui.button("Split").clicked() {
            app.dispatch(crate::command::Action::TimelineSplit);
        }
        if ui.button("Delete clip").clicked() {
            app.dispatch(crate::command::Action::TimelineDeleteClip);
        }
        if ui.button("Marker").clicked() {
            app.dispatch(crate::command::Action::TimelineMarker);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::DragValue::new(&mut app.ui.timeline_zoom)
                    .range(10.0..=400.0)
                    .speed(1.0)
                    .prefix("zoom "),
            );
        });
    });

    let zoom = app.ui.timeline_zoom.max(10.0);
    let ruler_h = 18.0;
    let row_h = 26.0;
    let height = (ui.available_height() - 4.0).max(60.0);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter_at(rect);
    let theme = app.theme();
    painter.rect_filled(rect, 2.0, rgb(theme.viewport));
    let ruler = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), ruler_h));
    painter.rect_filled(ruler, 2.0, rgb(theme.panel_alt));

    let mut markers_changed = false;
    // Ruler ticks.
    let step = if zoom >= 90.0 {
        1.0
    } else if zoom >= 40.0 {
        2.0
    } else {
        5.0
    };
    let mut t = 0.0;
    while t <= duration {
        let x = rect.left() + t * zoom;
        if x > rect.right() {
            break;
        }
        painter.line_segment(
            [
                egui::pos2(x, ruler.bottom() - 5.0),
                egui::pos2(x, ruler.bottom()),
            ],
            egui::Stroke::new(1.0_f32, super::theme::text_dim(&theme)),
        );
        painter.text(
            egui::pos2(x + 2.0, ruler.top() + 1.0),
            egui::Align2::LEFT_TOP,
            format!("{t:.0}s"),
            egui::FontId::proportional(9.0),
            super::theme::text_dim(&theme),
        );
        t += step;
    }

    if let Some(pointer) = response.interact_pointer_pos() {
        if response.clicked() || response.dragged() {
            playhead = ((pointer.x - rect.left()) / zoom).clamp(0.0, duration);
            markers_changed = false;
            if pointer.y < rect.top() + ruler_h {
                playhead = (playhead * 4.0).round() / 4.0;
            }
        }
    }

    // Tracks.
    let mut y = rect.top() + ruler_h + 2.0;
    let tracks: Vec<_> = timeline
        .tracks
        .iter()
        .map(|t| (t.id, t.name.clone(), t.kind, t.kind.is_audio()))
        .collect();
    for (track_id, name, _kind, is_audio) in &tracks {
        if y + row_h > rect.bottom() {
            break;
        }
        let row = egui::Rect::from_min_size(
            egui::pos2(rect.left(), y),
            egui::vec2(rect.width(), row_h - 2.0),
        );
        painter.rect_filled(
            row,
            2.0,
            if *is_audio {
                rgb(theme.panel)
            } else {
                rgb(theme.panel_alt)
            },
        );
        painter.text(
            egui::pos2(row.left() + 4.0, row.top() + 2.0),
            egui::Align2::LEFT_TOP,
            name,
            egui::FontId::proportional(10.0),
            super::theme::text_dim(&theme),
        );
        for clip in timeline.clips.iter().filter(|c| c.track == *track_id) {
            let x0 = rect.left() + clip.start * zoom;
            let x1 = rect.left() + (clip.start + clip.duration).max(clip.start + 0.01) * zoom;
            let clip_rect = egui::Rect::from_min_max(
                egui::pos2(x0, row.top() + 2.0),
                egui::pos2(x1.min(rect.right()), row.bottom() - 2.0),
            );
            if clip_rect.width() < 1.0 {
                continue;
            }
            let selected = app.ui.selected_clip == Some(clip.id);
            let color = if *is_audio {
                egui::Color32::from_rgb(70, 140, 120)
            } else {
                egui::Color32::from_rgb(62, 96, 160)
            };
            painter.rect_filled(clip_rect, 2.0, color);
            if selected {
                painter.rect_stroke(
                    clip_rect,
                    2.0,
                    egui::Stroke::new(1.5_f32, super::theme::accent(&theme)),
                );
            }
            let label = format!("{:?}  {:.2}s", clip.source, clip.duration);
            if clip_rect.width() > 24.0 {
                painter.text(
                    egui::pos2(clip_rect.left() + 3.0, clip_rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    label,
                    egui::FontId::proportional(9.0),
                    egui::Color32::WHITE,
                );
            }
            if response.clicked() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    if clip_rect.contains(pointer) {
                        app.ui.selected_clip = Some(clip.id);
                        app.ui.selected_track = Some(clip.track);
                    }
                }
            }
        }
        y += row_h;
    }

    // Markers.
    for marker in &timeline.markers {
        let x = rect.left() + marker.time * zoom;
        painter.line_segment(
            [
                egui::pos2(x, rect.top() + ruler_h),
                egui::pos2(x, rect.bottom()),
            ],
            egui::Stroke::new(1.0_f32, super::theme::warning(&theme)),
        );
    }

    // Playhead.
    let px = rect.left() + playhead * zoom;
    painter.line_segment(
        [egui::pos2(px, rect.top()), egui::pos2(px, rect.bottom())],
        egui::Stroke::new(1.5_f32, super::theme::error(&theme)),
    );
    painter.circle_filled(
        egui::pos2(px, rect.top() + ruler_h),
        3.5,
        super::theme::error(&theme),
    );

    timeline.playhead = playhead;
    if (playhead - app.ui.anim_time).abs() > 1e-4 {
        app.ui.anim_time = playhead;
    }
    if (timeline.playhead
        - app
            .doc
            .project
            .timeline
            .as_ref()
            .map(|t| t.playhead)
            .unwrap_or(-1.0))
    .abs()
        > 1e-6
    {
        app.doc.project.timeline = Some(timeline.clone());
        app.preview_dirty = true;
    }
    let _ = markers_changed;

    ui.horizontal(|ui| {
        if ui.button("Export sequence…").clicked() {
            app.dispatch(crate::command::Action::ExportSequence);
        }
        ui.label(
            egui::RichText::new(
                "Frames are written as individual images; audio mix is saved as mix.wav.",
            )
            .small()
            .color(super::theme::text_dim(&app.theme())),
        );
    });
}

fn dope_sheet(app: &mut RoEditApp, ui: &mut egui::Ui) {
    let tracks: Vec<_> = app
        .doc
        .project
        .animation
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            (
                i,
                t.name.clone(),
                t.target.label().to_string(),
                t.keys.iter().map(|k| k.time).collect::<Vec<_>>(),
                t.enabled,
            )
        })
        .collect();
    if tracks.is_empty() {
        ui.label("No animation tracks yet. Select an object and press K to insert a keyframe.");
        if ui.button("Insert keyframe").clicked() {
            app.dispatch(crate::command::Action::AddKeyframe);
        }
        return;
    }
    let mut seek = None;
    let mut toggle = None;
    let mut remove = None;
    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 26.0)
        .show(ui, |ui| {
            for (index, name, target, keys, enabled) in &tracks {
                ui.horizontal(|ui| {
                    let mut on = *enabled;
                    if ui.checkbox(&mut on, "").changed() {
                        toggle = Some((*index, on));
                    }
                    ui.label(egui::RichText::new(name.clone()).strong());
                    ui.label(
                        egui::RichText::new(target)
                            .small()
                            .color(super::theme::text_dim(&app.theme())),
                    );
                    if ui.small_button("×").on_hover_text("Delete track").clicked() {
                        remove = Some(*index);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for time in keys.iter().rev() {
                            if ui
                                .small_button(format!("◆ {time:.2}"))
                                .on_hover_text("Jump to key")
                                .clicked()
                            {
                                seek = Some(*time);
                            }
                        }
                    });
                });
            }
        });
    if let Some((index, on)) = toggle {
        app.doc.edit("Toggle track", |p| {
            if let Some(t) = p.animation.tracks.get_mut(index) {
                t.enabled = on;
            }
        });
    }
    if let Some(index) = remove {
        app.doc
            .edit("Delete track", |p| p.animation.remove_track(index));
    }
    if let Some(time) = seek {
        app.ui.anim_time = time;
        app.apply_animation_at(time);
        app.preview_dirty = true;
    }
    ui.horizontal(|ui| {
        if ui.button("Key selected").clicked() {
            app.dispatch(crate::command::Action::AddKeyframe);
        }
        if ui.button("Delete key at time").clicked() {
            app.dispatch(crate::command::Action::DeleteKeyframe);
        }
        ui.label(
            egui::RichText::new(format!("{} track(s)", tracks.len()))
                .small()
                .color(super::theme::text_dim(&app.theme())),
        );
    });
}

fn log_view(app: &mut RoEditApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{} entries", app.ui.log.len()))
                .small()
                .color(super::theme::text_dim(&app.theme())),
        );
        if ui.button("Clear").clicked() {
            app.dispatch(crate::command::Action::ClearLog);
        }
    });
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .max_height(ui.available_height() - 4.0)
        .show(ui, |ui| {
            let theme = app.theme();
            for line in &app.ui.log {
                let color = match line.level {
                    "error" => super::theme::error(&theme),
                    "success" => super::theme::success(&theme),
                    "warn" | "warning" => super::theme::warning(&theme),
                    _ => super::theme::text_dim(&theme),
                };
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&line.time)
                            .monospace()
                            .small()
                            .color(super::theme::text_dim(&theme)),
                    );
                    ui.label(egui::RichText::new(&line.message).small().color(color));
                });
            }
        });
}

fn perf_view(app: &mut RoEditApp, ui: &mut egui::Ui) {
    let stats = app.doc.project.scene.statistics();
    ui.horizontal_wrapped(|ui| {
        for (label, value) in [
            ("Frame", format!("{:.2} ms", app.ui.perf.frame_ms)),
            ("Average", format!("{:.2} ms", app.ui.perf.average_ms)),
            ("FPS", format!("{:.0}", app.ui.perf.fps())),
            ("Preview", format!("{:.0} ms", app.ui.perf.last_preview_ms)),
            ("Render", format!("{:.0} ms", app.ui.perf.last_render_ms)),
            ("Triangles", stats.triangles.to_string()),
            ("Vertices", stats.vertices.to_string()),
            (
                "Images",
                format!("{:.1} KB", stats.image_bytes as f32 / 1024.0),
            ),
        ] {
            ui.label(egui::RichText::new(format!("{label}: ")).small());
            ui.label(
                egui::RichText::new(value)
                    .small()
                    .monospace()
                    .color(super::theme::accent(&app.theme())),
            );
            ui.separator();
        }
    });
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(
            ui.available_width(),
            (ui.available_height() - 6.0).max(30.0),
        ),
        egui::Sense::hover(),
    );
    let theme = app.theme();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, rgb(theme.viewport));
    let samples = &app.ui.perf.samples;
    if samples.len() > 1 {
        let max = samples.iter().cloned().fold(1.0f32, f32::max).max(16.0);
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
            egui::Stroke::new(1.0_f32, super::theme::accent(&theme)),
        ));
        painter.text(
            egui::pos2(rect.left() + 4.0, rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            format!("UI frame time, max {max:.1} ms"),
            egui::FontId::proportional(9.0),
            super::theme::text_dim(&theme),
        );
    }
}
