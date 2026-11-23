//! Exportar el storyboard a PDF (Issue #134, fase S3 de docs/salas-storyboard-animatic.md).
//!
//! Escribe el PDF a mano, sin dependencias: páginas A4 en vertical con una cuadrícula de planos (miniatura en JPEG, número, título,
//! duración y los textos de acción, diálogo, cámara, transición y notas), en Helvetica. No depende de la interfaz.

use crate::model::{Layer, LayerKind, Panel, Project};

const PAGE_W: f32 = 595.0;
const PAGE_H: f32 = 842.0;
const MARGIN: f32 = 36.0;
const COLS: usize = 2;
const ROWS: usize = 3;
const GAP: f32 = 14.0;
/// Planos por página.
pub const PER_PAGE: usize = COLS * ROWS;

/// Convierte un texto a bytes WinAnsi (Latin-1) con escapes de PDF: lo que no cabe se cambia por «?».
fn pdf_text(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        let b = match c {
            '\n' | '\r' | '\t' => b' ',
            '“' | '”' => b'"',
            '‘' | '’' => b'\'',
            '–' | '—' => b'-',
            '…' => b'.',
            c if (c as u32) < 256 => c as u32 as u8,
            _ => b'?',
        };
        match b {
            b'(' | b')' | b'\\' => {
                out.push(b'\\');
                out.push(b);
            }
            0..=31 => out.push(b' '),
            _ => out.push(b),
        }
    }
    out
}

/// Parte un texto en líneas de como mucho `max` caracteres (Helvetica mide unos 0,5 del cuerpo por carácter).
fn wrap(text: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut cur = String::new();
        for word in para.split_whitespace() {
            let n = cur.chars().count();
            let w = word.chars().count();
            if n > 0 && n + 1 + w > max {
                lines.push(std::mem::take(&mut cur));
            }
            if w > max {
                // Palabra más larga que la línea: se corta.
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(max) {
                    if !cur.is_empty() {
                        lines.push(std::mem::take(&mut cur));
                    }
                    cur = chunk.iter().collect();
                }
                continue;
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
        lines.push(cur);
    }
    while lines.last().map_or(false, |l| l.is_empty()) {
        lines.pop();
    }
    lines
}

/// Una imagen JPEG del plano `i` (con el papel blanco): lo que hay dibujado en su composición, o su dibujo aplanado si no tiene.
fn panel_jpeg(p: &Project, panel: &Panel, width_px: u32) -> Result<(u32, u32, Vec<u8>), String> {
    let mut sub = Project::blank(p.width, p.height, p.fps.max(1), 1);
    sub.background = [255, 255, 255, 255];
    let comp_layers: Option<&Vec<Layer>> = panel.comp.and_then(|id| p.comp(id)).map(|c| &c.layers).filter(|l| !l.is_empty());
    match comp_layers {
        Some(layers) => {
            sub.layers = layers.clone();
            // Las precomposiciones de dentro necesitan las composiciones del proyecto.
            if sub.layers.iter().any(|l| l.kind == LayerKind::Precomp) {
                sub.comps = p.comps.clone();
            }
        }
        None => {
            let mut l = Layer::new("Dibujo");
            l.keys.insert(0, panel.drawing.clone());
            sub.layers = vec![l];
        }
    }
    let scale = (width_px as f32 / p.width.max(1) as f32).clamp(0.05, 2.0);
    let pm = crate::render::render_frame(&sub, 0, scale).ok_or("No se pudo dibujar un plano")?;
    let (w, h) = (pm.width(), pm.height());
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for px in pm.data().chunks_exact(4) {
        rgb.extend_from_slice(&px[..3]);
    }
    let img = image::RgbImage::from_raw(w, h, rgb).ok_or("Imagen inválida")?;
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85).encode_image(&img).map_err(|e| e.to_string())?;
    Ok((w, h, bytes))
}

