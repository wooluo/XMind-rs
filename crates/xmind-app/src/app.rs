//! Application shell: toolbar, outline, properties, sheets bar, dialogs,
//! global shortcuts, autosave and preferences.

use crate::canvas;
use crate::editor::{swatches, Editor};
use crate::i18n::{tr, Lang};
use crate::prefs::Prefs;
use egui::{Color32, Context, FontId, RichText};
use std::path::PathBuf;
use std::time::Instant;
use xmind_core::col::Col;
use xmind_core::id::new_id;
use xmind_core::marker::Marker;
use xmind_core::model::*;
use xmind_core::theme::THEMES;
use xmind_core::XRResult;

pub struct App {
    pub ed: Editor,
    pub lang: Lang,
    pub outline_visible: bool,
    pub props_visible: bool,
    pub show_new: bool,
    pub show_help: bool,
    pub show_about: bool,
    pub rename_sheet: Option<(usize, String)>,
    pub recent: Vec<PathBuf>,
    pub note_buf: Option<(String, String)>,
    pub link_buf: Option<(String, String)>,
    pub bnd_buf: Option<(String, String)>,
    pub rel_buf: Option<(String, String)>,
    pub label_input: String,
    pub selftest_frames: Option<u32>,
    pub last_autosave: Instant,
    pub recovery: Vec<PathBuf>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, open_path: Option<PathBuf>, selftest: bool) -> App {
        let prefs = Prefs::load();
        let _ = &cc;
        let mut app = App {
            ed: Editor::new(),
            lang: prefs.lang,
            outline_visible: prefs.outline,
            props_visible: true,
            show_new: false,
            show_help: false,
            show_about: false,
            rename_sheet: None,
            recent: prefs.recent.clone(),
            note_buf: None,
            link_buf: None,
            bnd_buf: None,
            rel_buf: None,
            label_input: String::new(),
            selftest_frames: selftest.then_some(40),
            last_autosave: Instant::now(),
            recovery: Vec::new(),
        };
        // fonts
        if let Some(fam) = crate::fonts::install(&cc.egui_ctx) {
            eprintln!("[xmind-rs] CJK font: {fam}");
        }
        // startup state
        match open_path {
            Some(p) => {
                if let Err(e) = app.ed.load(p.clone()) {
                    eprintln!("打开失败: {e}");
                } else {
                    app.push_recent(p);
                }
            }
            None => {
                app.ed.load_doc(xmind_core::templates::make("blank"));
            }
        }
        app.scan_recovery();
        app
    }

    fn push_recent(&mut self, p: PathBuf) {
        self.recent.retain(|x| x != &p);
        self.recent.insert(0, p);
        self.recent.truncate(10);
    }

    fn scan_recovery(&mut self) {
        let dir = autosave_dir();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|e| e.to_str()) == Some("xmind") {
                    self.recovery.push(p);
                }
            }
        }
    }

    fn autosave_path(&self) -> PathBuf {
        let stem = self
            .ed
            .path
            .as_ref()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "untitled".into());
        let mut name = String::new();
        for c in stem.chars() {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                name.push(c);
            }
        }
        if name.is_empty() {
            name = "untitled".into();
        }
        autosave_dir().join(format!("{name}.xmind"))
    }

    fn autosave(&mut self) {
        if !self.ed.dirty {
            return;
        }
        let p = self.autosave_path();
        if xmind_core::fmt::save_xmind(&self.ed.doc, &p).is_ok() {
            self.ed.dirty = true; // autosave does not clear dirty
        }
    }

    // ---------- file actions ----------

    fn act_open(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .add_filter("XMind 及可导入格式", &["xmind", "md", "txt", "opml", "mm"])
            .pick_file()
        else {
            return;
        };
        if self.ed.dirty && !self.ed.path.as_ref().is_some_and(|q| *q == p) {
            // best-effort: autosave before replacing
            self.autosave();
        }
        match self.ed.load(p.clone()) {
            Ok(()) => self.push_recent(p),
            Err(e) => self.toast(format!("打开失败: {e}")),
        }
    }

    fn act_save(&mut self) {
        if self.ed.path.is_none() {
            self.act_save_as();
            return;
        }
        if let Err(e) = self.ed.save() {
            self.toast(format!("保存失败: {e}"));
        } else {
            self.toast("已保存");
            let p = self.ed.path.clone().unwrap();
            self.push_recent(p);
        }
    }

    fn act_save_as(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .add_filter("XMind", &["xmind"])
            .set_file_name("未命名.xmind")
            .save_file()
        else {
            return;
        };
        match self.ed.save_as(p.clone()) {
            Ok(()) => {
                self.toast("已保存");
                self.push_recent(p);
            }
            Err(e) => self.toast(format!("保存失败: {e}")),
        }
    }

    fn act_new_template(&mut self, id: &str) {
        self.ed.load_doc(xmind_core::templates::make(id));
        self.show_new = false;
    }

    fn act_export(&mut self, ext: &str) {
        let name = self
            .ed
            .path
            .as_ref()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "导出".into());
        let Some(p) = rfd::FileDialog::new()
            .add_filter(ext, &[ext])
            .set_file_name(format!("{name}.{ext}"))
            .save_file()
        else {
            return;
        };
        match xmind_core::export::export(&self.ed.doc, &p, 2.0) {
            Ok(()) => self.toast(format!("已导出 {}", p.display())),
            Err(e) => self.toast(format!("导出失败: {e}")),
        }
    }

    fn act_insert_image(&mut self) {
        let Some(id) = self.ed.primary().cloned() else {
            self.toast("请先选择主题");
            return;
        };
        let Some(p) = rfd::FileDialog::new()
            .add_filter("图片", &["png", "jpg", "jpeg"])
            .pick_file()
        else {
            return;
        };
        let Ok(raw) = std::fs::read(&p) else {
            self.toast("读取图片失败");
            return;
        };
        let Ok(img) = image::load_from_memory(&raw) else {
            self.toast("不支持的图片格式");
            return;
        };
        let (w, h) = (img.width() as f32, img.height() as f32);
        // normalize to png
        let mut png_bytes = Vec::new();
        {
            let rgba = img.to_rgba8();
            let mut enc = image::codecs::png::PngEncoder::new(&mut png_bytes);
            use image::ImageEncoder as _;
            let _ = enc.write_image(rgba.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8);
        }
        let asset = new_id();
        self.ed.commit();
        self.ed.doc.assets.insert(asset.clone(), Asset { kind: "png".into(), data: png_bytes });
        if let Some(t) = self.ed.doc.sheet_mut().find_mut(&id) {
            t.image = Some(ImageRef { asset, w, h });
        }
        self.ed.scene_ver += 1;
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.ed.toast = Some((msg.into(), 2.5));
    }

    // ---------- frame ----------

    pub fn frame_update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if let Some(n) = &mut self.selftest_frames {
            *n = n.saturating_sub(1);
            if *n == 0 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }
        self.global_keys(ctx);
        self.dropped_files(ctx);
        self.autosave_tick();
        self.update_title(ctx);

        let zen = false; // zen handled below via editor-less flag; keep simple
        let _ = zen;

        // top toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(3.0);
            ui.horizontal_wrapped(|ui| {
                self.toolbar(ui, ctx);
            });
            ui.add_space(2.0);
        });

        // find bar
        if self.ed.find.is_some() {
            egui::TopBottomPanel::top("findbar").show(ctx, |ui| {
                self.find_bar(ui);
            });
        }

        // bottom sheets bar
        egui::TopBottomPanel::bottom("sheets").show(ctx, |ui| {
            self.sheets_bar(ui);
        });

        // outline
        if self.outline_visible {
            egui::SidePanel::left("outline")
                .resizable(true)
                .default_width(240.0)
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    ui.heading(tr(self.lang, "outline"));
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let root = self.ed.doc.sheet().root.clone();
                        self.outline_row(ui, &root, 0);
                    });
                });
        }

        // properties
        if self.props_visible {
            egui::SidePanel::right("props")
                .resizable(true)
                .default_width(280.0)
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    ui.heading(tr(self.lang, "props"));
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.properties(ui);
                    });
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                canvas::show(ui, &mut self.ed, self.lang);
            });

        self.dialogs(ctx);
    }

    // ---------- toolbar ----------

    fn toolbar(&mut self, ui: &mut egui::Ui, _ctx: &Context) {
        let b = |ui: &mut egui::Ui, label: &str| ui.add_sized([0.0, 24.0], egui::Button::new(RichText::new(label).size(13.0)));
        ui.menu_button(tr(self.lang, "file"), |ui| {
            self.file_menu(ui);
        });
        ui.separator();
        if b(ui, tr(self.lang, "new")).clicked() {
            self.show_new = true;
        }
        if b(ui, tr(self.lang, "open")).clicked() {
            self.act_open();
        }
        if b(ui, tr(self.lang, "save")).clicked() {
            self.act_save();
        }
        ui.separator();
        if b(ui, "↩").on_hover_text(tr(self.lang, "undo")).clicked() && self.ed.undo.is_empty() == false {
            self.ed.undo();
        }
        if b(ui, "↪").on_hover_text(tr(self.lang, "redo")).clicked() && !self.ed.redo.is_empty() {
            self.ed.redo();
        }
        ui.separator();
        if b(ui, tr(self.lang, "find")).clicked() {
            self.ed.start_find();
        }
        ui.separator();
        if b(ui, "−").clicked() {
            let c = ui.ctx().screen_rect().center();
            self.ed.view.zoom_at((c.x, c.y), 1.0 / 1.2);
        }
        let z = format!("{:.0}%", self.ed.view.zoom * 100.0);
        if b(ui, &z).clicked() {
            self.ed.view.zoom = 1.0;
        }
        if b(ui, "+").clicked() {
            let c = ui.ctx().screen_rect().center();
            self.ed.view.zoom_at((c.x, c.y), 1.2);
        }
        if b(ui, "⤢").on_hover_text(tr(self.lang, "fit")).clicked() {
            if let Some(sc) = &self.ed.scene {
                let b = sc.bounds;
                let sz = ui.ctx().screen_rect().size();
                let z = ((sz.x - 200.0) / b.w.max(1.0)).min((sz.y - 160.0) / b.h.max(1.0)).clamp(0.15, 1.6);
                self.ed.view.zoom = z;
                self.ed.view.center_on((sz.x, sz.y), b, None);
            }
        }
        ui.separator();
        ui.menu_button(tr(self.lang, "theme"), |ui| {
            for th in THEMES {
                ui.horizontal(|ui| {
                    if ui.button(th.name).clicked() {
                        self.ed.commit();
                        let cur = self.ed.doc.current;
                        self.ed.doc.sheets[cur].theme = Some(th.id.to_string());
                        self.ed.scene_ver += 1;
                        ui.close_menu();
                    }
                    for c in th.palette.iter().take(5) {
                        ui.add_sized([10.0, 10.0], egui::Button::new(" ").fill(Color32::from_rgb(c.0, c.1, c.2)));
                    }
                });
            }
        });
        ui.separator();
        let ol = if self.outline_visible { "☰◉" } else { "☰" };
        if b(ui, ol).on_hover_text(tr(self.lang, "outline")).clicked() {
            self.outline_visible = !self.outline_visible;
        }
        let pp = if self.props_visible { "▶◉" } else { "▶" };
        if b(ui, pp).on_hover_text(tr(self.lang, "props")).clicked() {
            self.props_visible = !self.props_visible;
        }
        ui.separator();
        if b(ui, tr(self.lang, "present")).clicked() {
            self.ed.start_presentation();
        }
        if b(ui, tr(self.lang, "help")).clicked() {
            self.show_help = true;
        }
    }

    fn file_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button(tr(self.lang, "new")).clicked() {
            self.show_new = true;
            ui.close_menu();
        }
        if ui.button(tr(self.lang, "open")).clicked() {
            self.act_open();
            ui.close_menu();
        }
        if ui.button(tr(self.lang, "save")).clicked() {
            self.act_save();
            ui.close_menu();
        }
        if ui.button(tr(self.lang, "save_as")).clicked() {
            self.act_save_as();
            ui.close_menu();
        }
        ui.separator();
        ui.menu_button(tr(self.lang, "import"), |ui| {
            if ui.button("Markdown / OPML / FreeMind / TXT / XMind…").clicked() {
                self.act_open();
                ui.close_menu();
            }
        });
        ui.menu_button(tr(self.lang, "export"), |ui| {
            for (ext, name) in [
                ("png", "PNG 图片"),
                ("svg", "SVG 矢量图"),
                ("pdf", "PDF 文档"),
                ("md", "Markdown"),
                ("txt", "纯文本大纲"),
                ("opml", "OPML"),
                ("mm", "FreeMind"),
                ("html", "HTML 网页"),
                ("docx", "Word 文档"),
                ("xlsx", "Excel 表格"),
            ] {
                if ui.button(name).clicked() {
                    self.act_export(ext);
                    ui.close_menu();
                }
            }
        });
        ui.separator();
        if !self.recent.is_empty() {
            ui.menu_button(tr(self.lang, "recent"), |ui| {
                for p in self.recent.clone() {
                    if ui.button(p.display().to_string()).clicked() {
                        if self.ed.load(p.clone()).is_ok() {
                            self.push_recent(p);
                        } else {
                            self.toast("打开失败");
                        }
                        ui.close_menu();
                    }
                }
            });
        }
        ui.separator();
        if ui.button(format!("{} ({} / English)", tr(self.lang, "lang"), "中文")).clicked() {
            self.lang = if self.lang == Lang::Zh { Lang::En } else { Lang::Zh };
            ui.close_menu();
        }
        if ui.button(tr(self.lang, "about")).clicked() {
            self.show_about = true;
            ui.close_menu();
        }
        if ui.button(tr(self.lang, "quit")).clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    // ---------- find bar ----------

    fn find_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("🔍");
            let mut q = self.ed.find.as_ref().unwrap().query.clone();
            let resp = ui.add(
                egui::TextEdit::singleline(&mut q)
                    .hint_text(tr(self.lang, "find_placeholder"))
                    .desired_width(180.0),
            );
            if resp.changed() {
                self.ed.find.as_mut().unwrap().query = q;
                self.ed.recompute_find();
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.ed.find_jump(true);
            }
            let n = self.ed.find.as_ref().unwrap().matches.len();
            ui.label(format!("{}/{} {}", self.ed.find.as_ref().unwrap().idx + 1.min(n), n, tr(self.lang, "matches")));
            if ui.button("↑").clicked() {
                self.ed.find_jump(false);
            }
            if ui.button("↓").clicked() {
                self.ed.find_jump(true);
            }
            let mut rep = self.ed.find.as_ref().unwrap().replace.clone();
            let r2 = ui.add(egui::TextEdit::singleline(&mut rep).hint_text(tr(self.lang, "replace")).desired_width(140.0));
            if r2.changed() {
                self.ed.find.as_mut().unwrap().replace = rep;
            }
            if ui.button(tr(self.lang, "replace")).clicked() {
                self.ed.replace_current();
            }
            if ui.button(tr(self.lang, "replace_all")).clicked() {
                self.ed.replace_all();
            }
            if ui.button("✕").clicked() {
                self.ed.find = None;
            }
        });
    }

    // ---------- sheets bar ----------

    fn sheets_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let cur = self.ed.doc.current;
            let titles: Vec<String> = self.ed.doc.sheets.iter().map(|s| s.title.clone()).collect();
            for (i, t) in titles.iter().enumerate() {
                let is_cur = i == cur;
                let label = RichText::new(format!(" {} ", t)).size(12.5).color(if is_cur {
                    Color32::WHITE
                } else {
                    Color32::from_gray(90)
                });
                let btn = ui.add(egui::Button::new(label).fill(if is_cur {
                    Color32::from_rgb(29, 122, 243)
                } else {
                    Color32::from_gray(225)
                }));
                if btn.clicked() {
                    self.ed.doc.current = i;
                    self.ed.scene_ver += 1;
                    self.ed.sel.clear();
                }
                if btn.double_clicked() {
                    self.rename_sheet = Some((i, t.clone()));
                }
            }
            if ui.button("＋").on_hover_text(tr(self.lang, "add_sheet")).clicked() {
                self.ed.commit();
                self.ed.doc.add_sheet();
                self.ed.scene_ver += 1;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let count = self.ed.doc.sheet().topic_count();
                ui.label(
                    RichText::new(format!("{} {} · {}", count, tr(self.lang, "topic_count"), if self.ed.dirty {
                        tr(self.lang, "modified")
                    } else {
                        tr(self.lang, "saved")
                    }))
                    .size(11.5)
                    .color(Color32::from_gray(120)),
                );
                if self.ed.doc.sheets.len() > 1 {
                    if ui.button("🗑").on_hover_text(tr(self.lang, "del_sheet")).clicked() {
                        self.ed.commit();
                        let i = self.ed.doc.current;
                        self.ed.doc.sheets.remove(i);
                        self.ed.doc.current = self.ed.doc.current.min(self.ed.doc.sheets.len() - 1);
                        self.ed.scene_ver += 1;
                    }
                    if ui.button("⧉").on_hover_text(tr(self.lang, "dup_sheet")).clicked() {
                        self.ed.commit();
                        let dup = self.ed.doc.sheet().clone();
                        let title = format!("{} 副本", dup.title);
                        self.ed.doc.sheets.push(Sheet { id: new_id(), title, root: dup.root, relationships: dup.relationships, theme: dup.theme });
                        self.ed.scene_ver += 1;
                    }
                }
            });
        });
    }

    // ---------- outline ----------

    fn outline_row(&mut self, ui: &mut egui::Ui, t: &Topic, depth: usize) {
        let is_sel = self.ed.primary().map(|p| *p == t.id).unwrap_or(false);
        let has_children = !t.children.is_empty();
        ui.horizontal(|ui| {
            ui.add_space(6.0 + depth as f32 * 16.0);
            let chev = if !has_children {
                "·"
            } else if t.expanded {
                "▾"
            } else {
                "▸"
            };
            if ui.small_button(chev).clicked() && has_children {
                let id = t.id.clone();
                self.ed.toggle_expand(&id);
            }
            let color = if is_sel { Color32::from_rgb(29, 122, 243) } else { Color32::from_gray(40) };
            let max_w = ui.available_width() - 34.0;
            let text = t.text.clone();
            let galley = ui.ctx().fonts(|f| f.layout(text, egui::FontId::proportional(13.0), color, max_w));
            let size = egui::vec2(galley.size().x.max(8.0), galley.size().y.max(14.0));
            let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
            let p = ui.painter().clone();
            p.galley(rect.min, galley, Color32::PLACEHOLDER);
            if resp.clicked() {
                let id = t.id.clone();
                self.ed.select_topic(&id, false);
            }
            if resp.double_clicked() {
                let id = t.id.clone();
                self.ed.start_editing(&id);
            }
        });
        if t.expanded {
            for c in &t.children {
                self.outline_row(ui, c, depth + 1);
            }
        }
    }

    // ---------- properties ----------

    fn properties(&mut self, ui: &mut egui::Ui) {
        if let Some((pid, bid)) = self.ed.sel_boundary.clone() {
            ui.label(RichText::new(tr(self.lang, "boundary")).size(14.0).strong());
            ui.add_space(4.0);
            ui.label(tr(self.lang, "color"));
            ui.horizontal(|ui| {
                for c in swatches() {
                    if ui.add(egui::Button::new("  ").fill(Color32::from_rgb(c.0, c.1, c.2))).clicked() {
                        let (pid2, bid2) = (pid.clone(), bid.clone());
                        self.ed.update_boundary(&pid2, &bid2, move |b| b.color = Some(c));
                    }
                }
            });
            ui.add_space(6.0);
            ui.label(tr(self.lang, "text"));
            let cur = self
                .ed
                .doc
                .sheet()
                .find(&pid)
                .and_then(|p| p.boundaries.iter().find(|b| b.id == bid))
                .and_then(|b| b.label.clone())
                .unwrap_or_default();
            let mut buf = self.bnd_buf.clone().map(|(id, b)| if id == bid { b } else { cur.clone() }).unwrap_or(cur);
            let r = ui.add(egui::TextEdit::singleline(&mut buf).desired_width(f32::INFINITY));
            self.bnd_buf = Some((bid.clone(), buf.clone()));
            if r.lost_focus() {
                let (pid2, bid2) = (pid.clone(), bid.clone());
                self.ed.update_boundary(&pid2, &bid2, move |b| b.label = if buf.trim().is_empty() { None } else { Some(buf.clone()) });
                self.bnd_buf = None;
            }
            ui.add_space(8.0);
            if ui.button(tr(self.lang, "delete")).clicked() {
                self.ed.delete_selected();
            }
            return;
        }
        if let Some(rid) = self.ed.sel_rel.clone() {
            ui.label(RichText::new(tr(self.lang, "relationship")).size(14.0).strong());
            let rel = self.ed.doc.sheet().relationships.iter().find(|r| r.id == rid).cloned();
            let Some(rel) = rel else { return };
            ui.add_space(4.0);
            ui.label(tr(self.lang, "text"));
            let mut buf = self.rel_buf.clone().map(|(id, b)| if id == rid { b } else { rel.label.clone().unwrap_or_default() }).unwrap_or_default();
            let r = ui.add(egui::TextEdit::singleline(&mut buf).desired_width(f32::INFINITY));
            self.rel_buf = Some((rid.clone(), buf.clone()));
            if r.lost_focus() {
                let rid2 = rid.clone();
                let b2 = buf.clone();
                self.ed.update_relationship(&rid2, move |x| x.label = if b2.trim().is_empty() { None } else { Some(b2.clone()) });
                self.rel_buf = None;
            }
            ui.add_space(4.0);
            let mut dashed = rel.dashed;
            if ui.checkbox(&mut dashed, "虚线").changed() {
                let rid2 = rid.clone();
                self.ed.update_relationship(&rid2, move |x| x.dashed = dashed);
            }
            ui.horizontal(|ui| {
                if ui.selectable_label(rel.arrows == Arrows::None, "无箭头").clicked() {
                    let rid2 = rid.clone();
                    self.ed.update_relationship(&rid2, |x| x.arrows = Arrows::None);
                }
                if ui.selectable_label(rel.arrows == Arrows::End, "→").clicked() {
                    let rid2 = rid.clone();
                    self.ed.update_relationship(&rid2, |x| x.arrows = Arrows::End);
                }
                if ui.selectable_label(rel.arrows == Arrows::Both, "↔").clicked() {
                    let rid2 = rid.clone();
                    self.ed.update_relationship(&rid2, |x| x.arrows = Arrows::Both);
                }
            });
            ui.label(tr(self.lang, "color"));
            ui.horizontal(|ui| {
                for c in swatches() {
                    if ui.add(egui::Button::new("  ").fill(Color32::from_rgb(c.0, c.1, c.2))).clicked() {
                        let rid2 = rid.clone();
                        self.ed.update_relationship(&rid2, move |x| x.color = Some(c));
                    }
                }
            });
            ui.add_space(8.0);
            if ui.button(tr(self.lang, "delete")).clicked() {
                self.ed.delete_selected();
            }
            return;
        }
        let Some(id) = self.ed.primary().cloned() else {
            ui.label(
                RichText::new("选择一个主题查看属性\n\n提示：Tab 新建子主题 · Enter 新建同级 · 双击编辑 · 右键更多操作")
                    .size(12.5)
                    .color(Color32::from_gray(110)),
            );
            return;
        };
        let Some(topic) = self.ed.doc.sheet().find(&id).cloned() else { return };
        let is_root = self.ed.doc.sheet().root.id == id;

        // marker quick rows
        ui.label(RichText::new(tr(self.lang, "marker")).size(13.0).strong());
        ui.horizontal_wrapped(|ui| {
            for (name, ms) in crate::editor::marker_groups() {
                ui.menu_button(name, |ui| {
                    for m in &ms {
                        let on = topic.markers.contains(m);
                        if ui.selectable_label(on, format!("{} {}", if on { "✓" } else { " " }, marker_name(m))).clicked() {
                            let m2 = *m;
                            self.ed.toggle_marker(&id, m2);
                            ui.close_menu();
                        }
                    }
                });
            }
            if ui.small_button(tr(self.lang, "remove_marker")).clicked() {
                self.ed.clear_markers(&id);
            }
        });
        ui.add_space(6.0);

        // text style
        ui.label(RichText::new(tr(self.lang, "style")).size(13.0).strong());
        ui.horizontal(|ui| {
            ui.label(tr(self.lang, "font_size"));
            let size = topic.style.as_ref().and_then(|s| s.font_size).unwrap_or(14.0);
            if ui.button("−").clicked() {
                let s2 = (size - 1.0).max(10.0);
                self.ed.with_style(&id, move |st| st.font_size = Some(s2));
            }
            ui.label(format!("{size:.0}"));
            if ui.button("＋").clicked() {
                let s2 = (size + 1.0).min(40.0);
                self.ed.with_style(&id, move |st| st.font_size = Some(s2));
            }
            let bold = topic.style.as_ref().and_then(|s| s.bold).unwrap_or(false);
            if ui.selectable_label(bold, "B").clicked() {
                self.ed.with_style(&id, move |st| st.bold = Some(!bold));
            }
        });
        ui.label(tr(self.lang, "fill"));
        ui.horizontal_wrapped(|ui| {
            if ui.small_button(tr(self.lang, "auto")).clicked() {
                self.ed.with_style(&id, |st| st.fill = None);
            }
            for c in swatches() {
                if ui.add(egui::Button::new("  ").fill(Color32::from_rgb(c.0, c.1, c.2))).clicked() {
                    self.ed.with_style(&id, move |st| st.fill = Some(c));
                }
            }
        });
        ui.label(tr(self.lang, "text"));
        ui.horizontal_wrapped(|ui| {
            if ui.small_button(tr(self.lang, "auto")).clicked() {
                self.ed.with_style(&id, |st| st.text = None);
            }
            for c in swatches() {
                if ui.add(egui::Button::new(RichText::new("A").color(Color32::from_rgb(c.0, c.1, c.2)))).clicked() {
                    self.ed.with_style(&id, move |st| st.text = Some(c));
                }
            }
        });
        ui.label(tr(self.lang, "shape"));
        ui.horizontal_wrapped(|ui| {
            for sh in [Shape::Round, Shape::Rect, Shape::Underline, Shape::Capsule, Shape::Ellipse, Shape::Diamond] {
                if ui.small_button(sh.name_zh()).clicked() {
                    self.ed.with_style(&id, move |st| st.shape = Some(sh));
                }
            }
        });
        ui.add_space(6.0);

        // structure (for children)
        ui.label(RichText::new(tr(self.lang, "structure")).size(13.0).strong());
        ui.horizontal_wrapped(|ui| {
            for st in [Structure::LogicRight, Structure::LogicLeft, Structure::Map, Structure::OrgDown, Structure::OrgUp, Structure::Tree, Structure::Timeline, Structure::Fishbone, Structure::Matrix] {
                if ui.small_button(st.name_zh()).clicked() {
                    self.ed.set_structure(&id, Some(st));
                }
            }
        });
        ui.add_space(6.0);

        // labels
        ui.label(RichText::new(tr(self.lang, "label")).size(13.0).strong());
        ui.horizontal(|ui| {
            let mut inp = self.label_input.clone();
            let r = ui.add(egui::TextEdit::singleline(&mut inp).desired_width(140.0).hint_text("新标签…"));
            self.label_input = inp;
            let add = ui.input(|i| i.key_pressed(egui::Key::Enter)) && (r.has_focus() || r.lost_focus());
            let blur_add = r.lost_focus();
            if ui.small_button("＋").clicked() || add || blur_add {
                let l = self.label_input.trim().to_string();
                if !l.is_empty() {
                    self.ed.add_label(&id, l);
                    self.label_input.clear();
                }
            }
        });
        for (i, l) in topic.labels.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("🏷 {l}"));
                if ui.small_button("✕").clicked() {
                    self.ed.remove_label(&id, i);
                }
            });
        }
        ui.add_space(6.0);

        // note
        ui.label(RichText::new(tr(self.lang, "note")).size(13.0).strong());
        let cur_note = topic.note.clone().unwrap_or_default();
        let mut buf = self
            .note_buf
            .clone()
            .map(|(nid, b)| if nid == id { b } else { cur_note.clone() })
            .unwrap_or(cur_note);
        let r = ui.add(
            egui::TextEdit::multiline(&mut buf)
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );
        self.note_buf = Some((id.clone(), buf.clone()));
        if r.lost_focus() || ui.button("保存备注").clicked() {
            self.ed.set_note(&id, buf.clone());
            self.note_buf = None;
        }

        // link
        ui.add_space(4.0);
        ui.label(RichText::new(tr(self.lang, "link")).size(13.0).strong());
        let cur_link = topic.link.clone().unwrap_or_default();
        let mut lbuf = self
            .link_buf
            .clone()
            .map(|(lid, b)| if lid == id { b } else { cur_link.clone() })
            .unwrap_or_else(|| cur_link.clone());
        let r = ui.add(egui::TextEdit::singleline(&mut lbuf).desired_width(f32::INFINITY).hint_text("https://…"));
        self.link_buf = Some((id.clone(), lbuf.clone()));
        if r.lost_focus() {
            let l = lbuf.trim().to_string();
            self.ed.set_link(&id, if l.is_empty() { None } else { Some(l) });
            self.link_buf = None;
        }
        if !cur_link.is_empty() {
            if ui.small_button("打开链接").clicked() {
                ui.ctx().output_mut(|o| o.open_url = Some(egui::OpenUrl::new_tab(cur_link.clone())));
            }
        }

        // image
        ui.add_space(6.0);
        ui.label(RichText::new(tr(self.lang, "image")).size(13.0).strong());
        ui.horizontal(|ui| {
            if ui.button("插入图片…").clicked() {
                self.act_insert_image();
            }
            if topic.image.is_some() && ui.button("移除").clicked() {
                self.ed.commit();
                if let Some(t) = self.ed.doc.sheet_mut().find_mut(&id) {
                    t.image = None;
                }
                self.ed.scene_ver += 1;
            }
        });

        // actions
        ui.add_space(8.0);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr(self.lang, "drill")).clicked() {
                self.ed.drill.push(id.clone());
                self.ed.scene_ver += 1;
                self.ed.view = crate::editor::View::new();
            }
            if !self.ed.drill.is_empty() && ui.button(tr(self.lang, "drill_exit")).clicked() {
                self.ed.drill.pop();
                self.ed.scene_ver += 1;
            }
            if !is_root && ui.button(tr(self.lang, "delete")).clicked() {
                self.ed.delete_selected();
            }
        });
    }

    // ---------- dialogs ----------

    fn dialogs(&mut self, ctx: &Context) {
        if self.show_new {
            let mut open = true;
            egui::Window::new(tr(self.lang, "new"))
                .open(&mut open)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                        egui::Grid::new("tpl").num_columns(3).spacing([10.0, 10.0]).show(ui, |ui| {
                            for t in xmind_core::templates::TEMPLATES {
                                let picked = ui.add_sized(
                                    [140.0, 64.0],
                                    egui::Button::new(
                                        egui::RichText::new(format!("{}\n{}", t.name, t.desc)).small().weak(),
                                    ),
                                );
                                if picked.clicked() {
                                    let id = t.id;
                                    self.act_new_template(id);
                                }
                            }
                        });
                    });
                });
            self.show_new = open;
        }
        if self.show_help {
            let mut open = true;
            egui::Window::new(tr(self.lang, "shortcuts"))
                .open(&mut open)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.monospace(HELP_TEXT);
                });
            self.show_help = open;
        }
        if self.show_about {
            let mut open = true;
            egui::Window::new(tr(self.lang, "about"))
                .open(&mut open)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("XMind-RS v{}", env!("CARGO_PKG_VERSION")));
                    ui.label("开源思维导图 · 完全本地运行 · 无账号 · 无订阅 · 无遥测");
                    ui.label("与 .xmind 文件格式互通（XMind Zen / 2020+，可读 XMind 8）");
                    ui.label("MIT License · Rust + egui");
                    ui.hyperlink_to("功能与格式说明 → README.md", "https://github.com/");
                });
            self.show_about = open;
        }
        if let Some((idx, name)) = &mut self.rename_sheet {
            let mut open = true;
            let mut done = false;
            egui::Window::new(tr(self.lang, "rename_sheet"))
                .open(&mut open)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.add(egui::TextEdit::singleline(name).desired_width(200.0));
                    if ui.button("确定").clicked() {
                        done = true;
                    }
                });
            if done || !open {
                let title = name.clone();
                if let Some(s) = self.ed.doc.sheets.get_mut(*idx) {
                    s.title = title;
                    self.ed.dirty = true;
                }
                self.rename_sheet = None;
            }
        }
        if !self.recovery.is_empty() {
            let mut open = true;
            let files = self.recovery.clone();
            egui::Window::new(tr(self.lang, "autosaved_found"))
                .open(&mut open)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("检测到自动保存的文件，是否恢复？");
                    ui.add_space(6.0);
                    for p in &files {
                        ui.horizontal(|ui| {
                            ui.monospace(p.display().to_string());
                            if ui.button(tr(self.lang, "restore")).clicked() {
                                if self.ed.load(p.clone()).is_ok() {
                                    self.toast("已恢复自动保存的文件");
                                    self.recovery.clear();
                                }
                            }
                        });
                    }
                    ui.add_space(6.0);
                    if ui.button(tr(self.lang, "discard")).clicked() {
                        for p in &files {
                            let _ = std::fs::remove_file(p);
                        }
                        self.recovery.clear();
                    }
                });
            if !open {
                self.recovery.clear();
            }
        }
    }

    // ---------- global shortcuts / misc ----------

    fn global_keys(&mut self, ctx: &Context) {
        let pressed = |k: egui::Key| ctx.input(|i| i.key_pressed(k) && i.modifiers.command);
        if self.ed.editing.is_some() {
            return;
        }
        if pressed(egui::Key::S) {
            self.act_save();
        } else if pressed(egui::Key::O) {
            self.act_open();
        } else if pressed(egui::Key::N) {
            self.show_new = true;
        } else if pressed(egui::Key::F) {
            self.ed.start_find();
        } else if pressed(egui::Key::Z) {
            if ctx.input(|i| i.modifiers.shift) {
                self.ed.redo();
            } else {
                self.ed.undo();
            }
        } else if pressed(egui::Key::C) {
            self.ed.copy_selection();
        } else if pressed(egui::Key::X) {
            self.ed.cut_selection();
        } else if pressed(egui::Key::V) {
            self.ed.paste();
        } else if pressed(egui::Key::A) {
            // select all topics
            let mut ids = Vec::new();
            self.ed.doc.sheet().walk(&mut |t| ids.push(t.id.clone()));
            self.ed.sel = ids;
        } else if pressed(egui::Key::P) {
            self.ed.start_presentation();
        }
    }

    fn dropped_files(&mut self, ctx: &Context) {
        let dropped: Vec<PathBuf> = ctx
            .input(|i| i.raw.dropped_files.clone())
            .iter()
            .filter_map(|d| d.path.clone())
            .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("xmind") | Some("md") | Some("opml") | Some("mm") | Some("txt")))
            .collect();
        if let Some(p) = dropped.first() {
            if self.ed.path.as_ref() != Some(p) {
                if self.ed.load(p.clone()).is_ok() {
                    self.push_recent(p.clone());
                    self.toast("已打开拖入的文件");
                }
            }
        }
    }

    fn autosave_tick(&mut self) {
        if self.ed.dirty && self.last_autosave.elapsed().as_secs() >= 25 {
            self.autosave();
            self.last_autosave = Instant::now();
        }
    }

    fn update_title(&mut self, ctx: &Context) {
        let name = self
            .ed
            .path
            .as_ref()
            .map(|p| p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default())
            .unwrap_or_else(|| "未命名".into());
        let mark = if self.ed.dirty { " ●" } else { "" };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("XMind-RS — {name}{mark}")));
    }
}

