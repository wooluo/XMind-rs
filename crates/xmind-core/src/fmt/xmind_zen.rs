use crate::col::Col;
use crate::id::new_id;
use crate::marker::Marker;
use crate::model::*;
use crate::XRResult;
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;

// ---------- write ----------

pub fn write(doc: &Doc, path: &Path) -> XRResult<()> {
    let file = std::fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // content.json (XMind Zen compatible subset)
    let sheets: Vec<Value> = doc.sheets.iter().map(sheet_to_json).collect();
    let content = serde_json::to_string_pretty(&sheets)?;
    zip.start_file("content.json", opts)?;
    zip.write_all(content.as_bytes())?;

    // metadata.json
    let meta = json!({
        "creator": { "name": "xmind-rs", "version": env!("CARGO_PKG_VERSION") },
        "dataStructureVersion": "2",
        "activeSheetId": doc.sheet().id,
    });
    zip.start_file("metadata.json", opts)?;
    zip.write_all(serde_json::to_string(&meta)?.as_bytes())?;

    // manifest.json
    let mut entries = serde_json::Map::new();
    entries.insert("content.json".into(), json!({}));
    entries.insert("metadata.json".into(), json!({}));
    entries.insert("xmindrs.json".into(), json!({}));
    for id in doc.assets.keys() {
        entries.insert(format!("resources/{id}.png"), json!({}));
    }
    let manifest = json!({ "file-entries": entries });
    zip.start_file("manifest.json", opts)?;
    zip.write_all(serde_json::to_string(&manifest)?.as_bytes())?;

    // xmindrs.json — our lossless full model
    zip.start_file("xmindrs.json", opts)?;
    zip.write_all(serde_json::to_string_pretty(&doc.core)?.as_bytes())?;

    // resources
    for (id, asset) in &doc.assets {
        let name = format!("resources/{id}.{}", asset.kind);
        zip.start_file(&name, opts)?;
        zip.write_all(&asset.data)?;
    }

    zip.finish()?;
    Ok(())
}

fn sheet_to_json(sheet: &Sheet) -> Value {
    let mut o = json!({
        "id": sheet.id,
        "class": "sheet",
        "title": sheet.title,
        "rootTopic": topic_to_json(&sheet.root, true),
    });
    if !sheet.relationships.is_empty() {
        o["relationships"] = json!(sheet
            .relationships
            .iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "endpoint1": { "id": r.a },
                    "endpoint2": { "id": r.b },
                    "title": { "text": r.label.clone().unwrap_or_default() },
                })
            })
            .collect::<Vec<_>>());
    }
    o
}

fn topic_to_json(t: &Topic, is_root: bool) -> Value {
    let mut o = json!({
        "id": t.id,
        "class": "topic",
        "title": { "text": t.text },
    });
    if is_root {
        if let Some(st) = t.structure {
            o["structure"] = json!({ "class": st.to_xmind_class() });
        }
    } else if let Some(st) = t.structure {
        o["structure"] = json!({ "class": st.to_xmind_class() });
    }
    let mut children = serde_json::Map::new();
    if !t.children.is_empty() {
        children.insert("attached".into(), json!(t.children.iter().map(|c| topic_to_json(c, false)).collect::<Vec<_>>()));
    }
    if !t.detached.is_empty() {
        children.insert("detached".into(), json!(t.detached.iter().map(|c| topic_to_json(c, false)).collect::<Vec<_>>()));
    }
    if !children.is_empty() {
        o["children"] = Value::Object(children);
    }
    if !t.markers.is_empty() {
        o["markers"] = json!(t.markers.iter().map(|m| json!({ "markerId": m.to_xmind_id() })).collect::<Vec<_>>());
    }
    if !t.labels.is_empty() {
        o["labels"] = json!(t.labels);
    }
    if let Some(n) = &t.note {
        if !n.trim().is_empty() {
            o["notes"] = json!({ "plain": { "content": n } });
        }
    }
    if let Some(l) = &t.link {
        o["hyperlink"] = json!(l);
    }
    if let Some(im) = &t.image {
        o["image"] = json!({
            "src": format!("xap:resources/{}.png", im.asset),
            "width": im.w.round() as i64,
            "height": im.h.round() as i64,
        });
    }
    if let Some(style) = &t.style {
        let mut p = serde_json::Map::new();
        if let Some(f) = style.fill {
            p.insert("svg:fill".into(), json!(f.to_hex()));
        }
        if let Some(s) = style.stroke {
            p.insert("svg:stroke".into(), json!(s.to_hex()));
        }
        if let Some(w) = style.stroke_w {
            p.insert("border-line-width".into(), json!(w as f64));
        }
        if let Some(fs) = style.font_size {
            p.insert("svg:font-size".into(), json!(fs as f64));
        }
        if let Some(true) = style.bold {
            p.insert("svg:font-weight".into(), json!("bold"));
        }
        if let Some(true) = style.italic {
            p.insert("svg:font-style".into(), json!("italic"));
        }
        if let Some(sh) = style.shape {
            p.insert("shape-class".into(), json!(shape_class(sh)));
        }
        if let Some(c) = style.text {
            p.insert("fo:color".into(), json!(c.to_hex()));
        }
        if !p.is_empty() {
            o["style"] = json!({ "properties": p });
        }
    }
    if !t.expanded {
        o["expanded"] = json!(false);
    }
    o
}

