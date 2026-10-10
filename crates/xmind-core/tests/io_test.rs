use xmind_core::fmt;
use xmind_core::marker::{ColKey, Marker};
use xmind_core::model::{Arrows, Boundary, Relationship, Structure, Topic};
use xmind_core::text::MockMeasurer;
use xmind_core::{Doc, Sheet};

fn sample_doc() -> Doc {
    let mut d = Doc::new();
    d.theme = "sakura".into();
    let s = &mut d.sheets[0];
    s.title = "测试画布".into();
    s.root.text = "根主题".into();
    s.root.structure = Some(Structure::Map);
    let mut a = Topic::new("分支甲");
    a.markers.push(Marker::Priority(2));
    a.labels.push("标签一".into());
    a.note = Some("备注内容".into());
    a.link = Some("https://example.com".into());
    a.insert_child(None, Topic::new("子项 1"));
    a.insert_child(None, Topic::new("子项 2"));
    s.root.insert_child(None, a);
    let mut b = Topic::new("分支乙");
    b.markers.push(Marker::Task(50.0));
    b.markers.push(Marker::Flag(ColKey::Red));
    b.expanded = false;
    s.root.insert_child(None, b);
    // boundary over first two children of root? root has 2 children: boundary [0..1]
    s.root.boundaries.push(Boundary {
        id: "bnd1".into(),
        start: 0,
        end: 1,
        label: Some("范围".into()),
        color: None,
    });
    let rid = s.root.id.clone();
    let cid = s.root.children[0].id.clone();
    s.relationships.push(Relationship {
        id: "rel1".into(),
        a: rid,
        b: cid,
        label: Some("关联".into()),
        color: None,
        dashed: true,
        arrows: Arrows::End,
    });
    d.add_sheet();
    d.current = 0;
    d
}

#[test]
fn xmind_roundtrip_lossless() {
    let doc = sample_doc();
    let path = std::env::temp_dir().join("xr_roundtrip.xmind");
    fmt::save_xmind(&doc, &path).unwrap();
    let loaded = fmt::load_xmind(&path).unwrap();
    assert_eq!(loaded.core, doc.core);
    std::fs::remove_file(&path).ok();
}

