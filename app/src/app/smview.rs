//! La capa Stop Motion en el visor (Issue #60): qué foto toca en cada cuadro del proyecto, carga de las fotos en segundo plano
//! (proxies de 1280 px), dibujo con la transformación y la opacidad de la capa y papel cebolla. La exportación usa la misma
//! lógica de cuadros (`Sequence::item_for_project_frame`), así que lo que se ve es lo que se exporta.

use super::*;
use crate::stopmotion::{Key, SmRef, SourceFrame};
use std::path::PathBuf;

/// Fotos que se mantienen como textura a la vez (cada proxy de 1280×720 ocupa unos 3,7 MB).
const KEEP_TEXTURES: usize = 24;
/// Cuántos cuadros del proyecto por delante se precargan al reproducir.
const PREFETCH_FRAMES: u32 = 24;

/// Lo necesario para pedir o dibujar la foto de un cuadro.
struct Want {
    key: Key,
    dir: PathBuf,
    source: SourceFrame,
}

impl LeonApp {
    /// La foto que se ve en el cuadro `f` del proyecto, si hay una (y su posición está visible).
    fn sm_want(&self, r: &SmRef, f: u32) -> Option<Want> {
        let folder = self.project.sm.folder(r, self.project.base_dir.as_deref())?;
        let idx = folder.seq.item_for_project_frame(r.start, self.project.fps, f)?;
        let item = &folder.seq.items[idx];
        if !item.visible {
            return None;
        }
        let source = folder.seq.source(&item.source)?.clone();
        Some(Want { key: (folder.seq.id.clone(), source.id.clone()), dir: folder.dir.clone(), source })
    }

    /// Antes de dibujar el visor: recoge las fotos que terminaron de cargar y pide las que hacen falta (cuadro actual,
    /// papel cebolla y, si se reproduce, los cuadros siguientes). Nunca espera: si una foto aún no llegó, se dibuja cuando llegue.
    pub(super) fn sm_prepare(&mut self, ctx: &egui::Context) {
        for (key, res) in self.sm_loader.poll() {
            match res {
                Ok(d) => {
                    let img = egui::ColorImage::from_rgba_unmultiplied([d.w as usize, d.h as usize], &d.rgba);
                    let tex = ctx.load_texture(format!("sm_{}_{}", key.0, key.1), img, egui::TextureOptions::LINEAR);
                    self.sm_tex.insert(key, (tex, self.frame_no));
                }
                Err(_) => {
                    self.sm_failed.insert(key);
                }
            }
        }
        let layers: Vec<(usize, SmRef)> = self.project.layers.iter().enumerate().filter(|(_, l)| l.kind == LayerKind::StopMotion && l.visible).filter_map(|(i, l)| l.stopmotion.clone().map(|r| (i, r))).collect();
        if layers.is_empty() {
            return;
        }
        let (f, len) = (self.frame, self.project.length);
        for (li, r) in layers {
            let mut frames = vec![f];
            if self.onion && li == self.cl {
                frames.extend((1..=self.onion_before).filter_map(|d| f.checked_sub(d)));
                frames.extend((1..=self.onion_after).map(|d| f + d).filter(|g| *g < len));
            }
            if self.playing {
                frames.extend((1..=PREFETCH_FRAMES).map(|k| f + k).filter(|g| *g < len));
            }
            for g in frames {
                let Some(w) = self.sm_want(&r, g) else { continue };
                if let Some((_, used)) = self.sm_tex.get_mut(&w.key) {
                    *used = self.frame_no;
                } else if !self.sm_failed.contains(&w.key) {
                    self.sm_loader.request(w.key, w.dir, w.source);
                }
            }
        }
        // Se sueltan las texturas que hace más tiempo que no se usan.
        while self.sm_tex.len() > KEEP_TEXTURES {
            let Some(oldest) = self.sm_tex.iter().min_by_key(|(_, (_, used))| *used).map(|(k, _)| k.clone()) else { break };
            self.sm_tex.remove(&oldest);
        }
        if self.sm_loader.pending() > 0 {
            ctx.request_repaint();
        }
    }

    /// Dibuja la foto de la capa en el cuadro `f` (y su papel cebolla) dentro del visor. La foto se ajusta al cuadro del
    /// proyecto sin deformarse y sigue la posición, el giro, la escala y la opacidad de la capa.
    pub(super) fn paint_stopmotion(&self, painter: &egui::Painter, li: usize, layer: &Layer, origin: Pos2, z: f32, f: u32, alpha: f32, live: bool, op: f32) {
        let Some(r) = &layer.stopmotion else { return };
        let (pw, ph) = (self.project.width as f32, self.project.height as f32);
        let quad = |fr: u32, w: &Want, color: Color32| {
            let Some((tex, _)) = self.sm_tex.get(&w.key) else { return };
            let (sw, sh) = (w.source.width.max(1) as f32, w.source.height.max(1) as f32);
            let fit = (pw / sw).min(ph / sh);
            let (rw, rh) = (sw * fit, sh * fit);
            let (x0, y0) = ((pw - rw) / 2.0, (ph - rh) / 2.0);
            let t = self.project.effective_at(li, fr);
            let map = |p: [f32; 2]| {
                let q = t.apply(pw, ph, p);
                Pos2::new(origin.x + q[0] * z, origin.y + q[1] * z)
            };
            let mut mesh = egui::Mesh::with_texture(tex.id());
            for (p, uv) in [([x0, y0], [0.0, 0.0]), ([x0 + rw, y0], [1.0, 0.0]), ([x0 + rw, y0 + rh], [1.0, 1.0]), ([x0, y0 + rh], [0.0, 1.0])] {
                mesh.vertices.push(egui::epaint::Vertex { pos: map(p), uv: Pos2::new(uv[0], uv[1]), color });
            }
            mesh.indices.extend([0, 1, 2, 0, 2, 3]);
            painter.add(Shape::mesh(mesh));
        };
        let current = self.sm_want(r, f);
        // La foto de este cuadro va debajo; el papel cebolla va ENCIMA, translúcido, porque una foto ocupa todo el cuadro
        // y, por debajo, no se vería. Las fotos vecinas llevan un toque del color de «antes» (ámbar) o «después» (azul).
        if let Some(w) = &current {
            quad(f, w, Color32::from_white_alpha((alpha.clamp(0.0, 1.0) * 255.0).round() as u8));
        }
        if live && self.onion && li == self.cl {
            let mut shown: Vec<Key> = current.iter().map(|w| w.key.clone()).collect();
            let mix = |tint: Color32| {
                let m = |a: u8| (a as f32 * 0.4 + 255.0 * 0.6).round() as u8;
                Color32::from_rgb(m(tint.r()), m(tint.g()), m(tint.b()))
            };
            for (dir, n, tint) in [(-1i64, self.onion_before, ONION_BEFORE), (1, self.onion_after, ONION_AFTER)] {
                for d in (1..=n).rev() {
                    let g = f as i64 + dir * d as i64;
                    if g < 0 || g >= self.project.length as i64 {
                        continue;
                    }
                    let Some(w) = self.sm_want(r, g as u32) else { continue };
                    if shown.contains(&w.key) {
                        continue;
                    }
                    shown.push(w.key.clone());
                    quad(g as u32, &w, with_alpha(mix(tint), self.onion_weight(d, n) * op));
                }
            }
        }
    }
}