fn autosave_dir() -> PathBuf {
    let dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("xmind-rs").join("autosave");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn marker_name(m: &Marker) -> String {
    match m {
        Marker::Priority(p) => format!("P{p}"),
        Marker::Task(t) => format!("进度{}%", *t as i32),
        Marker::Flag(c) => format!("{c:?}旗"),
        Marker::Star(c) => format!("{c:?}星"),
        Marker::People(c) => format!("{c:?}人"),
        Marker::Heart(_) => "心".into(),
        Marker::Face(f) => format!("{f:?}"),
        Marker::Symbol(s) => format!("{s:?}"),
    }
}

const HELP_TEXT: &str = "\
快捷键（macOS 用 ⌘，Windows/Linux 用 Ctrl）

  Tab          新建子主题并编辑
  Enter        新建同级主题并编辑
  F2 / Space   编辑当前主题
  Delete       删除当前主题（含子树）
  ↑ ↓          上/下一个同级主题
  ← →          父主题 / 子主题
  双击         编辑主题
  右键         完整菜单（插入/标记/样式/结构/下钻）
  拖拽主题     自由摆放；拖到其他主题上 = 移动为其子主题
  Shift+拖拽空隙 框选多个主题
  滚轮 / 触控板 平移画布；⌘+滚轮 / 双指缩放 = 缩放
  ⌘ +/-/0      放大 / 缩小 / 100%
  ⌘F 查找      Enter 下一个 · 支持替换
  ⌘Z / ⌘⇧Z     撤销 / 重做
  ⌘C / X / V   复制 / 剪切 / 粘贴子树
  ⌘A           全选主题
  ⌘S / ⌘O / ⌘N 保存 / 打开 / 新建
  ⌘P           演示模式（← → 翻页，Esc 退出）
  Esc          取消编辑 / 退出下钻 / 清除选择
  拖入 .xmind/.md/.opml/.mm 文件直接打开";

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        self.frame_update(ctx, frame);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let prefs = Prefs {
            lang: self.lang,
            outline: self.outline_visible,
            recent: self.recent.clone(),
        };
        prefs.save();
    }
}

pub type AppResult = XRResult<()>;
