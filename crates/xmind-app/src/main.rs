mod app;
mod canvas;
mod editor;
mod fonts;
mod i18n;
mod prefs;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let selftest = args.iter().any(|a| a == "--selftest");
    let fonttest = args.iter().any(|a| a == "--fonttest");
    let open_path = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(std::path::PathBuf::from)
        .filter(|p| p.exists());

    let icon = load_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1380.0, 880.0])
            .with_min_inner_size([960.0, 620.0])
            .with_title("XMind-RS")
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "XMind-RS",
        options,
        Box::new(move |cc| {
            egui_extras_fix(cc);
            Ok(if fonttest {
                Box::new(FontTest::new(cc)) as Box<dyn eframe::App>
            } else {
                Box::new(app::App::new(cc, open_path, selftest)) as Box<dyn eframe::App>
            })
        }),
    )
}

fn egui_extras_fix(_cc: &eframe::CreationContext) {}

fn load_icon() -> egui::IconData {
    const PNG: &[u8] = include_bytes!("../assets/icon_256.png");
    match image::load_from_memory(PNG) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            egui::IconData { width: w, height: h, rgba: rgba.into_raw() }
        }
        Err(_) => egui::IconData::default(),
    }
}

struct FontTest {
    frames: u32,
}

impl FontTest {
    fn new(cc: &eframe::CreationContext) -> Self {
        let fam = crate::fonts::install(&cc.egui_ctx);
        eprintln!("fonttest font: {fam:?}");
        FontTest { frames: 0 }
    }
}

impl eframe::App for FontTest {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frames += 1;
        if self.frames == 1 {
            for sz in [12.0f32, 12.5, 14.0, 16.0, 24.0] {
                let has = ctx.fonts(|f| f.has_glyphs(&egui::FontId::proportional(sz), "中心主题"));
                eprintln!("[fonttest] size {sz} has CJK glyphs: {has}");
            }
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            let p = ui.painter().clone();
            let mut y = 40.0;
            for (sz, tag) in [(12.0, "12"), (14.0, "14"), (16.0, "16"), (20.0, "20"), (24.0, "24"), (32.0, "32")] {
                p.text(
                    egui::pos2(20.0, y),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{tag}: 中心主题分支概要 P1"),
                    egui::FontId::proportional(sz),
                    egui::Color32::BLACK,
                );
                y += sz * 1.8;
            }
            ui.add_space(y);
            ui.label("ui.label: 中心主题分支概要 P1");
            ui.small_button("按钮: 中心主题");
            ui.monospace("mono: 中心主题");
            // size bisection probes
            let mut bx = 700.0;
            let mut by = 60.0;
            for sz in [12.0f32, 12.25, 12.4, 12.5, 12.75, 13.0, 14.0, 15.0, 16.0, 20.0, 24.0, 32.0] {
                p.text(
                    egui::pos2(bx, by + sz),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{sz}:中文"),
                    egui::FontId::proportional(sz),
                    egui::Color32::BLUE,
                );
                by += sz * 1.7;
            }
            // manual galley
            let galley = ctx.fonts(|f| f.layout_no_wrap(
                "手动galley: 中心主题".to_string(),
                egui::FontId::proportional(16.0),
                egui::Color32::BLACK,
            ));
            let (mut rx, mut ry) = ui.cursor().left_top().into();
            p.galley(egui::pos2(rx, ry), galley, egui::Color32::BLACK);
            ry += 26.0;
            // size 12.5 via painter (label body size)
            p.text(
                egui::pos2(rx, ry + 12.0),
                egui::Align2::LEFT_BOTTOM,
                "painter 12.5: 中心主题",
                egui::FontId::proportional(12.5),
                egui::Color32::BLACK,
            );
            ry += 26.0;
            // monospace family via painter
            p.text(
                egui::pos2(rx, ry + 16.0),
                egui::Align2::LEFT_BOTTOM,
                "painter mono16: 中心主题",
                egui::FontId::monospace(16.0),
                egui::Color32::BLACK,
            );
            ry += 34.0;
            ui.add_space(ry - 260.0);
        });
        if self.frames > 50 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
