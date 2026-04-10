use crate::plot::plot_config::Colormap;
use egui::Color32;

// ── 8-knot LUTs (sRGB u8) ────────────────────────────────────────────────────

const VIRIDIS: [[u8; 3]; 8] = [
    [68,  1,  84], [72,  40, 120], [62,  83, 157], [49, 120, 176],
    [53, 153, 150], [103, 188, 99], [180, 221, 29], [253, 231, 37],
];
const PLASMA: [[u8; 3]; 8] = [
    [13,   8, 135], [84,   2, 163], [139,  10, 165], [185, 50, 137],
    [219,  92, 104], [244, 136,  73], [254, 188,  43], [240, 249, 33],
];
const INFERNO: [[u8; 3]; 8] = [
    [0,   0,   4], [40,  11,  84], [101,  21, 110], [159, 42, 99],
    [212,  72,  66], [245, 125,  21], [252, 193,   7], [252, 255, 164],
];
const TURBO: [[u8; 3]; 8] = [
    [48,  18,  59], [50,  92, 199], [18, 196, 196], [103, 230, 96],
    [211, 230,  35], [255, 170,   0], [255,  80,   0], [189,   7,  7],
];
const GRAYSCALE: [[u8; 3]; 8] = [
    [20, 20, 20], [52, 52, 52], [84, 84, 84], [116, 116, 116],
    [148, 148, 148], [180, 180, 180], [212, 212, 212], [240, 240, 240],
];

/// Evaluate `colormap` at `t ∈ [0.0, 1.0]`, returning an sRGB `Color32`.
/// Values outside [0, 1] are clamped.
pub fn eval(colormap: &Colormap, t: f64) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lut: &[[u8; 3]] = match colormap {
        Colormap::Viridis  => &VIRIDIS,
        Colormap::Plasma   => &PLASMA,
        Colormap::Inferno  => &INFERNO,
        Colormap::Turbo    => &TURBO,
        Colormap::Grayscale => &GRAYSCALE,
    };

    let n = lut.len();
    let scaled = t * (n - 1) as f64;
    let lo = (scaled.floor() as usize).min(n - 2);
    let frac = scaled - lo as f64;

    let lerp = |a: u8, b: u8| ((a as f64).mul_add(1.0 - frac, b as f64 * frac).round() as u8);
    let [r0, g0, b0] = lut[lo];
    let [r1, g1, b1] = lut[lo + 1];

    Color32::from_rgb(lerp(r0, r1), lerp(g0, g1), lerp(b0, b1))
}

/// Pre-compute `count` evenly-spaced color samples from a colormap.
/// Useful for rendering gradient swatches in the legend pane.
pub fn sample_gradient(colormap: &Colormap, count: usize) -> Vec<Color32> {
    (0..count)
        .map(|i| eval(colormap, i as f64 / (count - 1).max(1) as f64))
        .collect()
}
