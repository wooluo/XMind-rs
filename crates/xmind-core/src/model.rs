use crate::col::Col;
use crate::id::new_id;
pub use crate::marker::Marker;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};

pub type Id = String;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Structure {
    LogicRight,
    LogicLeft,
    Map,
    OrgDown,
    OrgUp,
    Tree,
    Timeline,
    Fishbone,
    Matrix,
}

impl Structure {
    pub fn to_xmind_class(&self) -> &'static str {
        match self {
            Structure::LogicRight => "org.xmind.ui.logic.right",
            Structure::LogicLeft => "org.xmind.ui.logic.left",
            Structure::Map => "org.xmind.ui.map",
            Structure::OrgDown => "org.xmind.ui.org-chart.down",
            Structure::OrgUp => "org.xmind.ui.org-chart.up",
            Structure::Tree => "org.xmind.ui.tree.right",
            Structure::Timeline => "org.xmind.ui.timeline.horizontal",
            Structure::Fishbone => "org.xmind.ui.fishbone.left",
            Structure::Matrix => "org.xmind.ui.spreadsheet", // XMind calls matrix "spreadsheet"
        }
    }
    pub fn from_xmind_class(s: &str) -> Structure {
        let s = s.to_ascii_lowercase();
        if s.contains("fishbone") {
            Structure::Fishbone
        } else if s.contains("timeline") {
            Structure::Timeline
        } else if s.contains("spreadsheet") || s.contains("matrix") {
            Structure::Matrix
        } else if s.contains("org-chart") || s.contains("org_chart") {
            if s.contains("up") { Structure::OrgUp } else { Structure::OrgDown }
        } else if s.contains("logic") && s.contains("left") {
            Structure::LogicLeft
        } else if s.contains("logic") || s.contains("map") {
            if s.contains("map") { Structure::Map } else { Structure::LogicRight }
        } else if s.contains("tree") {
            Structure::Tree
        } else {
            Structure::LogicRight
        }
    }
    pub fn name_zh(&self) -> &'static str {
        match self {
            Structure::LogicRight => "思维导图",
            Structure::LogicLeft => "逻辑图（左）",
            Structure::Map => "双向导图",
            Structure::OrgDown => "组织结构图（下）",
            Structure::OrgUp => "组织结构图（上）",
            Structure::Tree => "树形图",
            Structure::Timeline => "时间线",
            Structure::Fishbone => "鱼骨图",
            Structure::Matrix => "矩阵图",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    Round,
    Rect,
    Underline,
    Capsule,
    Ellipse,
    Diamond,
}