/// Escribe el storyboard completo como PDF. `title` sale en la cabecera de cada página.
pub fn storyboard_pdf(p: &Project, title: &str) -> Result<Vec<u8>, String> {
    let panels = &p.storyboard;
    if panels.is_empty() {
        return Err("No hay planos que exportar".into());
    }
    let n = panels.len();
    let pages = n.div_ceil(PER_PAGE);
    let cell_w = (PAGE_W - 2.0 * MARGIN - GAP * (COLS as f32 - 1.0)) / COLS as f32;
    let top = PAGE_H - MARGIN - 22.0;
    let bottom = MARGIN + 14.0;
    let cell_h = (top - bottom - GAP * (ROWS as f32 - 1.0)) / ROWS as f32;
    let img_h = cell_w * p.height as f32 / p.width.max(1) as f32;
    let text_lines = (((cell_h - img_h - 20.0) / 9.5).floor() as usize).max(2);
    let max_chars = ((cell_w - 8.0) / (7.5 * 0.5)) as usize;

    // Numeración de objetos: 1 catálogo, 2 páginas, 3 y 4 las fuentes, luego las imágenes, luego (contenido, página) por página.
    let first_img = 5usize;
    let first_page = first_img + n;
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let put = |objs: &mut Vec<Vec<u8>>, body: Vec<u8>| objs.push(body);
    put(&mut objs, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    let kids: Vec<String> = (0..pages).map(|k| format!("{} 0 R", first_page + k * 2 + 1)).collect();
    put(&mut objs, format!("<< /Type /Pages /Kids [{}] /Count {pages} >>", kids.join(" ")).into_bytes());
    put(&mut objs, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec());
    put(&mut objs, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".to_vec());
    for panel in panels {
        let (w, h, jpg) = panel_jpeg(p, panel, 640)?;
        let mut body = format!("<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n", jpg.len()).into_bytes();
        body.extend_from_slice(&jpg);
        body.extend_from_slice(b"\nendstream");
        put(&mut objs, body);
    }
    for page in 0..pages {
        let range = page * PER_PAGE..((page + 1) * PER_PAGE).min(n);
        let mut c: Vec<u8> = Vec::new();
        let text = |c: &mut Vec<u8>, font: u8, size: f32, x: f32, y: f32, s: &str| {
            c.extend_from_slice(format!("BT /F{font} {size} Tf {x:.1} {y:.1} Td (").as_bytes());
            c.extend_from_slice(&pdf_text(s));
            c.extend_from_slice(b") Tj ET\n");
        };
        text(&mut c, 2, 11.0, MARGIN, PAGE_H - MARGIN - 8.0, title);
        text(&mut c, 1, 8.0, PAGE_W - MARGIN - 70.0, PAGE_H - MARGIN - 8.0, &format!("Página {} de {pages}", page + 1));
        c.extend_from_slice(format!("0.7 w {MARGIN:.1} {:.1} m {:.1} {:.1} l S\n", top + 6.0, PAGE_W - MARGIN, top + 6.0).as_bytes());
        let mut xobjs = Vec::new();
        for (slot, i) in range.clone().enumerate() {
            let (col, row) = (slot % COLS, slot / COLS);
            let x = MARGIN + col as f32 * (cell_w + GAP);
            let y_top = top - row as f32 * (cell_h + GAP);
            let panel = &panels[i];
            let img_id = first_img + i;
            xobjs.push(format!("/Im{i} {img_id} 0 R"));
            c.extend_from_slice(format!("q {cell_w:.1} 0 0 {img_h:.1} {x:.1} {:.1} cm /Im{i} Do Q\n", y_top - img_h).as_bytes());
            c.extend_from_slice(format!("0.5 w {x:.1} {:.1} {cell_w:.1} {img_h:.1} re S\n", y_top - img_h).as_bytes());
            let heading = format!("{}  {}  ({:.1} s)", panel.label(i), if panel.title.trim().is_empty() { "Plano" } else { panel.title.trim() }, panel.dur);
            let max_h = ((cell_w - 4.0) / (9.0 * 0.55)) as usize;
            let heading: String = heading.chars().take(max_h).collect();
            text(&mut c, 2, 9.0, x, y_top - img_h - 12.0, &heading);
            let mut lines: Vec<String> = Vec::new();
            for (label, value) in [("Acción", &panel.action), ("Diálogo", &panel.dialogue), ("Cámara", &panel.camera), ("Transición", &panel.trans), ("Notas", &panel.notes)] {
                if !value.trim().is_empty() {
                    lines.extend(wrap(&format!("{label}: {}", value.trim()), max_chars));
                }
            }
            if panel.cam.is_some() {
                lines.push("Cámara animada (zoom y desplazamiento)".into());
            }
            let truncated = lines.len() > text_lines;
            lines.truncate(text_lines);
            if truncated {
                if let Some(last) = lines.last_mut() {
                    let keep: String = last.chars().take(max_chars.saturating_sub(3)).collect();
                    *last = format!("{keep}...");
                }
            }
            for (k, line) in lines.iter().enumerate() {
                text(&mut c, 1, 7.5, x, y_top - img_h - 24.0 - k as f32 * 9.5, line);
            }
        }
        let mut content = format!("<< /Length {} >>\nstream\n", c.len()).into_bytes();
        content.extend_from_slice(&c);
        content.extend_from_slice(b"endstream");
        put(&mut objs, content);
        let content_id = first_page + page * 2;
        put(
            &mut objs,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] /Contents {content_id} 0 R /Resources << /Font << /F1 3 0 R /F2 4 0 R >> /XObject << {} >> >> >>",
                xobjs.join(" ")
            )
            .into_bytes(),
        );
    }
    // Cuerpo y tabla de referencias cruzadas.
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objs.len());
    for (k, body) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", k + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(n: usize) -> Project {
        let mut p = Project::blank(1280, 720, 24, 24);
        for i in 0..n {
            let mut pn = Panel::new(i + 1);
            pn.action = format!("Entra el personaje por la izquierda y se detiene (plano {}). Texto largo para comprobar que se parte en varias líneas sin salirse de la celda.", i + 1);
            pn.dialogue = "—¿Qué haces aquí? (dijo) \\ ñandú".into();
            p.storyboard.push(pn);
        }
        p
    }

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    #[test]
    fn the_pdf_has_one_page_per_six_panels_and_a_valid_cross_reference_table() {
        for (n, pages) in [(1usize, 1usize), (6, 1), (7, 2), (13, 3)] {
            let pdf = storyboard_pdf(&project(n), "Prueba").unwrap();
            assert!(pdf.starts_with(b"%PDF-1.4"));
            assert!(pdf.ends_with(b"%%EOF\n"));
            assert!(find(&pdf, format!("/Count {pages} ").as_bytes()).is_some(), "{n} planos → {pages} páginas");
            // Cada entrada de la tabla apunta al «N 0 obj» que dice.
            let sx = find(&pdf, b"startxref\n").unwrap() + "startxref\n".len();
            let xref_off: usize = std::str::from_utf8(&pdf[sx..]).unwrap().lines().next().unwrap().parse().unwrap();
            assert!(pdf[xref_off..].starts_with(b"xref\n"));
            let table = std::str::from_utf8(&pdf[xref_off..find(&pdf, b"trailer").unwrap()]).unwrap();
            let entries: Vec<&str> = table.lines().skip(3).collect();
            assert!(!entries.is_empty());
            for (k, line) in entries.iter().enumerate() {
                let off: usize = line[..10].parse().unwrap();
                assert!(pdf[off..].starts_with(format!("{} 0 obj", k + 1).as_bytes()), "objeto {} en su sitio", k + 1);
            }
        }
    }

    #[test]
    fn accents_are_kept_and_parentheses_and_backslashes_are_escaped() {
        assert_eq!(pdf_text("Acción ñ ¿?"), b"Acci\xF3n \xF1 \xBF?".to_vec());
        assert_eq!(pdf_text("a(b)c\\d"), b"a\\(b\\)c\\\\d".to_vec());
        assert_eq!(pdf_text("日本"), b"??".to_vec());
        let pdf = storyboard_pdf(&project(1), "T").unwrap();
        // Ningún paréntesis ni barra sin escapar de los textos rompe el flujo: el diálogo aparece con sus escapes.
        assert!(find(&pdf, b"(dijo\\)").is_some());
    }

    #[test]
    fn long_text_is_wrapped_and_an_empty_storyboard_is_refused() {
        let lines = wrap("uno dos tres cuatro cinco seis siete ocho", 12);
        assert!(lines.len() >= 3 && lines.iter().all(|l| l.chars().count() <= 12), "{lines:?}");
        assert_eq!(wrap("abcdefghijklmnopqrstuvwxyz", 10).len(), 3);
        assert!(storyboard_pdf(&Project::blank(1280, 720, 24, 24), "T").is_err());
    }
}
