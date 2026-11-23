//! Herramienta Selección: elegir trazos (clic o recuadro), moverlos, escalarlos, girarlos,
//! voltearlos, recolorearlos, copiarlos y borrarlos.

use super::*;
use std::sync::Arc;

/// Distancia en píxeles de pantalla para acertar en una esquina o el asa de giro.
const HANDLE_PX: f32 = 9.0;
/// Separación del asa de giro sobre la caja, en píxeles de pantalla.
const ROT_PX: f32 = 26.0;

impl LeonApp {
    /// Olvida la selección si cambió la capa o el dibujo que se ve.
    pub(super) fn validate_pick(&mut self) {
        let owner = self.layer().key_at_or_before(self.frame).map(|k| (self.cl, k));
        if owner != self.picked_owner {
            self.picked.clear();
            self.picked_owner = owner;
        }
        let n = self.layer().drawing_at(self.frame).map_or(0, |d| d.strokes.len());
        self.picked.retain(|i| *i < n);
        self.picked.sort_unstable();
        self.picked.dedup();
    }

    /// Caja de lo elegido, en coordenadas del dibujo: [x0, y0, x1, y1].
    pub(super) fn picked_bounds(&self) -> Option<[f32; 4]> {
        let d = self.layer().drawing_at(self.frame)?;
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for i in &self.picked {
            let s = d.strokes.get(*i)?;
            let r = s.bounds();
            let pad = s.width * 0.5;
            b = [b[0].min(r[0] - pad), b[1].min(r[1] - pad), b[2].max(r[2] + pad), b[3].max(r[3] + pad)];
        }
        (b[0] <= b[2]).then_some(b)
    }

    /// Escala de la vista para la capa actual (píxeles de pantalla por unidad del dibujo).
    pub(super) fn view_scale(&self) -> f32 {
        (self.zoom * self.project.effective_at(self.cl, self.frame).s.abs()).max(0.01)
    }

    /// Qué hacer al pulsar con la Selección en el punto `p` (del dibujo).
    pub(super) fn select_begin(&mut self, p: [f32; 2], shift: bool, command: bool) -> Gesture {
        self.validate_pick();
        // Con una curva de nodos elegida, un clic en un nodo o tirador lo edita antes que mover el trazo.
        if let Some(g) = self.node_begin(p, command) {
            return g;
        }
        self.sel_node = None;
        let tol = HANDLE_PX / self.view_scale();
        if let Some(b) = self.picked_bounds() {
            let c = [(b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5];
            let rot = [c[0], b[1] - ROT_PX / self.view_scale()];
            let near = |q: [f32; 2]| (q[0] - p[0]).abs() <= tol && (q[1] - p[1]).abs() <= tol;
            if near(rot) {
                return Gesture::SelRotate { center: c, last: (p[1] - c[1]).atan2(p[0] - c[0]) };
            }
            let corners = [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]];
            for (i, q) in corners.iter().enumerate() {
                if near(*q) {
                    return Gesture::SelScale { anchor: corners[(i + 2) % 4], last: p, keep: !shift };
                }
            }
            if p[0] >= b[0] - tol && p[0] <= b[2] + tol && p[1] >= b[1] - tol && p[1] <= b[3] + tol && !shift {
                return Gesture::Move { last: p };
            }
        }
        // ¿Trazo bajo el cursor? (el de más arriba)
        let hit = self.layer().drawing_at(self.frame).and_then(|d| {
            d.strokes.iter().enumerate().rev().find(|(_, s)| s.hits_outline(p, tol * 0.6) || s.hits_fill(p)).map(|(i, _)| i)
        });
        match hit {
            Some(i) => {
                if shift {
                    if let Some(pos) = self.picked.iter().position(|x| *x == i) {
                        self.picked.remove(pos);
                    } else {
                        self.picked.push(i);
                    }
                } else if !self.picked.contains(&i) {
                    self.picked = vec![i];
                }
                Gesture::Move { last: p }
            }
            None => Gesture::Marquee { start: p, cur: p, add: shift },
        }
    }

    /// Aplica `f` a cada trazo elegido (o a todos si no hay selección).
    pub(super) fn transform_picked(&mut self, f: impl Fn(&mut Stroke)) -> bool {
        let fr = self.frame;
        let picked = self.picked.clone();
        let Some(d) = self.project.layers[self.cl].drawing_at_mut(fr) else { return false };
        if picked.is_empty() {
            d.strokes.iter_mut().for_each(&f);
        } else {
            for i in picked {
                if let Some(s) = d.strokes.get_mut(i) {
                    f(s);
                }
            }
        }
        true
    }

