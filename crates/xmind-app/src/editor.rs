//! Editor state: document + selection + undo/redo + view transform, and every
//! document mutation used by the GUI.

use std::collections::HashMap;
use std::path::PathBuf;
use xmind_core::col::Col;
use xmind_core::geo::Rect;
use xmind_core::id::new_id;
use xmind_core::marker::{ColKey, Face, Marker, Sym};
use xmind_core::model::*;
use xmind_core::scene::Scene;
use xmind_core::XRResult;

pub const UNDO_LIMIT: usize = 120;

#[derive(Clone, Copy, Debug, Default)]
pub struct View {
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
}

impl View {
    pub fn new() -> Self {
        View { pan_x: 60.0, pan_y: 60.0, zoom: 1.0 }
    }
    pub fn to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        (x * self.zoom + self.pan_x, y * self.zoom + self.pan_y)
    }
    pub fn to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        ((sx - self.pan_x) / self.zoom, (sy - self.pan_y) / self.zoom)
    }
    pub fn zoom_at(&mut self, anchor: (f32, f32), factor: f32) {
        let z0 = self.zoom;
        self.zoom = (self.zoom * factor).clamp(0.1, 5.0);
        let k = self.zoom / z0;
        self.pan_x = anchor.0 - (anchor.0 - self.pan_x) * k;
        self.pan_y = anchor.1 - (anchor.1 - self.pan_y) * k;
    }
    pub fn center_on(&mut self, canvas_size: (f32, f32), r: Rect, target_zoom: Option<f32>) {
        if let Some(z) = target_zoom {
            self.zoom = z;
        }
        let (sx, sy) = self.to_screen(r.cx(), r.cy());
        self.pan_x += canvas_size.0 / 2.0 - sx;
        self.pan_y += canvas_size.1 / 2.0 - sy;
    }
}

