//! Selección rectangular de píxeles en capas de mapa de bits: elegir con un arrastre (o Ctrl+A),
//! mover, copiar al portapapeles y borrar.

use super::*;
use std::sync::Arc;
use tiny_skia::Pixmap;

/// Arrastre de una zona: la capa sin la zona, la zona levantada y dónde empezó.
pub(super) struct RDrag {
    base: crate::raster::Raster,
    float: Pixmap,
    org: [i32; 2],
    start: [f32; 2],
}

fn region(r: &crate::raster::Raster, b: [i32; 4]) -> Option<Pixmap> {
    let (w, h) = ((b[2] - b[0]) as u32, (b[3] - b[1]) as u32);
    let mut pm = Pixmap::new(w, h)?;
    let d = pm.data_mut();
    for y in 0..h {
        let si = (((b[1] as u32 + y) * r.w + b[0] as u32) * 4) as usize;
        let di = (y * w * 4) as usize;
        d[di..di + (w * 4) as usize].copy_from_slice(r.data.get(si..si + (w * 4) as usize)?);
    }
    Some(pm)
}

fn clear(r: &mut crate::raster::Raster, b: [i32; 4]) {
    r.edit(|pm| {
        let w = pm.width();
        let d = pm.data_mut();
        for y in b[1]..b[3] {
            let s = ((y as u32 * w + b[0] as u32) * 4) as usize;
            d[s..s + (b[2] - b[0]) as usize * 4].fill(0);
        }
    });
}

impl LeonApp {
    /// Zona elegida de la capa de mapa de bits actual, recortada a la escena.
    pub(super) fn rsel_rect(&self) -> Option<[i32; 4]> {
        let (name, b) = self.rsel.as_ref()?;
        let b = *b;
        let (w, h) = (self.project.width as i32, self.project.height as i32);
        let b = [b[0].max(0), b[1].max(0), b[2].min(w), b[3].min(h)];
        (*name == self.layer().name && self.layer().kind == LayerKind::Bitmap && b[0] < b[2] && b[1] < b[3]).then_some(b)
    }

    pub(super) fn rsel_all(&mut self) {
        self.rsel = Some((self.layer().name.clone(), [0, 0, self.project.width as i32, self.project.height as i32]));
        self.say("Lienzo entero elegido: Ctrl+C copia, Supr borra, arrastra para desplazar");
    }

    fn raster_mut(&mut self) -> Option<&mut crate::raster::Raster> {
        let f = self.frame;
        self.project.layers[self.cl].drawing_at_mut(f)?.raster.as_mut().map(Arc::make_mut)
    }

    /// Pulsar con la Selección en una capa de mapa de bits: arrastrar la zona o empezar otra.
    pub(super) fn rsel_begin(&mut self, p: [f32; 2]) -> Gesture {
        if let Some(b) = self.rsel_rect() {
            if p[0] >= b[0] as f32 && p[0] < b[2] as f32 && p[1] >= b[1] as f32 && p[1] < b[3] as f32 && !self.layer().locked {
                let lifted = self.raster_mut().and_then(|r| {
                    let float = region(r, b)?;
                    let mut base = r.clone();
                    clear(&mut base, b);
                    Some((base, float))
                });
                if let Some((base, float)) = lifted {
                    self.rdrag = Some(RDrag { base, float, org: [b[0], b[1]], start: p });
                    return Gesture::Move { last: p };
                }
            }
        }
        Gesture::Marquee { start: p, cur: p, add: false }
    }

    /// Sigue el arrastre de la zona levantada.
    pub(super) fn rsel_move(&mut self, p: [f32; 2]) {
        let Some(d) = self.rdrag.take() else { return };
        let nx = d.org[0] + (p[0] - d.start[0]).round() as i32;
        let ny = d.org[1] + (p[1] - d.start[1]).round() as i32;
        let (w, h) = (d.float.width() as i32, d.float.height() as i32);
        let paint = tiny_skia::PixmapPaint::default();
        if let Some(r) = self.raster_mut() {
            r.edit(|pm| {
                pm.data_mut().copy_from_slice(&d.base.data);
                pm.draw_pixmap(nx, ny, d.float.as_ref(), &paint, tiny_skia::Transform::identity(), None);
            });
            self.changed = true;
        }
        self.rsel = Some((self.layer().name.clone(), [nx, ny, nx + w, ny + h]));
        self.rdrag = Some(d);
    }

    pub(super) fn rsel_finish(&mut self, start: [f32; 2], cur: [f32; 2]) {
        let (w, h) = (self.project.width as i32, self.project.height as i32);
        let b = [
            start[0].min(cur[0]).floor().max(0.0) as i32,
            start[1].min(cur[1]).floor().max(0.0) as i32,
            (start[0].max(cur[0]).ceil() as i32).min(w),
            (start[1].max(cur[1]).ceil() as i32).min(h),
        ];
        if (b[2] - b[0]).max(b[3] - b[1]) as f32 * self.view_scale() < 3.0 || b[0] >= b[2] || b[1] >= b[3] {
            self.rsel = None;
            return;
        }
        self.rsel = Some((self.layer().name.clone(), b));
        self.say(format!("Zona elegida {}×{}: Ctrl+C copia, Supr borra, arrastra para desplazar", b[2] - b[0], b[3] - b[1]));
    }

    pub(super) fn rsel_delete(&mut self) -> bool {
        let Some(b) = self.rsel_rect() else { return false };
        if self.locked() {
            return true;
        }
        self.checkpoint();
        if let Some(r) = self.raster_mut() {
            clear(r, b);
        }
        self.say("Zona borrada");
        true
    }

    pub(super) fn rsel_copy(&mut self) -> bool {
        let Some(b) = self.rsel_rect() else { return false };
        let Some(pm) = self.layer().drawing_at(self.frame).and_then(|d| d.raster.as_ref()).and_then(|r| region(r, b)) else {
            self.say("No hay píxeles que copiar");
            return true;
        };
        let mut bytes = Vec::with_capacity(pm.data().len());
        for p in pm.data().chunks_exact(4) {
            let a = p[3] as u32;
            if a == 0 {
                bytes.extend([0, 0, 0, 0]);
            } else {
                bytes.extend([0, 1, 2].map(|c| ((p[c] as u32 * 255 + a / 2) / a).min(255) as u8));
                bytes.push(p[3]);
            }
        }
        let img = arboard::ImageData { width: pm.width() as usize, height: pm.height() as usize, bytes: bytes.into() };
        match arboard::Clipboard::new().and_then(|mut c| c.set_image(img)) {
            Ok(()) => self.say(format!("Zona copiada al portapapeles ({}×{})", pm.width(), pm.height())),
            Err(_) => self.say("No se pudo copiar al portapapeles"),
        }
        true
    }

    /// Recuadro de la zona elegida en el visor.
    pub(super) fn paint_rsel(&self, p: &egui::Painter, map: &dyn Fn([f32; 2]) -> Pos2) {
        let Some(b) = self.rsel.as_ref().filter(|(n, _)| *n == self.layer().name && self.layer().kind == LayerKind::Bitmap).map(|(_, b)| *b) else { return };
        let c = [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]].map(|q| map([q[0] as f32, q[1] as f32]));
        p.add(Shape::convex_polygon(c.to_vec(), Color32::from_rgba_unmultiplied(61, 180, 255, 25), EStroke::NONE));
        for i in 0..4 {
            p.extend(Shape::dashed_line(&[c[i], c[(i + 1) % 4]], stk(1.0, ACCENT), 5.0, 4.0));
        }
    }
}
