//! Minimal Excel (.xlsx) export: outline rows (level / text / markers / labels / note).

use crate::model::{Doc, Topic};
use crate::XRResult;
use std::io::Write;
use std::path::Path;

pub fn export(doc: &Doc, path: &Path) -> XRResult<()> {
    let sheet = doc.sheet();
    let mut rows = String::new();
    rows.push_str(&row(&["层级", "内容", "标记", "标签", "备注"]));
    emit(&sheet.root, 0, &mut rows);
    let ws = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
<cols><col min=\"1\" max=\"1\" width=\"6\"/><col min=\"2\" max=\"2\" width=\"48\"/><col min=\"3\" max=\"3\" width=\"18\"/><col min=\"4\" max=\"4\" width=\"18\"/><col min=\"5\" max=\"5\" width=\"40\"/></cols>\
<sheetData>{rows}</sheetData></worksheet>"
    );
    let file = std::fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file("[Content_Types].xml", opts)?;
    zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/></Types>")?;
    zip.start_file("_rels/.rels", opts)?;
    zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>")?;
    zip.start_file("xl/workbook.xml", opts)?;
    zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"Outline\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>")?;
    zip.start_file("xl/_rels/workbook.xml.rels", opts)?;
    zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>")?;
    zip.start_file("xl/worksheets/sheet1.xml", opts)?;
    zip.write_all(ws.as_bytes())?;
    zip.finish()?;
    Ok(())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn cell(text: &str) -> String {
    format!("<c t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>", esc(text))
}

fn row(cells: &[&str]) -> String {
    let c: String = cells.iter().map(|x| cell(x)).collect();
    format!("<row>{c}</row>")
}

fn emit(t: &Topic, level: usize, out: &mut String) {
    let markers: Vec<String> = t.markers.iter().map(|m| m.to_xmind_id()).collect();
    let r = row(&[
        &level.to_string(),
        &t.text,
        &markers.join(" "),
        &t.labels.join(", "),
        t.note.as_deref().unwrap_or(""),
    ]);
    out.push_str(&r);
    for c in &t.children {
        emit(c, level + 1, out);
    }
}
