//! Vista 3D: composición por planos en profundidad (BG, MG, FG), cámara y navegación,
//! como en el prototipo v0.3 (proyección en perspectiva u ortográfica, rejilla de suelo,
//! rieles de la mesa multiplano, cámara con su encuadre y gizmo de ejes).

use super::canvas::paint_stroke;
use super::*;
use crate::model::{plane_name, plane_z, CAM_DIST};

pub(super) const V3_NAMES: [&str; 8] = ["Cámara", "Usuario", "Superior", "Lateral derecha", "Frontal", "Posterior", "Lateral izquierda", "Inferior"];
const AX_RED: Color32 = Color32::from_rgb(0xe0, 0x55, 0x3a);
const AX_GREEN: Color32 = Color32::from_rgb(0x7f, 0xb0, 0x69);
const AX_BLUE: Color32 = Color32::from_rgb(0x4a, 0x8f, 0xd8);

/// Gizmo del ítem elegido: origen en pantalla y punta de cada eje (X, Y = profundidad, Z = altura).
pub(super) struct Giz {
    pub o: Pos2,
    pub ax: [Pos2; 3],
}

pub(super) struct GizDrag {
    pub axis: usize,
    pub start: Pos2,
    /// Vector en pantalla del eje al empezar a arrastrar.
    pub dir: Vec2,
    pub xk: XKey,
    pub depth: f32,
}

struct Cam {
    x: f32,
    y: f32,
    z: f32,
}

/// Cámara virtual de la vista 3D (prototipo: `proj`).
struct Proj {
    cam_view: bool,
    cs: Cam,
    yaw: f32,
    pit: f32,
    ortho: bool,
    pw: f32,
    ph: f32,
    cx: f32,
    cy: f32,
    k: f32,
}

impl Proj {
    /// Punto (x, y de la hoja y z de profundidad) → (pantalla, escala, profundidad en la vista).
    fn p(&self, x: f32, y: f32, z: f32) -> (Pos2, f32, f32) {
        let d = if self.cam_view { -self.cs.z } else { 5600.0 };
        let (ox, oy) = if self.cam_view { (self.cs.x, self.cs.y) } else { (0.0, 0.0) };
        let (xx, yy) = (x - self.pw / 2.0 - ox, y - self.ph / 2.0 - oy);
        let (yaw, pit) = if self.cam_view { (0.0, 0.0) } else { (self.yaw.to_radians(), self.pit.to_radians()) };
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pit.sin_cos();
        let x1 = xx * cy + z * sy;
        let z1 = -xx * sy + z * cy;
        let y2 = yy * cp - z1 * sp;
        let z2 = yy * sp + z1 * cp;
        let mut s = if self.ortho && !self.cam_view { 0.85 } else { d / (d + z2).max(40.0) };
        if self.cam_view {
            s *= CAM_DIST / d;
        }
        (Pos2::new(self.cx + x1 * s * self.k, self.cy + y2 * s * self.k), s, z2)
    }
}

impl LeonApp {
    fn cam_state(&self) -> Cam {
        for l in &self.project.layers {
            if l.kind == LayerKind::Camera && l.visible {
                let t = l.transform_at(self.frame);
                return Cam { x: t.x, y: t.y, z: l.depth };
            }
        }
        Cam { x: 0.0, y: 0.0, z: -CAM_DIST }
    }

    fn proj3(&self, rect: Rect, n_w: f32) -> Proj {
        let (pw, ph) = (self.project.width as f32, self.project.height as f32);
        let cam_view = self.v3 == 0;
        let fit = ((rect.width() - n_w - 80.0) / pw).min((rect.height() - 110.0) / ph).max(0.02) * self.zoom3 * if cam_view { 1.0 } else { 0.42 };
        Proj {
            cam_view,
            cs: self.cam_state(),
            yaw: self.yaw,
            pit: self.pitch,
            ortho: self.ortho,
            pw,
            ph,
            cx: rect.min.x + (rect.width() - n_w) / 2.0 + self.pan3.x,
            cy: rect.min.y + rect.height() / 2.0 + self.pan3.y,
            k: fit,
        }
    }

