// RoEdit Studio - native Windows creative suite for Roblox 3D, GFX and video.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use roedit_app::RoEditApp;

fn icon() -> egui::IconData {
    // A 64x64 dark tile with the studio accent gradient. Generated at startup so
    // the binary needs no external icon asset.
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;
            let vignette = 1.0 - ((fx - 0.5).abs() + (fy - 0.5).abs()).min(1.0) * 0.6;
            rgba[i] = (28.0 * vignette) as u8;
            rgba[i + 1] = (118.0 + 60.0 * fy) as u8;
            rgba[i + 2] = (190.0 + 40.0 * fx) as u8;
            rgba[i + 3] = 255;
            // A simple "R" glyph made from blocks.
            let in_ring = {
                let cx = x as f32 - 32.0;
                let cy = y as f32 - 26.0;
                let d = (cx * cx + cy * cy).sqrt();
                (d > 10.0 && d < 15.0) || (cx > 0.0 && cx < 6.0 && cy > 0.0 && cy < 20.0)
            };
            if in_ring {
                rgba[i] = 235;
                rgba[i + 1] = 240;
                rgba[i + 2] = 250;
            }
        }
    }
    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 680.0])
            .with_title("RoEdit Studio")
            .with_icon(icon()),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    let app = RoEditApp::bootstrap(cc_settings());
    eframe::run_native(
        "RoEdit Studio",
        options,
        Box::new(move |cc| Ok(Box::new(app.attach(cc)))),
    )
}

fn cc_settings() -> roedit_core::settings::AppSettings {
    roedit_core::settings::AppSettings::load()
}
