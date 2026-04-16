use crate::plot::colormap::eval as colormap_eval;
use crate::plot::plot_config::{AlphaConfig, ColorMode, Colormap, SizeConfig};
use egui::Color32;
use polars::prelude::{DataFrame, DataType};
use std::collections::HashMap;

// ── Categorical palette ───────────────────────────────────────────────────────

/// Distinguishable colors designed for dark backgrounds, cycling on overflow.
pub const CATEGORICAL_PALETTE: [Color32; 12] = [
    Color32::from_rgb(92,  183, 255),  // sky blue
    Color32::from_rgb(255, 140,  66),  // orange
    Color32::from_rgb(80,  225, 130),  // green
    Color32::from_rgb(255,  85, 140),  // pink
    Color32::from_rgb(180, 120, 255),  // purple
    Color32::from_rgb(255, 218,  68),  // yellow
    Color32::from_rgb(68,  210, 210),  // teal
    Color32::from_rgb(255, 165, 140),  // salmon
    Color32::from_rgb(130, 230, 130),  // light green
    Color32::from_rgb(200, 200, 255),  // lavender
    Color32::from_rgb(255, 145, 200),  // light pink
    Color32::from_rgb(140, 205, 170),  // sage
];

/// Return palette color for the given category index (cycles on overflow).
pub fn categorical_color(idx: usize) -> Color32 {
    CATEGORICAL_PALETTE[idx % CATEGORICAL_PALETTE.len()]
}

// ── Legend data types ─────────────────────────────────────────────────────────

/// Computed color legend metadata for one plot.
#[derive(Debug, Clone)]
pub enum ColorLegend {
    Solid { color: Color32 },
    Categorical { col: String, entries: Vec<(String, Color32)> },
    Continuous   { col: String, colormap: Colormap, data_min: f64, data_max: f64, reverse: bool },
}

/// Computed size legend metadata.
#[derive(Debug, Clone)]
pub struct SizeLegend {
    pub col: String,
    pub min_px: f32,
    pub max_px: f32,
    pub data_min: f64,
    pub data_max: f64,
}

/// Computed alpha legend metadata.
#[derive(Debug, Clone)]
pub struct AlphaLegend {
    pub col: String,
    pub min_alpha: f32,
    pub max_alpha: f32,
    pub data_min: f64,
    pub data_max: f64,
}

/// All legend display data for one plot, computed at sync time.
#[derive(Debug, Clone)]
pub struct PlotLegendData {
    pub plot_id: usize,
    pub plot_title: String,
    pub color: ColorLegend,
    pub size: Option<SizeLegend>,
    pub alpha: Option<AlphaLegend>,
}

// ── Color computation ─────────────────────────────────────────────────────────

/// Compute per-row colors for `n` rows from a DataFrame according to `mode`.
/// Returns (colors, legend_metadata, category_indices). Colors are fully opaque (alpha=255).
/// Caller is responsible for applying alpha afterward.
/// `category_indices` is non-empty only for Categorical mode.
pub fn compute_colors(
    df: &DataFrame,
    mode: &ColorMode,
    solid_color: Color32,
    n: usize,
) -> (Vec<Color32>, ColorLegend, Vec<Option<usize>>) {
    puffin::profile_function!();
    match mode {
        ColorMode::Solid => {
            let c = Color32::from_rgb(solid_color.r(), solid_color.g(), solid_color.b());
            (vec![c; n], ColorLegend::Solid { color: solid_color }, Vec::new())
        }
        ColorMode::Categorical { col } => compute_categorical_colors(df, col, n, solid_color),
        ColorMode::Continuous { col, colormap, color_min, color_max, reverse } => {
            let (colors, legend) = compute_continuous_colors(df, col, colormap, n, solid_color, *color_min, *color_max, *reverse);
            (colors, legend, Vec::new())
        }
    }
}

fn compute_categorical_colors(
    df: &DataFrame,
    col: &str,
    n: usize,
    fallback: Color32,
) -> (Vec<Color32>, ColorLegend, Vec<Option<usize>>) {
    let series = match df.column(col).ok().and_then(|c| c.as_series()).map(|s| s.clone()) {
        Some(s) => s,
        None => {
            return (
                vec![Color32::from_rgb(fallback.r(), fallback.g(), fallback.b()); n],
                ColorLegend::Solid { color: fallback },
                Vec::new(),
            );
        }
    };

    let cast = series.cast(&DataType::String).unwrap_or(series);
    let ca = match cast.str() {
        Ok(c) => c.clone(),
        Err(_) => {
            return (
                vec![Color32::from_rgb(fallback.r(), fallback.g(), fallback.b()); n],
                ColorLegend::Solid { color: fallback },
                Vec::new(),
            );
        }
    };

    // Build ordered label → color mapping on first-seen order.
    // Uses HashMap for O(1) lookup instead of O(k) linear scan per row.
    let mut order: Vec<String> = Vec::new();
    let mut label_to_idx: HashMap<String, usize> = HashMap::new();
    let mut cat_indices: Vec<Option<usize>> = Vec::with_capacity(n);
    let colors: Vec<Color32> = ca
        .into_iter()
        .map(|opt| {
            let s = opt.unwrap_or("(null)");
            let idx = if let Some(&pos) = label_to_idx.get(s) {
                pos
            } else {
                let pos = order.len();
                label_to_idx.insert(s.to_string(), pos);
                order.push(s.to_string());
                pos
            };
            cat_indices.push(Some(idx));
            categorical_color(idx)
        })
        .collect();

    let entries: Vec<(String, Color32)> = order
        .iter()
        .enumerate()
        .map(|(i, lbl)| (lbl.clone(), categorical_color(i)))
        .collect();

    (colors, ColorLegend::Categorical { col: col.to_string(), entries }, cat_indices)
}