impl Shape {
    pub fn name_zh(&self) -> &'static str {
        match self {
            Shape::Round => "圆角矩形",
            Shape::Rect => "矩形",
            Shape::Underline => "无框下划线",
            Shape::Capsule => "胶囊",
            Shape::Ellipse => "椭圆",
            Shape::Diamond => "菱形",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TopicStyle {
    pub fill: Option<Col>,
    pub text: Option<Col>,
    pub stroke: Option<Col>,
    pub stroke_w: Option<f32>,
    pub font_size: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub shape: Option<Shape>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageRef {
    pub asset: Id,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Boundary {
    pub id: Id,
    /// inclusive index range into parent's `children`
    pub start: usize,
    pub end: usize,
    pub label: Option<String>,
    pub color: Option<Col>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub id: Id,
    pub start: usize,
    pub end: usize,
    pub topic: Topic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Arrows {
    #[default]
    None,
    End,
    Both,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    pub id: Id,
    pub a: Id,
    pub b: Id,
    pub label: Option<String>,
    pub color: Option<Col>,
    #[serde(default)]
    pub dashed: bool,
    #[serde(default)]
    pub arrows: Arrows,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Topic {
    pub id: Id,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageRef>,
    #[serde(default)]
    pub children: Vec<Topic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detached: Vec<Topic>,
    /// layout applied to this topic's children
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<Structure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<Side>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<TopicStyle>,
    /// free nudge from the layout position
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<(f32, f32)>,
    /// absolute position for detached (floating) topics
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abs_pos: Option<(f32, f32)>,
    #[serde(default = "default_true")]
    pub expanded: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boundaries: Vec<Boundary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub summaries: Vec<Summary>,
}

fn default_true() -> bool {
    true
}

impl Topic {
    pub fn new(text: impl Into<String>) -> Topic {
        Topic { id: new_id(), text: text.into(), expanded: true, ..Default::default() }
    }

    pub fn with_text(text: impl Into<String>) -> Topic {
        Topic::new(text)
    }

    pub fn child_mut(&mut self, id: &str) -> Option<&mut Topic> {
        // 1) direct children / detached
        let pos = self
            .children
            .iter()
            .chain(self.detached.iter())
            .position(|c| c.id == id);
        if let Some(i) = pos {
            return if i < self.children.len() {
                self.children.get_mut(i)
            } else {
                self.detached.get_mut(i - self.children.len())
            };
        }
        // 2) recurse into subtrees
        for c in self.children.iter_mut() {
            if let Some(t) = c.child_mut(id) {
                return Some(t);
            }
        }
        for c in self.detached.iter_mut() {
            if let Some(t) = c.child_mut(id) {
                return Some(t);
            }
        }
        // 3) summary topics
        for s in self.summaries.iter_mut() {
            if s.topic.id == id {
                return Some(&mut s.topic);
            }
            if let Some(t) = s.topic.child_mut(id) {
                return Some(t);
            }
        }
        None
    }

    pub fn find(&self, id: &str) -> Option<&Topic> {
        for c in self.children.iter().chain(self.detached.iter()) {
            if c.id == id {
                return Some(c);
            }
            if let Some(t) = c.find(id) {
                return Some(t);
            }
            for s in &c.summaries {
                if s.topic.id == id {
                    return Some(&s.topic);
                }
                if let Some(t) = s.topic.find(id) {
                    return Some(t);
                }
            }
        }
        None
    }

    /// (parent, index in parent.children) — also searches summaries' subtrees.
    pub fn parent_of(&self, id: &str) -> Option<(&Topic, usize)> {
        for (i, c) in self.children.iter().enumerate() {
            if c.id == id {
                return Some((self, i));
            }
            if let Some(p) = c.parent_of(id) {
                return Some(p);
            }
        }
        for (i, c) in self.detached.iter().enumerate() {
            if c.id == id {
                return Some((self, 10_000 + i)); // detached marker offset
            }
            if let Some(p) = c.parent_of(id) {
                return Some(p);
            }
        }
        for c in self.children.iter().chain(self.detached.iter()) {
            for s in &c.summaries {
                if s.topic.id == id {
                    return Some((&s.topic, usize::MAX)); // summary root: parent is special
                }
                if let Some(p) = s.topic.parent_of(id) {
                    return Some(p);
                }
            }
        }
        None
    }

    pub fn is_descendant_of(&self, id: &str, maybe_ancestor: &str) -> bool {
        if id == maybe_ancestor {
            return true;
        }
        match self.find(maybe_ancestor) {
            Some(a) => a.find(id).is_some(),
            None => false,
        }
    }

    /// Depth-first walk over children/detached/summary topics (not self).
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Topic)) {
        for c in self.children.iter().chain(self.detached.iter()) {
            f(c);
            c.walk(f);
            for s in &c.summaries {
                f(&s.topic);
                s.topic.walk(f);
            }
        }
    }

    pub fn walk_mut(&mut self, f: &mut dyn FnMut(&mut Topic)) {
        for c in self.children.iter_mut().chain(self.detached.iter_mut()) {
            f(c);
            c.walk_mut(f);
            for s in c.summaries.iter_mut() {
                f(&mut s.topic);
                s.topic.walk_mut(f);
            }
        }
    }

    /// All topic ids in subtree (including self).
    pub fn subtree_ids(&self) -> Vec<Id> {
        let mut v = vec![self.id.clone()];
        self.walk(&mut |t| v.push(t.id.clone()));
        v
    }

    pub fn topic_count(&self) -> usize {
        let mut n = 1;
        self.walk(&mut |_| n += 1);
        n
    }

    /// Detach (cut out) a topic by id; returns it. Rejects self and summary roots.
    /// Fixes boundary/summary ranges on the direct parent.
    pub fn detach_subtree(&mut self, id: &str) -> Option<Topic> {
        if self.id == id {
            return None;
        }
        for i in 0..self.children.len() {
            if self.children[i].id == id {
                let t = self.children.remove(i);
                self.fix_ranges_after_remove(i);
                return Some(t);
            }
        }
        for i in 0..self.detached.len() {
            if self.detached[i].id == id {
                return Some(self.detached.remove(i));
            }
        }
        for c in self.children.iter_mut().chain(self.detached.iter_mut()) {
            if let Some(t) = c.detach_subtree(id) {
                return Some(t);
            }
            for s in c.summaries.iter_mut() {
                if s.topic.id == id {
                    return None; // summary roots are managed via summary ranges
                }
                if let Some(t) = s.topic.detach_subtree(id) {
                    return Some(t);
                }
            }
        }
        None
    }

    pub fn insert_child(&mut self, at: Option<usize>, topic: Topic) {
        match at {
            Some(i) => self.children.insert(i.min(self.children.len()), topic),
            None => self.children.push(topic),
        };
        // fix boundary/summary ranges
        let idx = match at {
            Some(i) => i.min(self.children.len() - 1),
            None => self.children.len() - 1,
        };
        for b in self.boundaries.iter_mut() {
            if b.start > idx {
                b.start += 1;
            }
            if b.end >= idx {
                b.end += 1;
            }
        }
        for s in self.summaries.iter_mut() {
            if s.start > idx {
                s.start += 1;
            }
            if s.end >= idx {
                s.end += 1;
            }
        }
    }

    /// Remove direct child by id (children + detached), fixing ranges. Returns it.
    pub fn remove_child(&mut self, id: &str) -> Option<Topic> {
        if let Some(pos) = self.children.iter().position(|c| c.id == id) {
            let t = self.children.remove(pos);
            self.fix_ranges_after_remove(pos);
            return Some(t);
        }
        if let Some(pos) = self.detached.iter().position(|c| c.id == id) {
            return Some(self.detached.remove(pos));
        }
        None
    }

    fn fix_ranges_after_remove(&mut self, pos: usize) {
        self.boundaries.retain_mut(|b| {
            if b.end < pos {
                return true;
            }
            if b.start > b.end || (b.start == pos && b.end == pos) {
                return false; // emptied
            }
            if pos < b.start {
                b.start -= 1;
            }
            if pos <= b.end {
                b.end -= 1;
            }
            true
        });
        self.summaries.retain_mut(|s| {
            if s.end < pos {
                return true;
            }
            if s.start > s.end || (s.start == pos && s.end == pos) {
                return false;
            }
            if pos < s.start {
                s.start -= 1;
            }
            if pos <= s.end {
                s.end -= 1;
            }
            true
        });
    }

    /// Effective structure for this topic's children.
    pub fn eff_structure(&self, default: Structure) -> Structure {
        self.structure.unwrap_or(default)
    }

    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        self.write_outline(&mut s, 0);
        s
    }

    fn write_outline(&self, out: &mut String, depth: usize) {
        for _ in 0..depth {
            out.push('\t');
        }
        out.push_str(self.text.trim());
        out.push('\n');
        for c in self.children.iter().chain(self.detached.iter()) {
            c.write_outline(out, depth + 1);
        }
        for s in &self.summaries {
            s.topic.write_outline(out, depth + 1);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub id: Id,
    pub title: String,
    pub root: Topic,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    #[serde(default)]
    pub theme: Option<String>,
}

impl Sheet {
    pub fn new(title: impl Into<String>, root_text: impl Into<String>) -> Sheet {
        Sheet {
            id: new_id(),
            title: title.into(),
            root: Topic::new(root_text),
            relationships: Vec::new(),
            theme: None,
        }
    }

    pub fn find(&self, id: &str) -> Option<&Topic> {
        if self.root.id == id {
            Some(&self.root)
        } else {
            self.root.find(id)
        }
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut Topic> {
        if self.root.id == id {
            Some(&mut self.root)
        } else {
            self.root.child_mut(id)
        }
    }

    pub fn parent_of(&self, id: &str) -> Option<(&Topic, usize)> {
        self.root.parent_of(id)
    }

    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Topic)) {
        f(&self.root);
        self.root.walk(f);
    }

    pub fn topic_count(&self) -> usize {
        self.root.topic_count()
    }

    /// Delete a whole summary (bracket + summary topic + its subtree) by the
    /// summary topic's id. Returns true if one was removed.
    pub fn delete_summary_by_topic(&mut self, tid: &str) -> bool {
        fn walk(t: &mut Topic, tid: &str) -> bool {
            let before = t.summaries.len();
            t.summaries.retain(|s| s.topic.id != tid);
            if t.summaries.len() != before {
                return true;
            }
            for c in t.children.iter_mut().chain(t.detached.iter_mut()) {
                if walk(c, tid) {
                    return true;
                }
            }
            false
        }
        walk(&mut self.root, tid)
    }

    /// Delete a topic (not root). Fixes relationships pointing into it.
    pub fn delete_topic(&mut self, id: &str) -> Option<Topic> {
        if self.root.id == id {
            return None;
        }
        let t = self.root.detach_subtree(id)?;
        let ids: std::collections::HashSet<Id> = t.subtree_ids().into_iter().collect();
        self.relationships
            .retain(|r| !ids.contains(&r.a) && !ids.contains(&r.b));
        Some(t)
    }
}

/// Binary image asset stored inside the document.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub kind: String, // "png" | "jpeg"
    pub data: Vec<u8>,
}

/// Serializable core of a document (everything except binary assets).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocCore {
    pub sheets: Vec<Sheet>,
    pub current: usize,
    pub theme: String,
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub core: DocCore,
    pub assets: BTreeMap<Id, Asset>,
}

impl Deref for Doc {
    type Target = DocCore;
    fn deref(&self) -> &DocCore {
        &self.core
    }
}

impl DerefMut for Doc {
    fn deref_mut(&mut self) -> &mut DocCore {
        &mut self.core
    }
}

impl Doc {
    pub fn new() -> Doc {
        Doc {
            core: DocCore {
                sheets: vec![Sheet::new("画布 1", "中心主题")],
                current: 0,
                theme: "classic-blue".into(),
            },
            assets: BTreeMap::new(),
        }
    }

    pub fn sheet(&self) -> &Sheet {
        &self.sheets[self.current.min(self.sheets.len() - 1)]
    }

    pub fn sheet_mut(&mut self) -> &mut Sheet {
        let i = self.current.min(self.sheets.len() - 1);
        &mut self.sheets[i]
    }

    pub fn topic(&self, id: &str) -> Option<&Topic> {
        self.sheet().find(id)
    }

    pub fn topic_mut(&mut self, id: &str) -> Option<&mut Topic> {
        self.sheet_mut().find_mut(id)
    }

    pub fn add_sheet(&mut self) {
        let n = self.sheets.len() + 1;
        self.sheets.push(Sheet::new(format!("画布 {n}"), "中心主题"));
        self.current = self.sheets.len() - 1;
    }

    /// Snapshot for undo/redo (excludes assets — they only grow).
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::to_value(&self.core).unwrap_or(serde_json::Value::Null)
    }

    pub fn restore(&mut self, v: &serde_json::Value) {
        if let Ok(core) = serde_json::from_value::<DocCore>(v.clone()) {
            self.core = core;
        }
    }
}

impl Default for DocCore {
    fn default() -> Self {
        DocCore { sheets: Vec::new(), current: 0, theme: "classic-blue".into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Doc {
        let mut d = Doc::new();
        let root = d.sheet_mut().root.id.clone();
        d.topic_mut(&root).unwrap().insert_child(None, Topic::new("A"));
        d.topic_mut(&root).unwrap().insert_child(None, Topic::new("B"));
        d
    }

    #[test]
    fn tree_ops() {
        let mut d = doc();
        let root = d.sheet().root.id.clone();
        let a = d.topic(&root).unwrap().children[0].id.clone();
        d.topic_mut(&a).unwrap().insert_child(None, Topic::new("A1"));
        assert_eq!(d.topic(&root).unwrap().children.len(), 2);
        assert!(d.topic(&a).unwrap().children[0].text == "A1");
        let cut = d.sheet_mut().root.detach_subtree(&a).unwrap();
        assert_eq!(cut.children.len(), 1);
        assert_eq!(d.topic(&root).unwrap().children.len(), 1);
    }

    #[test]
    fn snapshot_restore() {
        let mut d = doc();
        let snap = d.snapshot();
        let root = d.sheet().root.id.clone();
        d.topic_mut(&root).unwrap().insert_child(None, Topic::new("C"));
        assert_eq!(d.topic(&root).unwrap().children.len(), 3);
        d.restore(&snap);
        assert_eq!(d.topic(&root).unwrap().children.len(), 2);
    }
}
