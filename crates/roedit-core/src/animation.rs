//! Keyframe animation: easing curves, typed tracks, sampling and application
//! of a pose onto the scene at a given time.

use crate::ids::{ClipId, Counter, EntityId, LightId, MaterialId, TrackId};
use crate::math::{Vec2, Vec3};
use crate::scene::Scene;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ease {
    Linear,
    Step,
    In,
    Out,
    InOut,
    Sine,
    Quad,
    Cubic,
    Quart,
    Expo,
    Back,
    Bounce,
    Elastic,
}

impl Ease {
    pub const ALL: [Ease; 13] = [
        Ease::Linear,
        Ease::Step,
        Ease::In,
        Ease::Out,
        Ease::InOut,
        Ease::Sine,
        Ease::Quad,
        Ease::Cubic,
        Ease::Quart,
        Ease::Expo,
        Ease::Back,
        Ease::Bounce,
        Ease::Elastic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Ease::Linear => "Linear",
            Ease::Step => "Constant",
            Ease::In => "Ease In",
            Ease::Out => "Ease Out",
            Ease::InOut => "Ease In/Out",
            Ease::Sine => "Sine",
            Ease::Quad => "Quad",
            Ease::Cubic => "Cubic",
            Ease::Quart => "Quart",
            Ease::Expo => "Expo",
            Ease::Back => "Back",
            Ease::Bounce => "Bounce",
            Ease::Elastic => "Elastic",
        }
    }

    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::Step => {
                if t >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
            Ease::In => t * t,
            Ease::Out => 1.0 - (1.0 - t) * (1.0 - t),
            Ease::InOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }
            Ease::Sine => 0.5 - 0.5 * (std::f32::consts::PI * t).cos(),
            Ease::Quad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Ease::Cubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Ease::Quart => {
                if t < 0.5 {
                    8.0 * t * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
                }
            }
            Ease::Expo => {
                if t <= 0.0 {
                    0.0
                } else if t >= 1.0 {
                    1.0
                } else if t < 0.5 {
                    2f32.powf(20.0 * t - 10.0) / 2.0
                } else {
                    (2.0 - 2f32.powf(-20.0 * t + 10.0)) / 2.0
                }
            }
            Ease::Back => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Ease::Bounce => {
                let n1 = 7.5625;
                let d1 = 2.75;
                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    let t = t - 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    let t = t - 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d1;
                    n1 * t * t + 0.984375
                }
            }
            Ease::Elastic => {
                if t <= 0.0 {
                    0.0
                } else if t >= 1.0 {
                    1.0
                } else {
                    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                    2f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }
        }
    }
}