    pub(super) fn select_continue(&mut self, g: Gesture, p: [f32; 2]) -> Gesture {
        match g {
            Gesture::Marquee { start, add, .. } => Gesture::Marquee { start, cur: p, add },
            Gesture::SelScale { anchor, last, keep } => {
                let (dx0, dy0) = (last[0] - anchor[0], last[1] - anchor[1]);
                let (dx1, dy1) = (p[0] - anchor[0], p[1] - anchor[1]);
                let (sx, sy) = if keep {
                    let l0 = (dx0 * dx0 + dy0 * dy0).sqrt();
                    let s = if l0 > 1e-3 { (dx1 * dx1 + dy1 * dy1).sqrt() / l0 } else { 1.0 };
                    (s, s)
                } else {
                    let r = |a: f32, b: f32| if a.abs() > 1e-3 { b / a } else { 1.0 };
                    (r(dx0, dx1), r(dy0, dy1))
                };
                if (sx - 1.0).abs() > 1e-5 || (sy - 1.0).abs() > 1e-5 {
                    if sx.abs() > 0.01 && sy.abs() > 0.01 && self.transform_picked(|s| s.scale_about(anchor, sx, sy)) {
                        self.changed = true;
                    }
                }
                Gesture::SelScale { anchor, last: p, keep }
            }
            Gesture::SelRotate { center, last } => {
                let a = (p[1] - center[1]).atan2(p[0] - center[0]);
                let d = (a - last).to_degrees();
                if d.abs() > 1e-4 && self.transform_picked(|s| s.rotate_about(center, d)) {
                    self.changed = true;
                }
                Gesture::SelRotate { center, last: a }
            }
            other => other,
        }
    }

    pub(super) fn select_finish(&mut self, start: [f32; 2], cur: [f32; 2], add: bool) {
        let r = [start[0].min(cur[0]), start[1].min(cur[1]), start[0].max(cur[0]), start[1].max(cur[1])];
        if (r[2] - r[0]).max(r[3] - r[1]) * self.view_scale() < 3.0 {
            if !add {
                self.picked.clear();
            }
            return;
        }
        let inside = |q: &[f32; 2]| q[0] >= r[0] && q[0] <= r[2] && q[1] >= r[1] && q[1] <= r[3];
        let found: Vec<usize> = self
            .layer()
            .drawing_at(self.frame)
            .map(|d| d.strokes.iter().enumerate().filter(|(_, s)| s.points.iter().any(inside)).map(|(i, _)| i).collect())
            .unwrap_or_default();
        if add {
            self.picked.extend(found);
        } else {
            self.picked = found;
        }
        self.validate_pick();
        let n = self.picked.len();
        self.say(if n == 0 { "Nada elegido".to_string() } else { format!("{n} trazos elegidos") });
    }

    // ---------- Órdenes de la barra de opciones ----------

    pub(super) fn select_all(&mut self) {
        let n = self.layer().drawing_at(self.frame).map_or(0, |d| d.strokes.len());
        self.validate_pick();
        self.picked = (0..n).collect();
        self.say(format!("{n} trazos elegidos"));
    }

