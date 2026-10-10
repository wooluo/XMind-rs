use crate::model::{Doc, Topic};
use crate::XRResult;

/// Parse tab/2-space indented text into a tree.
pub fn parse(text: &str) -> XRResult<Doc> {
    // normalize: count leading tabs or 2-space units
    let mut stack: Vec<(usize, Topic)> = Vec::new();
    let mut root = Topic::new("中心主题");
    for raw in text.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        let indent = indent_of(raw);
        let content = raw.trim().trim_start_matches(['-', '*', '•']).trim();
        if content.is_empty() {
            continue;
        }
        let topic = Topic::new(content);
        while let Some(last) = stack.last() {
            if last.0 >= indent {
                stack.pop();
            } else {
                break;
            }
        }
        match stack.last_mut() {
            Some((_, t)) => t.insert_child(None, topic.clone()),
            None => root.insert_child(None, topic.clone()),
        }
        stack.push((indent, topic));
    }
    let mut doc = Doc::new();
    doc.sheets[0].root = root;
    Ok(doc)
}

fn indent_of(line: &str) -> usize {
    let mut n = 0;
    for ch in line.chars() {
        match ch {
            '\t' => n += 4,
            ' ' => n += 1,
            _ => break,
        }
    }
    n / 2
}

pub fn render(doc: &Doc) -> String {
    doc.sheet().root.plain_text()
}

pub fn render_topic(t: &Topic) -> String {
    t.plain_text()
}
