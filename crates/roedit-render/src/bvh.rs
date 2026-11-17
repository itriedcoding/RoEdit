//! Bounding volume hierarchy over render triangles.
//!
//! Built with a median split on the longest axis; traversal is an explicit
//! stack walk. Verified against brute force in the tests.

use crate::pathtracer::RenderTriangle;
use roedit_core::math::{Aabb, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f32,
    pub triangle: u32,
    /// Barycentric coordinates (u, v) of the second and third vertices.
    pub bary: [f32; 2],
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct BvhNode {
    pub bounds: Aabb,
    /// Start slot in `Bvh::order`.
    pub start: u32,
    /// Triangle count for leaves, 0 for interior nodes.
    pub count: u32,
    pub left: u32,
    pub right: u32,
}

impl BvhNode {
    pub fn is_leaf(&self) -> bool {
        self.count > 0
    }
}

#[derive(Clone, Debug, Default)]
pub struct Bvh {
    pub nodes: Vec<BvhNode>,
    /// Permutation of triangle indices.
    pub order: Vec<u32>,
    pub bounds: Aabb,
}

const LEAF_SIZE: usize = 4;

fn centroid(b: &Aabb) -> Vec3 {
    (b.min + b.max) * 0.5
}

impl Bvh {
    pub fn build(triangles: &[RenderTriangle]) -> Bvh {
        let bounds: Vec<Aabb> = triangles.iter().map(|t| t.bounds()).collect();
        Bvh::build_with_bounds(triangles.len(), &bounds)
    }

    /// Builds from precomputed triangle bounds (indexed like the triangles).
    pub fn build_with_bounds(count: usize, bounds: &[Aabb]) -> Bvh {
        let mut bvh = Bvh::default();
        if count == 0 || bounds.is_empty() {
            return bvh;
        }
        let mut order: Vec<u32> = (0..count as u32).collect();
        bvh.bounds = bounds.iter().fold(Aabb::empty(), |acc, b| acc.union(b));
        bvh.nodes.reserve(count * 2);
        build_node(&mut bvh.nodes, &mut order, 0, bounds, 0);
        bvh.order = order;
        bvh
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn intersect(
        &self,
        triangles: &[RenderTriangle],
        origin: Vec3,
        direction: Vec3,
        t_min: f32,
        t_max: f32,
    ) -> Option<Hit> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = Vec3::new(
            safe_inv(direction.x),
            safe_inv(direction.y),
            safe_inv(direction.z),
        );
        let mut stack = [0u32; 96];
        let mut stack_len = 0usize;
        stack[stack_len] = 0;
        stack_len += 1;
        let mut best: Option<Hit> = None;
        let mut best_t = t_max;
        while stack_len > 0 {
            stack_len -= 1;
            let node = self.nodes[stack[stack_len] as usize];
            if !slab_hit(&node.bounds, origin, inv, t_min, best_t) {
                continue;
            }
            if node.is_leaf() {
                for slot in node.start..node.start + node.count {
                    let tri_index = match self.order.get(slot as usize) {
                        Some(i) => *i,
                        None => continue,
                    };
                    if let Some(tri) = triangles.get(tri_index as usize) {
                        if let Some(hit) = tri.intersect_triangle(origin, direction, t_min, best_t)
                        {
                            if hit.t < best_t {
                                best_t = hit.t;
                                best = Some(Hit {
                                    triangle: tri_index,
                                    ..hit
                                });
                            }
                        }
                    }
                }
            } else if stack_len + 2 <= stack.len() {
                stack[stack_len] = node.left;
                stack[stack_len + 1] = node.right;
                stack_len += 2;
            }
        }
        best
    }

    /// Any-hit test used by shadow rays.
    pub fn occluded(
        &self,
        triangles: &[RenderTriangle],
        origin: Vec3,
        direction: Vec3,
        t_min: f32,
        t_max: f32,
    ) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        let inv = Vec3::new(
            safe_inv(direction.x),
            safe_inv(direction.y),
            safe_inv(direction.z),
        );
        let mut stack = [0u32; 96];
        let mut stack_len = 0usize;
        stack[stack_len] = 0;
        stack_len += 1;
        while stack_len > 0 {
            stack_len -= 1;
            let node = self.nodes[stack[stack_len] as usize];
            if !slab_hit(&node.bounds, origin, inv, t_min, t_max) {
                continue;
            }
            if node.is_leaf() {
                for slot in node.start..node.start + node.count {
                    let tri_index = match self.order.get(slot as usize) {
                        Some(i) => *i,
                        None => continue,
                    };
                    if let Some(tri) = triangles.get(tri_index as usize) {
                        if tri
                            .intersect_triangle(origin, direction, t_min, t_max)
                            .is_some()
                        {
                            return true;
                        }
                    }
                }
            } else if stack_len + 2 <= stack.len() {
                stack[stack_len] = node.left;
                stack[stack_len + 1] = node.right;
                stack_len += 2;
            }
        }
        false
    }

    /// (nodes, leaves, max depth)
    pub fn stats(&self) -> (usize, usize, u32) {
        let leaves = self.nodes.iter().filter(|n| n.is_leaf()).count();
        (self.nodes.len(), leaves, depth_of(&self.nodes, 0))
    }
}

