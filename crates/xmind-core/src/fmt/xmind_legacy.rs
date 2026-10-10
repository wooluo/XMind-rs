//! Reader for legacy XMind 8 (content.xml) files. Best effort: text tree,
//! markers, labels, notes, structure class, detached topics.

use crate::id::new_id;
use crate::marker::Marker;
use crate::model::*;
use crate::XRResult;
use std::collections::HashMap;

use super::xml_util::{parse_xml, XmlElem};

pub fn parse(content: &str) -> XRResult<Doc> {
    let root = parse_xml(content)?;
    // root is <xmap-content> (or <xmap-content> wrapped); find sheets anywhere
    let mut sheets_el = Vec::new();
    root.all("sheet", &mut sheets_el);
    if sheets_el.is_empty() {
        // maybe root itself is a sheet
        let mut doc = Doc::new();
        if let Some(t) = root.child("topic") {
            doc.sheets[0].root = parse_topic(t)?;
        }
        return Ok(doc);
    }
    let mut sheets = Vec::new();
    for sh in sheets_el {
        let title = sh
            .child("title")
            .map(|t| t.text.clone())
            .or_else(|| sh.attr("title").map(|s| s.to_string()))
            .unwrap_or_else(|| "画布".into());
        let root_topic = sh.child("topic").ok_or_else(|| crate::XRErr::Other("sheet 缺少根主题".into()))?;
        let mut root = parse_topic(root_topic)?;
        if let Some(sc) = sh.attr("structure-class") {
            root.structure = Some(Structure::from_xmind_class(sc));
        }
        // relationships: <relationships><relationship ... endpoint1 id end1id end2id>
        let mut rels = Vec::new();
        let mut rel_els = Vec::new();
        sh.all("relationship", &mut rel_els);
        for r in rel_els {
            let a = r.attr("end1id").map(|s| s.to_string());
            let b = r.attr("end2id").map(|s| s.to_string());
            if let (Some(a), Some(b)) = (a, b) {
                rels.push(Relationship {
                    id: r.attr("id").map(|s| s.to_string()).unwrap_or_else(new_id),
                    a,
                    b,
                    label: None,
                    color: None,
                    dashed: false,
                    arrows: Arrows::None,
                });
            }
        }
        sheets.push(Sheet {
            id: sh.attr("id").map(|s| s.to_string()).unwrap_or_else(new_id),
            title,
            root,
            relationships: rels,
            theme: None,
        });
    }
    Ok(Doc { core: DocCore { sheets, current: 0, theme: "classic-blue".into() }, assets: Default::default() })
}

fn parse_topic(el: &XmlElem) -> XRResult<Topic> {
    let mut t = Topic::new(el.child("title").map(|x| x.text.clone()).unwrap_or_default());
    t.id = el.attr("id").map(|s| s.to_string()).unwrap_or_else(new_id);
    if let Some(sc) = el.attr("structure-class") {
        t.structure = Some(Structure::from_xmind_class(sc));
    }
    if let Some(children) = el.child("children") {
        for topics in children.children_named("topics") {
            let typ = topics.attr("type").unwrap_or("attached");
            for c in topics.children_named("topic") {
                let topic = parse_topic(c)?;
                if typ == "detached" {
                    t.detached.push(topic);
                } else {
                    t.children.push(topic);
                }
            }
        }
    }
    // marker refs
    if let Some(refs) = el.child("marker-refs") {
        for m in refs.children_named("marker-ref") {
            if let Some(id) = m.attr("marker-id") {
                if let Some(mk) = Marker::from_xmind_id(id) {
                    t.markers.push(mk);
                }
            }
        }
    }
    // labels
    if let Some(labels) = el.child("labels") {
        for l in labels.children_named("label") {
            let s = if l.text.is_empty() { l.attr("name").unwrap_or("").to_string() } else { l.text.clone() };
            if !s.is_empty() {
                t.labels.push(s);
            }
        }
    }
    // notes
    if let Some(notes) = el.child("notes") {
        if let Some(plain) = notes.child("plain") {
            if !plain.text.trim().is_empty() {
                t.note = Some(plain.text.clone());
            }
        }
    }
    // boundaries
    for b in el.children_named("boundary") {
        let (start, end) = parse_range_attr(b.attr("range"));
        t.boundaries.push(Boundary {
            id: b.attr("id").map(|s| s.to_string()).unwrap_or_else(new_id),
            start,
            end,
            label: b.child("title").map(|x| x.text.clone()).filter(|s| !s.is_empty()),
            color: None,
        });
    }
    // summaries
    for s in el.children_named("summary") {
        let (start, end) = parse_range_attr(s.attr("range"));
        let topic = s
            .child("topic")
            .map(parse_topic)
            .transpose()?
            .unwrap_or_else(|| Topic::new("概要"));
        t.summaries.push(Summary { id: s.attr("id").map(|x| x.to_string()).unwrap_or_else(new_id), start, end, topic });
    }
    Ok(t)
}

/// Legacy range format: "[0,2)" style or "0/2".
fn parse_range_attr(s: Option<&str>) -> (usize, usize) {
    let Some(s) = s else { return (0, 0) };
    let nums: Vec<usize> = s
        .split(|c: char| !c.is_ascii_digit())
        .filter(|x| !x.is_empty())
        .filter_map(|x| x.parse().ok())
        .collect();
    match nums.len() {
        0 => (0, 0),
        1 => (nums[0], nums[0]),
        _ => (nums[0], nums[1].saturating_sub(1).max(nums[0])),
    }
}

/// Keep track of seen element ids to remap potential duplicates.
pub type IdMap = HashMap<String, String>;
