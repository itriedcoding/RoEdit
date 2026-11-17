//! Deterministic procedural node graph evaluation.
//!
//! Every generator is a closed-form mathematical function of UV, seed and time.
//! Evaluating the same graph with the same seed always produces identical
//! pixels, which is what makes procedural materials reproducible.

use crate::color;
use crate::{adjust, filters, ImageError};
use roedit_core::ids::ImageId;
use roedit_core::nodegraph::{MathOp, Node, NodeGraph, NodeId, NodeKind};
use roedit_core::raster::RasterImage;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub struct EvalContext<'a> {
    pub width: u32,
    pub height: u32,
    pub time: f32,
    pub seed: u32,
    pub images: &'a HashMap<ImageId, Arc<RasterImage>>,
}

static EMPTY_IMAGES: OnceLock<HashMap<ImageId, Arc<RasterImage>>> = OnceLock::new();

impl<'a> EvalContext<'a> {
    pub fn new(width: u32, height: u32) -> Self {
        let images = EMPTY_IMAGES.get_or_init(HashMap::new);
        Self {
            width: width.max(1),
            height: height.max(1),
            time: 0.0,
            seed: 0,
            images,
        }
    }

    pub fn with_images(mut self, images: &'a HashMap<ImageId, Arc<RasterImage>>) -> Self {
        self.images = images;
        self
    }

    pub fn with_seed(mut self, seed: u32) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_time(mut self, time: f32) -> Self {
        self.time = time;
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct GraphOutput {
    pub albedo: Option<RasterImage>,
    pub roughness: Option<RasterImage>,
    pub normal: Option<RasterImage>,
    pub emission: Option<RasterImage>,
    pub composite: Option<RasterImage>,
}

impl GraphOutput {
    /// The most useful single image for a UI preview.
    pub fn preview(&self) -> Option<RasterImage> {
        self.albedo
            .clone()
            .or_else(|| self.composite.clone())
            .or_else(|| self.emission.clone())
    }
}

fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(0x27d4_eb2d)
        .wrapping_add((y as u32).wrapping_mul(0x1656_67b1))
        .wrapping_add(seed.wrapping_mul(0x9e37_79b9));
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    (h & 0x00ff_ffff) as f32 / 0x00ff_ffff as f32
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let tx = fade(x - x0);
    let ty = fade(y - y0);
    let (xi, yi) = (x0 as i32, y0 as i32);
    let a = hash2(xi, yi, seed);
    let b = hash2(xi + 1, yi, seed);
    let c = hash2(xi, yi + 1, seed);
    let d = hash2(xi + 1, yi + 1, seed);
    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * ty
}

fn fbm(x: f32, y: f32, seed: u32, octaves: u32, roughness: f32) -> f32 {
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for o in 0..octaves.clamp(1, 8) {
        sum += value_noise(x * frequency, y * frequency, seed.wrapping_add(o * 7919)) * amplitude;
        norm += amplitude;
        amplitude *= roughness.clamp(0.0, 1.0);
        frequency *= 2.0;
    }
    if norm > 0.0 {
        sum / norm
    } else {
        0.0
    }
}

fn voronoi(x: f32, y: f32, seed: u32, jitter: f32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let mut min_dist = f32::MAX;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let cx = xi + dx as f32;
            let cy = yi + dy as f32;
            let ox = hash2(cx as i32, cy as i32, seed);
            let oy = hash2(cx as i32, cy as i32, seed ^ 0x5bd1_e995);
            let px = cx + 0.5 + (ox - 0.5) * jitter;
            let py = cy + 0.5 + (oy - 0.5) * jitter;
            let d = ((px - x).powi(2) + (py - y).powi(2)).sqrt();
            min_dist = min_dist.min(d);
        }
    }
    min_dist.clamp(0.0, 1.0)
}

