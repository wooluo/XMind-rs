pub mod freemind;
pub mod markdown;
pub mod opml;
pub mod plaintext;
pub mod xmind_legacy;
pub mod xmind_zen;
pub mod xml_util;

use crate::model::Doc;
use crate::XRResult;
use std::path::Path;

/// Load a .xmind file (new JSON format or legacy XMind 8 XML).
pub fn load_xmind(path: &Path) -> XRResult<Doc> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let read_entry = |zip: &mut zip::ZipArchive<std::fs::File>, name: &str| -> Option<String> {
        let mut f = zip.by_name(name).ok()?;
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut bytes).ok()?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    };
    // prefer our own lossless extension
    if let Some(s) = read_entry(&mut zip, "xmindrs.json") {
        if let Ok(core) = serde_json::from_str::<crate::model::DocCore>(&s) {
            let mut doc = Doc { core, assets: Default::default() };
            load_assets(&mut zip, &mut doc);
            return Ok(doc);
        }
    }
    if let Some(s) = read_entry(&mut zip, "content.json") {
        let mut doc = xmind_zen::parse_content(&s)?;
        load_assets(&mut zip, &mut doc);
        return Ok(doc);
    }
    if let Some(s) = read_entry(&mut zip, "content.xml") {
        let mut doc = xmind_legacy::parse(&s)?;
        load_assets(&mut zip, &mut doc);
        return Ok(doc);
    }
    Err(crate::XRErr::Other("不是有效的 .xmind 文件（缺少 content.json / content.xml）".into()))
}

fn load_assets<R: std::io::Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, doc: &mut Doc) {
    let names: Vec<String> = zip.file_names().map(|s| s.to_string()).collect();
    for name in names {
        let short = name.trim_start_matches('/');
        if let Some(file) = short.strip_prefix("resources/") {
            if file.is_empty() {
                continue;
            }
            let id = file.split('.').next().unwrap_or(file).to_string();
            let kind = if file.to_ascii_lowercase().ends_with(".jpg") || file.to_ascii_lowercase().ends_with(".jpeg") {
                "jpeg"
            } else {
                "png"
            };
            if let Ok(mut f) = zip.by_name(&name) {
                let mut data = Vec::new();
                if std::io::Read::read_to_end(&mut f, &mut data).is_ok() && !data.is_empty() {
                    doc.assets.insert(id, crate::model::Asset { kind: kind.to_string(), data });
                }
            }
        }
    }
}

/// Save a .xmind file (XMind Zen compatible JSON + our lossless extension).
pub fn save_xmind(doc: &Doc, path: &Path) -> XRResult<()> {
    xmind_zen::write(doc, path)
}
