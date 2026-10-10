use crate::model::{Doc, Topic};

/// Parse Markdown (headings + nested bullet lists) into a document.
pub fn parse(md: &str) -> Doc {
    let mut roots_extra: Vec<Topic> = Vec::new();
    let mut stack: Vec<Topic> = Vec::new(); // heading chain, deepest last
    let mut default_root = Topic::new("中心主题"); // used when no headings exist

    fn attach(stack: &mut Vec<Topic>, roots_extra: &mut Vec<Topic>, t: Topic) {
        match stack.last_mut() {
            Some(p) => p.insert_child(None, t),
            None => roots_extra.push(t),
        }
    }

    for raw in md.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if hashes > 0 && trimmed[hashes..].starts_with(' ') {
            let text = trimmed[hashes + 1..].trim().to_string();
            let level = hashes.min(6);
            // deeper headings are completed — attach them to their parents
            while stack.len() >= level {
                let done = stack.pop().unwrap();
                attach(&mut stack, &mut roots_extra, done);
            }
            stack.push(Topic::new(text));
            continue;
        }
        let after_bullet = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
            .map(|s| s.to_string());
        let text = match after_bullet {
            Some(t) => strip_md(&t),
            None => strip_md(trimmed),
        };
        if text.is_empty() {
            continue;
        }
        let topic = Topic::new(text);
        if let Some(t) = stack.last_mut() {
            t.insert_child(None, topic);
        } else {
            default_root.insert_child(None, topic);
        }
    }
    while let Some(done) = stack.pop() {
        attach(&mut stack, &mut roots_extra, done);
    }
    let mut doc = Doc::new();
    if roots_extra.is_empty() {
        doc.sheets[0].root = default_root;
    } else {
        let mut first = roots_extra.remove(0);
        for extra in roots_extra {
            first.insert_child(None, extra);
        }
        for c in default_root.children {
            first.insert_child(None, c);
        }
        doc.sheets[0].root = first;
    }
    doc
}

fn strip_md(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix("**").unwrap_or(s);
    let s = s.strip_suffix("**").unwrap_or(s);
    let s = s.strip_prefix("__").unwrap_or(s);
    let s = s.strip_suffix("__").unwrap_or(s);
    let s = s.strip_prefix("`").unwrap_or(s);
    let s = s.strip_suffix("`").unwrap_or(s);
    let s = s.strip_prefix("[ ]").map(|x| format!("☐ {x}")).unwrap_or_else(|| s.to_string());
    let s = s.strip_prefix("[x]").map(|x| format!("☑ {x}")).unwrap_or(s);
    s.trim().to_string()
}

/// Render a document as Markdown (headings for the first levels, bullets deeper).
pub fn render(doc: &Doc) -> String {
    let sheet = doc.sheet();
    let mut out = String::new();
    out.push_str(&format!("# {}\n", sheet.root.text));
    if let Some(n) = sheet.root.note.as_ref() {
        if !n.trim().is_empty() {
            out.push_str(&format!("> 备注：{n}\n"));
        }
    }
    out.push('\n');
    for c in &sheet.root.children {
        render_topic(c, 2, &mut out);
    }
    out
}

fn render_topic(t: &Topic, level: usize, out: &mut String) {
    if level <= 6 {
        out.push_str(&format!("{} {}\n", "#".repeat(level), t.text));
    } else {
        out.push_str(&format!("{}- {}\n", "  ".repeat(level - 6), t.text));
    }
    if let Some(n) = &t.note {
        if !n.trim().is_empty() {
            for line in n.lines() {
                out.push_str(&format!("> {line}\n"));
            }
        }
    }
    for c in &t.children {
        render_topic(c, level + 1, out);
    }
    if level <= 6 {
        out.push('\n');
    }
}