    fn picked_op(&mut self, msg: &str, f: impl Fn(&mut Stroke, [f32; 2])) {
        if self.layer().locked {
            self.say("La capa está bloqueada");
            return;
        }
        self.validate_pick();
        let b = self.picked_bounds().or_else(|| {
            let d = self.layer().drawing_at(self.frame)?;
            let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for s in &d.strokes {
                let r = s.bounds();
                b = [b[0].min(r[0]), b[1].min(r[1]), b[2].max(r[2]), b[3].max(r[3])];
            }
            (b[0] <= b[2]).then_some(b)
        });
        let Some(b) = b else {
            self.say("No hay nada que cambiar en este dibujo");
            return;
        };
        let c = [(b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5];
        self.checkpoint();
        self.transform_picked(|s| f(s, c));
        self.say(msg);
    }

    pub(super) fn picked_flip(&mut self, horizontal: bool) {
        self.picked_op(if horizontal { "Volteado en horizontal" } else { "Volteado en vertical" }, move |s, c| {
            if horizontal {
                s.scale_about(c, -1.0, 1.0)
            } else {
                s.scale_about(c, 1.0, -1.0)
            }
        });
    }

    pub(super) fn picked_rotate90(&mut self) {
        self.picked_op("Girado 90°", |s, c| s.rotate_about(c, 90.0));
    }

    pub(super) fn picked_recolor(&mut self) {
        let (line, fill) = (rgba(self.color), rgba(self.fill_color));
        self.picked_op("Color aplicado", move |s, _| {
            s.color = line;
            if s.fill.is_some() {
                s.fill = Some(fill);
            }
        });
    }

    pub(super) fn picked_delete(&mut self) -> bool {
        self.validate_pick();
        if self.picked.is_empty() || self.layer().locked {
            return false;
        }
        self.checkpoint();
        let picked = std::mem::take(&mut self.picked);
        let fr = self.frame;
        if let Some(d) = self.project.layers[self.cl].drawing_at_mut(fr) {
            let mut i = 0;
            d.strokes.retain(|_| {
                let keep = !picked.contains(&i);
                i += 1;
                keep
            });
        }
        self.say(format!("{} trazos borrados", picked.len()));
        true
    }

    /// Copia lo elegido; devuelve false si no había selección.
    pub(super) fn picked_copy(&mut self) -> bool {
        self.validate_pick();
        if self.picked.is_empty() {
            return false;
        }
        let Some(d) = self.layer().drawing_at(self.frame) else { return false };
        let strokes = self.picked.iter().filter_map(|i| d.strokes.get(*i).cloned()).collect();
        self.clipboard = Some(Drawing { strokes, ..Default::default() });
        self.say(format!("{} trazos copiados", self.picked.len()));
        true
    }

    /// Con la Selección, pegar añade los trazos al dibujo actual y los deja elegidos.
    pub(super) fn picked_paste(&mut self) {
        let Some(clip) = self.clipboard.clone() else {
            self.say("Primero copia algo");
            return;
        };
        if self.layer().locked {
            self.say("La capa está bloqueada");
            return;
        }
        self.checkpoint();
        let f = self.frame;
        let layer = &mut self.project.layers[self.cl];
        if layer.drawing_at(f).is_none() {
            layer.keys.insert(f, Arc::new(Drawing::default()));
        }
        let d = layer.drawing_at_mut(f).unwrap();
        let start = d.strokes.len();
        d.strokes.extend(clip.strokes.iter().cloned());
        let end = d.strokes.len();
        self.picked_owner = None;
        self.validate_pick();
        self.picked = (start..end).collect();
        self.say("Pegado: arrástralo para colocarlo");
    }

    /// Dibuja la selección y el recuadro en el visor.
    pub(super) fn paint_selection(&self, p: &egui::Painter, origin: Pos2, z: f32) {
        if self.tool != Tool::Select || self.obj_mode {
            return;
        }
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        let t = self.project.effective_at(self.cl, self.frame);
        let map = |q: [f32; 2]| {
            let s = t.apply(w, h, q);
            Pos2::new(origin.x + s[0] * z, origin.y + s[1] * z)
        };
        let sel_col = Color32::from_rgb(0x3d, 0xb4, 0xff);
        if let Some(d) = self.layer().drawing_at(self.frame) {
            for i in &self.picked {
                if let Some(s) = d.strokes.get(*i) {
                    let pts: Vec<Pos2> = s.points.iter().map(|q| map(*q)).collect();
                    if s.closed {
                        p.add(Shape::closed_line(pts, stk(1.2, sel_col)));
                    } else {
                        p.add(Shape::line(pts, stk(1.2, sel_col)));
                    }
                }
            }
        }
        if let Some(b) = self.picked_bounds() {
            let corners = [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]].map(map);
            for i in 0..4 {
                dashed(p, corners[i], corners[(i + 1) % 4], ACCENT);
            }
            for c in corners {
                p.rect_filled(Rect::from_center_size(c, Vec2::splat(8.0)), 1.0, Color32::WHITE);
                p.rect_stroke(Rect::from_center_size(c, Vec2::splat(8.0)), 1.0, stk(1.0, ACCENT), EStrokeKind::Inside);
            }
            let top = map([(b[0] + b[2]) * 0.5, b[1]]);
            let rot = map([(b[0] + b[2]) * 0.5, b[1] - ROT_PX / self.view_scale()]);
            p.line_segment([top, rot], stk(1.0, ACCENT));
            p.circle_filled(rot, 5.0, Color32::WHITE);
            p.circle_stroke(rot, 5.0, stk(1.2, ACCENT));
        }
        if let Some(Gesture::Marquee { start, cur, .. }) = &self.gesture {
            let a = [start[0].min(cur[0]), start[1].min(cur[1])];
            let b = [start[0].max(cur[0]), start[1].max(cur[1])];
            let cs = [[a[0], a[1]], [b[0], a[1]], [b[0], b[1]], [a[0], b[1]]].map(map);
            p.add(Shape::convex_polygon(cs.to_vec(), Color32::from_rgba_unmultiplied(61, 180, 255, 25), EStroke::NONE));
            for i in 0..4 {
                dashed(p, cs[i], cs[(i + 1) % 4], sel_col);
            }
        }
    }
}

fn dashed(p: &egui::Painter, a: Pos2, b: Pos2, col: Color32) {
    p.extend(Shape::dashed_line(&[a, b], stk(1.0, col), 5.0, 4.0));
}