    /// Cambia el modo de vista 3D (0 cámara, 1 usuario, 2 superior, 3 lateral, 4 frontal, 5 posterior, 6 izquierda, 7 inferior).
    pub(super) fn set_v3(&mut self, v: u8) {
        self.v3 = v;
        let (yaw, pitch) = match v {
            1 => (-32.0, 16.0),
            2 => (0.0, 85.0),
            3 => (-90.0, 0.0),
            4 => (0.0, 0.0),
            5 => (180.0, 0.0),
            6 => (90.0, 0.0),
            7 => (0.0, -85.0),
            _ => (self.yaw, self.pitch),
        };
        self.yaw = yaw;
        self.pitch = pitch;
    }

    pub(super) fn toggle_view3d(&mut self) {
        self.view3d = !self.view3d;
        if self.view3d {
            self.say("Vista 3D: clic central o Alt+arrastrar para orbitar, rueda para acercar");
        }
    }

    /// Teclado de la vista 3D (el teclado numérico de Blender; egui no distingue las teclas numéricas del teclado normal).
    pub(super) fn keys_3d(&mut self, ctx: &egui::Context) {
        if !self.view3d {
            return;
        }
        let k = |m: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(m, key));
        let (none, alt) = (Modifiers::NONE, Modifiers::ALT);
        if k(none, Key::Num1) {
            self.set_v3(4);
        } else if k(alt, Key::Num1) {
            self.set_v3(5);
        } else if k(none, Key::Num3) {
            self.set_v3(3);
        } else if k(alt, Key::Num3) {
            self.set_v3(6);
        } else if k(none, Key::Num7) {
            self.set_v3(2);
        } else if k(alt, Key::Num7) {
            self.set_v3(7);
        } else if k(none, Key::Num0) {
            self.set_v3(0);
        } else if k(none, Key::Num5) {
            self.ortho = !self.ortho;
            self.say(if self.ortho { "Vista ortográfica" } else { "Vista en perspectiva" });
        }
        for (key, dy, dp) in [(Key::Num4, 15.0, 0.0), (Key::Num6, -15.0, 0.0), (Key::Num8, 0.0, -15.0), (Key::Num2, 0.0, 15.0)] {
            if k(none, key) {
                self.yaw += dy;
                self.pitch = (self.pitch + dp).clamp(-85.0, 85.0);
                self.v3 = 1;
            }
        }
        if k(none, Key::Plus) || k(none, Key::Equals) {
            self.zoom3 = (self.zoom3 * 1.15).clamp(0.1, 4.0);
        }
        if k(none, Key::Minus) {
            self.zoom3 = (self.zoom3 / 1.15).clamp(0.1, 4.0);
        }
        if k(none, Key::Period) {
            self.pan3 = Vec2::ZERO;
            self.zoom3 = 1.0;
        }
    }

    /// Cambia el plano (BG → MG → FG) o activa el 3D de la capa `i`, como la etiqueta de plano del prototipo.
    pub(super) fn cycle_plane(&mut self, i: usize) {
        if self.project.layers[i].kind == LayerKind::Camera || self.project.layers[i].locked {
            return;
        }
        self.checkpoint();
        let l = &mut self.project.layers[i];
        if !l.is3d {
            l.is3d = true;
        } else {
            l.plane = (l.plane + 1) % 3;
        }
        l.depth = plane_z(l.plane);
    }

    /// Mueve la cámara en X en el fotograma actual (control «Cámara X»).
    fn set_camera_x(&mut self, x: f32) {
        if let Some(i) = self.project.layers.iter().position(|l| l.kind == LayerKind::Camera && l.visible) {
            let old = self.project.layers[i].transform_at(self.frame);
            let mut t = old;
            t.x = x;
            self.project.layers[i].tracks.set_changed(self.frame, &t, &old);
        }
    }

    // ---------- Entrada ----------

    fn giz_hit(&self, p: Pos2) -> Option<usize> {
        let g = self.giz.as_ref()?;
        let mut best = None;
        let mut bd = 14.0;
        for (i, q) in g.ax.iter().enumerate() {
            let d = *q - g.o;
            let l2 = d.length_sq().max(1.0);
            let t = (((p - g.o).dot(d)) / l2).clamp(0.0, 1.0);
            let dist = (g.o + d * t - p).length();
            if dist < bd && t > 0.25 {
                bd = dist;
                best = Some(i);
            }
        }
        best
    }

    fn input_3d(&mut self, ui: &egui::Ui, resp: &egui::Response) {
        let ctx = ui.ctx();
        if resp.hovered() {
            let sc = ctx.input(|i| i.raw_scroll_delta.y);
            if sc != 0.0 {
                self.zoom3 = (self.zoom3 * 1.0015f32.powf(sc)).clamp(0.1, 4.0);
            }
        }
        let primary = ctx.input(|i| i.pointer.primary_down());
        // Arrastrar un eje del gizmo mueve el ítem elegido.
        if resp.drag_started_by(egui::PointerButton::Primary) {
            if let Some(p) = resp.interact_pointer_pos() {
                if let Some(axis) = self.giz_hit(p) {
                    if !self.locked() {
                        self.checkpoint();
                        let g = self.giz.as_ref().unwrap();
                        self.giz_drag = Some(GizDrag {
                            axis,
                            start: p,
                            dir: g.ax[axis] - g.o,
                            xk: self.layer().transform_at(self.frame),
                            depth: self.layer().depth,
                        });
                    }
                }
            }
        }
        if let Some(gd) = &self.giz_drag {
            if primary {
                if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                    let d = p - gd.start;
                    let t = d.dot(gd.dir) / gd.dir.length_sq().max(1.0);
                    let mut xk = gd.xk;
                    let mut depth = gd.depth;
                    match gd.axis {
                        0 => xk.x += 150.0 * t,
                        1 => depth += 360.0 * t,
                        _ => xk.y -= 150.0 * t,
                    }
                    let f = self.frame;
                    let cl = self.cl;
                    let l = &mut self.project.layers[cl];
                    l.tracks.set_changed(f, &xk, &gd.xk);
                    if gd.axis == 1 {
                        l.depth = depth;
                    }
                }
            } else {
                self.giz_drag = None;
            }
            return;
        }
        let space = !Self::typing(ctx) && ctx.input(|i| i.key_down(Key::Space));
        let alt = ctx.input(|i| i.modifiers.alt);
        let pan = (resp.dragged_by(egui::PointerButton::Primary) && (self.tool == Tool::Hand || space)) || (resp.dragged_by(egui::PointerButton::Secondary));
        if pan {
            self.pan3 += resp.drag_delta();
            if space {
                self.space_panned = true;
            }
        } else if resp.dragged_by(egui::PointerButton::Middle) || resp.dragged_by(egui::PointerButton::Primary) {
            let d = resp.drag_delta();
            let _ = alt;
            self.yaw += d.x * 0.35;
            self.pitch = (self.pitch + d.y * 0.35).clamp(-85.0, 85.0);
            self.v3 = 1;
        }
    }

    // ---------- Dibujo ----------

    pub(super) fn viewer_3d(&mut self, ui: &mut egui::Ui, rect: Rect, resp: egui::Response, n_w: f32) {
        self.input_3d(ui, &resp);
        self.paint_3d(ui, rect, n_w);
        self.viewport_hud(ui, rect);
        self.hud_3d(ui, rect);
        if resp.hovered() {
            let ctx = ui.ctx();
            if self.giz_drag.is_some() || resp.hover_pos().map_or(false, |p| self.giz_hit(p).is_some()) {
                ctx.set_cursor_icon(egui::CursorIcon::Grab);
            } else {
                ctx.set_cursor_icon(egui::CursorIcon::Move);
            }
        }
    }

    fn paint_3d(&mut self, ui: &mut egui::Ui, rect: Rect, n_w: f32) {
        let painter = ui.painter_at(rect);
        let v = self.proj3(rect, n_w);
        let f = self.frame;
        let (pw, ph) = (v.pw, v.ph);
        let g = (self.light_gray / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8;
        painter.rect_filled(rect, 0.0, Color32::from_gray(g));
        let light_bg = g > 140;
        let ink = if light_bg { Color32::from_rgba_unmultiplied(42, 36, 32, 190) } else { Color32::from_rgba_unmultiplied(242, 232, 218, 190) };

        // Rejilla de suelo
        if !v.cam_view {
            let mut gx = -1800.0;
            while gx <= 1800.0 {
                let (a, b) = (v.p(pw / 2.0 + gx, ph + 60.0, -2600.0).0, v.p(pw / 2.0 + gx, ph + 60.0, 1600.0).0);
                let col = if gx == 0.0 { Color32::from_rgba_unmultiplied(127, 176, 105, 140) } else { Color32::from_rgba_unmultiplied(128, 128, 128, 36) };
                painter.line_segment([a, b], stk(1.0, col));
                gx += 200.0;
            }
            let mut gz = -2600.0;
            while gz <= 1600.0 {
                let (a, b) = (v.p(pw / 2.0 - 1800.0, ph + 60.0, gz).0, v.p(pw / 2.0 + 1800.0, ph + 60.0, gz).0);
                let col = if gz == 0.0 { Color32::from_rgba_unmultiplied(224, 108, 90, 140) } else { Color32::from_rgba_unmultiplied(128, 128, 128, 36) };
                painter.line_segment([a, b], stk(1.0, col));
                gz += 200.0;
            }
        }

        // Capas visibles, de la más lejana a la más cercana
        let any_solo = self.project.layers.iter().any(|l| l.solo);
        let mut order: Vec<(usize, f32)> = (0..self.project.layers.len())
            .filter(|&i| {
                let l = &self.project.layers[i];
                l.shows_at(f, any_solo) && !matches!(l.kind, LayerKind::Camera | LayerKind::Light)
            })
            .map(|i| (i, v.p(pw / 2.0, ph / 2.0, self.project.layers[i].z()).2))
            .collect();
        order.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Rieles de la mesa multiplano
        if !v.cam_view && order.len() > 1 {
            let zs: Vec<f32> = order.iter().map(|(i, _)| self.project.layers[*i].z()).collect();
            let zmin = zs.iter().cloned().fold(f32::MAX, f32::min);
            let zmax = zs.iter().cloned().fold(f32::MIN, f32::max);
            for (qx, qy) in [(-60.0, -60.0), (pw + 60.0, -60.0), (pw + 60.0, ph + 60.0), (-60.0, ph + 60.0)] {
                painter.line_segment([v.p(qx, qy, zmin - 60.0).0, v.p(qx, qy, zmax + 60.0).0], stk(1.5, Color32::from_rgba_unmultiplied(197, 143, 94, 90)));
            }
        }

        let op = (self.view_op / 100.0).clamp(0.1, 1.0);
        // Desde la cámara solo se ve lo que cae dentro del encuadre.
        let frame_rect = Rect::from_two_pos(v.p(0.0, 0.0, 0.0).0, v.p(pw, ph, 0.0).0);
        let layers_p = if v.cam_view { painter.with_clip_rect(frame_rect.intersect(rect)) } else { painter.clone() };
        let farthest = order.first().map(|o| o.0);
        let painter_all = painter;
        let painter = layers_p;
        for (i, _) in &order {
            let l = &self.project.layers[*i];
            let sel = *i == self.cl;
            let z = l.z();
            let ls = if l.is3d { (CAM_DIST + z) / CAM_DIST } else { 1.0 };
            let t = l.transform_at(f);
            let c = (pw / 2.0 + t.x, ph / 2.0 + t.y);
            if l.kind == LayerKind::Null {
                let q = v.p(c.0, c.1, z).0;
                let col = if sel { ACCENT } else { Color32::from_rgb(0xb7, 0xa2, 0xe6) };
                let r = Rect::from_center_size(q, Vec2::splat(20.0));
                painter.add(Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], stk(1.2, col), 3.0, 3.0));
                continue;
            }
            let corners: Vec<Pos2> = [(-30.0, -30.0), (pw + 30.0, -30.0), (pw + 30.0, ph + 30.0), (-30.0, ph + 30.0)]
                .iter()
                .map(|(qx, qy)| v.p(pw / 2.0 + (qx - pw / 2.0) * ls, ph / 2.0 + (qy - ph / 2.0) * ls, z).0)
                .collect();
            if l.kind == LayerKind::Solid {
                painter.add(Shape::convex_polygon(corners.clone(), with_alpha(c32(l.solid), l.opacity * t.o * op), EStroke::NONE));
            } else if l.is3d && l.plane == 0 && Some(*i) == farthest {
                painter.add(Shape::convex_polygon(corners.clone(), with_alpha(c32(self.project.background), 0.5), EStroke::NONE));
            } else if !v.cam_view {
                let fill = if sel { Color32::from_rgba_unmultiplied(253, 103, 36, 18) } else { Color32::from_rgba_unmultiplied(220, 235, 245, 13) };
                painter.add(Shape::convex_polygon(corners.clone(), fill, EStroke::NONE));
            }
            if !v.cam_view {
                let mut ring = corners.clone();
                ring.push(corners[0]);
                if sel {
                    painter.add(Shape::line(ring.clone(), stk(1.3, ACCENT)));
                } else {
                    painter.add(Shape::dashed_line(&ring, stk(1.3, Color32::from_rgba_unmultiplied(200, 215, 225, 115)), 6.0, 4.0));
                }
                let sh: Vec<Pos2> = ring.iter().map(|q| *q + Vec2::new(2.0, 3.0)).collect();
                painter.add(Shape::line(sh, stk(1.0, Color32::from_rgba_unmultiplied(200, 215, 225, 38))));
            }
            // Dibujo de la capa en su plano
            if let Some(d) = l.drawing_at(f) {
                let map = |p: [f32; 2]| {
                    let q = t.apply(pw, ph, p);
                    v.p(c.0 + (q[0] - c.0) * ls, c.1 + (q[1] - c.1) * ls, z).0
                };
                let sc = v.p(c.0, c.1, z).1 * v.k * ls * t.s.abs();
                let alpha = l.opacity * t.o * op;
                super::canvas::paint_raster(&painter, d, &map, alpha, 0);
                for s in &d.strokes {
                    paint_stroke(&painter, s, &map, sc, None, alpha);
                }
            }
            if !v.cam_view {
                let tint = match (l.is3d, l.plane) {
                    (true, 0) => Color32::from_rgb(0x8f, 0xb3, 0xcf),
                    (true, 2) => Color32::from_rgb(0xa8, 0xcf, 0x8f),
                    _ => Color32::from_rgb(0xf0, 0xb0, 0x80),
                };
                let tag = if l.is3d { format!("{}  {}  Y {:.0}", plane_name(l.plane), l.name, z) } else { format!("2D  {}", l.name) };
                painter.text(corners[0] + Vec2::new(4.0, -6.0), Align2::LEFT_BOTTOM, tag, semi(10.0), tint);
            }
        }

        let painter = painter_all;
        if v.cam_view {
            painter.rect_stroke(frame_rect, 0.0, stk(1.0, Color32::from_black_alpha(160)), EStrokeKind::Outside);
        }
        // Cámaras y luces como objetos en la escena
        if !v.cam_view {
            for (i, l) in self.project.layers.iter().enumerate() {
                if !l.visible {
                    continue;
                }
                let sel = i == self.cl;
                let t = l.transform_at(f);
                let (cxp, cyp) = (pw / 2.0 + t.x, ph / 2.0 + t.y);
                if l.kind == LayerKind::Camera {
                    let col = if sel { ACCENT } else if light_bg { Color32::from_rgb(0x1a, 0x1a, 0x1a) } else { Color32::from_rgb(0xd8, 0xcf, 0xc2) };
                    let ap = v.p(cxp, cyp, l.depth).0;
                    let dz = l.depth * 0.72;
                    let (hw, hh) = (pw * 0.13, ph * 0.13);
                    let b: Vec<Pos2> = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].iter().map(|(dx, dy)| v.p(cxp + dx, cyp + dy, dz).0).collect();
                    let st = stk(if sel { 1.8 } else { 1.4 }, col);
                    for q in &b {
                        painter.line_segment([ap, *q], st);
                    }
                    painter.add(Shape::closed_line(b.clone(), st));
                    let t0 = v.p(cxp - hw * 0.5, cyp - hh - 12.0, dz).0;
                    let t1 = v.p(cxp + hw * 0.5, cyp - hh - 12.0, dz).0;
                    let t2 = v.p(cxp, cyp - hh - 90.0, dz).0;
                    painter.add(Shape::convex_polygon(vec![t0, t1, t2], col, EStroke::NONE));
                    painter.text(ap + Vec2::new(6.0, 14.0), Align2::LEFT_CENTER, &l.name, FontId::proportional(10.0), ink);
                    // Encuadre de la cámara en el plano medio
                    let fr: Vec<Pos2> = [(0.0, 0.0), (pw, 0.0), (pw, ph), (0.0, ph), (0.0, 0.0)].iter().map(|(qx, qy)| v.p(qx + t.x, qy + t.y, 0.0).0).collect();
                    painter.add(Shape::dashed_line(&fr, stk(1.0, Color32::from_rgba_unmultiplied(128, 128, 128, 180)), 2.0, 4.0));
                }
                if l.kind == LayerKind::Light {
                    let q = v.p(cxp, cyp, l.depth).0;
                    let col = if sel { ACCENT } else { Color32::from_rgb(0xf2, 0xd2, 0x7a) };
                    painter.circle_stroke(q, 8.0, stk(1.5, col));
                    for a in 0..8 {
                        let an = a as f32 * std::f32::consts::FRAC_PI_4;
                        let d = Vec2::new(an.cos(), an.sin());
                        painter.line_segment([q + d * 12.0, q + d * 18.0], stk(1.5, col));
                    }
                }
            }
        }

        // Gizmo del ítem elegido (X rojo, Y verde = profundidad, Z azul = altura)
        self.giz = None;
        let l = &self.project.layers[self.cl];
        if (l.is3d || l.kind == LayerKind::Camera) && !(v.cam_view && l.kind == LayerKind::Camera) {
            let t = l.transform_at(f);
            let base = (pw / 2.0 + t.x, ph / 2.0 + t.y, l.z());
            let len = 150.0;
            let o = v.p(base.0, base.1, base.2).0;
            let ends = [(AX_RED, [len, 0.0, 0.0]), (AX_GREEN, [0.0, 0.0, len * 2.4]), (AX_BLUE, [0.0, -len, 0.0])];
            let mut ax = [Pos2::ZERO; 3];
            for (n, (col, w)) in ends.iter().enumerate() {
                let q = v.p(base.0 + w[0], base.1 + w[1], base.2 + w[2]).0;
                ax[n] = q;
                painter.line_segment([o, q], stk(2.5, *col));
                let d = (q - o).normalized();
                let n2 = Vec2::new(-d.y, d.x);
                painter.add(Shape::convex_polygon(vec![q + d * 9.0, q + n2 * 5.0, q - n2 * 5.0], *col, EStroke::NONE));
            }
            painter.circle_filled(o, 4.0, Color32::WHITE);
            self.giz = Some(Giz { o, ax });
        }

        // Rótulo de la vista
        let l = &self.project.layers[self.cl];
        let proj_name = if v.cam_view { "" } else if self.ortho { " ortográfica" } else { " perspectiva" };
        painter.text(rect.left_bottom() + Vec2::new(12.0, -50.0), Align2::LEFT_BOTTOM, format!("{}{}   |   {}", V3_NAMES[self.v3 as usize], proj_name, l.name), FontId::proportional(11.0), ink);

        // Gizmo de navegación (arriba a la derecha)
        let gc = Pos2::new(rect.max.x - n_w - 46.0, rect.min.y + 86.0);
        painter.circle_filled(gc, 34.0, Color32::from_rgba_unmultiplied(20, 19, 18, 140));
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        for (name, col, a) in [("X", AX_RED, [60.0, 0.0, 0.0]), ("Y", AX_GREEN, [0.0, 0.0, 60.0]), ("Z", AX_BLUE, [0.0, -60.0, 0.0])] {
            let (x1, z1) = (a[0] * cy + a[2] * sy, -a[0] * sy + a[2] * cy);
            let y2 = a[1] * cp - z1 * sp;
            let mut d = Vec2::new(x1, y2);
            d = if d.length() > 0.5 { d.normalized() * 24.0 } else { Vec2::ZERO };
            if v.cam_view {
                d = match name {
                    "X" => Vec2::new(24.0, 0.0),
                    "Z" => Vec2::new(0.0, -24.0),
                    _ => Vec2::ZERO,
                };
            }
            painter.line_segment([gc, gc + d], stk(2.0, col));
            painter.circle_filled(gc + d, 7.0, col);
            painter.text(gc + d, Align2::CENTER_CENTER, name, semi(9.0), Color32::from_rgb(0x14, 0x13, 0x12));
        }
    }

    /// Controles inferiores de la vista 3D: vistas, cámara X y ayuda.
    fn hud_3d(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let band = Rect::from_min_size(Pos2::new(rect.min.x + 8.0, rect.max.y - 38.0), Vec2::new(620.0, 30.0));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(band).layout(egui::Layout::left_to_right(egui::Align::Center)));
        c.spacing_mut().item_spacing.x = 6.0;
        let views: [(&str, u8); 4] = [("Cámara", 0), ("Personalizada", 1), ("Superior", 2), ("Lateral", 3)];
        let mut set = None;
        hud::pill_frame().show(&mut c, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (name, id) in views {
                let on = self.v3 == id;
                let b = egui::Button::new(RichText::new(name).size(12.0).color(if on { ACCENT_INK } else { MU }))
                    .fill(if on { ACCENT } else { Color32::TRANSPARENT })
                    .corner_radius(10.0);
                if ui.add(b).clicked() {
                    set = Some(id);
                }
            }
        });
        if let Some(id) = set {
            self.set_v3(id);
        }
        let cam = self.project.layers.iter().position(|l| l.kind == LayerKind::Camera && l.visible);
        hud::pill_frame().show(&mut c, |ui| {
            ui.label(RichText::new("Cámara X").size(12.0).color(MU));
            let mut x = cam.map_or(0.0, |i| self.project.layers[i].transform_at(self.frame).x);
            ui.spacing_mut().slider_width = 90.0;
            let r = ui.add_enabled(cam.is_some(), egui::Slider::new(&mut x, -400.0..=400.0).show_value(false));
            if r.drag_started() || (r.changed() && !r.dragged()) {
                self.checkpoint();
            }
            if r.changed() {
                self.set_camera_x(x);
            }
            if cam.is_none() {
                r.on_disabled_hover_text("Agrega una capa Cámara con Mayús+A");
            }
        });
        hud::pill_frame().show(&mut c, |ui| {
            ui.label(RichText::new("Clic central: orbitar").size(12.0).color(MU));
        });
    }
}
