//! Capas de mapa de bits: píxeles RGBA (premultiplicados) de un dibujo, guardados como PNG dentro del `.ltoon`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::atomic::{AtomicU64, Ordering};
use tiny_skia::{IntSize, Pixmap};

static NEXT_VER: AtomicU64 = AtomicU64::new(1);

fn next_ver() -> u64 {
    NEXT_VER.fetch_add(1, Ordering::Relaxed)
}

/// Lado máximo de un mapa de bits (el de la escena).
const MAX_SIDE: u32 = 8192;

#[derive(Clone)]
pub struct Raster {
    pub w: u32,
    pub h: u32,
    /// RGBA premultiplicado, `w * h * 4` bytes.
    pub data: Vec<u8>,
    /// Cambia con cada edición: sirve para saber cuándo hay que volver a subir la textura.
    pub ver: u64,
    inked: bool,
}

impl std::fmt::Debug for Raster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Raster({}x{}, v{})", self.w, self.h, self.ver)
    }
}

impl Raster {
    /// Mapa de bits transparente del tamaño de la escena.
    pub fn new(w: u32, h: u32) -> Self {
        let (w, h) = (w.clamp(1, MAX_SIDE), h.clamp(1, MAX_SIDE));
        Raster { w, h, data: vec![0; (w * h * 4) as usize], ver: next_ver(), inked: false }
    }

    fn from_pixmap(pm: Pixmap) -> Self {
        let (w, h) = (pm.width(), pm.height());
        let data = pm.take();
        let inked = data.chunks_exact(4).any(|p| p[3] != 0);
        Raster { w, h, data, ver: next_ver(), inked }
    }

    /// `true` si no tiene ningún píxel visible.
    pub fn is_blank(&self) -> bool {
        !self.inked
    }

    /// Edita los píxeles con tiny-skia y actualiza la versión.
    pub fn edit(&mut self, f: impl FnOnce(&mut Pixmap)) {
        let data = std::mem::take(&mut self.data);
        let pm = IntSize::from_wh(self.w, self.h).and_then(|s| Pixmap::from_vec(data, s)).or_else(|| Pixmap::new(self.w, self.h));
        if let Some(mut pm) = pm {
            f(&mut pm);
            self.data = pm.take();
        } else {
            self.data = vec![0; (self.w * self.h * 4) as usize];
        }
        self.inked = self.data.chunks_exact(4).any(|p| p[3] != 0);
        self.ver = next_ver();
    }