#[derive(Clone, Debug)]
pub struct FindState {
    pub query: String,
    pub replace: String,
    pub matches: Vec<String>, // topic ids
    pub idx: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DragMode {
    None,
    Pan,
    MoveTopics,
    RectSelect,
}

pub struct Editor {
    pub doc: Doc,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub undo: Vec<serde_json::Value>,
    pub redo: Vec<serde_json::Value>,
    pub sel: Vec<String>,
    pub sel_boundary: Option<(String, String)>, // (parent topic id, boundary id)
    pub sel_rel: Option<String>,
    pub editing: Option<(String, String)>, // (topic id, live buffer)
    pub view: View,
    pub clipboard_topic: Option<Topic>,
    pub find: Option<FindState>,
    pub pending_rel: Option<String>,
    pub drill: Vec<String>,
    pub presentation: Option<(Vec<String>, usize)>,
    pub drag_mode: DragMode,
    pub drag_origin: (f32, f32),
    pub drag_last: (f32, f32),
    pub drag_moved: bool,
    pub scene: Option<Scene>,
    pub scene_key: (u64, usize, String, String, String), // (ver, sheet, drill, edit buf, theme)
    pub scene_ver: u64,
    pub scene_edit_buf: String,
    pub tex_cache: HashMap<String, egui::TextureHandle>,
    pub toast: Option<(String, f64)>,
    /// 切主题/画布等大变更后，下一帧自动“适应画布”重新取景
    pub pending_fit: bool,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            doc: Doc::new(),
            path: None,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            sel: Vec::new(),
            sel_boundary: None,
            sel_rel: None,
            editing: None,
            view: View::new(),
            clipboard_topic: None,
            find: None,
            pending_rel: None,
            drill: Vec::new(),
            presentation: None,
            drag_mode: DragMode::None,
            drag_origin: (0.0, 0.0),
            drag_last: (0.0, 0.0),
            drag_moved: false,
            scene: None,
            scene_key: (0, 0, String::new(), String::new(), String::new()),
            scene_ver: 1,
            scene_edit_buf: String::new(),
            tex_cache: HashMap::new(),
            toast: None,
            pending_fit: false,
        }
    }

    // ---------- undo ----------

    pub fn commit(&mut self) {
        let snap = self.doc.snapshot();
        self.undo.push(snap);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
        self.scene_ver += 1;
    }

    pub fn undo(&mut self) {
        if let Some(snap) = self.undo.pop() {
            let cur = self.doc.snapshot();
            self.redo.push(cur);
            self.doc.restore(&snap);
            self.after_history_jump();
        }
    }

    pub fn redo(&mut self) {
        if let Some(snap) = self.redo.pop() {
            let cur = self.doc.snapshot();
            self.undo.push(cur);
            self.doc.restore(&snap);
            self.after_history_jump();
        }
    }

    fn after_history_jump(&mut self) {
        self.dirty = true;
        self.scene_ver += 1;
        self.editing = None;
        let live: std::collections::HashSet<String> = self.all_visible_ids();
        self.sel.retain(|id| live.contains(id));
        self.sel_boundary = None;
        self.sel_rel = None;
        self.pending_rel = None;
    }

    fn all_visible_ids(&self) -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        for sh in &self.doc.sheets {
            sh.walk(&mut |t| {
                set.insert(t.id.clone());
            });
        }
        set
    }

    // ---------- selection helpers ----------

    pub fn primary(&self) -> Option<&String> {
        self.sel.first()
    }

    pub fn select_topic(&mut self, id: &str, extend: bool) {
        self.sel_boundary = None;
        self.sel_rel = None;
        if extend {
            if let Some(p) = self.sel.iter().position(|x| x == id) {
                self.sel.remove(p);
            } else {
                self.sel.push(id.to_string());
            }
        } else {
            self.sel = vec![id.to_string()];
        }
    }

    pub fn topic(&self, id: &str) -> Option<&Topic> {
        self.doc.sheet().find(id)
    }

    pub fn topic_mut(&mut self, id: &str) -> Option<&mut Topic> {
        self.doc.sheet_mut().find_mut(id)
    }

    /// (parent id, index) of a topic in the current sheet. Detached children
    /// report index >= 10000. Summary roots return None.
    pub fn parent_info(&self, id: &str) -> Option<(String, usize)> {
        let sheet = self.doc.sheet();
        if sheet.root.id == id {
            return None;
        }
        // search children+detached recursively
        fn walk(t: &Topic, id: &str, path: &mut Vec<String>) -> Option<(String, usize)> {
            for (i, c) in t.children.iter().enumerate() {
                if c.id == id {
                    path.push(t.id.clone());
                    return Some((t.id.clone(), i));
                }
                if let Some(r) = walk(c, id, path) {
                    path.push(t.id.clone());
                    return Some(r);
                }
            }
            for (i, c) in t.detached.iter().enumerate() {
                if c.id == id {
                    return Some((t.id.clone(), 10_000 + i));
                }
                if let Some(r) = walk(c, id, path) {
                    return Some(r);
                }
            }
            for s in &t.summaries {
                if s.topic.id == id {
                    return None; // summary root has no normal parent
                }
                if let Some(r) = walk(&s.topic, id, path) {
                    return Some(r);
                }
            }
            None
        }
        walk(&sheet.root, id, &mut Vec::new())
    }

    // ---------- structural edits ----------

    pub fn add_child(&mut self, parent_id: &str, text: &str) -> Option<String> {
        self.commit();
        let id = new_id();
        self.doc.sheet_mut().find_mut(parent_id).map(|p| {
            p.insert_child(None, Topic { id: id.clone(), text: text.into(), ..Default::default() });
        });
        Some(id)
    }

    pub fn add_sibling(&mut self, id: &str, text: &str) -> Option<String> {
        let (pid, idx) = self.parent_info(id)?;
        if idx >= 10_000 {
            self.commit();
            let nid = new_id();
            self.doc.sheet_mut().find_mut(&pid).map(|p| {
                p.detached.push(Topic { id: nid.clone(), text: text.into(), ..Default::default() });
            });
            return Some(nid);
        }
        self.commit();
        let nid = new_id();
        self.doc.sheet_mut().find_mut(&pid).map(|p| {
            p.insert_child(Some(idx + 1), Topic { id: nid.clone(), text: text.into(), ..Default::default() });
        });
        Some(nid)
    }

    pub fn add_floating(&mut self, abs: (f32, f32)) -> Option<String> {
        self.commit();
        let id = new_id();
        let mut t = Topic { id: id.clone(), text: "自由主题".into(), ..Default::default() };
        t.abs_pos = Some(abs);
        self.doc.sheet_mut().root.detached.push(t);
        Some(id)
    }

    pub fn insert_parent_for(&mut self, id: &str, text: &str) -> Option<String> {
        let (pid, idx) = self.parent_info(id)?;
        if idx >= 10_000 {
            return None;
        }
        self.commit();
        let mut wrapped = Topic::new(text);
        let child = self.doc.sheet_mut().find_mut(&pid)?.remove_child(id)?;
        wrapped.id = new_id();
        let new_id = wrapped.id.clone();
        wrapped.insert_child(None, child);
        self.doc.sheet_mut().find_mut(&pid)?.insert_child(Some(idx), wrapped);
        Some(new_id)
    }

    pub fn delete_selected(&mut self) {
        if !self.sel.is_empty() {
            self.commit();
            let ids: Vec<String> = self.sel.clone();
            for id in ids {
                self.doc.sheet_mut().delete_topic(&id);
            }
            self.sel.clear();
        } else if let Some(rid) = self.sel_rel.take() {
            self.commit();
            self.doc.sheet_mut().relationships.retain(|r| r.id != rid);
        } else if let Some((pid, bid)) = self.sel_boundary.take() {
            self.commit();
            if let Some(p) = self.doc.sheet_mut().find_mut(&pid) {
                p.boundaries.retain(|b| b.id != bid);
            }
        }
    }

    pub fn move_subtree(&mut self, id: &str, new_parent: &str, index: Option<usize>) -> bool {
        if id == new_parent || self.doc.sheet().root.id == id {
            return false;
        }
        // reject cycles: new_parent must not be inside subtree of id
        if let Some(t) = self.doc.sheet().find(id) {
            if t.find(new_parent).is_some() {
                return false;
            }
        }
        self.commit();
        let Some(mut subtree) = self.doc.sheet_mut().root.detach_subtree(id) else { return false };
        subtree.offset = None;
        let sheet = self.doc.sheet_mut();
        if let Some(p) = sheet.find_mut(new_parent) {
            match index {
                Some(i) => p.insert_child(Some(i.min(p.children.len())), subtree),
                None => p.children.push(subtree),
            }
            true
        } else {
            false
        }
    }

    pub fn toggle_expand(&mut self, id: &str) {
        self.commit();
        if let Some(t) = self.doc.sheet_mut().find_mut(id) {
            t.expanded = !t.expanded;
        }
    }

    pub fn set_expand_all(&mut self, expanded: bool) {
        self.commit();
        self.doc.sheet_mut().root.walk_mut(&mut |t: &mut Topic| t.expanded = expanded);
        self.doc.sheet_mut().root.expanded = true;
    }

    // ---------- markers / style / meta ----------

    pub fn toggle_marker(&mut self, id: &str, m: Marker) {
        self.commit();
        if let Some(t) = self.doc.sheet_mut().find_mut(id) {
            if let Some(p) = t.markers.iter().position(|x| *x == m) {
                t.markers.remove(p);
            } else {
                // only one of each kind (one priority, one task, one flag…)
                let kind = |x: &Marker| std::mem::discriminant(x);
                t.markers.retain(|x| kind(x) != kind(&m));
                t.markers.push(m);
            }
        }
    }

    pub fn clear_markers(&mut self, id: &str) {
        self.commit();
        if let Some(t) = self.doc.sheet_mut().find_mut(id) {
            t.markers.clear();
        }
    }

    pub fn with_style(&mut self, id: &str, f: impl FnOnce(&mut TopicStyle)) {
        self.commit();
        if let Some(t) = self.doc.sheet_mut().find_mut(id) {
            let mut st = t.style.clone().unwrap_or_default();
            f(&mut st);
            t.style = Some(st);
        }
    }

    pub fn set_structure(&mut self, id: &str, st: Option<Structure>) {
        self.commit();
        if let Some(t) = self.doc.sheet_mut().find_mut(id) {
            t.structure = st;
        }
        self.pending_fit = true;
    }

    pub fn set_side(&mut self, id: &str, side: Option<Side>) {
        self.commit();
        if let Some(t) = self.doc.sheet_mut().find_mut(id) {
            t.side = side;
        }
        self.pending_fit = true;
    }

    pub fn add_label(&mut self, id: &str, label: String) {
        if label.trim().is_empty() {
            return;
        }
        self.commit();
        if let Some(t) = self.doc.sheet_mut().find_mut(id) {
            if !t.labels.contains(&label) {
                t.labels.push(label);
            }
        }
    }

    pub fn remove_label(&mut self, id: &str, idx: usize) {
        self.commit();
        if let Some(t) = self.doc.sheet_mut().find_mut(id) {
            if idx < t.labels.len() {
                t.labels.remove(idx);
            }
        }
    }

    pub fn set_note(&mut self, id: &str, note: String) {
        self.commit();
        if let Some(t) = self.doc.sheet_mut().find_mut(id) {
            t.note = if note.trim().is_empty() { None } else { Some(note) };
        }
    }

    pub fn set_link(&mut self, id: &str, link: Option<String>) {
        self.commit();
        if let Some(t) = self.doc.sheet_mut().find_mut(id) {
            t.link = link.filter(|l| !l.trim().is_empty());
        }
    }

    // ---------- boundary / summary / relationship ----------

    /// Create a boundary over the currently selected sibling range.
    pub fn add_boundary_from_selection(&mut self) -> bool {
        if self.sel.len() < 2 {
            return false;
        }
        let first = self.sel[0].clone();
        let Some((pid, _)) = self.parent_info(&first) else { return false };
        // all selected must be siblings of first
        for id in &self.sel {
            match self.parent_info(id) {
                Some((p, _)) if p == pid => {}
                _ => return false,
            }
        }
        let sheet = self.doc.sheet();
        let Some(parent) = sheet.find(&pid) else { return false };
        let idxs: Vec<usize> = parent
            .children
            .iter()
            .enumerate()
            .filter(|(_, c)| self.sel.contains(&c.id))
            .map(|(i, _)| i)
            .collect();
        if idxs.is_empty() {
            return false;
        }
        let (start, end) = (*idxs.first().unwrap(), *idxs.last().unwrap());
        self.commit();
        let bid = new_id();
        self.doc.sheet_mut().find_mut(&pid).unwrap().boundaries.push(Boundary {
            id: bid,
            start,
            end,
            label: None,
            color: None,
        });
        true
    }

    pub fn add_summary_from_selection(&mut self) -> bool {
        if self.sel.is_empty() {
            return false;
        }
        let first = self.sel[0].clone();
        let Some((pid, _)) = self.parent_info(&first) else { return false };
        let sheet = self.doc.sheet();
        let Some(parent) = sheet.find(&pid) else { return false };
        let idxs: Vec<usize> = parent
            .children
            .iter()
            .enumerate()
            .filter(|(_, c)| self.sel.contains(&c.id))
            .map(|(i, _)| i)
            .collect();
        if idxs.is_empty() {
            return false;
        }
        let (start, end) = (*idxs.first().unwrap(), *idxs.last().unwrap());
        self.commit();
        let sid = new_id();
        self.doc
            .sheet_mut()
            .find_mut(&pid)
            .unwrap()
            .summaries
            .push(Summary { id: sid, start, end, topic: Topic::new("概要") });
        true
    }

    pub fn begin_relationship(&mut self, from: &str) {
        self.pending_rel = Some(from.to_string());
    }

    pub fn complete_relationship(&mut self, to: &str) -> bool {
        let Some(from) = self.pending_rel.take() else { return false };
        if from == to {
            return false;
        }
        self.commit();
        self.doc.sheet_mut().relationships.push(Relationship {
            id: new_id(),
            a: from,
            b: to.to_string(),
            label: None,
            color: None,
            dashed: false,
            arrows: Arrows::End,
        });
        true
    }

    pub fn update_relationship(&mut self, rid: &str, f: impl FnOnce(&mut Relationship)) {
        self.commit();
        if let Some(r) = self.doc.sheet_mut().relationships.iter_mut().find(|r| r.id == rid) {
            f(r);
        }
    }

    pub fn update_boundary(&mut self, pid: &str, bid: &str, f: impl FnOnce(&mut Boundary)) {
        self.commit();
        if let Some(p) = self.doc.sheet_mut().find_mut(pid) {
            if let Some(b) = p.boundaries.iter_mut().find(|b| b.id == bid) {
                f(b);
            }
        }
    }

    // ---------- clipboard ----------

    pub fn copy_selection(&mut self) {
        if let Some(id) = self.primary().cloned() {
            if let Some(t) = self.doc.sheet().find(&id) {
                self.clipboard_topic = Some(t.clone());
            }
        }
    }

    pub fn cut_selection(&mut self) {
        self.copy_selection();
        self.delete_selected();
    }

    pub fn paste(&mut self) {
        let Some(t) = self.clipboard_topic.clone() else { return };
        let Some(target) = self.primary().cloned().or_else(|| Some(self.doc.sheet().root.id.clone())) else { return };
        self.commit();
        let mut copy = t;
        reassign_ids(&mut copy);
        if let Some(p) = self.doc.sheet_mut().find_mut(&target) {
            p.insert_child(None, copy);
        }
    }

    /// Paste external text (indented) as a subtree of the selection.
    pub fn paste_text_as_tree(&mut self, text: &str) {
        let Ok(mut doc) = xmind_core::fmt::plaintext::parse(text) else { return };
        let tree = std::mem::take(&mut doc.sheets[0].root);
        let target = self.primary().cloned().unwrap_or_else(|| self.doc.sheet().root.id.clone());
        self.commit();
        let mut copy = tree;
        reassign_ids(&mut copy);
        if let Some(p) = self.doc.sheet_mut().find_mut(&target) {
            for c in copy.children {
                p.insert_child(None, c);
            }
        }
    }

    /// (字号, 文字色, 填充色) of a topic as currently themed — used to make
    /// the in-place editor visually match the node.
    pub fn topic_visual(&self, id: &str) -> (f32, Col, Option<Col>) {
        let sheet = self.doc.sheet();
        let theme_id = sheet.theme.clone().unwrap_or_else(|| self.doc.theme.clone());
        let th = xmind_core::theme::theme_by_id(&theme_id);
        // depth + level-1 index for branch color
        let mut level = 0usize;
        let mut cur = id.to_string();
        let mut l1_idx: Option<usize> = None;
        while let Some((parent, idx)) = self.parent_info(&cur) {
            level += 1;
            if parent == sheet.root.id && idx < 10_000 {
                l1_idx = Some(idx);
            }
            cur = parent;
        }
        let branch = match (&l1_idx, level) {
            (Some(i), l) if l >= 1 => xmind_core::theme::branch_color(th, *i),
            _ => th.root_fill,
        };
        let style = sheet.find(id).and_then(|t| t.style.as_ref());
        let res = xmind_core::theme::resolve(th, level, branch, style, false);
        (res.size, res.text, res.fill)
    }

    // ---------- rename / editing ----------

    pub fn start_editing(&mut self, id: &str) {
        let text = self.doc.sheet().find(id).map(|t| t.text.clone()).unwrap_or_default();
        self.editing = Some((id.to_string(), text));
        self.scene_ver += 1;
    }

    pub fn cancel_editing(&mut self) {
        if self.editing.take().is_some() {
            self.scene_ver += 1;
        }
    }

    pub fn commit_editing(&mut self) {
        if let Some((id, buf)) = self.editing.take() {
            let trimmed = buf.trim_end();
            if trimmed != self.doc.sheet().find(&id).map(|t| t.text.as_str()).unwrap_or("") {
                self.commit();
                if let Some(t) = self.doc.sheet_mut().find_mut(&id) {
                    t.text = trimmed.to_string();
                }
            }
            self.scene_ver += 1;
        }
    }

    // ---------- find / replace ----------

    pub fn start_find(&mut self) {
        let q = self.find.as_ref().map(|f| f.query.clone()).unwrap_or_default();
        self.find = Some(FindState { query: q, replace: String::new(), matches: Vec::new(), idx: 0 });
        self.recompute_find();
    }

    pub fn recompute_find(&mut self) {
        let Some(f) = &mut self.find else { return };
        f.matches.clear();
        let q = f.query.to_lowercase();
        if q.is_empty() {
            f.idx = 0;
            return;
        }
        let mut ms = Vec::new();
        self.doc.sheet().walk(&mut |t| {
            if t.text.to_lowercase().contains(&q)
                || t.note.as_ref().map(|n| n.to_lowercase().contains(&q)).unwrap_or(false)
            {
                ms.push(t.id.clone());
            }
        });
        f.matches = ms;
        f.idx = f.idx.min(f.matches.len().saturating_sub(1));
    }

    pub fn find_jump(&mut self, forward: bool) {
        let Some(f) = &mut self.find else { return };
        if f.matches.is_empty() {
            return;
        }
        if forward {
            f.idx = (f.idx + 1) % f.matches.len();
        } else {
            f.idx = (f.idx + f.matches.len() - 1) % f.matches.len();
        }
        let id = f.matches[f.idx].clone();
        self.select_topic(&id, false);
    }

    pub fn replace_current(&mut self) {
        let Some(f) = self.find.clone() else { return };
        if f.matches.is_empty() {
            return;
        }
        let id = f.matches[f.idx].clone();
        let rep = f.replace.clone();
        let q = f.query.clone();
        self.commit();
        if let Some(t) = self.doc.sheet_mut().find_mut(&id) {
            t.text = t.text.replace(&q, &rep);
        }
        self.recompute_find();
    }

    pub fn replace_all(&mut self) {
        let Some(f) = self.find.clone() else { return };
        if f.query.is_empty() {
            return;
        }
        self.commit();
        let (q, rep) = (f.query.clone(), f.replace.clone());
        self.doc.sheet_mut().root.walk_mut(&mut |t: &mut Topic| {
            t.text = t.text.replace(&q, &rep);
        });
        self.recompute_find();
    }

    // ---------- file io ----------

    pub fn save(&mut self) -> XRResult<()> {
        let Some(path) = self.path.clone() else { return Ok(()) };
        xmind_core::fmt::save_xmind(&self.doc, &path)?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> XRResult<()> {
        xmind_core::fmt::save_xmind(&self.doc, &path)?;
        self.path = Some(path);
        self.dirty = false;
        Ok(())
    }

    pub fn load(&mut self, path: PathBuf) -> XRResult<()> {
        let doc = xmind_core::export::import(&path)?;
        self.doc = doc;
        self.path = Some(path);
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
        self.sel.clear();
        self.editing = None;
        self.drill.clear();
        self.presentation = None;
        self.scene_ver += 1;
        self.view = View::new();
        Ok(())
    }

    pub fn load_doc(&mut self, doc: Doc) {
        self.doc = doc;
        self.path = None;
        self.dirty = true;
        self.undo.clear();
        self.redo.clear();
        self.sel.clear();
        self.editing = None;
        self.drill.clear();
        self.presentation = None;
        self.scene_ver += 1;
        self.view = View::new();
    }

    // ---------- presentation ----------

    pub fn start_presentation(&mut self) {
        let root_id = self.drill.last().cloned().unwrap_or_else(|| self.doc.sheet().root.id.clone());
        let mut order = Vec::new();
        if let Some(t) = self.doc.sheet().find(&root_id) {
            collect_dfs(t, &mut order);
        }
        self.presentation = Some((order, 0));
    }

    pub fn present_step(&mut self, forward: bool) {
        if let Some((order, idx)) = &mut self.presentation {
            if order.is_empty() {
                return;
            }
            if forward {
                *idx = (*idx + 1).min(order.len() - 1);
            } else {
                *idx = idx.saturating_sub(1);
            }
            let id = order[*idx].clone();
            self.select_topic(&id, false);
        }
    }

    pub fn current_present_id(&self) -> Option<String> {
        self.presentation.as_ref().and_then(|(o, i)| o.get(*i).cloned())
    }
}

