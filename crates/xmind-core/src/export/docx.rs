//! Minimal Word (.docx) export: headings + text + notes.

use crate::model::{Doc, Topic};
use crate::XRResult;
use std::io::Write;
use std::path::Path;

pub fn export(doc: &Doc, path: &Path) -> XRResult<()> {
    let sheet = doc.sheet();
    let mut body = String::new();
    emit(&sheet.root, 0, &mut body);
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:body>{body}<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/></w:sectPr></w:body></w:document>"
    );
    let file = std::fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file("[Content_Types].xml", opts)?;
    zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>")?;
    zip.start_file("_rels/.rels", opts)?;
    zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>")?;
    zip.start_file("word/document.xml", opts)?;
    zip.write_all(document.as_bytes())?;
    zip.finish()?;
    Ok(())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn emit(t: &Topic, level: usize, out: &mut String) {
    let style = if level == 0 {
        String::new()
    } else {
        format!("<w:pStyle w:val=\"Heading{}\"/>", level.min(6))
    };
    out.push_str(&format!("<w:p><w:pPr>{style}</w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>", esc(&t.text)));
    if !t.labels.is_empty() {
        out.push_str(&format!(
            "<w:p><w:pPr></w:pPr><w:r><w:rPr><w:i/></w:rPr><w:t xml:space=\"preserve\">🏷 {}</w:t></w:r></w:p>",
            esc(&t.labels.join(", "))
        ));
    }
    if let Some(n) = &t.note {
        if !n.trim().is_empty() {
            for line in n.lines() {
                out.push_str(&format!(
                    "<w:p><w:pPr><w:ind w:left=\"420\"/></w:pPr><w:r><w:rPr><w:i/><w:color w:val=\"666666\"/></w:rPr><w:t xml:space=\"preserve\">📝 {line}</w:t></w:r></w:p>",
                    line = esc(line)
                ));
            }
        }
    }
    for c in &t.children {
        emit(c, level + 1, out);
    }
}
