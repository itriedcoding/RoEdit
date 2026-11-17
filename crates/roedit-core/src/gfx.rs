//! GFX designer document: canvas presets, layers, groups, text and masks.

use crate::ids::{Counter, LayerId};
use crate::math::Vec2;
use crate::raster::RasterImage;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    SoftLight,
    HardLight,
    ColorDodge,
    ColorBurn,
    LinearBurn,
    Difference,
    Exclusion,
    Add,
    Subtract,
    Darken,
    Lighten,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    pub const ALL: [BlendMode; 19] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::SoftLight,
        BlendMode::HardLight,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::LinearBurn,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Add,
        BlendMode::Subtract,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Multiply => "Multiply",
            BlendMode::Screen => "Screen",
            BlendMode::Overlay => "Overlay",
            BlendMode::SoftLight => "Soft Light",
            BlendMode::HardLight => "Hard Light",
            BlendMode::ColorDodge => "Color Dodge",
            BlendMode::ColorBurn => "Color Burn",
            BlendMode::LinearBurn => "Linear Burn",
            BlendMode::Difference => "Difference",
            BlendMode::Exclusion => "Exclusion",
            BlendMode::Add => "Linear Add",
            BlendMode::Subtract => "Subtract",
            BlendMode::Darken => "Darken",
            BlendMode::Lighten => "Lighten",
            BlendMode::Hue => "Hue",
            BlendMode::Saturation => "Saturation",
            BlendMode::Color => "Color",
            BlendMode::Luminosity => "Luminosity",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextShadow {
    pub offset: Vec2,
    pub blur: f32,
    pub color: [f32; 4],
}

