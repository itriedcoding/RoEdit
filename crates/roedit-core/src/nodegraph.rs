//! Node graph data model used by the material, texture, and compositing node
//! editors. Evaluation happens in `roedit-image::graph`.

use crate::ids::Counter;
use crate::math::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortKind {
    /// RGBA image.
    Image,
    /// Single channel value, stored as a grayscale image.
    Scalar,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    // ---- generators -------------------------------------------------------
    UvCoords,
    Value,
    SolidColor,
    Gradient,
    Checker,
    Brick,
    Noise { octaves: u32 },
    Voronoi,
    Image,
    Time,
    // ---- processing -------------------------------------------------------
    Mix,
    Math { op: MathOp },
    ColorRamp,
    Blur,
    Invert,
    Levels,
    HueSaturation,
    BrightnessContrast,
    Transform,
    NormalFromHeight,
    ChannelSplit,
    // ---- outputs ----------------------------------------------------------
    OutputAlbedo,
    OutputRoughness,
    OutputNormal,
    OutputEmission,
    OutputComposite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MathOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Min,
    Max,
    Abs,
    OneMinus,
    SmoothStep,
}

impl MathOp {
    pub const ALL: [MathOp; 10] = [
        MathOp::Add,
        MathOp::Subtract,
        MathOp::Multiply,
        MathOp::Divide,
        MathOp::Power,
        MathOp::Min,
        MathOp::Max,
        MathOp::Abs,
        MathOp::OneMinus,
        MathOp::SmoothStep,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MathOp::Add => "Add",
            MathOp::Subtract => "Subtract",
            MathOp::Multiply => "Multiply",
            MathOp::Divide => "Divide",
            MathOp::Power => "Power",
            MathOp::Min => "Minimum",
            MathOp::Max => "Maximum",
            MathOp::Abs => "Absolute",
            MathOp::OneMinus => "One Minus",
            MathOp::SmoothStep => "Smooth Step",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    Color([f32; 4]),
    Text(String),
}

impl ParamValue {
    pub fn as_float(&self) -> f32 {
        match self {
            ParamValue::Float(v) => *v,
            ParamValue::Int(v) => *v as f32,
            ParamValue::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            ParamValue::Color(c) => c[0],
            ParamValue::Text(_) => 0.0,
        }
    }

    pub fn as_int(&self) -> i32 {
        self.as_float() as i32
    }

    pub fn as_bool(&self) -> bool {
        self.as_float() > 0.5
    }

    pub fn as_color(&self) -> [f32; 4] {
        match self {
            ParamValue::Color(c) => *c,
            v => {
                let f = v.as_float();
                [f, f, f, 1.0]
            }
        }
    }

