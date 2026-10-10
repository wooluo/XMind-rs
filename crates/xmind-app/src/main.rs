mod app;
mod canvas;
mod editor;
mod fonts;
mod i18n;
mod prefs;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let selftest = args.iter().any(|a| a == "--selftest");
    let open_path = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(std::path::PathBuf::from)
        .filter(|p| p.exists());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1380.0, 880.0])
            .with_min_inner_size([960.0, 620.0])
            .with_title("XMind-RS"),
        ..Default::default()
    };

    eframe::run_native(
        "XMind-RS",
        options,
        Box::new(move |cc| {
            egui_extras_fix(cc);
            Ok(Box::new(app::App::new(cc, open_path, selftest)))
        }),
    )
}

fn egui_extras_fix(_cc: &eframe::CreationContext) {}