/// Cubic-bezier easing used by graph-editor handles. `x1`,`x2` are clamped to
/// 0..1 to keep the curve a function of time.
pub fn bezier_ease(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let x1 = x1.clamp(0.0, 1.0);
    let x2 = x2.clamp(0.0, 1.0);
    let bx =
        |u: f32| 3.0 * (1.0 - u) * (1.0 - u) * u * x1 + 3.0 * (1.0 - u) * u * u * x2 + u * u * u;
    let by =
        |u: f32| 3.0 * (1.0 - u) * (1.0 - u) * u * y1 + 3.0 * (1.0 - u) * u * u * y2 + u * u * u;
    // Newton-Raphson with bisection fallback.
    let mut u = t;
    for _ in 0..8 {
        let x = bx(u) - t;
        if x.abs() < 1e-5 {
            break;
        }
        let dx = 3.0 * (1.0 - u) * (1.0 - u) * x1
            + 6.0 * (1.0 - u) * u * (x2 - x1)
            + 3.0 * u * u * (1.0 - x2);
        if dx.abs() < 1e-6 {
            break;
        }
        u = (u - x / dx).clamp(0.0, 1.0);
    }
    by(u)
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimValue {
    Scalar(f32),
    Vector(Vec3),
    Color([f32; 4]),
}

impl AnimValue {
    pub fn as_scalar(self) -> f32 {
        match self {
            AnimValue::Scalar(v) => v,
            AnimValue::Vector(v) => v.x,
            AnimValue::Color(c) => c[0],
        }
    }

    pub fn as_vector(self) -> Vec3 {
        match self {
            AnimValue::Scalar(v) => Vec3::splat(v),
            AnimValue::Vector(v) => v,
            AnimValue::Color(c) => Vec3::new(c[0], c[1], c[2]),
        }
    }

    pub fn as_color(self) -> [f32; 4] {
        match self {
            AnimValue::Color(c) => c,
            AnimValue::Vector(v) => [v.x, v.y, v.z, 1.0],
            AnimValue::Scalar(v) => [v, v, v, 1.0],
        }
    }

    pub fn lerp(a: Self, b: Self, t: f32) -> Self {
        match (a, b) {
            (AnimValue::Vector(x), AnimValue::Vector(y)) => AnimValue::Vector(x.lerp(y, t)),
            (AnimValue::Scalar(x), AnimValue::Vector(y)) => {
                AnimValue::Vector(Vec3::splat(x).lerp(y, t))
            }
            (AnimValue::Vector(x), AnimValue::Scalar(y)) => {
                AnimValue::Vector(x.lerp(Vec3::splat(y), t))
            }
            (AnimValue::Color(x), AnimValue::Color(y)) => {
                AnimValue::Color(std::array::from_fn(|i| x[i] + (y[i] - x[i]) * t))
            }
            (a, b) => AnimValue::Scalar(a.as_scalar() + (b.as_scalar() - a.as_scalar()) * t),
        }
    }

    pub fn value_type(self) -> &'static str {
        match self {
            AnimValue::Scalar(_) => "scalar",
            AnimValue::Vector(_) => "vector",
            AnimValue::Color(_) => "colour",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialProp {
    Metallic,
    Roughness,
    EmissionStrength,
    Opacity,
}

impl MaterialProp {
    pub const ALL: [MaterialProp; 4] = [
        MaterialProp::Metallic,
        MaterialProp::Roughness,
        MaterialProp::EmissionStrength,
        MaterialProp::Opacity,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MaterialProp::Metallic => "Metallic",
            MaterialProp::Roughness => "Roughness",
            MaterialProp::EmissionStrength => "Emission",
            MaterialProp::Opacity => "Opacity",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    EntityTranslation(EntityId),
    EntityRotation(EntityId),
    EntityScale(EntityId),
    CameraPosition,
    CameraTarget,
    CameraFov,
    CameraRoll,
    CameraFstop,
    CameraFocus,
    Material(MaterialId, MaterialProp),
    LightIntensity(LightId),
    LightColor(LightId),
    ClipOpacity(ClipId),
    ClipVolume(ClipId),
    ClipSpeed(ClipId),
}

impl Target {
    pub fn label(&self) -> String {
        match self {
            Target::EntityTranslation(id) => format!("{id} Translation"),
            Target::EntityRotation(id) => format!("{id} Rotation"),
            Target::EntityScale(id) => format!("{id} Scale"),
            Target::CameraPosition => "Camera Position".into(),
            Target::CameraTarget => "Camera Target".into(),
            Target::CameraFov => "Camera FOV".into(),
            Target::CameraRoll => "Camera Roll".into(),
            Target::CameraFstop => "Camera F-stop".into(),
            Target::CameraFocus => "Camera Focus".into(),
            Target::Material(id, prop) => format!("{id} {}", prop.label()),
            Target::LightIntensity(id) => format!("{id} Intensity"),
            Target::LightColor(id) => format!("{id} Colour"),
            Target::ClipOpacity(id) => format!("{id} Opacity"),
            Target::ClipVolume(id) => format!("{id} Volume"),
            Target::ClipSpeed(id) => format!("{id} Speed"),
        }
    }

    pub fn expects_vector(&self) -> bool {
        matches!(
            self,
            Target::EntityTranslation(_)
                | Target::EntityRotation(_)
                | Target::EntityScale(_)
                | Target::CameraPosition
                | Target::CameraTarget
                | Target::LightColor(_)
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Key {
    pub time: f32,
    pub value: AnimValue,
    pub ease: Ease,
    /// Optional graph-editor handles in normalised (time, value) space.
    pub out_handle: Option<Vec2>,
    pub in_handle: Option<Vec2>,
}

impl Key {
    pub fn new(time: f32, value: AnimValue) -> Self {
        Self {
            time,
            value,
            ease: Ease::InOut,
            out_handle: None,
            in_handle: None,
        }
    }

    pub fn with_ease(mut self, ease: Ease) -> Self {
        self.ease = ease;
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub target: Target,
    pub keys: Vec<Key>,
    pub enabled: bool,
    pub muted: bool,
    pub locked: bool,
    pub color: [u8; 3],
}

impl Track {
    pub fn new(id: TrackId, target: Target) -> Self {
        let name = target.label();
        Self {
            id,
            name,
            target,
            keys: Vec::new(),
            enabled: true,
            muted: false,
            locked: false,
            color: [120, 170, 255],
        }
    }

    pub fn insert_key(&mut self, time: f32, value: AnimValue) -> usize {
        let time = time.max(0.0);
        if let Some(pos) = self.keys.iter().position(|k| (k.time - time).abs() < 1e-4) {
            self.keys[pos].value = value;
            return pos;
        }
        let pos = self
            .keys
            .iter()
            .position(|k| k.time > time)
            .unwrap_or(self.keys.len());
        self.keys.insert(pos, Key::new(time, value));
        pos
    }

    pub fn remove_key_at(&mut self, time: f32) -> bool {
        let before = self.keys.len();
        self.keys.retain(|k| (k.time - time).abs() > 1e-4);
        self.keys.len() != before
    }

    pub fn sample(&self, t: f32) -> Option<AnimValue> {
        if self.keys.is_empty() {
            return None;
        }
        if t <= self.keys[0].time {
            return Some(self.keys[0].value);
        }
        let last = self.keys.last().unwrap();
        if t >= last.time {
            return Some(last.value);
        }
        for w in self.keys.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if t >= a.time && t <= b.time {
                let span = (b.time - a.time).max(1e-6);
                let raw = (t - a.time) / span;
                let eased = match (a.out_handle, b.in_handle) {
                    (Some(h1), Some(h2)) => bezier_ease(raw, h1.x, h1.y, h2.x, h2.y),
                    _ => a.ease.apply(raw),
                };
                return Some(AnimValue::lerp(a.value, b.value, eased));
            }
        }
        Some(last.value)
    }

    pub fn duration(&self) -> f32 {
        self.keys.last().map(|k| k.time).unwrap_or(0.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Marker {
    pub time: f32,
    pub name: String,
    pub color: [u8; 3],
}

impl Marker {
    pub fn new(time: f32, name: impl Into<String>) -> Self {
        Self {
            time,
            name: name.into(),
            color: [240, 180, 80],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Animation {
    pub name: String,
    pub tracks: Vec<Track>,
    pub fps: f32,
    pub duration: f32,
    pub looping: bool,
    pub markers: Vec<Marker>,
    next_track: Counter,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            name: "Animation".into(),
            tracks: Vec::new(),
            fps: 30.0,
            duration: 10.0,
            looping: false,
            markers: Vec::new(),
            next_track: Counter::default(),
        }
    }
}

impl Animation {
    pub fn add_track(&mut self, target: Target) -> usize {
        let id = TrackId(self.next_track.next() as u32);
        self.tracks.push(Track::new(id, target));
        self.tracks.len() - 1
    }

    pub fn remove_track(&mut self, index: usize) {
        if index < self.tracks.len() {
            self.tracks.remove(index);
        }
    }

    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// Remove tracks that reference an entity (used when deleting objects).
    pub fn remove_tracks_for_entity(&mut self, entity: EntityId) -> usize {
        let before = self.tracks.len();
        self.tracks.retain(|t| match t.target {
            Target::EntityTranslation(e) | Target::EntityRotation(e) | Target::EntityScale(e) => {
                e != entity
            }
            _ => true,
        });
        before - self.tracks.len()
    }

    pub fn remove_tracks_for_material(&mut self, mat: MaterialId) -> usize {
        let before = self.tracks.len();
        self.tracks
            .retain(|t| !matches!(t.target, Target::Material(m, _) if m == mat));
        before - self.tracks.len()
    }

    pub fn remove_tracks_for_clip(&mut self, clip: ClipId) -> usize {
        let before = self.tracks.len();
        self.tracks.retain(|t| {
            !matches!(
                t.target,
                Target::ClipOpacity(c) | Target::ClipVolume(c) | Target::ClipSpeed(c) if c == clip
            )
        });
        before - self.tracks.len()
    }

    pub fn repair_counters(&mut self) {
        let max = self
            .tracks
            .iter()
            .map(|t| t.id.0 as u64 + 1)
            .max()
            .unwrap_or(0);
        self.next_track.bump_to(max.saturating_sub(1));
    }

    /// Applies every enabled track at `time` to the scene.
    /// Returns the number of values applied.
    pub fn apply_at(&self, time: f32, scene: &mut Scene) -> usize {
        let mut applied = 0;
        for track in &self.tracks {
            if !track.enabled || track.muted {
                continue;
            }
            let Some(value) = track.sample(time) else {
                continue;
            };
            match track.target {
                Target::EntityTranslation(e) => {
                    let v = value.as_vector();
                    let world_parent = scene
                        .entity(e)
                        .and_then(|ent| ent.parent)
                        .map(|p| scene.world_matrix(p))
                        .unwrap_or(crate::math::Mat4::IDENTITY);
                    let local = world_parent.inverse().transform_point3(v);
                    if let Some(ent) = scene.entity_mut(e) {
                        ent.transform.translation = local;
                        applied += 1;
                    }
                }
                Target::EntityRotation(e) => {
                    if let Some(ent) = scene.entity_mut(e) {
                        let d = value.as_vector();
                        ent.transform.set_rotation_euler_deg(d);
                        applied += 1;
                    }
                }
                Target::EntityScale(e) => {
                    if let Some(ent) = scene.entity_mut(e) {
                        ent.transform.scale = value.as_vector();
                        applied += 1;
                    }
                }
                Target::CameraPosition => {
                    scene.camera.position = value.as_vector();
                    applied += 1;
                }
                Target::CameraTarget => {
                    scene.camera.target = value.as_vector();
                    applied += 1;
                }
                Target::CameraFov => {
                    scene.camera.fov_y_deg = value.as_scalar();
                    applied += 1;
                }
                Target::CameraRoll => {
                    scene.camera.roll_deg = value.as_scalar();
                    applied += 1;
                }
                Target::CameraFstop => {
                    scene.camera.fstop = value.as_scalar();
                    applied += 1;
                }
                Target::CameraFocus => {
                    scene.camera.focus_distance = value.as_scalar();
                    applied += 1;
                }
                Target::Material(id, prop) => {
                    if let Some(m) = scene.material_mut(id) {
                        let v = value.as_scalar();
                        match prop {
                            MaterialProp::Metallic => m.metallic = v,
                            MaterialProp::Roughness => m.roughness = v,
                            MaterialProp::EmissionStrength => m.emission_strength = v,
                            MaterialProp::Opacity => m.opacity = v,
                        }
                        applied += 1;
                    }
                }
                Target::LightIntensity(id) => {
                    if let Some(l) = scene.light_mut(id) {
                        l.intensity = value.as_scalar();
                        applied += 1;
                    }
                }
                Target::LightColor(id) => {
                    if let Some(l) = scene.light_mut(id) {
                        let c = value.as_color();
                        l.color = [c[0], c[1], c[2]];
                        applied += 1;
                    }
                }
                // Clip parameters are applied by the video compositor.
                Target::ClipOpacity(_) | Target::ClipVolume(_) | Target::ClipSpeed(_) => {}
            }
        }
        applied
    }

    pub fn max_time(&self) -> f32 {
        self.tracks.iter().map(|t| t.duration()).fold(0.0, f32::max)
    }

    pub fn keyframe_count(&self) -> usize {
        self.tracks.iter().map(|t| t.keys.len()).sum()
    }

    pub fn add_marker(&mut self, time: f32, name: impl Into<String>) {
        self.markers.push(Marker::new(time.max(0.0), name));
        self.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_endpoints() {
        for e in Ease::ALL {
            let a = e.apply(0.0);
            let b = e.apply(1.0);
            assert!(a.abs() < 1e-3, "{:?} start {}", e, a);
            assert!((b - 1.0).abs() < 1e-3, "{:?} end {}", e, b);
        }
    }

    #[test]
    fn bezier_midpoint_is_between() {
        let v = bezier_ease(0.5, 0.25, 0.1, 0.25, 1.0);
        assert!(v > 0.0 && v < 1.0);
        assert!((bezier_ease(0.0, 0.2, 0.0, 0.8, 1.0)).abs() < 1e-4);
        assert!((bezier_ease(1.0, 0.2, 0.0, 0.8, 1.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn track_sampling_and_keys() {
        let mut t = Track::new(TrackId(0), Target::CameraFov);
        t.insert_key(0.0, AnimValue::Scalar(45.0));
        t.insert_key(2.0, AnimValue::Scalar(85.0));
        assert!((t.sample(-1.0).unwrap().as_scalar() - 45.0).abs() < 1e-4);
        assert!((t.sample(3.0).unwrap().as_scalar() - 85.0).abs() < 1e-4);
        assert!((t.sample(1.0).unwrap().as_scalar() - 65.0).abs() < 1e-3);
        assert_eq!(t.keys.len(), 2);
        // Replacing an existing key keeps the count stable.
        t.insert_key(2.0, AnimValue::Scalar(90.0));
        assert_eq!(t.keys.len(), 2);
        assert!(t.remove_key_at(2.0));
        assert_eq!(t.keys.len(), 1);
    }

    #[test]
    fn vector_lerp_preserves_type() {
        let a = AnimValue::Vector(Vec3::ZERO);
        let b = AnimValue::Vector(Vec3::new(2.0, 4.0, 6.0));
        let m = AnimValue::lerp(a, b, 0.5).as_vector();
        assert!((m - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-5);
    }

    #[test]
    fn apply_animates_scene() {
        let mut scene = Scene::new("t");
        let e = scene.add_entity("Cube", None);
        let mut anim = Animation::default();
        let idx = anim.add_track(Target::EntityTranslation(e));
        anim.tracks[idx].insert_key(0.0, AnimValue::Vector(Vec3::ZERO));
        anim.tracks[idx].insert_key(1.0, AnimValue::Vector(Vec3::new(0.0, 2.0, 0.0)));
        let n = anim.apply_at(0.5, &mut scene);
        assert_eq!(n, 1);
        assert!((scene.world_position(e).y - 1.0).abs() < 1e-3);
    }

    #[test]
    fn removing_entity_tracks() {
        let mut anim = Animation::default();
        anim.add_track(Target::EntityTranslation(EntityId(1)));
        anim.add_track(Target::CameraFov);
        assert_eq!(anim.remove_tracks_for_entity(EntityId(1)), 1);
        assert_eq!(anim.tracks.len(), 1);
    }
}