fn lerp_rgb(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

fn uv_grid(width: u32, height: u32) -> impl Iterator<Item = (u32, u32, f32, f32)> {
    (0..height).flat_map(move |y| {
        (0..width).map(move |x| {
            (
                x,
                y,
                (x as f32 + 0.5) / width as f32,
                (y as f32 + 0.5) / height as f32,
            )
        })
    })
}

fn generate(width: u32, height: u32, mut f: impl FnMut(f32, f32) -> [f32; 4]) -> RasterImage {
    let mut img = RasterImage::new(width, height);
    for (x, y, u, v) in uv_grid(width, height) {
        img.set_f32(x, y, f(u, v));
    }
    img
}

fn input(
    graph: &NodeGraph,
    cache: &HashMap<NodeId, Arc<RasterImage>>,
    node: &Node,
    port: u16,
) -> Option<RasterImage> {
    let from = graph.input_of(node.id, port)?;
    cache.get(&from).map(|i| i.as_ref().clone())
}

fn input_or_black(
    graph: &NodeGraph,
    cache: &HashMap<NodeId, Arc<RasterImage>>,
    node: &Node,
    port: u16,
    width: u32,
    height: u32,
) -> RasterImage {
    input(graph, cache, node, port)
        .map(|img| {
            if img.width == width && img.height == height {
                img
            } else {
                img.resized(width, height)
            }
        })
        .unwrap_or_else(|| RasterImage::new(width, height))
}

fn math_op_index(op: MathOp) -> i32 {
    MathOp::ALL.iter().position(|m| *m == op).unwrap_or(0) as i32
}

fn apply_math(op: MathOp, a: f32, b: f32) -> f32 {
    match op {
        MathOp::Add => a + b,
        MathOp::Subtract => a - b,
        MathOp::Multiply => a * b,
        MathOp::Divide => {
            if b.abs() < 1e-6 {
                0.0
            } else {
                a / b
            }
        }
        MathOp::Power => a.max(0.0).powf(b.max(0.0)),
        MathOp::Min => a.min(b),
        MathOp::Max => a.max(b),
        MathOp::Abs => (a - 0.5).abs() * 2.0,
        MathOp::OneMinus => 1.0 - a,
        MathOp::SmoothStep => {
            let t = ((a - b) / (1.0 - b).max(1e-4)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        }
    }
}

fn eval_node(
    graph: &NodeGraph,
    node: &Node,
    cache: &HashMap<NodeId, Arc<RasterImage>>,
    ctx: &EvalContext,
) -> Result<RasterImage, ImageError> {
    let (w, h) = (ctx.width, ctx.height);
    let seed = ctx.seed;
    let img = match &node.kind {
        NodeKind::UvCoords => generate(w, h, |u, v| [u, v, 0.0, 1.0]),
        NodeKind::Value => {
            let v = node.float("value", 0.5);
            RasterImage::filled(
                w,
                h,
                [
                    (v.clamp(0.0, 1.0) * 255.0) as u8,
                    (v.clamp(0.0, 1.0) * 255.0) as u8,
                    (v.clamp(0.0, 1.0) * 255.0) as u8,
                    255,
                ],
            )
        }
        NodeKind::Time => {
            let t = ctx.time.rem_euclid(1.0);
            RasterImage::filled(
                w,
                h,
                [(t * 255.0) as u8, (t * 255.0) as u8, (t * 255.0) as u8, 255],
            )
        }
        NodeKind::SolidColor => {
            let c = node.color("color", [0.8, 0.2, 0.2, 1.0]);
            generate(w, h, |_, _| c)
        }
        NodeKind::Gradient => {
            let a = node.color("color_a", [0.1, 0.15, 0.3, 1.0]);
            let b = node.color("color_b", [0.9, 0.6, 0.3, 1.0]);
            let angle = node.float("angle", 0.0).to_radians();
            let dir = (angle.cos(), angle.sin());
            let max_proj = dir.0.abs() + dir.1.abs();
            generate(w, h, |u, v| {
                let t = (((u - 0.5) * dir.0 + (v - 0.5) * dir.1) / max_proj + 0.5).clamp(0.0, 1.0);
                lerp_rgb(a, b, t)
            })
        }
        NodeKind::Checker => {
            let scale = node.float("scale", 8.0);
            let a = node.color("color_a", [0.9, 0.9, 0.9, 1.0]);
            let b = node.color("color_b", [0.1, 0.1, 0.1, 1.0]);
            generate(w, h, |u, v| {
                let cx = (u * scale).floor() as i32;
                let cy = (v * scale).floor() as i32;
                if (cx + cy) % 2 == 0 {
                    a
                } else {
                    b
                }
            })
        }
        NodeKind::Brick => {
            let rows = node.int("rows", 8).max(1);
            let cols = node.int("cols", 4).max(1);
            let mortar = node.float("mortar", 0.05).clamp(0.0, 0.5);
            let a = node.color("color_a", [0.55, 0.26, 0.2, 1.0]);
            let b = node.color("color_b", [0.8, 0.78, 0.74, 1.0]);
            generate(w, h, |u, v| {
                let row = (v * rows as f32).floor();
                let offset = if row as i32 % 2 == 0 { 0.0 } else { 0.5 };
                let bx = u * cols as f32 + offset;
                let fx = bx - bx.floor();
                let fy = v * rows as f32 - row;
                let edge = fx < mortar || fx > 1.0 - mortar * 0.5 || fy < mortar;
                if edge {
                    b
                } else {
                    a
                }
            })
        }
        NodeKind::Noise { octaves } => {
            let scale = node.float("scale", 8.0);
            let oct = node.int("octaves", *octaves as i32).clamp(1, 8) as u32;
            let roughness = node.float("roughness", 0.5);
            let a = node.color("color_a", [0.05, 0.05, 0.06, 1.0]);
            let b = node.color("color_b", [0.95, 0.9, 0.85, 1.0]);
            let noise_seed = node.int("seed", 1337) as u32 ^ seed;
            generate(w, h, |u, v| {
                let n = fbm(u * scale, v * scale, noise_seed, oct, roughness);
                lerp_rgb(a, b, n)
            })
        }
        NodeKind::Voronoi => {
            let scale = node.float("scale", 6.0);
            let jitter = node.float("jitter", 0.85);
            let a = node.color("color_a", [0.0, 0.0, 0.0, 1.0]);
            let b = node.color("color_b", [1.0, 1.0, 1.0, 1.0]);
            let vseed = node.int("seed", 7) as u32 ^ seed;
            generate(w, h, |u, v| {
                let d = voronoi(u * scale, v * scale, vseed, jitter);
                lerp_rgb(a, b, (d * 2.0).clamp(0.0, 1.0))
            })
        }
        NodeKind::Image => {
            let id: Option<ImageId> = node
                .params
                .get("image")
                .and_then(|p| p.as_text().parse::<u32>().ok().map(ImageId));
            match id.and_then(|i| ctx.images.get(&i)) {
                Some(src) => {
                    if src.width == w && src.height == h {
                        src.as_ref().clone()
                    } else {
                        src.resized(w, h)
                    }
                }
                None => RasterImage::new(w, h),
            }
        }
        NodeKind::Mix => {
            let a = input_or_black(graph, cache, node, 0, w, h);
            let b = input_or_black(graph, cache, node, 1, w, h);
            let factor_img = input(graph, cache, node, 2);
            let factor = node.float("factor", 0.5);
            let mut out = RasterImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let t = factor_img
                        .as_ref()
                        .map(|f| {
                            let p = f.get_f32(
                                (x * f.width / w.max(1)).min(f.width - 1),
                                (y * f.height / h.max(1)).min(f.height - 1),
                            );
                            color::luma([p[0], p[1], p[2]])
                        })
                        .unwrap_or(factor)
                        .clamp(0.0, 1.0);
                    let ca = a.get_f32(x, y);
                    let cb = b.get_f32(x, y);
                    out.set_f32(x, y, lerp_rgb(ca, cb, t));
                }
            }
            out
        }
        NodeKind::Math { op } => {
            let a_img = input(graph, cache, node, 0);
            let b_img = input(graph, cache, node, 1);
            let op_index = node.int("op", math_op_index(*op)) as usize;
            let op = MathOp::ALL[op_index % MathOp::ALL.len()];
            let a_param = node.float("a", 0.5);
            let b_param = node.float("b", 0.5);
            let mut out = RasterImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let a = match &a_img {
                        Some(i) => {
                            let p = i.get_f32(x.min(i.width - 1), y.min(i.height - 1));
                            color::luma([p[0], p[1], p[2]])
                        }
                        None => a_param,
                    };
                    let b = match &b_img {
                        Some(i) => {
                            let p = i.get_f32(x.min(i.width - 1), y.min(i.height - 1));
                            color::luma([p[0], p[1], p[2]])
                        }
                        None => b_param,
                    };
                    let v = apply_math(op, a, b).clamp(0.0, 1.0);
                    out.set_f32(x, y, [v, v, v, 1.0]);
                }
            }
            out
        }
        NodeKind::ColorRamp => {
            let src = input_or_black(graph, cache, node, 0, w, h);
            let stops = node.int("stops", 4).clamp(2, 8);
            let colors: Vec<[f32; 4]> = (0..stops)
                .map(|i| {
                    node.color(
                        &format!("color_{i}"),
                        [i as f32 / (stops - 1) as f32, 0.5, 0.5, 1.0],
                    )
                })
                .collect();
            let mut out = RasterImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let p = src.get_f32(x, y);
                    let t = color::luma([p[0], p[1], p[2]]).clamp(0.0, 1.0);
                    let scaled = t * (stops - 1) as f32;
                    let i = (scaled.floor() as usize).min(stops as usize - 2);
                    let local = scaled - i as f32;
                    let mut c = lerp_rgb(colors[i], colors[i + 1], local);
                    c[3] *= 1.0;
                    out.set_f32(x, y, c);
                }
            }
            out
        }
        NodeKind::Blur => {
            let src = input_or_black(graph, cache, node, 0, w, h);
            filters::gaussian_blur(&src, node.float("radius", 4.0))
        }
        NodeKind::Invert => {
            let src = input_or_black(graph, cache, node, 0, w, h);
            adjust::invert(&src)
        }
        NodeKind::Levels => {
            let src = input_or_black(graph, cache, node, 0, w, h);
            adjust::levels(
                &src,
                node.float("in_black", 0.0),
                node.float("in_white", 1.0),
                node.float("gamma", 1.0),
                node.float("out_black", 0.0),
                node.float("out_white", 1.0),
            )
        }
        NodeKind::HueSaturation => {
            let src = input_or_black(graph, cache, node, 0, w, h);
            adjust::hue_saturation(
                &src,
                node.float("hue", 0.0),
                node.float("saturation", 1.0),
                node.float("value", 1.0) - 1.0,
            )
        }
        NodeKind::BrightnessContrast => {
            let src = input_or_black(graph, cache, node, 0, w, h);
            adjust::exposure_contrast(
                &src,
                node.float("brightness", 0.0),
                node.float("contrast", 1.0),
                0.0,
            )
        }
        NodeKind::Transform => {
            let src = input_or_black(graph, cache, node, 0, w, h);
            let tx = node.float("tx", 0.0);
            let ty = node.float("ty", 0.0);
            let scale = node.float("scale", 1.0).max(0.01);
            let rot = node.float("rotate", 0.0).to_radians();
            let (sin, cos) = (-rot).sin_cos();
            let mut out = RasterImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let u = (x as f32 + 0.5) / w as f32 - 0.5 - tx;
                    let v = (y as f32 + 0.5) / h as f32 - 0.5 - ty;
                    let rx = (u * cos - v * sin) / scale + 0.5;
                    let ry = (u * sin + v * cos) / scale + 0.5;
                    let sx = rx * src.width as f32;
                    let sy = ry * src.height as f32;
                    if sx < 0.0 || sy < 0.0 || sx >= src.width as f32 || sy >= src.height as f32 {
                        continue;
                    }
                    out.set_f32(x, y, src.sample_bilinear(sx - 0.5, sy - 0.5));
                }
            }
            out
        }
        NodeKind::NormalFromHeight => {
            let src = input_or_black(graph, cache, node, 0, w, h);
            let strength = node.float("strength", 1.0);
            let height: Vec<f32> = src.to_gray();
            let mut out = RasterImage::new(w, h);
            let at = |x: i32, y: i32| {
                let cx = x.clamp(0, w as i32 - 1) as usize;
                let cy = y.clamp(0, h as i32 - 1) as usize;
                height[cy * w as usize + cx]
            };
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    let dx = (at(x + 1, y) - at(x - 1, y)) * strength;
                    let dy = (at(x, y + 1) - at(x, y - 1)) * strength;
                    let n = glam::Vec3::new(-dx, -dy, 1.0).normalize();
                    out.set_f32(
                        x as u32,
                        y as u32,
                        [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, 1.0],
                    );
                }
            }
            out
        }
        NodeKind::ChannelSplit => {
            let src = input_or_black(graph, cache, node, 0, w, h);
            let channel = node.int("channel", 0).clamp(0, 3) as usize;
            let mut out = RasterImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let p = src.get_f32(x, y);
                    let v = p[channel];
                    out.set_f32(x, y, [v, v, v, 1.0]);
                }
            }
            out
        }
        NodeKind::OutputAlbedo
        | NodeKind::OutputRoughness
        | NodeKind::OutputNormal
        | NodeKind::OutputEmission
        | NodeKind::OutputComposite => input_or_black(graph, cache, node, 0, w, h),
    };
    Ok(img)
}