fn compute_continuous_colors(
    df: &DataFrame,
    col: &str,
    colormap: &Colormap,
    n: usize,
    fallback: Color32,
    user_min: Option<f64>,
    user_max: Option<f64>,
    reverse: bool,
) -> (Vec<Color32>, ColorLegend) {
    let vals = match get_f64_vec(df, col) {
        Some(v) => v,
        None => {
            return (
                vec![Color32::from_rgb(fallback.r(), fallback.g(), fallback.b()); n],
                ColorLegend::Solid { color: fallback },
            );
        }
    };

    let finite: Vec<f64> = vals.iter().flatten().copied().collect();
    let auto_min = finite.iter().copied().fold(f64::INFINITY, f64::min);
    let auto_max = finite.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    // Use user overrides if provided, otherwise auto-detect from data.
    let eff_min = user_min.unwrap_or(if auto_min.is_finite() { auto_min } else { 0.0 });
    let eff_max = user_max.unwrap_or(if auto_max.is_finite() { auto_max } else { 1.0 });
    let range = eff_max - eff_min;

    let colors: Vec<Color32> = vals
        .iter()
        .map(|opt| {
            let mut t = opt
                .map(|v| {
                    if range > 0.0 {
                        ((v - eff_min) / range).clamp(0.0, 1.0)
                    } else {
                        0.5
                    }
                })
                .unwrap_or(0.5);
            if reverse { t = 1.0 - t; }
            colormap_eval(colormap, t)
        })
        .collect();

    (
        colors,
        ColorLegend::Continuous {
            col: col.to_string(),
            colormap: colormap.clone(),
            data_min: eff_min,
            data_max: eff_max,
            reverse,
        },
    )
}

// ── Size computation ──────────────────────────────────────────────────────────

/// Compute per-row radii (in screen pixels). Returns `base_radius` for all rows if no config.
/// Also returns `Option<SizeLegend>` with the computed data range.
pub fn compute_radii(
    df: &DataFrame,
    config: Option<&SizeConfig>,
    base_radius: f32,
    n: usize,
) -> (Vec<f32>, Option<SizeLegend>) {
    match config {
        None => (vec![base_radius; n], None),
        Some(cfg) => {
            let vals = get_f64_vec(df, &cfg.col).unwrap_or_else(|| vec![None; n]);
            let finite: Vec<f64> = vals.iter().flatten().copied().collect();
            let data_min = finite.iter().copied().fold(f64::INFINITY, f64::min);
            let data_max = finite.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let range = data_max - data_min;

            let radii: Vec<f32> = vals
                .iter()
                .map(|opt| {
                    opt.map(|v| {
                        let t = if range > 0.0 { ((v - data_min) / range).clamp(0.0, 1.0) } else { 0.5 };
                        cfg.min_px + (cfg.max_px - cfg.min_px) * t as f32
                    })
                    .unwrap_or(base_radius)
                })
                .collect();

            let legend = SizeLegend {
                col: cfg.col.clone(),
                min_px: cfg.min_px,
                max_px: cfg.max_px,
                data_min: if data_min.is_finite() { data_min } else { 0.0 },
                data_max: if data_max.is_finite() { data_max } else { 1.0 },
            };
            (radii, Some(legend))
        }
    }
}

// ── Alpha computation ─────────────────────────────────────────────────────────

/// Compute per-row opacity values (0.0–1.0). Returns `base_alpha` for all rows if no config.
/// Also returns `Option<AlphaLegend>` with the computed data range.
pub fn compute_alphas(
    df: &DataFrame,
    config: Option<&AlphaConfig>,
    base_alpha: f32,
    n: usize,
) -> (Vec<f32>, Option<AlphaLegend>) {
    match config {
        None => (vec![base_alpha; n], None),
        Some(cfg) => {
            let vals = get_f64_vec(df, &cfg.col).unwrap_or_else(|| vec![None; n]);
            let finite: Vec<f64> = vals.iter().flatten().copied().collect();
            let data_min = finite.iter().copied().fold(f64::INFINITY, f64::min);
            let data_max = finite.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let range = data_max - data_min;

            let alphas: Vec<f32> = vals
                .iter()
                .map(|opt| {
                    opt.map(|v| {
                        let t = if range > 0.0 { ((v - data_min) / range).clamp(0.0, 1.0) } else { 0.5 };
                        cfg.min_alpha + (cfg.max_alpha - cfg.min_alpha) * t as f32
                    })
                    .unwrap_or(base_alpha)
                })
                .collect();

            let legend = AlphaLegend {
                col: cfg.col.clone(),
                min_alpha: cfg.min_alpha,
                max_alpha: cfg.max_alpha,
                data_min: if data_min.is_finite() { data_min } else { 0.0 },
                data_max: if data_max.is_finite() { data_max } else { 1.0 },
            };
            (alphas, Some(legend))
        }
    }
}

/// Bake per-row alpha values into colors in-place.
pub fn apply_alpha(colors: &mut [Color32], alphas: &[f32]) {
    for (c, &a) in colors.iter_mut().zip(alphas.iter()) {
        *c = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn get_f64_vec(df: &DataFrame, col: &str) -> Option<Vec<Option<f64>>> {
    let series = df.column(col).ok()?.as_series()?.clone();
    let cast = series.cast(&DataType::Float64).ok()?;
    let ca = cast.f64().ok()?;
    Some(ca.into_iter().collect())
}
