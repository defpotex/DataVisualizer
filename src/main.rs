mod app;
mod theme;
mod ui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DataVisualizer")
            .with_min_inner_size([800.0, 500.0])
            .with_inner_size([1400.0, 900.0])
            .with_icon(load_icon()),
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "DataVisualizer",
        native_options,
        Box::new(|cc| Ok(Box::new(app::DataVisualizerApp::new(cc)))),
    )
}

fn load_icon() -> egui::IconData {
    // Programmatically draw a simple cyan diamond ◈ as the window icon.
    // Replace with an actual .png via `include_bytes!` in a future phase.
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0;
    let half = size as f32 / 2.0 - 2.0;

    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - cx).abs();
            let dy = (y as f32 - cy).abs();
            let idx = ((y * size + x) * 4) as usize;
            if dx + dy < half {
                // Solid cyan fill
                rgba[idx]     = 0x00; // R
                rgba[idx + 1] = 0xD4; // G
                rgba[idx + 2] = 0xFF; // B
                rgba[idx + 3] = 0xFF; // A
            } else if dx + dy < half + 2.0 {
                // Border ring — slightly darker
                rgba[idx]     = 0x00;
                rgba[idx + 1] = 0x8A;
                rgba[idx + 2] = 0xA8;
                rgba[idx + 3] = 0xFF;
            }
        }
    }

    egui::IconData { rgba, width: size, height: size }
}
