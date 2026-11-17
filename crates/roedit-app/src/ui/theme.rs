//! Dark professional theme derived from the persisted colour palette.

use super::rgb;
use eframe::egui;
use roedit_core::settings::{AppSettings, Theme};

/// Slightly darker than the viewport colour so the preview frame reads.
pub fn viewport_bg(theme: &Theme) -> egui::Color32 {
    let c = theme.viewport;
    egui::Color32::from_rgb(
        c[0].saturating_sub(4),
        c[1].saturating_sub(4),
        c[2].saturating_sub(4),
    )
}

pub fn apply(ctx: &egui::Context, settings: &AppSettings) {
    let theme = settings.theme_obj();
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = rgb(theme.panel);
    visuals.window_fill = rgb(theme.window);
    visuals.extreme_bg_color = rgb(theme.viewport);
    visuals.faint_bg_color = rgb(theme.panel_alt);
    visuals.window_stroke = egui::Stroke::new(1.0_f32, rgb(theme.border));
    visuals.selection.bg_fill = rgb(theme.accent);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, rgb(theme.text));
    visuals.hyperlink_color = rgb(theme.accent);
    visuals.override_text_color = Some(rgb(theme.text));

    visuals.widgets.noninteractive.bg_fill = rgb(theme.panel_alt);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, rgb(theme.border));
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, rgb(theme.text_dim));
    visuals.widgets.inactive.bg_fill = rgb(theme.widget);
    visuals.widgets.inactive.weak_bg_fill = rgb(theme.panel_alt);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, rgb(theme.border));
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, rgb(theme.text));
    visuals.widgets.hovered.bg_fill = rgb(theme.widget_hover);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, rgb(theme.accent));
    visuals.widgets.active.bg_fill = rgb(theme.widget_active);
    visuals.widgets.open.bg_fill = rgb(theme.widget_hover);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(7.0, 3.0);
    style.spacing.slider_width = 140.0;
    style.spacing.interact_size.y = 20.0;
    style.animation_time = if settings.reduce_motion { 0.0 } else { 0.12 };
    ctx.set_style(style);
}

/// Accent colour for the current theme (used by custom painting).
pub fn accent(theme: &Theme) -> egui::Color32 {
    rgb(theme.accent)
}

pub fn text_dim(theme: &Theme) -> egui::Color32 {
    rgb(theme.text_dim)
}

pub fn success(theme: &Theme) -> egui::Color32 {
    rgb(theme.success)
}

pub fn warning(theme: &Theme) -> egui::Color32 {
    rgb(theme.warning)
}

pub fn error(theme: &Theme) -> egui::Color32 {
    rgb(theme.error)
}
