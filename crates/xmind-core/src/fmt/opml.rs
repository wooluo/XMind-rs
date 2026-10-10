use crate::fmt::xml_util::{parse_xml, xml_escape};
use crate::model::{Doc, Topic};
use crate::XRResult;

pub fn parse(opml: &str) -> XRResult<Doc> {
    let root = parse_xml(opml)?;
    let body = root
        .child("body")
        .ok_or_else(|| crate::XRErr::Other("OPML 缺少 body".into()))?;
    let mut title = String::new();
    if let Some(head) = root.child("head") {
        if let Some(t) = head.child("title") {
            title = t.text.clone();
        }
    }
    let outlines: Vec<&crate::fmt::xml_util::XmlElem> = body.children_named("outline").collect();
    let mut doc = Doc::new();
    match outlines.split_first() {
        Some((first, rest)) => {
            let mut root_topic = parse_outline(first);
            for o in rest {
                root_topic.insert_child(None, parse_outline(o));
            }
            if root_topic.text.is_empty() {
                root_topic.text = "中心主题".into();
            }
            doc.sheets[0].root = root_topic;
        }
        None => {}
    }
    if !title.is_empty() {
        doc.sheets[0].title = title;
    }
    Ok(doc)
}

fn parse_outline(el: &crate::fmt::xml_util::XmlElem) -> Topic {
    let text = el
        .attr("text")
        .or_else(|| el.attr("title"))
        .unwrap_or("")
        .to_string();
    let mut t = Topic::new(text);
    if let Some(note) = el.attr("_note") {
        if !note.trim().is_empty() {
            t.note = Some(note.to_string());
        }
    }
    for o in el.children_named("outline") {
        t.insert_child(None, parse_outline(o));
    }
    t
}

pub fn render(doc: &Doc) -> String {
    let sheet = doc.sheet();
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<opml version=\"2.0\">\n");
    out.push_str("  <head>\n");
    out.push_str(&format!("    <title>{}</title>\n", xml_escape(&sheet.title)));
    out.push_str("  </head>\n");
    out.push_str("  <body>\n");
    render_outline(&sheet.root, 2, &mut out);
    out.push_str("  </body>\n</opml>\n");
    out
}

fn render_outline(t: &Topic, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    let note_attr = t
        .note
        .as_ref()
        .map(|n| format!(" _note=\"{}\"", xml_escape(n)))
        .unwrap_or_default();
    if t.children.is_empty() {
        out.push_str(&format!("{pad}<outline text=\"{}\"{note_attr}/>\n", xml_escape(&t.text)));
    } else {
        out.push_str(&format!("{pad}<outline text=\"{}\"{note_attr}>\n", xml_escape(&t.text)));
        for c in &t.children {
            render_outline(c, depth + 1, out);
        }
        out.push_str(&format!("{pad}</outline>\n"));
    }
}