fn collect_dfs(t: &Topic, out: &mut Vec<String>) {
    out.push(t.id.clone());
    for c in &t.children {
        collect_dfs(c, out);
    }
}

fn reassign_ids(t: &mut Topic) {
    t.id = new_id();
    for c in t.children.iter_mut() {
        reassign_ids(c);
    }
    for d in t.detached.iter_mut() {
        reassign_ids(d);
    }
    for s in t.summaries.iter_mut() {
        s.id = new_id();
        reassign_ids(&mut s.topic);
    }
}

// color helpers for the GUI ------------------------------------------------

pub fn swatches() -> Vec<Col> {
    vec![
        Col::rgb(239, 68, 68),
        Col::rgb(249, 115, 22),
        Col::rgb(245, 158, 11),
        Col::rgb(34, 197, 94),
        Col::rgb(16, 185, 129),
        Col::rgb(6, 182, 212),
        Col::rgb(29, 122, 243),
        Col::rgb(99, 102, 241),
        Col::rgb(139, 92, 246),
        Col::rgb(236, 72, 153),
        Col::rgb(17, 24, 39),
        Col::rgb(107, 114, 128),
    ]
}

pub fn marker_groups() -> Vec<(&'static str, Vec<Marker>)> {
    vec![
        ("优先级", (1..=9).map(Marker::Priority).collect()),
        (
            "任务进度",
            vec![
                Marker::Task(0.0),
                Marker::Task(12.5),
                Marker::Task(25.0),
                Marker::Task(37.5),
                Marker::Task(50.0),
                Marker::Task(62.5),
                Marker::Task(75.0),
                Marker::Task(87.5),
                Marker::Task(100.0),
            ],
        ),
        (
            "红旗",
            [ColKey::Red, ColKey::Orange, ColKey::Yellow, ColKey::Green, ColKey::Blue, ColKey::Purple, ColKey::Black]
                .iter()
                .map(|c| Marker::Flag(*c))
                .collect(),
        ),
        (
            "星标",
            [ColKey::Red, ColKey::Orange, ColKey::Yellow, ColKey::Green, ColKey::Blue, ColKey::Purple, ColKey::Black]
                .iter()
                .map(|c| Marker::Star(*c))
                .collect(),
        ),
        (
            "人物",
            [ColKey::Red, ColKey::Orange, ColKey::Green, ColKey::Blue, ColKey::Purple, ColKey::Black]
                .iter()
                .map(|c| Marker::People(*c))
                .collect(),
        ),
        (
            "其他",
            vec![
                Marker::Heart(ColKey::Red),
                Marker::Face(Face::Laugh),
                Marker::Face(Face::Neutral),
                Marker::Face(Face::Sad),
                Marker::Symbol(Sym::Attention),
                Marker::Symbol(Sym::Question),
                Marker::Symbol(Sym::Idea),
            ],
        ),
    ]
}
