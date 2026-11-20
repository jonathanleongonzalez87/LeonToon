//! Escritura de ABR **versión 2** (el formato clásico de Photoshop).
//!
//! La versión 2 solo guarda: pinceles redondos calculados (diámetro, dureza, espaciado, redondez, ángulo) y
//! formas de imagen (con nombre y espaciado). **No** guarda dinámicas, dispersión, textura, doble pincel, color,
//! bordes húmedos, flujo, opacidad ni suavizado. Por eso cada pincel exportado trae una lista de lo que no cupo,
//! y nada se descarta sin avisar.

use super::reader::packbits;
use crate::brush::{BrushParams, Control, Gray, TexKind};
use crate::presets::BrushPreset;

/// Qué se escribió y qué no cupo en el formato.
#[derive(Clone, Debug, Default)]
pub struct ExportReport {
    pub written: usize,
    /// Por pincel: nombre y lo que no se pudo guardar.
    pub per_brush: Vec<(String, Vec<String>)>,
    /// Avisos de todo el archivo.
    pub general: Vec<String>,
}

fn be16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_be_bytes());
}
fn be32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

/// Propiedades del pincel que ABR v2 no puede guardar.
fn lost(p: &BrushPreset, sampled: bool) -> Vec<String> {
    let d = BrushParams::default();
    let (s, q) = (&p.setup, &p.setup.params);
    let mut n: Vec<String> = Vec::new();
    if !sampled {
        n.push("el nombre (los pinceles redondos de ABR v2 no tienen nombre; Photoshop los llama por su tamaño)".into());
    } else {
        n.push("el tamaño (las formas de imagen se usan con el tamaño de la propia imagen)".into());
        if q.hardness < 0.999 {
            n.push("la dureza (no se aplica a formas de imagen)".into());
        }
        if (q.roundness - d.roundness).abs() > 1e-3 || q.angle.abs() > 1e-3 {
            n.push("el ángulo y la redondez de la forma".into());
        }
    }
    if s.opacity < 0.999 {
        n.push(format!("la opacidad de {:.0} %", s.opacity * 100.0));
    }
    if q.flow < 0.999 {
        n.push(format!("el flujo de {:.0} %", q.flow * 100.0));
    }
    if s.smoothing > 0.0 {
        n.push("el suavizado".into());
    }
    let shape = q.size_jitter > 0.0 || q.size_control != Control::Off || q.angle_jitter > 0.0 || q.roundness_jitter > 0.0;
    if shape {
        n.push("la dinámica de forma (variación y control de tamaño, ángulo y redondez)".into());
    }
    if q.opacity_jitter > 0.0 || q.opacity_control != Control::Off || q.flow_jitter > 0.0 || q.flow_control != Control::Off {
        n.push("la transferencia (variación y control de opacidad y flujo)".into());
    }
    if q.scatter > 0.0 {
        n.push("la dispersión".into());
    }
    if q.texture != TexKind::None {
        n.push("la textura".into());
    }
    if q.dual {
        n.push("el doble pincel".into());
    }
    if q.hue_jitter > 0.0 || q.sat_jitter > 0.0 || q.bri_jitter > 0.0 || q.fg_bg_jitter > 0.0 {
        n.push("la dinámica de color".into());
    }
    if q.wet_edges > 0.0 {
        n.push("los bordes húmedos".into());
    }
    n
}

fn write_pixels(out: &mut Vec<u8>, g: &Gray) {
    let (w, h) = (g.w as i32, g.h as i32);
    for v in [0, 0, h, w] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    be16(out, 8); // profundidad
    out.push(1); // compresión RLE
    let rows: Vec<Vec<u8>> = g.data.chunks(g.w as usize).map(packbits).collect();
    for r in &rows {
        be16(out, r.len().min(u16::MAX as usize) as u16);
    }
    for r in &rows {
        out.extend_from_slice(r);
    }
}

/// Arma un ABR v2 con los pinceles dados. `tip` es la imagen de la punta de cada pincel, si la usa.
pub fn export_v2(brushes: &[(BrushPreset, Option<Gray>)]) -> Result<(Vec<u8>, ExportReport), String> {
    if brushes.is_empty() {
        return Err("No hay pinceles que exportar".into());
    }
    if brushes.len() > u16::MAX as usize {
        return Err("Hay demasiados pinceles para un ABR (el máximo es 65535)".into());
    }
    let mut out = Vec::new();
    be16(&mut out, 2);
    be16(&mut out, brushes.len() as u16);
    let mut rep = ExportReport::default();
    for (p, tip) in brushes {
        let q = &p.setup.params;
        let sampled = q.use_tip_image && tip.is_some();
        let mut notes = lost(p, sampled);
        if q.use_tip_image && tip.is_none() {
            notes.push("la forma de imagen no está disponible: se exporta como pincel redondo".into());
        }
        let mut body = Vec::new();
        let spacing = q.spacing.round().clamp(1.0, 999.0) as u16;
        match tip.as_ref().filter(|_| sampled) {
            Some(g) => {
                if g.w > 2500 || g.h > 2500 {
                    return Err(format!("La forma de «{}» mide {}×{} px y ABR admite hasta 2500", p.name, g.w, g.h));
                }
                be32(&mut body, 0); // misc
                be16(&mut body, spacing);
                let units: Vec<u16> = p.name.encode_utf16().chain(std::iter::once(0)).collect();
                be32(&mut body, units.len() as u32);
                for u in units {
                    be16(&mut body, u);
                }
                body.push(1); // suavizado
                for v in [0u16, 0, g.h as u16, g.w as u16] {
                    be16(&mut body, v);
                }
                write_pixels(&mut body, g);
                be16(&mut out, 2);
            }
            None => {
                be32(&mut body, 0); // misc
                be16(&mut body, spacing);
                let diameter = p.setup.size.round();
                if diameter > 999.0 {
                    notes.push(format!("el diámetro de {diameter:.0} px (ABR v2 admite hasta 999)"));
                }
                be16(&mut body, diameter.clamp(1.0, 999.0) as u16);
                be16(&mut body, (q.roundness * 100.0).round().clamp(1.0, 100.0) as u16);
                be16(&mut body, (q.hardness * 100.0).round().clamp(0.0, 100.0) as u16);
                be16(&mut body, q.angle.rem_euclid(360.0).round() as u16);
                be16(&mut out, 1);
            }
        }
        be32(&mut out, body.len() as u32);
        out.extend_from_slice(&body);
        rep.written += 1;
        rep.per_brush.push((p.name.clone(), notes));
    }
    rep.general.push("Se escribió ABR versión 2. Solo se probó releyéndolo con el propio lector de LeonToon; no se ha abierto en Photoshop.".into());
    if rep.per_brush.iter().any(|(_, n)| !n.is_empty()) {
        rep.general.push("Los pinceles con dinámicas, efectos, flujo u opacidad pierden esas propiedades: ABR v2 no las guarda. Mira el detalle por pincel.".into());
    }
    Ok((out, rep))
}