/// Evaluates a graph and returns every connected output.
pub fn evaluate_graph(graph: &NodeGraph, ctx: &EvalContext) -> Result<GraphOutput, ImageError> {
    let order = graph.topological_order().map_err(|cycle| {
        ImageError::Graph(format!(
            "graph contains a cycle: nodes {}",
            cycle
                .iter()
                .map(|c| c.0.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;
    let mut cache: HashMap<NodeId, std::sync::Arc<RasterImage>> = HashMap::new();
    for id in order {
        let Some(node) = graph.node(id) else { continue };
        let img = eval_node(graph, node, &cache, ctx)?;
        cache.insert(id, std::sync::Arc::new(img));
    }
    let mut out = GraphOutput::default();
    for node in graph.nodes.iter().filter(|n| n.is_output()) {
        let Some(img) = cache.get(&node.id).map(|i| i.as_ref().clone()) else {
            continue;
        };
        match node.kind {
            NodeKind::OutputAlbedo => out.albedo = Some(img),
            NodeKind::OutputRoughness => out.roughness = Some(img),
            NodeKind::OutputNormal => out.normal = Some(img),
            NodeKind::OutputEmission => out.emission = Some(img),
            NodeKind::OutputComposite => out.composite = Some(img),
            _ => {}
        }
    }
    Ok(out)
}

/// Renders a thumbnail of the graph's primary output.
pub fn render_graph_preview(
    graph: &NodeGraph,
    size: u32,
    images: &HashMap<ImageId, Arc<RasterImage>>,
) -> Option<RasterImage> {
    let ctx = EvalContext::new(size, size).with_images(images);
    evaluate_graph(graph, &ctx).ok().and_then(|o| o.preview())
}

/// Validates a graph for evaluation, including a missing-output check.
pub fn ensure_valid(graph: &NodeGraph) -> Result<(), ImageError> {
    let problems = graph.validate();
    if problems.is_empty() {
        Ok(())
    } else {
        Err(ImageError::Graph(problems.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roedit_core::math::Vec2;

    #[test]
    fn starter_graph_produces_albedo() {
        let graph = NodeGraph::starter_material("Test");
        let ctx = EvalContext::new(64, 64);
        let out = evaluate_graph(&graph, &ctx).unwrap();
        let albedo = out.albedo.expect("albedo output");
        assert_eq!(albedo.width, 64);
        let distinct: std::collections::BTreeSet<[u8; 4]> = albedo
            .data
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect();
        assert_eq!(distinct.len(), 2, "checker should have two colours");
    }

    #[test]
    fn noise_is_deterministic_and_seeded() {
        let mut graph = NodeGraph::new("noise");
        let noise = graph.add_node(NodeKind::Noise { octaves: 3 }, Vec2::ZERO);
        let out = graph.add_node(NodeKind::OutputAlbedo, Vec2::new(200.0, 0.0));
        graph.link_ports(noise, 0, out, 0);
        let a = evaluate_graph(&graph, &EvalContext::new(48, 48).with_seed(7)).unwrap();
        let b = evaluate_graph(&graph, &EvalContext::new(48, 48).with_seed(7)).unwrap();
        let c = evaluate_graph(&graph, &EvalContext::new(48, 48).with_seed(8)).unwrap();
        let a_data = a.albedo.unwrap().data;
        assert_eq!(a_data, b.albedo.unwrap().data);
        assert_ne!(a_data, c.albedo.unwrap().data);
    }

    #[test]
    fn cycles_are_rejected() {
        let mut graph = NodeGraph::new("cycle");
        let a = graph.add_node(NodeKind::Value, Vec2::ZERO);
        let b = graph.add_node(NodeKind::Invert, Vec2::ZERO);
        graph.link_ports(a, 0, b, 0);
        // Forcing a cycle by hand (link_ports refuses): validate must catch it.
        graph.links.push(roedit_core::nodegraph::Link {
            from: b,
            from_port: 0,
            to: b,
            to_port: 0,
        });
        assert!(evaluate_graph(&graph, &EvalContext::new(8, 8)).is_err());
        assert!(ensure_valid(&graph).is_err());
    }

    #[test]
    fn blur_node_reduces_contrast() {
        let mut graph = NodeGraph::new("blur");
        let checker = graph.add_node(NodeKind::Checker, Vec2::ZERO);
        let blur = graph.add_node(NodeKind::Blur, Vec2::new(100.0, 0.0));
        let out = graph.add_node(NodeKind::OutputAlbedo, Vec2::new(200.0, 0.0));
        graph.link_ports(checker, 0, blur, 0);
        graph.link_ports(blur, 0, out, 0);
        let blurred = evaluate_graph(&graph, &EvalContext::new(32, 32)).unwrap();
        let img = blurred.albedo.unwrap();
        let values: Vec<u8> = img.data.chunks_exact(4).map(|p| p[0]).collect();
        let min = *values.iter().min().unwrap();
        let max = *values.iter().max().unwrap();
        assert!(max - min < 255, "blur should reduce contrast");
    }

    #[test]
    fn normal_map_is_flat_for_flat_height() {
        let mut graph = NodeGraph::new("normal");
        let value = graph.add_node(NodeKind::Value, Vec2::ZERO);
        graph.node_mut(value).unwrap().set("value", 0.5f32);
        let normal = graph.add_node(NodeKind::NormalFromHeight, Vec2::ZERO);
        let out = graph.add_node(NodeKind::OutputNormal, Vec2::ZERO);
        graph.link_ports(value, 0, normal, 0);
        graph.link_ports(normal, 0, out, 0);
        let result = evaluate_graph(&graph, &EvalContext::new(8, 8)).unwrap();
        let img = result.normal.unwrap();
        let px = img.get(4, 4);
        assert!((px[0] as i32 - 128).abs() <= 2);
        assert!((px[1] as i32 - 128).abs() <= 2);
        assert!(px[2] > 250);
    }

    #[test]
    fn image_node_reads_from_context() {
        let mut images = HashMap::new();
        images.insert(
            ImageId(3),
            Arc::new(RasterImage::filled(8, 8, [10, 20, 30, 255])),
        );
        let mut graph = NodeGraph::new("img");
        let img = graph.add_node(NodeKind::Image, Vec2::ZERO);
        graph.node_mut(img).unwrap().set("image", "3");
        let out = graph.add_node(NodeKind::OutputAlbedo, Vec2::ZERO);
        graph.link_ports(img, 0, out, 0);
        let ctx = EvalContext::new(8, 8).with_images(&images);
        let result = evaluate_graph(&graph, &ctx).unwrap();
        assert_eq!(result.albedo.unwrap().get(0, 0), [10, 20, 30, 255]);
    }

    #[test]
    fn math_node_combines_inputs() {
        let mut graph = NodeGraph::new("math");
        let a = graph.add_node(NodeKind::Value, Vec2::ZERO);
        let b = graph.add_node(NodeKind::Value, Vec2::ZERO);
        graph.node_mut(a).unwrap().set("value", 0.25f32);
        graph.node_mut(b).unwrap().set("value", 0.5f32);
        let add = graph.add_node(NodeKind::Math { op: MathOp::Add }, Vec2::ZERO);
        graph.node_mut(add).unwrap().set("op", 0i32);
        let out = graph.add_node(NodeKind::OutputAlbedo, Vec2::ZERO);
        graph.link_ports(a, 0, add, 0);
        graph.link_ports(b, 0, add, 1);
        graph.link_ports(add, 0, out, 0);
        let result = evaluate_graph(&graph, &EvalContext::new(4, 4)).unwrap();
        let px = result.albedo.unwrap().get(0, 0);
        assert!((px[0] as i32 - 191).abs() <= 2, "{px:?}");
    }

    #[test]
    fn color_ramp_and_preview_helpers() {
        let graph = NodeGraph::starter_material("preview");
        let preview = render_graph_preview(&graph, 32, &HashMap::new()).expect("preview");
        assert_eq!(preview.width, 32);
        assert!(ensure_valid(&graph).is_ok());
    }
}