impl Default for TextShadow {
    fn default() -> Self {
        Self {
            offset: Vec2::new(1.0, 2.0),
            blur: 3.0,
            color: [0.0, 0.0, 0.0, 0.6],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextSpec {
    pub text: String,
    pub font_size: f32,
    pub color: [f32; 4],
    pub position: Vec2,
    pub alignment: TextAlign,
    pub letter_spacing: f32,
    pub line_height: f32,
    pub rotation_deg: f32,
    pub stroke_width: f32,
    pub stroke_color: [f32; 4],
    pub shadow: Option<TextShadow>,
    pub italic: bool,
    pub bold: bool,
}

impl Default for TextSpec {
    fn default() -> Self {
        Self {
            text: "RoEdit Studio".into(),
            font_size: 96.0,
            color: [1.0, 1.0, 1.0, 1.0],
            position: Vec2::ZERO,
            alignment: TextAlign::Center,
            letter_spacing: 0.0,
            line_height: 1.2,
            rotation_deg: 0.0,
            stroke_width: 0.0,
            stroke_color: [0.0, 0.0, 0.0, 1.0],
            shadow: None,
            italic: false,
            bold: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
    pub blend: BlendMode,
    pub clip_to_below: bool,
    pub is_group: bool,
    pub parent: Option<LayerId>,
    pub offset: [i32; 2],
    pub pixels: Option<Arc<RasterImage>>,
    pub text: Option<TextSpec>,
    pub mask: Option<Arc<RasterImage>>,
    pub mask_enabled: bool,
}

impl Layer {
    pub fn pixel(id: LayerId, name: impl Into<String>, image: RasterImage) -> Self {
        Self {
            id,
            name: name.into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend: BlendMode::Normal,
            clip_to_below: false,
            is_group: false,
            parent: None,
            offset: [0, 0],
            pixels: Some(Arc::new(image)),
            text: None,
            mask: None,
            mask_enabled: true,
        }
    }

    pub fn group(id: LayerId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend: BlendMode::Normal,
            clip_to_below: false,
            is_group: true,
            parent: None,
            offset: [0, 0],
            pixels: None,
            text: None,
            mask: None,
            mask_enabled: true,
        }
    }

    pub fn text_layer(id: LayerId, name: impl Into<String>, spec: TextSpec) -> Self {
        Self {
            id,
            name: name.into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend: BlendMode::Normal,
            clip_to_below: false,
            is_group: false,
            parent: None,
            offset: [0, 0],
            pixels: None,
            text: Some(spec),
            mask: None,
            mask_enabled: true,
        }
    }

    pub fn pixel_mut(&mut self) -> Option<&mut RasterImage> {
        self.pixels.as_mut().map(Arc::make_mut)
    }
}

/// Common Roblox GFX deliverables, with their standard pixel dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasPreset {
    YoutubeThumbnail,
    GameIcon,
    GameBanner,
    GroupAd,
    UgcAd,
    ClothingAd,
    AvatarShowcase,
    PromotionalPoster,
    CinematicPoster,
    ProfileGraphic,
    DiscordPromo,
    RobloxScreenshot,
    SquarePost,
    Custom,
}

impl CanvasPreset {
    pub const ALL: [CanvasPreset; 14] = [
        CanvasPreset::YoutubeThumbnail,
        CanvasPreset::GameIcon,
        CanvasPreset::GameBanner,
        CanvasPreset::GroupAd,
        CanvasPreset::UgcAd,
        CanvasPreset::ClothingAd,
        CanvasPreset::AvatarShowcase,
        CanvasPreset::PromotionalPoster,
        CanvasPreset::CinematicPoster,
        CanvasPreset::ProfileGraphic,
        CanvasPreset::DiscordPromo,
        CanvasPreset::RobloxScreenshot,
        CanvasPreset::SquarePost,
        CanvasPreset::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CanvasPreset::YoutubeThumbnail => "YouTube Thumbnail (1280x720)",
            CanvasPreset::GameIcon => "Game Icon (512x512)",
            CanvasPreset::GameBanner => "Game Banner (1920x480)",
            CanvasPreset::GroupAd => "Group Ad (728x90)",
            CanvasPreset::UgcAd => "UGC Ad (1024x1024)",
            CanvasPreset::ClothingAd => "Clothing Ad (512x512)",
            CanvasPreset::AvatarShowcase => "Avatar Showcase (1080x1080)",
            CanvasPreset::PromotionalPoster => "Promo Poster (2048x2560)",
            CanvasPreset::CinematicPoster => "Cinematic Poster (1920x1080)",
            CanvasPreset::ProfileGraphic => "Profile Graphic (800x800)",
            CanvasPreset::DiscordPromo => "Discord Promo (1200x630)",
            CanvasPreset::RobloxScreenshot => "Roblox Screenshot (1920x1080)",
            CanvasPreset::SquarePost => "Square Post (1080x1080)",
            CanvasPreset::Custom => "Custom",
        }
    }

    pub fn size(self) -> (u32, u32) {
        match self {
            CanvasPreset::YoutubeThumbnail => (1280, 720),
            CanvasPreset::GameIcon => (512, 512),
            CanvasPreset::GameBanner => (1920, 480),
            CanvasPreset::GroupAd => (728, 90),
            CanvasPreset::UgcAd => (1024, 1024),
            CanvasPreset::ClothingAd => (512, 512),
            CanvasPreset::AvatarShowcase => (1080, 1080),
            CanvasPreset::PromotionalPoster => (2048, 2560),
            CanvasPreset::CinematicPoster => (1920, 1080),
            CanvasPreset::ProfileGraphic => (800, 800),
            CanvasPreset::DiscordPromo => (1200, 630),
            CanvasPreset::RobloxScreenshot => (1920, 1080),
            CanvasPreset::SquarePost => (1080, 1080),
            CanvasPreset::Custom => (1080, 1080),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GfxDoc {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Layer index 0 is the bottom of the stack.
    pub layers: Vec<Layer>,
    pub active: Option<LayerId>,
    /// Canvas fill behind all layers, used for opaque exports.
    pub background: [f32; 4],
    pub preset: CanvasPreset,
    next_layer: Counter,
}

impl Default for GfxDoc {
    fn default() -> Self {
        Self::new(1920, 1080)
    }
}

impl GfxDoc {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            name: "GFX Document".into(),
            width: width.max(1),
            height: height.max(1),
            layers: Vec::new(),
            active: None,
            background: [0.0, 0.0, 0.0, 0.0],
            preset: CanvasPreset::Custom,
            next_layer: Counter::default(),
        }
    }

    pub fn from_preset(preset: CanvasPreset) -> Self {
        let (w, h) = preset.size();
        let mut doc = Self::new(w, h);
        doc.preset = preset;
        doc.name = preset.label().to_string();
        doc
    }

    fn new_layer_id(&mut self) -> LayerId {
        LayerId(self.next_layer.next() as u32)
    }

    pub fn add_pixel_layer(&mut self, name: impl Into<String>, image: RasterImage) -> LayerId {
        let id = self.new_layer_id();
        self.layers.push(Layer::pixel(id, name, image));
        self.active = Some(id);
        id
    }

    pub fn add_empty_layer(&mut self, name: impl Into<String>) -> LayerId {
        let img = RasterImage::new(self.width, self.height);
        self.add_pixel_layer(name, img)
    }

    pub fn add_text_layer(&mut self, name: impl Into<String>, spec: TextSpec) -> LayerId {
        let id = self.new_layer_id();
        let mut spec = spec;
        if spec.position == Vec2::ZERO {
            spec.position = Vec2::new(self.width as f32 * 0.5, self.height as f32 * 0.5);
        }
        self.layers.push(Layer::text_layer(id, name, spec));
        self.active = Some(id);
        id
    }

    pub fn add_group(&mut self, name: impl Into<String>, parent: Option<LayerId>) -> LayerId {
        let id = self.new_layer_id();
        let mut g = Layer::group(id, name);
        g.parent = parent;
        self.layers.push(g);
        self.active = Some(id);
        id
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn layer_index(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn active_layer(&self) -> Option<&Layer> {
        self.active.and_then(|id| self.layer(id))
    }

    pub fn active_layer_mut(&mut self) -> Option<&mut Layer> {
        let id = self.active?;
        self.layer_mut(id)
    }

    pub fn remove_layer(&mut self, id: LayerId) -> bool {
        // Groups take their descendants with them.
        let mut doomed = vec![id];
        let mut i = 0;
        while i < doomed.len() {
            let parent = doomed[i];
            for l in &self.layers {
                if l.parent == Some(parent) && !doomed.contains(&l.id) {
                    doomed.push(l.id);
                }
            }
            i += 1;
        }
        let before = self.layers.len();
        self.layers.retain(|l| !doomed.contains(&l.id));
        if self.active == Some(id) {
            self.active = self.layers.last().map(|l| l.id);
        }
        self.layers.len() != before
    }

    pub fn duplicate_layer(&mut self, id: LayerId) -> Option<LayerId> {
        let idx = self.layer_index(id)?;
        let mut copy = self.layers[idx].clone();
        let new_id = self.new_layer_id();
        copy.id = new_id;
        copy.name = format!("{} Copy", copy.name);
        self.layers.insert(idx + 1, copy);
        self.active = Some(new_id);
        Some(new_id)
    }

    /// Moves a layer to a new stack position (0 = bottom).
    pub fn move_layer(&mut self, id: LayerId, new_index: usize) -> bool {
        let Some(idx) = self.layer_index(id) else {
            return false;
        };
        let layer = self.layers.remove(idx);
        let target = new_index.min(self.layers.len());
        self.layers.insert(target, layer);
        true
    }

    pub fn move_layer_up(&mut self, id: LayerId) -> bool {
        match self.layer_index(id) {
            Some(i) if i + 1 < self.layers.len() => self.move_layer(id, i + 1),
            _ => false,
        }
    }

    pub fn move_layer_down(&mut self, id: LayerId) -> bool {
        match self.layer_index(id) {
            Some(i) if i > 0 => self.move_layer(id, i - 1),
            _ => false,
        }
    }

    pub fn children_of(&self, parent: Option<LayerId>) -> Vec<&Layer> {
        self.layers.iter().filter(|l| l.parent == parent).collect()
    }

    pub fn repair_counters(&mut self) {
        let max = self
            .layers
            .iter()
            .map(|l| l.id.0 as u64 + 1)
            .max()
            .unwrap_or(0);
        self.next_layer.bump_to(max.saturating_sub(1));
        if self
            .active
            .map(|a| self.layer(a).is_none())
            .unwrap_or(false)
        {
            self.active = self.layers.last().map(|l| l.id);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
    }

    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.width == 0 || self.height == 0 {
            problems.push("Canvas has zero size".into());
        }
        let ids: Vec<LayerId> = self.layers.iter().map(|l| l.id).collect();
        for l in &self.layers {
            if let Some(p) = l.parent {
                if !ids.contains(&p) {
                    problems.push(format!("Layer '{}' has a missing group parent", l.name));
                }
                if p == l.id {
                    problems.push(format!("Layer '{}' is its own parent", l.name));
                }
            }
            if let Some(px) = &l.pixels {
                if px.width != self.width || px.height != self.height {
                    problems.push(format!(
                        "Layer '{}' is {}x{} but the canvas is {}x{}",
                        l.name, px.width, px.height, self.width, self.height
                    ));
                }
            }
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_stack_operations() {
        let mut doc = GfxDoc::new(64, 64);
        let a = doc.add_empty_layer("A");
        let b = doc.add_empty_layer("B");
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.active, Some(b));
        assert!(doc.move_layer_down(b));
        assert_eq!(doc.layers[0].id, b);
        assert!(doc.move_layer_up(b));
        assert_eq!(doc.layers[1].id, b);
        assert!(doc.duplicate_layer(a).is_some());
        assert_eq!(doc.layers.len(), 3);
    }

    #[test]
    fn removing_group_removes_children() {
        let mut doc = GfxDoc::new(32, 32);
        let g = doc.add_group("Group", None);
        let child = doc.add_empty_layer("Child");
        doc.layer_mut(child).unwrap().parent = Some(g);
        let inner = doc.add_group("Inner", Some(g));
        assert_eq!(doc.layers.len(), 3);
        doc.remove_layer(g);
        assert!(doc.layers.is_empty());
        let _ = inner;
    }

    #[test]
    fn preset_sizes_are_stable() {
        assert_eq!(CanvasPreset::YoutubeThumbnail.size(), (1280, 720));
        assert_eq!(CanvasPreset::GameIcon.size(), (512, 512));
        let doc = GfxDoc::from_preset(CanvasPreset::DiscordPromo);
        assert_eq!((doc.width, doc.height), (1200, 630));
    }

    #[test]
    fn validation_detects_size_mismatch() {
        let mut doc = GfxDoc::new(10, 10);
        let id = doc.add_pixel_layer("big", RasterImage::new(20, 20));
        let problems = doc.validate();
        assert!(problems.iter().any(|p| p.contains("canvas is")));
        doc.layer_mut(id).unwrap().pixels = Some(Arc::new(RasterImage::new(10, 10)));
        assert!(doc.validate().is_empty());
    }
}
