use crate::XRResult;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

/// Minimal DOM for tolerant XML parsing.
#[derive(Debug, Clone)]
pub struct XmlElem {
    pub name: String,
    pub attrs: HashMap<String, String>,
    pub text: String,
    pub children: Vec<XmlElem>,
}

impl XmlElem {
    pub fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.get(k).map(|s| s.as_str())
    }
    pub fn child(&self, name: &str) -> Option<&XmlElem> {
        self.children.iter().find(|c| c.name == name)
    }
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a XmlElem> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
    /// All descendant elements with the given name (DFS).
    pub fn all<'a>(&'a self, name: &'a str, out: &mut Vec<&'a XmlElem>) {
        for c in &self.children {
            if c.name == name {
                out.push(c);
            }
            c.all(name, out);
        }
    }
    /// Concatenated text of all descendants (for html-ish richcontent).
    pub fn deep_text(&self) -> String {
        let mut s = self.text.clone();
        for c in &self.children {
            s.push_str(&c.deep_text());
        }
        s
    }
}

/// Parse XML into a DOM tree (returns the document root element).
pub fn parse_xml(data: &str) -> XRResult<XmlElem> {
    let mut reader = Reader::from_str(data);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<XmlElem> = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = HashMap::new();
                for a in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                    let val = a
                        .unescape_value()
                        .map(|v| v.to_string())
                        .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).to_string());
                    attrs.insert(key, val);
                }
                stack.push(XmlElem { name, attrs, text: String::new(), children: Vec::new() });
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = HashMap::new();
                for a in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                    let val = a
                        .unescape_value()
                        .map(|v| v.to_string())
                        .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).to_string());
                    attrs.insert(key, val);
                }
                let el = XmlElem { name, attrs, text: String::new(), children: Vec::new() };
                if let Some(p) = stack.last_mut() {
                    p.children.push(el);
                } else {
                    // empty document root — should not happen
                    stack.push(el);
                }
            }
            Ok(Event::Text(t)) => {
                if let Some(p) = stack.last_mut() {
                    let s = t
                        .unescape()
                        .map_err(|e| crate::XRErr::Xml(e.to_string()))?;
                    p.text.push_str(&s);
                }
            }
            Ok(Event::CData(t)) => {
                if let Some(p) = stack.last_mut() {
                    p.text.push_str(&String::from_utf8_lossy(t.as_ref()));
                }
            }
            Ok(Event::End(_)) => {
                let el = stack.pop();
                match (el, stack.last_mut()) {
                    (Some(el), Some(p)) => p.children.push(el),
                    (Some(el), None) => return Ok(el),
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(crate::XRErr::Xml(e.to_string())),
        }
        buf.clear();
    }
    stack
        .pop()
        .ok_or_else(|| crate::XRErr::Xml("empty document".into()))
}

pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}