#[test]
fn xmind_content_json_compat() {
    // strip our private entry by loading only content.json path: simulate by
    // writing, then reading with xmindrs ignored — verify structure/text survive
    let doc = sample_doc();
    let path = std::env::temp_dir().join("xr_compat.xmind");
    fmt::save_xmind(&doc, &path).unwrap();
    let file = std::fs::File::open(&path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut content = String::new();
    use std::io::Read;
    zip.by_name("content.json").unwrap().read_to_string(&mut content).unwrap();
    let parsed = xmind_core::fmt::xmind_zen::parse_content(&content).unwrap();
    let s = &parsed.sheets[0];
    assert_eq!(s.root.text, "根主题");
    assert_eq!(s.root.children.len(), 2);
    assert_eq!(s.root.children[0].text, "分支甲");
    assert_eq!(s.root.children[0].note.as_deref(), Some("备注内容"));
    assert_eq!(s.root.children[0].labels, vec!["标签一"]);
    assert_eq!(s.root.children[0].markers, vec![Marker::Priority(2)]);
    assert_eq!(s.root.children[1].expanded, false);
    assert_eq!(s.relationships.len(), 1);
    std::fs::remove_file(&path).ok();
}

#[test]
fn markdown_roundtrip() {
    let md = "# 根\n\n## 甲\n\n- 甲1\n- 甲2\n\n## 乙\n";
    let doc = xmind_core::fmt::markdown::parse(md);
    let sheet = doc.sheet();
    assert_eq!(sheet.root.text, "根");
    assert_eq!(sheet.root.children.len(), 2);
    assert_eq!(sheet.root.children[0].children.len(), 2);
}

#[test]
fn opml_and_freemind_roundtrip() {
    let doc = sample_doc();
    let opml = xmind_core::fmt::opml::render(&doc);
    let back = xmind_core::fmt::opml::parse(&opml).unwrap();
    assert_eq!(back.sheet().root.text, "根主题");
    assert_eq!(back.sheet().root.children.len(), 2);

    let mm = xmind_core::fmt::freemind::render(&doc);
    let back2 = xmind_core::fmt::freemind::parse(&mm).unwrap();
    assert_eq!(back2.sheet().root.children.len(), 2);
    assert_eq!(back2.sheet().root.children[0].note.as_deref(), Some("备注内容"));
}

#[test]
fn legacy_xmind8_parse() {
    let xml = r#"<?xml version="1.0"?>
<xmap-content version="2.0">
  <sheet id="s1" structure-class="org.xmind.ui.logic.right">
    <topic id="root">
      <title>旧版根</title>
      <children>
        <topics type="attached">
          <topic id="c1"><title>子一</title>
            <marker-refs><marker-ref marker-id="priority-1"/></marker-refs>
            <labels><label>标</label></labels>
            <notes><plain>旧备注</plain></notes>
          </topic>
        </topics>
        <topics type="detached">
          <topic id="f1"><title>浮动</title></topic>
        </topics>
      </children>
    </topic>
  </sheet>
</xmap-content>"#;
    let doc = xmind_core::fmt::xmind_legacy::parse(xml).unwrap();
    let s = doc.sheet();
    assert_eq!(s.root.text, "旧版根");
    assert_eq!(s.root.children[0].text, "子一");
    assert_eq!(s.root.children[0].markers, vec![Marker::Priority(1)]);
    assert_eq!(s.root.detached[0].text, "浮动");
    assert_eq!(s.root.children[0].note.as_deref(), Some("旧备注"));
}

#[test]
fn scene_renders_all_structures() {
    for st in [
        Structure::LogicRight,
        Structure::LogicLeft,
        Structure::Map,
        Structure::OrgDown,
        Structure::OrgUp,
        Structure::Tree,
        Structure::Timeline,
        Structure::Fishbone,
        Structure::Matrix,
    ] {
        let mut sh = Sheet::new("s", "中心主题");
        sh.root.structure = Some(st);
        for i in 0..3 {
            let mut c = Topic::new(format!("分支{i}"));
            for j in 0..2 {
                c.insert_child(None, Topic::new(format!("子{i}-{j}")));
            }
            sh.root.insert_child(None, c);
        }
        let scene = xmind_core::layout::build_scene(&sh, "classic-blue", &MockMeasurer::default(), None);
        assert!(scene.rects.len() >= 10, "{st:?} rects={}", scene.rects.len());
        assert!(!scene.prims.is_empty());
    }
}

#[test]
fn export_formats_smoke() {
    let doc = sample_doc();
    let dir = std::env::temp_dir();
    for (name, expect_binary) in [
        ("xr_out.md", false),
        ("xr_out.txt", false),
        ("xr_out.opml", false),
        ("xr_out.mm", false),
        ("xr_out.svg", false),
        ("xr_out.html", false),
        ("xr_out.docx", true),
        ("xr_out.xlsx", true),
    ] {
        let p = dir.join(name);
        xmind_core::export::export(&doc, &p, 1.0).unwrap();
        let meta = std::fs::metadata(&p).unwrap();
        assert!(meta.len() > 40, "{name} too small");
        std::fs::remove_file(&p).ok();
        let _ = expect_binary;
    }
}

#[test]
fn font_probe() {
    let book = xmind_core::export::raster::FontBook::load();
    println!("family={:?} available={}", book.family, book.available());
    if !book.available() {
        return; // fontless environment
    }
    use xmind_core::text::TextMeasurer;
    assert!(book.line_width("你好ABC", 16.0, false) > 10.0);
}

#[test]
fn glyph_render_probe() {
    use xmind_core::export::raster::FontBook;
    use xmind_core::text::TextMeasurer;
    let book = FontBook::load();
    if !book.available() {
        return; // fontless environment
    }
    // measure a "你好"
    let w = book.line_width("你好", 20.0, false) as u32 + 8;
    let mut pm = tiny_skia::Pixmap::new(w, 40).unwrap();
    pm.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
    // call internal draw via public render path: build scene with one text prim
    use xmind_core::scene::{Prim, Scene};
    use xmind_core::col::Col;
    use xmind_core::geo::{Rect, V2};
    let mut scene = Scene { bg: Col::rgb(255,255,255), ..Default::default() };
    scene.prims.push(Prim::Text { p: V2::new(4.0, 30.0), text: "你好ABC".into(), size: 20.0, color: Col::rgb(0,0,0), bold: false, italic: false, center: false });
    scene.bounds = Rect::new(0.0, 0.0, w as f32, 40.0);
    let png = xmind_core::export::raster::render_png_with(&scene, &Doc::new(), 1.0, &book).unwrap();
    std::fs::write("/tmp/glyph_probe.png", &png).unwrap();
    // count dark pixels
    let img = image::load_from_memory(&png).unwrap().to_rgba8();
    let dark = img.pixels().filter(|p| p.0[0] < 128).count();
    let min_r = img.pixels().map(|p| p.0[0]).min().unwrap();
    let uniq: std::collections::BTreeSet<u8> = img.pixels().map(|p| p.0[0]).collect();
    println!("dark pixels: {dark}, min_r={min_r}, uniq_r={:?}", uniq.into_iter().take(6).collect::<Vec<_>>());
    assert!(dark > 30, "text not rendered: {dark} dark pixels");
}

#[test]
fn diag_left_fish_alignment() {
    use xmind_core::text::MockMeasurer;
    let mk = |st: Structure| {
        let mut sh = Sheet::new("s", "中心主题");
        sh.root.structure = Some(st);
        for name in ["分支甲", "分支乙"] {
            let mut c = Topic::new(name);
            c.insert_child(None, Topic::new("子项一"));
            c.insert_child(None, Topic::new("子项二"));
            sh.root.insert_child(None, c);
        }
        sh
    };
    for st in [Structure::LogicLeft, Structure::Fishbone] {
        let sh = mk(st);
        let book = xmind_core::export::raster::FontBook::load();
        let scene = xmind_core::layout::build_scene(&sh, "classic-blue", &book, None);
        println!("== {st:?} (FontBook) ==");
        for (id, r) in &scene.rects {
            if let Some((s, e)) = scene.node_prims.get(id.as_str()) {
                let mut xmin = f32::MAX; let mut xmax = f32::MIN;
                for p in &scene.prims[*s..*e] {
                    if let xmind_core::scene::Prim::Text { p, .. } = p {
                        xmin = xmin.min(p.x); xmax = xmax.max(p.x);
                    }
                }
                if xmin <= xmax {
                    let text = sh.find(id).map(|t| t.text.clone()).unwrap_or_default();
                    println!("  {text}: rect.x=[{:.0},{:.0}] text.x=[{:.0},~{:.0}] inside={}",
                        r.x, r.right(), xmin, xmax, xmin >= r.x - 1.0 && xmax <= r.right() + 60.0);
                }
            }
        }
    }
}
