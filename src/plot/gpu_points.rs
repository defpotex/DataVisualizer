//! Batched mesh rendering for large point clouds.
//!
//! Instead of submitting one `Shape::Circle` per point (each individually
//! tessellated by egui), this module pre-builds a single [`egui::Mesh`] that
//! contains triangle-fan approximations of all circles. The painter then
//! submits the mesh as a single shape, which the GPU renders in one draw call.
//!
//! Circle quality (number of segments) scales with radius so that small points
//! use fewer triangles and large points stay smooth.

use egui::{Color32, Mesh, Pos2, Shape, TextureId};
use egui::epaint::Vertex;

/// Minimum segments for the smallest circles.
const MIN_SEGMENTS: usize = 6;
/// Maximum segments for large circles.
const MAX_SEGMENTS: usize = 32;

/// Build a single [`Shape::Mesh`] containing filled circles for every point.
///
/// Each circle is tessellated as a triangle fan: one center vertex plus
/// `segments` perimeter vertices, producing `segments` triangles.
///
/// # Arguments
/// * `points` — iterator of `(center, radius, color)` tuples.
/// * `count_hint` — approximate number of points for pre-allocation.
pub fn build_circle_mesh(
    points: impl Iterator<Item = (Pos2, f32, Color32)>,
    count_hint: usize,
) -> Shape {
    // Average ~10 segments per circle for allocation estimate.
    let avg_segs = 10;
    let est_verts = count_hint * (1 + avg_segs);
    let est_indices = count_hint * avg_segs * 3;

    let mut mesh = Mesh {
        indices: Vec::with_capacity(est_indices),
        vertices: Vec::with_capacity(est_verts),
        texture_id: TextureId::default(),
    };

    // Pre-compute sin/cos tables for each segment count we might use.
    // We cache them lazily — in practice most points share the same radius
    // band so we'll hit the same segment count repeatedly.
    let mut sin_cos_cache: [Option<Vec<(f32, f32)>>; MAX_SEGMENTS + 1] =
        std::array::from_fn(|_| None);

    for (center, radius, color) in points {
        let segments = circle_segments(radius);

        let table = sin_cos_cache[segments].get_or_insert_with(|| {
            (0..segments)
                .map(|i| {
                    let angle = std::f32::consts::TAU * (i as f32) / (segments as f32);
                    (angle.sin(), angle.cos())
                })
                .collect()
        });

        let base = mesh.vertices.len() as u32;

        // Center vertex
        mesh.vertices.push(Vertex::untextured(center, color));

        // Perimeter vertices
        for &(sin, cos) in table.iter() {
            let pos = Pos2::new(center.x + cos * radius, center.y + sin * radius);
            mesh.vertices.push(Vertex::untextured(pos, color));
        }

        // Triangle fan indices: center → i → i+1 (wrapping)
        let segs = segments as u32;
        for i in 0..segs {
            mesh.indices.push(base); // center
            mesh.indices.push(base + 1 + i); // current perimeter
            mesh.indices.push(base + 1 + (i + 1) % segs); // next perimeter
        }
    }

    Shape::mesh(mesh)
}

/// Determine the number of segments for a circle of the given radius.
/// Smaller circles get fewer segments; larger ones get more.
fn circle_segments(radius: f32) -> usize {
    if radius <= 2.0 {
        MIN_SEGMENTS
    } else if radius >= 20.0 {
        MAX_SEGMENTS
    } else {
        // Linear interpolation between MIN and MAX over radius 2..20
        let t = (radius - 2.0) / 18.0;
        let segs = MIN_SEGMENTS as f32 + t * (MAX_SEGMENTS - MIN_SEGMENTS) as f32;
        segs.round() as usize
    }
}

/// Returns `true` if batched mesh rendering should be used given the current
/// settings and point count.
pub fn should_use_batched(
    mode: crate::state::perf_settings::GpuPointsMode,
    threshold: usize,
    point_count: usize,
) -> bool {
    use crate::state::perf_settings::GpuPointsMode;
    match mode {
        GpuPointsMode::Off => false,
        GpuPointsMode::On => true,
        GpuPointsMode::Auto => point_count >= threshold,
    }
}