fn shape_class(sh: Shape) -> &'static str {
    match sh {
        Shape::Round => "rounded-rect",
        Shape::Rect => "rect",
        Shape::Underline => "none",
        Shape::Capsule => "rounded-rect",
        Shape::Ellipse => "ellipse",
        Shape::Diamond => "diamond",
    }
}

fn shape_from_class(s: &str) -> Option<Shape> {
    Some(match s {
        "rect" | "rectangle" => Shape::Rect,
        "ellipse" | "oval" => Shape::Ellipse,
        "diamond" => Shape::Diamond,
        "none" => Shape::Underline,
        "underline" => Shape::Underline,
        _ => Shape::Round,
    })
}

// ---------- parse (content.json from official XMind) ----------

fn v_str(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string())
}

/// Title may be a plain string or {"text": "..."}.
fn parse_title(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(o) => o
            .get("text")
            .and_then(|t| t.as_str())
            .or_else(|| o.get("content").and_then(|t| t.as_str()))
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

fn parse_structure(v: &Value) -> Option<Structure> {
    match v {
        Value::String(s) => Some(Structure::from_xmind_class(s)),
        Value::Object(o) => o.get("class").and_then(|c| c.as_str()).map(Structure::from_xmind_class),
        _ => None,
    }
}

pub fn parse_content(content: &str) -> XRResult<Doc> {
    let sheets_v: Vec<Value> = serde_json::from_str(content)?;
    if sheets_v.is_empty() {
        return Err(crate::XRErr::Other("content.json 为空".into()));
    }
    let mut sheets = Vec::new();
    for sv in &sheets_v {
        let title = v_str(sv, "title").unwrap_or_else(|| "画布".into());
        let root_v = sv
            .get("rootTopic")
            .ok_or_else(|| crate::XRErr::Other("缺少 rootTopic".into()))?;
        let mut root = parse_topic(root_v)?;
        if root.text.is_empty() {
            root.text = "中心主题".into();
        }
        if let Some(st) = sv.get("structure").and_then(parse_structure) {
            root.structure = Some(st);
        }
        let mut rels = Vec::new();
        let rels_v = match sv.get("relationships") {
            Some(Value::Array(a)) => a.clone(),
            Some(Value::Object(o)) => o
                .get("array")
                .and_then(|a| a.as_array())
                .cloned()
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        for rv in &rels_v {
            let (Some(a), Some(b)) = (endpoint_id(rv, "endpoint1"), endpoint_id(rv, "endpoint2")) else {
                continue;
            };
            rels.push(Relationship {
                id: v_str(rv, "id").unwrap_or_else(new_id),
                a,
                b,
                label: rv.get("title").map(parse_title).filter(|s| !s.is_empty()),
                color: None,
                dashed: false,
                arrows: Arrows::End,
            });
        }
        sheets.push(Sheet {
            id: v_str(sv, "id").unwrap_or_else(new_id),
            title,
            root,
            relationships: rels,
            theme: None,
        });
    }
    Ok(Doc {
        core: DocCore { sheets, current: 0, theme: "classic-blue".into() },
        assets: Default::default(),
    })
}

fn endpoint_id(rv: &Value, key: &str) -> Option<String> {
    rv.get(key).and_then(|e| e.get("id")).and_then(|i| i.as_str()).map(|s| s.to_string())
}

fn parse_topic(v: &Value) -> XRResult<Topic> {
    let mut t = Topic::new(parse_title(v.get("title").unwrap_or(&Value::Null)));
    t.id = v_str(v, "id").unwrap_or_else(new_id);
    if let Some(st) = v.get("structure").and_then(parse_structure) {
        t.structure = Some(st);
    }
    // children
    if let Some(ch) = v.get("children") {
        let (attached, detached) = match ch {
            Value::Array(a) => (a.clone(), Vec::new()),
            Value::Object(o) => (
                o.get("attached").and_then(|x| x.as_array()).cloned().unwrap_or_default(),
                o.get("detached").and_then(|x| x.as_array()).cloned().unwrap_or_default(),
            ),
            _ => (Vec::new(), Vec::new()),
        };
        for cv in attached {
            t.children.push(parse_topic(&cv)?);
        }
        for cv in detached {
            t.detached.push(parse_topic(&cv)?);
        }
    }
    // markers
    if let Some(Value::Array(ms)) = v.get("markers") {
        for mv in ms {
            let id = v_str(mv, "markerId")
                .or_else(|| mv.as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            if let Some(m) = Marker::from_xmind_id(&id) {
                t.markers.push(m);
            }
        }
    }
    // labels
    if let Some(Value::Array(ls)) = v.get("labels") {
        for lv in ls {
            let s = match lv {
                Value::String(s) => s.clone(),
                Value::Object(o) => parse_title(o.get("text").unwrap_or(lv)),
                _ => continue,
            };
            if !s.is_empty() {
                t.labels.push(s);
            }
        }
    }
    // notes
    if let Some(notes) = v.get("notes") {
        let s = notes
            .get("plain")
            .and_then(|p| match p {
                Value::String(s) => Some(s.clone()),
                Value::Object(o) => o.get("content").and_then(|c| c.as_str()).map(|s| s.to_string()),
                _ => None,
            })
            .filter(|s| !s.trim().is_empty());
        t.note = s;
    }
    // hyperlink
    if let Some(l) = v.get("hyperlink").and_then(|h| h.as_str()) {
        t.link = Some(l.to_string());
    }
    // image
    if let Some(img) = v.get("image") {
        let src = v_str(img, "src").unwrap_or_default();
        let file = src.trim_start_matches("xap:").trim_start_matches("resources/").to_string();
        let id = file.split('.').next().unwrap_or("").to_string();
        if !id.is_empty() {
            t.image = Some(ImageRef {
                asset: id,
                w: img.get("width").and_then(|w| w.as_f64()).unwrap_or(150.0) as f32,
                h: img.get("height").and_then(|h| h.as_f64()).unwrap_or(100.0) as f32,
            });
        }
    }
    // style
    if let Some(props) = v.get("style").and_then(|s| s.get("properties")).and_then(|p| p.as_object()) {
        let mut st = TopicStyle::default();
        for (k, val) in props {
            let s = val.as_str().map(|s| s.to_string());
            match k.as_str() {
                "svg:fill" | "fill" => {
                    if let Some(hex) = s.as_deref().and_then(Col::from_hex) {
                        st.fill = Some(hex);
                    }
                }
                "svg:stroke" | "border-line-color" | "stroke" => {
                    if let Some(hex) = s.as_deref().and_then(Col::from_hex) {
                        st.stroke = Some(hex);
                    }
                }
                "fo:color" | "color" => {
                    if let Some(hex) = s.as_deref().and_then(Col::from_hex) {
                        st.text = Some(hex);
                    }
                }
                "svg:font-size" | "font-size" => {
                    if let Some(f) = val.as_f64() {
                        st.font_size = Some(f as f32);
                    }
                }
                "svg:font-weight" | "font-weight" => {
                    if s.as_deref() == Some("bold") {
                        st.bold = Some(true);
                    }
                }
                "svg:font-style" | "font-style" => {
                    if s.as_deref() == Some("italic") {
                        st.italic = Some(true);
                    }
                }
                "shape-class" | "shape" => {
                    if let Some(sh) = s.as_deref().and_then(shape_from_class) {
                        st.shape = Some(sh);
                    }
                }
                "border-line-width" => {
                    if let Some(f) = val.as_f64() {
                        st.stroke_w = Some(f as f32);
                    }
                }
                _ => {}
            }
        }
        if st != TopicStyle::default() {
            t.style = Some(st);
        }
    }
    // boundaries / summaries (tolerant)
    if let Some(Value::Array(bs)) = v.get("boundaries") {
        for bv in bs {
            let (start, end) = parse_range(bv);
            t.boundaries.push(Boundary {
                id: v_str(bv, "id").unwrap_or_else(new_id),
                start,
                end,
                label: bv.get("title").map(parse_title).filter(|s| !s.is_empty()),
                color: None,
            });
        }
    }
    if let Some(Value::Array(ss)) = v.get("summaries") {
        for sv in ss {
            let (start, end) = parse_range(sv);
            let mut topic = sv
                .get("topic")
                .map(parse_topic)
                .transpose()?
                .unwrap_or_else(|| Topic::new("概要"));
            if topic.text.is_empty() {
                topic.text = "概要".into();
            }
            t.summaries.push(Summary { id: v_str(sv, "id").unwrap_or_else(new_id), start, end, topic });
        }
    }
    if let Some(false) = v.get("expanded").and_then(|e| e.as_bool()) {
        t.expanded = false;
    }
    Ok(t)
}

fn parse_range(v: &Value) -> (usize, usize) {
    let n = |x: &Value| x.as_f64().map(|f| f.max(0.0) as usize);
    if let (Some(a), Some(b)) = (v.get("start").and_then(n), v.get("end").and_then(n)) {
        return (a, b.saturating_sub(1).max(a));
    }
    if let Some(s) = v.get("range").and_then(|r| r.as_str()) {
        let nums: Vec<usize> = s
            .split(|c: char| !c.is_ascii_digit())
            .filter(|x| !x.is_empty())
            .filter_map(|x| x.parse().ok())
            .collect();
        if nums.len() >= 2 {
            return (nums[0], nums[1].saturating_sub(1).max(nums[0]));
        }
        if nums.len() == 1 {
            return (nums[0], nums[0]);
        }
    }
    (0, 0)
}
