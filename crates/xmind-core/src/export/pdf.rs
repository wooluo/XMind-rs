//! PDF export. v1 embeds a high-resolution raster of the map into a single
//! page PDF (reliable for CJK without font subsetting).

use crate::model::Doc;
use crate::scene::Scene;
use crate::XRResult;
use std::io::Write;

pub fn render_pdf(scene: &Scene, doc: &Doc, scale: f32) -> XRResult<Vec<u8>> {
    let book = super::raster::FontBook::load();
    let png = super::raster::render_png_with(scene, doc, scale.max(2.0), &book)?;
    // decode back to RGB (pdf streams must be raw samples)
    let img = image::load_from_memory(&png)
        .map_err(|e| crate::XRErr::Other(format!("png decode: {e}")))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for px in img.pixels() {
        rgb.push(px.0[0]);
        rgb.push(px.0[1]);
        rgb.push(px.0[2]);
    }
    // flate compress
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&rgb)?;
    let zdata = z.finish()?;

    // page size in points (72 dpi); map px at 96dpi => *0.75
    let pw = w as f32 * 0.75;
    let ph = h as f32 * 0.75;

    let mut objs: Vec<Vec<u8>> = Vec::new();
    objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objs.push(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec());
    objs.push(format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>"
    ).into_bytes());
    let content = format!("q {pw:.2} 0 0 {ph:.2} 0 0 cm /Im0 Do Q");
    let content_obj = format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len());
    objs.push(content_obj.into_bytes());
    objs.push(format!(
        "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
        zdata.len()
    ).into_bytes());
    // image object needs binary stream appended after obj 5 header — handle below
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets = Vec::new();
    for (i, obj) in objs.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        pdf.extend_from_slice(obj);
        if i == 4 {
            pdf.extend_from_slice(&zdata);
            pdf.extend_from_slice(b"\nendstream");
        }
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref_pos = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objs.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n", objs.len() + 1).as_bytes(),
    );
    Ok(pdf)
}
