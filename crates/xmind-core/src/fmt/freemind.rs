use crate::fmt::xml_util::{parse_xml, xml_escape};
use crate::model::{Doc, Topic};
use crate::XRResult;

pub fn parse(mm: &str) -> XRResult<Doc> {
    let root = parse_xml(mm)?;
    let node = root
        .child("node")
        .ok_or_else(|| crate::XRErr::Other("FreeMind 缺少根 node".into()))?;
    let mut root_topic = parse_node(node);
    if root_topic.text.is_empty() {
        root_topic.text = "中心主题".into();
    }
    let mut doc = Doc::new();
    doc.sheets[0].root = root_topic;
    Ok(doc)
}

fn parse_node(el: &crate::fmt::xml_util::XmlElem) -> Topic {
    let text = el.attr("TEXT").unwrap_or("").to_string();
    let mut t = Topic::new(text);
    if let Some(f) = el.attr("FOLDED") {
        if f.eq_ignore_ascii_case("true") {
            t.expanded = false;
        }
    }
    // richcontent notes
    let mut rc_els = Vec::new();
    el.all("richcontent", &mut rc_els);
    for rc in rc_els {
        if rc.attr("TYPE") == Some("NOTE") {
            let mut s = rc.deep_text();
            s = s.replace("\n\n", "\n");
            let s = s.trim().to_string();
            if !s.is_empty() {
                t.note = Some(s);
            }
        }
    }
    for c in el.children_named("node") {
        t.insert_child(None, parse_node(c));
    }
    t
}

pub fn render(doc: &Doc) -> String {
    let sheet = doc.sheet();
    let mut out = String::new();
    out.push_str("<map version=\"1.0.1\">\n");
    render_node(&sheet.root, 1, &mut out);
    out.push_str("</map>\n");
    out
}

fn render_node(t: &Topic, depth: usize, out: &mut String) {
    let pad = "\t".repeat(depth);
    let folded = if t.expanded { "" } else { " FOLDED=\"true\"" };
    let note = t.note.as_ref().map(|n| {
        let esc = xml_escape(n);
        format!(
            "\n{pad}\t<richcontent TYPE=\"NOTE\">\n{pad}\t\t<html>\n{pad}\t\t\t<head/>\n{pad}\t\t\t<body>\n{pad}\t\t\t\t<p>{esc}</p>\n{pad}\t\t\t</body>\n{pad}\t\t</html>\n{pad}\t</richcontent>"
        )
    });
    if t.children.is_empty() {
        out.push_str(&format!(
            "{pad}<node TEXT=\"{}\"{folded}>{}</node>\n",
            xml_escape(&t.text),
            note.unwrap_or_default()
        ));
    } else {
        out.push_str(&format!("{pad}<node TEXT=\"{}\"{folded}>{}\n", xml_escape(&t.text), note.unwrap_or_default()));
        for c in &t.children {
            render_node(c, depth + 1, out);
        }
        out.push_str(&format!("{pad}</node>\n"));
    }
}
