//! Immediate-mode UI. Each panel lives in its own module and is a free function
//! taking the app so borrowing stays simple.

pub mod bottom;
pub mod canvas;
pub mod dialogs;
pub mod left;
pub mod palette;
pub mod right;
pub mod theme;
pub mod top;
pub mod viewport;

pub use bottom::bottom_panel;
pub use dialogs::dialogs;
pub use left::left_panel;
pub use palette::palette;
pub use right::right_panel;
pub use top::top_bar;

use crate::app::RoEditApp;
use crate::state::Workspace;
use eframe::egui;

/// The central area, switched by workspace.
pub fn central_panel(app: &mut RoEditApp, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme::viewport_bg(&app.theme())))
        .show(ctx, |ui| match app.workspace {
            Workspace::Model
            | Workspace::Sculpt
            | Workspace::Rig
            | Workspace::Animate
            | Workspace::Render
            | Workspace::Materials => viewport::viewport(app, ui),
            Workspace::Texture => canvas::paint_canvas(app, ui),
            Workspace::Gfx => canvas::gfx_canvas(app, ui),
            Workspace::Uv => canvas::uv_canvas(app, ui),
            Workspace::Assets => canvas::assets_panel(app, ui),
            Workspace::Plugins => canvas::plugins_panel(app, ui),
            Workspace::Video => canvas::video_preview(app, ui),
        });
}

impl RoEditApp {
    pub(crate) fn theme(&self) -> roedit_core::settings::Theme {
        roedit_core::settings::Theme::find(&self.settings.theme)
    }
}

pub(crate) fn rgb(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

pub(crate) fn rgb_f(c: [f32; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

pub(crate) fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(title).strong().size(12.0));
    ui.add_space(2.0);
    body(ui);
    ui.add_space(2.0);
}

/// Formats a float without trailing zeros for compact labels.
pub(crate) fn num(v: f32) -> String {
    if v.abs() >= 1000.0 || (v.abs() < 0.01 && v != 0.0) {
        format!("{v:.2e}")
    } else if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}