fn depth_of(nodes: &[BvhNode], index: u32) -> u32 {
    match nodes.get(index as usize) {
        Some(n) if !n.is_leaf() => 1 + depth_of(nodes, n.left).max(depth_of(nodes, n.right)),
        Some(_) => 1,
        None => 0,
    }
}

fn build_node(
    nodes: &mut Vec<BvhNode>,
    order: &mut [u32],
    absolute_start: usize,
    bounds: &[Aabb],
    depth: usize,
) -> u32 {
    let count = order.len();
    let node_bounds = order
        .iter()
        .filter_map(|i| bounds.get(*i as usize))
        .fold(Aabb::empty(), |acc, b| acc.union(b));
    let index = nodes.len() as u32;
    nodes.push(BvhNode {
        bounds: node_bounds,
        start: absolute_start as u32,
        count: count as u32,
        left: 0,
        right: 0,
    });
    if count <= LEAF_SIZE || depth >= 48 {
        return index;
    }
    let extent = node_bounds.size();
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    let key = |i: &u32| -> f32 {
        bounds
            .get(*i as usize)
            .map(|b| match axis {
                0 => centroid(b).x,
                1 => centroid(b).y,
                _ => centroid(b).z,
            })
            .unwrap_or(0.0)
    };
    order.sort_by(|a, b| {
        key(a)
            .partial_cmp(&key(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mid = count / 2;
    let (left_slice, right_slice) = order.split_at_mut(mid);
    let left = build_node(nodes, left_slice, absolute_start, bounds, depth + 1);
    let right = build_node(nodes, right_slice, absolute_start + mid, bounds, depth + 1);
    if let Some(node) = nodes.get_mut(index as usize) {
        node.left = left;
        node.right = right;
        node.count = 0;
    }
    index
}

fn safe_inv(v: f32) -> f32 {
    if v.abs() < 1e-8 {
        if v < 0.0 {
            -1e8
        } else {
            1e8
        }
    } else {
        1.0 / v
    }
}

fn slab_hit(bounds: &Aabb, origin: Vec3, inv: Vec3, t_min: f32, t_max: f32) -> bool {
    let t0 = (bounds.min - origin) * inv;
    let t1 = (bounds.max - origin) * inv;
    let t_small = Vec3::new(t0.x.min(t1.x), t0.y.min(t1.y), t0.z.min(t1.z));
    let t_big = Vec3::new(t0.x.max(t1.x), t0.y.max(t1.y), t0.z.max(t1.z));
    let enter = t_small.x.max(t_small.y).max(t_small.z).max(t_min);
    let exit = t_big.x.min(t_big.y).min(t_big.z).min(t_max);
    enter <= exit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Pcg32;

    fn grid_scene(n: usize) -> Vec<RenderTriangle> {
        let mut out = Vec::new();
        for i in 0..n {
            for j in 0..n {
                let x = i as f32 - n as f32 * 0.5;
                let z = j as f32 - n as f32 * 0.5;
                out.push(RenderTriangle::new(
                    Vec3::new(x, 0.0, z),
                    Vec3::new(x + 0.8, 0.0, z),
                    Vec3::new(x, 0.9, z + 0.7),
                    Vec3::new(x, 0.0, z),
                    Vec3::new(x, 1.0, z),
                    Vec3::new(x + 1.0, 0.0, z + 1.0),
                    0,
                ));
            }
        }
        out
    }

    #[test]
    fn matches_brute_force() {
        let tris = grid_scene(12);
        let bvh = Bvh::build(&tris);
        let mut rng = Pcg32::new(1234, 5);
        let mut hits = 0;
        for _ in 0..300 {
            let origin = Vec3::new(
                rng.range(-12.0, 12.0),
                rng.range(0.2, 8.0),
                rng.range(-12.0, 12.0),
            );
            let target = Vec3::new(rng.range(-6.0, 6.0), 0.0, rng.range(-6.0, 6.0));
            let dir = (target - origin).normalize_or_zero();
            let bvh_hit = bvh.intersect(&tris, origin, dir, 1e-4, 1e9);
            let mut brute: Option<f32> = None;
            for t in &tris {
                if let Some(h) = t.intersect_triangle(origin, dir, 1e-4, 1e9) {
                    brute = Some(brute.map_or(h.t, |b: f32| b.min(h.t)));
                }
            }
            match (bvh_hit, brute) {
                (Some(h), Some(b)) => {
                    assert!((h.t - b).abs() < 1e-3, "{} vs {}", h.t, b);
                    hits += 1;
                }
                (None, None) => {}
                (a, b) => panic!("mismatch: {:?} vs {:?}", a.map(|x| x.t), b),
            }
        }
        assert!(
            hits > 50,
            "expected a meaningful number of hits, got {hits}"
        );
    }

    #[test]
    fn empty_bvh_misses() {
        let bvh = Bvh::build(&[]);
        assert!(bvh.is_empty());
        assert!(bvh
            .intersect(&[], Vec3::ZERO, Vec3::X, 0.0, 100.0)
            .is_none());
        assert!(!bvh.occluded(&[], Vec3::ZERO, Vec3::X, 0.0, 100.0));
    }

    #[test]
    fn occlusion_agrees_with_intersection() {
        let tris = grid_scene(6);
        let bvh = Bvh::build(&tris);
        let mut rng = Pcg32::new(77, 2);
        for _ in 0..100 {
            let origin = Vec3::new(rng.range(-4.0, 4.0), 3.0, rng.range(-4.0, 4.0));
            let dir = Vec3::new(rng.range(-1.0, 1.0), -1.0, rng.range(-1.0, 1.0)).normalize();
            let hit = bvh.intersect(&tris, origin, dir, 1e-4, 1000.0).is_some();
            assert_eq!(hit, bvh.occluded(&tris, origin, dir, 1e-4, 1000.0));
        }
    }

    #[test]
    fn node_stats_are_reasonable() {
        let tris = grid_scene(8);
        let bvh = Bvh::build(&tris);
        let (nodes, leaves, depth) = bvh.stats();
        assert!(nodes >= 2);
        assert!(leaves >= 1);
        assert!(depth < 32);
        // Every triangle appears exactly once in the permutation.
        let mut sorted = bvh.order.clone();
        sorted.sort();
        assert_eq!(sorted, (0..tris.len() as u32).collect::<Vec<_>>());
    }
}