    /// Color (sin premultiplicar) del píxel en el punto de la escena, si no es transparente.
    pub fn pick(&self, p: [f32; 2]) -> Option<[u8; 4]> {
        if p[0] < 0.0 || p[1] < 0.0 {
            return None;
        }
        let (x, y) = (p[0] as u32, p[1] as u32);
        if x >= self.w || y >= self.h {
            return None;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let px = self.data.get(i..i + 4)?;
        if px[3] == 0 {
            return None;
        }
        let a = px[3] as f32 / 255.0;
        let un = |c: u8| ((c as f32 / a).round().min(255.0)) as u8;
        Some([un(px[0]), un(px[1]), un(px[2]), px[3]])
    }
}

// ---------- Guardado: PNG en base64 ----------

#[derive(Serialize, Deserialize)]
struct RasterFile {
    w: u32,
    h: u32,
    png: String,
}

impl Serialize for Raster {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let size = IntSize::from_wh(self.w, self.h).ok_or_else(|| serde::ser::Error::custom("tamaño de mapa de bits no válido"))?;
        let pm = Pixmap::from_vec(self.data.clone(), size).ok_or_else(|| serde::ser::Error::custom("datos de mapa de bits incorrectos"))?;
        let png = pm.encode_png().map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        RasterFile { w: self.w, h: self.h, png: base64_encode(&png) }.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Raster {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let f = RasterFile::deserialize(d)?;
        let bytes = base64_decode(&f.png).ok_or_else(|| serde::de::Error::custom("mapa de bits dañado (base64)"))?;
        let pm = Pixmap::decode_png(&bytes).map_err(|e| serde::de::Error::custom(format!("mapa de bits dañado (PNG): {e}")))?;
        if pm.width() > MAX_SIDE || pm.height() > MAX_SIDE {
            return Err(serde::de::Error::custom("mapa de bits demasiado grande"));
        }
        Ok(Raster::from_pixmap(pm))
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut val = [255u8; 256];
    for (i, c) in B64.iter().enumerate() {
        val[*c as usize] = i as u8;
    }
    let bytes: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for c in bytes.chunks(4) {
        let pad = c.iter().rev().take_while(|b| **b == b'=').count();
        if pad > 2 {
            return None;
        }
        let mut n = 0u32;
        for (i, b) in c.iter().enumerate() {
            let v = if *b == b'=' && i >= 4 - pad { 0 } else { val[*b as usize] };
            if v == 255 {
                return None;
            }
            n = n << 6 | v as u32;
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        for n in 0..20usize {
            let data: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(base64_decode(&base64_encode(&data)).unwrap(), data);
        }
        assert!(base64_decode("abc").is_none());
        assert!(base64_decode("ab!d").is_none());
    }

    fn line(color: [u8; 4], width: f32) -> crate::model::Stroke {
        crate::model::Stroke::shape(crate::model::ShapeKind::Line, [4.0, 20.0], [60.0, 20.0], color, width, None)
    }

    #[test]
    fn stamping_a_stroke_paints_pixels_and_erasing_clears_them() {
        let mut r = Raster::new(64, 40);
        crate::render::stamp_stroke(&mut r, &line([10, 200, 30, 255], 6.0));
        assert!(!r.is_blank());
        assert_eq!(r.pick([30.0, 20.0]), Some([10, 200, 30, 255]));
        assert_eq!(r.pick([30.0, 2.0]), None);
        crate::render::erase_segment(&mut r, [30.0, 20.0], [30.0, 20.0], 12.0);
        assert_eq!(r.pick([30.0, 20.0]), None);
        assert!(r.pick([8.0, 20.0]).is_some());
    }

    #[test]
    fn drawing_with_raster_roundtrips_and_old_drawings_still_load() {
        let mut r = Raster::new(64, 40);
        crate::render::stamp_stroke(&mut r, &line([200, 20, 20, 255], 4.0));
        let d = crate::model::Drawing { strokes: vec![], raster: Some(std::sync::Arc::new(r)) };
        assert!(!d.is_empty());
        let back: crate::model::Drawing = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back.pick([30.0, 20.0]), Some([200, 20, 20, 255]));
        // Un dibujo guardado antes de existir los mapas de bits no trae el campo `raster`.
        let old: crate::model::Drawing = serde_json::from_str(r#"{"strokes":[]}"#).unwrap();
        assert!(old.raster.is_none() && old.is_empty());
    }

    #[test]
    fn bitmap_layers_export_their_pixels() {
        let mut p = crate::model::Project::demo();
        p.layers.clear();
        let mut l = crate::model::Layer::new("Píxeles");
        l.kind = crate::model::LayerKind::Bitmap;
        let mut r = Raster::new(p.width, p.height);
        crate::render::stamp_stroke(&mut r, &crate::model::Stroke::shape(crate::model::ShapeKind::Line, [100.0, 100.0], [300.0, 100.0], [255, 0, 0, 255], 20.0, None));
        l.keys.insert(0, std::sync::Arc::new(crate::model::Drawing { strokes: vec![], raster: Some(std::sync::Arc::new(r)) }));
        p.layers.push(l);
        let pm = crate::render::render_frame_opts(&p, 0, 1.0, true).unwrap();
        let px = pm.pixel(200, 100).unwrap();
        assert!(px.red() > 200 && px.alpha() == 255, "el píxel rojo debe salir en la exportación");
        assert_eq!(pm.pixel(200, 400).unwrap().alpha(), 0);
    }

    #[test]
    fn raster_png_roundtrip_keeps_pixels() {
        let mut r = Raster::new(32, 16);
        assert!(r.is_blank());
        r.edit(|pm| pm.fill(tiny_skia::Color::from_rgba8(200, 40, 20, 255)));
        assert!(!r.is_blank());
        let json = serde_json::to_string(&r).unwrap();
        let back: Raster = serde_json::from_str(&json).unwrap();
        assert_eq!((back.w, back.h), (32, 16));
        assert_eq!(back.data, r.data);
        assert_eq!(back.pick([3.0, 3.0]), Some([200, 40, 20, 255]));
        assert_eq!(back.pick([99.0, 3.0]), None);
    }
}
