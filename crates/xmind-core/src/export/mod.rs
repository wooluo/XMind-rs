pub mod docx;
pub mod pdf;
pub mod raster;
pub mod svg;
pub mod xlsx;

use crate::model::Doc;
use crate::XRResult;
use std::path::Path;

/// Export the current sheet to `path`, choosing the format from its extension.
/// Supported: png svg pdf md txt opml mm html docx xlsx
pub fn export(doc: &Doc, path: &Path, scale: f32) -> XRResult<()> {
    let theme_id = doc.sheet().theme.clone().unwrap_or_else(|| doc.theme.clone());
    let sheet = doc.sheet();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "md" | "markdown" => std::fs::write(path, crate::fmt::markdown::render(doc))?,
        "txt" => std::fs::write(path, crate::fmt::plaintext::render(doc))?,
        "opml" => std::fs::write(path, crate::fmt::opml::render(doc))?,
        "mm" => std::fs::write(path, crate::fmt::freemind::render(doc))?,
        "svg" => {
            let book = raster::FontBook::load();
            let scene = crate::layout::build_scene(sheet, &theme_id, &book, None);
            std::fs::write(path, svg::render(&scene, doc, 1.0))?;
        }
        "html" => {
            let book = raster::FontBook::load();
            let scene = crate::layout::build_scene(sheet, &theme_id, &book, None);
            std::fs::write(path, svg::wrap_html(&scene, doc, &sheet.title))?;
        }
        "png" => {
            let book = raster::FontBook::load();
            let scene = crate::layout::build_scene(sheet, &theme_id, &book, None);
            let bytes = raster::render_png(&scene, doc, scale)?;
            std::fs::write(path, bytes)?;
        }
        "pdf" => {
            let book = raster::FontBook::load();
            let scene = crate::layout::build_scene(sheet, &theme_id, &book, None);
            let bytes = pdf::render_pdf(&scene, doc, scale)?;
            std::fs::write(path, bytes)?;
        }
        "docx" => docx::export(doc, path)?,
        "xlsx" => xlsx::export(doc, path)?,
        other => return Err(crate::XRErr::Other(format!("不支持的导出格式: .{other}"))),
    }
    Ok(())
}

/// Import a document from md / txt / opml / mm / xmind by extension.
pub fn import(path: &Path) -> XRResult<Doc> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "xmind" => crate::fmt::load_xmind(path),
        "md" | "markdown" => {
            let s = std::fs::read_to_string(path)?;
            Ok(crate::fmt::markdown::parse(&s))
        }
        "txt" => {
            let s = std::fs::read_to_string(path)?;
            crate::fmt::plaintext::parse(&s)
        }
        "opml" => {
            let s = std::fs::read_to_string(path)?;
            crate::fmt::opml::parse(&s)
        }
        "mm" => {
            let s = std::fs::read_to_string(path)?;
            crate::fmt::freemind::parse(&s)
        }
        other => Err(crate::XRErr::Other(format!("不支持的导入格式: .{other}"))),
    }
}