    pub fn as_text(&self) -> &str {
        match self {
            ParamValue::Text(s) => s,
            _ => "",
        }
    }
}

impl From<f32> for ParamValue {
    fn from(v: f32) -> Self {
        ParamValue::Float(v)
    }
}

impl From<i32> for ParamValue {
    fn from(v: i32) -> Self {
        ParamValue::Int(v)
    }
}

impl From<bool> for ParamValue {
    fn from(v: bool) -> Self {
        ParamValue::Bool(v)
    }
}

impl From<[f32; 4]> for ParamValue {
    fn from(v: [f32; 4]) -> Self {
        ParamValue::Color(v)
    }
}

impl From<&str> for ParamValue {
    fn from(v: &str) -> Self {
        ParamValue::Text(v.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub name: String,
    pub position: Vec2,
    pub params: BTreeMap<String, ParamValue>,
    pub preview_visible: bool,
}

impl Node {
    pub fn new(id: NodeId, kind: NodeKind, position: Vec2) -> Self {
        let name = node_kind_label(&kind).to_string();
        let mut params = BTreeMap::new();
        for (key, value) in default_params(&kind) {
            params.insert(key.to_string(), value);
        }
        Self {
            id,
            kind,
            name,
            position,
            params,
            preview_visible: false,
        }
    }

    pub fn float(&self, key: &str, default: f32) -> f32 {
        self.params
            .get(key)
            .map(|v| v.as_float())
            .unwrap_or(default)
    }

    pub fn int(&self, key: &str, default: i32) -> i32 {
        self.params.get(key).map(|v| v.as_int()).unwrap_or(default)
    }

    pub fn color(&self, key: &str, default: [f32; 4]) -> [f32; 4] {
        self.params
            .get(key)
            .map(|v| v.as_color())
            .unwrap_or(default)
    }

    pub fn text(&self, key: &str, default: &str) -> String {
        self.params
            .get(key)
            .map(|v| v.as_text().to_string())
            .unwrap_or_else(|| default.to_string())
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<ParamValue>) {
        self.params.insert(key.into(), value.into());
    }

    pub fn input_count(&self) -> usize {
        match self.kind {
            NodeKind::UvCoords
            | NodeKind::Value
            | NodeKind::SolidColor
            | NodeKind::Gradient
            | NodeKind::Checker
            | NodeKind::Brick
            | NodeKind::Noise { .. }
            | NodeKind::Voronoi
            | NodeKind::Image
            | NodeKind::Time => 0,
            NodeKind::Mix => 3,
            NodeKind::Math { .. } => 2,
            NodeKind::ColorRamp => 1,
            NodeKind::Blur => 1,
            NodeKind::Invert => 1,
            NodeKind::Levels => 1,
            NodeKind::HueSaturation => 1,
            NodeKind::BrightnessContrast => 1,
            NodeKind::Transform => 1,
            NodeKind::NormalFromHeight => 1,
            NodeKind::ChannelSplit => 1,
            NodeKind::OutputAlbedo
            | NodeKind::OutputRoughness
            | NodeKind::OutputNormal
            | NodeKind::OutputEmission
            | NodeKind::OutputComposite => 1,
        }
    }

    pub fn input_label(&self, port: usize) -> &'static str {
        match (self.kind.clone(), port) {
            (NodeKind::Mix, 0) => "A",
            (NodeKind::Mix, 1) => "B",
            (NodeKind::Mix, 2) => "Factor",
            (NodeKind::Math { .. }, 0) => "A",
            (NodeKind::Math { .. }, 1) => "B",
            _ => "Input",
        }
    }

    pub fn is_output(&self) -> bool {
        matches!(
            self.kind,
            NodeKind::OutputAlbedo
                | NodeKind::OutputRoughness
                | NodeKind::OutputNormal
                | NodeKind::OutputEmission
                | NodeKind::OutputComposite
        )
    }
}

pub fn node_kind_label(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::UvCoords => "UV Coordinates",
        NodeKind::Value => "Value",
        NodeKind::SolidColor => "Solid Colour",
        NodeKind::Gradient => "Gradient",
        NodeKind::Checker => "Checker",
        NodeKind::Brick => "Bricks",
        NodeKind::Noise { .. } => "Noise",
        NodeKind::Voronoi => "Voronoi",
        NodeKind::Image => "Image",
        NodeKind::Time => "Time",
        NodeKind::Mix => "Mix",
        NodeKind::Math { .. } => "Math",
        NodeKind::ColorRamp => "Colour Ramp",
        NodeKind::Blur => "Blur",
        NodeKind::Invert => "Invert",
        NodeKind::Levels => "Levels",
        NodeKind::HueSaturation => "Hue / Saturation",
        NodeKind::BrightnessContrast => "Brightness / Contrast",
        NodeKind::Transform => "Transform",
        NodeKind::NormalFromHeight => "Normal From Height",
        NodeKind::ChannelSplit => "Channel Split",
        NodeKind::OutputAlbedo => "Albedo Output",
        NodeKind::OutputRoughness => "Roughness Output",
        NodeKind::OutputNormal => "Normal Output",
        NodeKind::OutputEmission => "Emission Output",
        NodeKind::OutputComposite => "Composite Output",
    }
}

fn default_params(kind: &NodeKind) -> Vec<(&'static str, ParamValue)> {
    match kind {
        NodeKind::Noise { octaves } => vec![
            ("scale", 8.0f32.into()),
            ("seed", 1337i32.into()),
            ("octaves", (*octaves as i32).into()),
            ("roughness", 0.5f32.into()),
            ("color_a", [0.05, 0.05, 0.06, 1.0].into()),
            ("color_b", [0.95, 0.9, 0.85, 1.0].into()),
        ],
        NodeKind::Checker => vec![
            ("scale", 8.0f32.into()),
            ("color_a", [0.9, 0.9, 0.9, 1.0].into()),
            ("color_b", [0.1, 0.1, 0.1, 1.0].into()),
        ],
        NodeKind::Brick => vec![
            ("rows", 8i32.into()),
            ("cols", 4i32.into()),
            ("mortar", 0.05f32.into()),
            ("color_a", [0.55, 0.26, 0.2, 1.0].into()),
            ("color_b", [0.8, 0.78, 0.74, 1.0].into()),
        ],
        NodeKind::Voronoi => vec![
            ("scale", 6.0f32.into()),
            ("seed", 7i32.into()),
            ("jitter", 0.85f32.into()),
            ("color_a", [0.0, 0.0, 0.0, 1.0].into()),
            ("color_b", [1.0, 1.0, 1.0, 1.0].into()),
        ],
        NodeKind::Gradient => vec![
            ("angle", 0.0f32.into()),
            ("color_a", [0.1, 0.15, 0.3, 1.0].into()),
            ("color_b", [0.9, 0.6, 0.3, 1.0].into()),
        ],
        NodeKind::SolidColor => vec![("color", [0.8, 0.2, 0.2, 1.0].into())],
        NodeKind::Value => vec![("value", 0.5f32.into())],
        NodeKind::Image => vec![("image", "".into())],
        NodeKind::Time => vec![("fps", 30.0f32.into())],
        NodeKind::Mix => vec![("factor", 0.5f32.into()), ("clamp", true.into())],
        NodeKind::Math { op } => vec![
            ("a", 0.5f32.into()),
            ("b", 0.5f32.into()),
            ("op", (*op as i32).into()),
        ],
        NodeKind::ColorRamp => vec![
            ("stops", 4i32.into()),
            ("color_0", [0.0, 0.0, 0.0, 1.0].into()),
            ("color_1", [0.25, 0.4, 0.8, 1.0].into()),
            ("color_2", [0.9, 0.7, 0.3, 1.0].into()),
            ("color_3", [1.0, 1.0, 1.0, 1.0].into()),
        ],
        NodeKind::Blur => vec![("radius", 4.0f32.into())],
        NodeKind::Invert => vec![("amount", 1.0f32.into())],
        NodeKind::Levels => vec![
            ("in_black", 0.0f32.into()),
            ("in_white", 1.0f32.into()),
            ("gamma", 1.0f32.into()),
            ("out_black", 0.0f32.into()),
            ("out_white", 1.0f32.into()),
        ],
        NodeKind::HueSaturation => vec![
            ("hue", 0.0f32.into()),
            ("saturation", 1.0f32.into()),
            ("value", 1.0f32.into()),
        ],
        NodeKind::BrightnessContrast => {
            vec![("brightness", 0.0f32.into()), ("contrast", 1.0f32.into())]
        }
        NodeKind::Transform => vec![
            ("tx", 0.0f32.into()),
            ("ty", 0.0f32.into()),
            ("scale", 1.0f32.into()),
            ("rotate", 0.0f32.into()),
        ],
        NodeKind::NormalFromHeight => vec![("strength", 1.0f32.into())],
        NodeKind::ChannelSplit => vec![("channel", 0i32.into())],
        NodeKind::UvCoords
        | NodeKind::OutputAlbedo
        | NodeKind::OutputRoughness
        | NodeKind::OutputNormal
        | NodeKind::OutputEmission
        | NodeKind::OutputComposite => vec![],
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Link {
    pub from: NodeId,
    pub from_port: u16,
    pub to: NodeId,
    pub to_port: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeGraph {
    pub name: String,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    next_node: Counter,
}

impl Default for NodeGraph {
    fn default() -> Self {
        Self::new("Material Graph")
    }
}

impl NodeGraph {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            links: Vec::new(),
            next_node: Counter::default(),
        }
    }

    /// Default editable graph: checker -> albedo output.
    pub fn starter_material(name: impl Into<String>) -> Self {
        let mut g = Self::new(name);
        let checker = g.add_node(NodeKind::Checker, Vec2::new(0.0, 0.0));
        let out = g.add_node(NodeKind::OutputAlbedo, Vec2::new(260.0, 0.0));
        g.link_ports(checker, 0, out, 0);
        g
    }

    pub fn add_node(&mut self, kind: NodeKind, position: Vec2) -> NodeId {
        let id = NodeId(self.next_node.next() as u32);
        self.nodes.push(Node::new(id, kind, position));
        id
    }

    pub fn remove_node(&mut self, id: NodeId) {
        self.nodes.retain(|n| n.id != id);
        self.links.retain(|l| l.from != id && l.to != id);
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Connects `from` to `to`'s input port. Replaces any existing link into
    /// that port and rejects self links or cycles.
    pub fn link_ports(&mut self, from: NodeId, from_port: u16, to: NodeId, to_port: u16) -> bool {
        if from == to {
            return false;
        }
        let Some(to_node) = self.node(to) else {
            return false;
        };
        if to_port as usize >= to_node.input_count() {
            return false;
        }
        if self.node(from).is_none() {
            return false;
        }
        self.links.retain(|l| !(l.to == to && l.to_port == to_port));
        self.links.push(Link {
            from,
            from_port,
            to,
            to_port,
        });
        if self.has_cycle() {
            self.links.pop();
            return false;
        }
        true
    }

    pub fn unlink_input(&mut self, to: NodeId, to_port: u16) -> bool {
        let before = self.links.len();
        self.links.retain(|l| !(l.to == to && l.to_port == to_port));
        self.links.len() != before
    }

    pub fn input_of(&self, to: NodeId, to_port: u16) -> Option<NodeId> {
        self.links
            .iter()
            .find(|l| l.to == to && l.to_port == to_port)
            .map(|l| l.from)
    }

    /// Output nodes in the graph.
    pub fn outputs(&self) -> Vec<&Node> {
        self.nodes.iter().filter(|n| n.is_output()).collect()
    }

    /// Kahn topological sort; returns Err with the nodes left in a cycle.
    pub fn topological_order(&self) -> Result<Vec<NodeId>, Vec<NodeId>> {
        use std::collections::{HashMap, VecDeque};
        let mut indegree: HashMap<NodeId, usize> = self.nodes.iter().map(|n| (n.id, 0)).collect();
        let mut edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        for l in &self.links {
            if !indegree.contains_key(&l.from) || !indegree.contains_key(&l.to) {
                continue;
            }
            edges.entry(l.from).or_default().push(l.to);
            *indegree.get_mut(&l.to).unwrap() += 1;
        }
        let mut queue: VecDeque<NodeId> = self
            .nodes
            .iter()
            .filter(|n| indegree[&n.id] == 0)
            .map(|n| n.id)
            .collect();
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(next) = edges.get(&n) {
                for m in next {
                    let d = indegree.get_mut(m).unwrap();
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(*m);
                    }
                }
            }
        }
        if order.len() == self.nodes.len() {
            Ok(order)
        } else {
            let mut cyclic: Vec<NodeId> = indegree
                .into_iter()
                .filter(|(_k, v)| *v > 0)
                .map(|(k, _)| k)
                .collect();
            cyclic.sort();
            Err(cyclic)
        }
    }

    pub fn has_cycle(&self) -> bool {
        self.topological_order().is_err()
    }

    /// Problems that prevent evaluation.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let ids: Vec<NodeId> = self.nodes.iter().map(|n| n.id).collect();
        for l in &self.links {
            if !ids.contains(&l.from) {
                problems.push(format!("Link references missing source node {}", l.from.0));
            }
            if !ids.contains(&l.to) {
                problems.push(format!("Link references missing target node {}", l.to.0));
            }
            if let Some(n) = self.node(l.to) {
                if l.to_port as usize >= n.input_count() {
                    problems.push(format!(
                        "Link targets port {} of '{}' which only has {} input(s)",
                        l.to_port,
                        n.name,
                        n.input_count()
                    ));
                }
            }
        }
        if let Err(cycle) = self.topological_order() {
            problems.push(format!(
                "Graph contains a cycle involving node(s): {}",
                cycle
                    .iter()
                    .map(|c| c.0.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if self.outputs().is_empty() {
            problems.push("Graph has no output node".into());
        }
        for out in self.outputs() {
            if self.input_of(out.id, 0).is_none() {
                problems.push(format!("Output '{}' has no input connected", out.name));
            }
        }
        problems
    }

    pub fn repair_counters(&mut self) {
        let max = self
            .nodes
            .iter()
            .map(|n| n.id.0 as u64 + 1)
            .max()
            .unwrap_or(0);
        self.next_node.bump_to(max.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_graph_is_valid() {
        let g = NodeGraph::starter_material("Test");
        assert_eq!(g.nodes.len(), 2);
        assert!(g.validate().is_empty());
        assert!(g.topological_order().is_ok());
    }

    #[test]
    fn links_replace_and_reject_cycles() {
        let mut g = NodeGraph::new("g");
        let a = g.add_node(NodeKind::Value, Vec2::ZERO);
        let b = g.add_node(NodeKind::Math { op: MathOp::Add }, Vec2::ZERO);
        let c = g.add_node(NodeKind::Mix, Vec2::ZERO);
        assert!(g.link_ports(a, 0, b, 0));
        assert!(g.link_ports(b, 0, c, 0));
        // Replacing an input port keeps only one link.
        assert!(g.link_ports(a, 0, c, 0));
        assert_eq!(
            g.links
                .iter()
                .filter(|l| l.to == c && l.to_port == 0)
                .count(),
            1
        );
        // Restore b -> c so that c -> b closes a cycle.
        assert!(g.link_ports(b, 0, c, 0));
        assert!(!g.link_ports(c, 0, b, 1));
        assert!(!g.link_ports(a, 0, a, 0));
        // Invalid port index.
        assert!(!g.link_ports(a, 0, a, 9));
    }

    #[test]
    fn removing_node_removes_links() {
        let mut g = NodeGraph::new("g");
        let a = g.add_node(NodeKind::Value, Vec2::ZERO);
        let out = g.add_node(NodeKind::OutputAlbedo, Vec2::ZERO);
        g.link_ports(a, 0, out, 0);
        g.remove_node(a);
        assert!(g.links.is_empty());
        assert!(g.validate().iter().any(|p| p.contains("no input")));
    }

    #[test]
    fn params_have_defaults_and_types() {
        let mut g = NodeGraph::new("g");
        let n = g.add_node(NodeKind::Noise { octaves: 3 }, Vec2::ZERO);
        let node = g.node(n).unwrap();
        assert_eq!(node.float("scale", -1.0), 8.0);
        assert_eq!(node.int("seed", -1), 1337);
        assert_eq!(node.color("color_a", [0.0; 4])[3], 1.0);
        g.node_mut(n).unwrap().set("scale", 24.0f32);
        assert_eq!(g.node(n).unwrap().float("scale", 0.0), 24.0);
    }
}
