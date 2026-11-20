//! Visor: la cámara sobre fondo oscuro, papel cebolla, guías y dibujo con el ratón.

use super::*;

impl LeonApp {
    pub(super) fn viewer(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.test_panic_frame != 0 && self.frame_no == self.test_panic_frame {
            panic!("fallo de prueba provocado a propósito");
        }
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        // Zona útil para centrar la hoja: sin la barra lateral N.
        let n_w = 236.0 * panel_t(&ctx, "nside", self.show_n);
        let vrect = Rect::from_min_max(rect.min, Pos2::new((rect.max.x - n_w).max(rect.min.x + 100.0), rect.max.y));
        if self.view3d {
            self.viewer_3d(ui, rect, resp, n_w);
            return;
        }
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        if self.need_fit && rect.width() > 50.0 {
            self.zoom = ((vrect.width() - 60.0) / w).min((vrect.height() - 90.0) / h).clamp(0.05, 16.0);
            self.pan = Vec2::ZERO;
            self.need_fit = false;
        }

        // Zoom con la rueda, alrededor del cursor
        // Como el original (0x14047b850): un paso de la escalera de zoom por cada muesca de la rueda,
        // alrededor del cursor.
        if resp.hovered() {
            self.wheel_acc += ctx.input(|i| i.raw_scroll_delta.y);
            if let Some(hp) = resp.hover_pos() {
                while self.wheel_acc.abs() >= WHEEL_NOTCH {
                    let dir = if self.wheel_acc > 0.0 { 1 } else { -1 };
                    let target = zoom_ladder_step(self.zoom, dir);
                    self.zoom_at(vrect, hp, target / self.zoom);
                    self.wheel_acc -= WHEEL_NOTCH * dir as f32;
                }
            }
        } else {
            self.wheel_acc = 0.0;
        }

        let space = !Self::typing(&ctx) && ctx.input(|i| i.key_down(Key::Space));
        let middle = ctx.input(|i| i.pointer.middle_down());
        let panning = self.tool == Tool::Hand || space || middle;
        if panning && (resp.dragged_by(egui::PointerButton::Primary) || resp.dragged_by(egui::PointerButton::Middle)) {
            self.pan += resp.drag_delta();
            if space {
                self.space_panned = true;
            }
        }
        if self.tool == Tool::Zoom && !panning {
            if let Some(hp) = resp.interact_pointer_pos() {
                let out = ctx.input(|i| i.modifiers.alt);
                if resp.clicked() {
                    let target = zoom_ladder_step(self.zoom, if out { -1 } else { 1 });
                    self.zoom_at(vrect, hp, target / self.zoom);
                } else if resp.secondary_clicked() {
                    let target = zoom_ladder_step(self.zoom, -1);
                    self.zoom_at(vrect, hp, target / self.zoom);
                }
            }
        }

        let z = self.zoom;
        let origin = vrect.center() + Vec2::new(0.0, 14.0) + self.pan - Vec2::new(w, h) * z * 0.5;
        let to_canvas = |p: Pos2| [(p.x - origin.x) / z, (p.y - origin.y) / z];
        self.cursor_canvas = resp.hover_pos().map(to_canvas);

        let primary = ctx.input(|i| i.pointer.primary_down());
        let (shift, command) = ctx.input(|i| (i.modifiers.shift, i.modifiers.command));
        if !panning && self.tool != Tool::Zoom && resp.is_pointer_button_down_on() && primary {
            if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                let cp = to_canvas(p);
                if self.gesture.is_none() {
                    if ctx.input(|i| i.pointer.primary_pressed()) {
                        self.playing = false;
                        self.begin_gesture(cp, shift, command);
                    }
                } else {
                    self.continue_gesture(cp, shift);
                }
            }
        } else if self.gesture.is_some() {
            self.finish_gesture();
        }

        // ---- Dibujo ----
        self.sync_brush_tex(&ctx);
        let painter = ui.painter_at(rect);
        // Mesa de luz: el gris del fondo va de 0 (negro) a 100 (blanco).
        let g = (self.light_gray / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8;
        painter.rect_filled(rect, 0.0, Color32::from_rgb(g, g.saturating_sub(1), g.saturating_sub(2)));
        let cam = Rect::from_min_size(origin, Vec2::new(w, h) * z);
        // Luz de la mesa: resplandor del color de la temperatura sobre el fondo.
        let glow_c = with_alpha(light_color(self.light_temp), self.light_amt / 100.0 * 0.55);
        let rr = w.max(h) * z * 0.85;
        glow(&painter, cam.center(), rr * 0.1, rr, glow_c);
        painter.rect_filled(cam.translate(Vec2::new(0.0, 6.0)).expand(5.0), 10.0, Color32::from_black_alpha(110));
        let scene = ui.painter_at(cam.intersect(rect));
        self.paint_scene(&scene, origin, z, self.frame, !self.playing, true);
        painter.rect_stroke(cam, 0.0, stk(1.0, Color32::from_black_alpha(160)), EStrokeKind::Outside);
        if self.pegbar {
            // Barra de registro: tres perforaciones bajo la hoja, como en la mesa de animación.
            for (dx, w2) in [(-0.25, 0.022), (0.0, 0.012), (0.25, 0.022)] {
                let c = Pos2::new(cam.center().x + cam.width() * dx, cam.max.y - cam.height() * 0.018);
                painter.rect_filled(Rect::from_center_size(c, Vec2::new(cam.width() * w2, cam.height() * 0.014)), 4.0, Color32::from_rgb(0x26, 0x24, 0x22));
            }
        }
        self.paint_guides(&scene, cam);
        self.paint_selection(&painter, origin, z);

        // Rótulo (como «Modo edición | Pelota | 1» del prototipo)
        let hud = format!(
            "{}  |  {}  |  {}{}",
            if self.obj_mode { "Modo objeto" } else { "Modo edición" },
            self.layer().name,
            self.frame + 1,
            if self.layer().locked { "  |  bloqueada" } else { "" }
        );
        painter.text(cam.left_top() + Vec2::new(0.0, -8.0), Align2::LEFT_BOTTOM, hud, FontId::proportional(11.5), MU);
        if self.playing {
            painter.text(rect.left_bottom() + Vec2::new(10.0, -10.0), Align2::LEFT_BOTTOM, "Reproduciendo…", FontId::proportional(12.0), ACCENT2);
        }
        self.viewport_hud(ui, rect);
        // Cursor de la herramienta
        if let Some(hp) = resp.hover_pos() {
            let rad = match self.tool {
                Tool::Brush => Some(self.brush_size * 0.5 * z),
                Tool::Vector => Some(self.pencil_size * 0.5 * z),
                Tool::Eraser => Some(self.eraser_size * 0.5 * z),
                _ => None,
            };
            if panning {
                ctx.set_cursor_icon(if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
            } else if let Some(rad) = rad {
                painter.circle_stroke(hp, rad.max(1.5), stk(2.5, Color32::from_black_alpha(140)));
                painter.circle_stroke(hp, rad.max(1.5), stk(1.0, Color32::WHITE));
                painter.circle_filled(hp, 1.2, Color32::WHITE);
                ctx.set_cursor_icon(egui::CursorIcon::None);
            } else {
                ctx.set_cursor_icon(match self.tool {
                    Tool::Shape | Tool::Picker | Tool::Fill => egui::CursorIcon::Crosshair,
                    Tool::Select | Tool::RotateObj => egui::CursorIcon::Move,
                    t if t.pending() => egui::CursorIcon::NotAllowed,
                    Tool::Zoom => egui::CursorIcon::ZoomIn,
                    _ => egui::CursorIcon::Default,
                });
            }
        }
    }

    fn zoom_at(&mut self, rect: Rect, at: Pos2, factor: f32) {
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        let rect = Rect::from_center_size(rect.center() + Vec2::new(0.0, 14.0), rect.size());
        let origin = rect.center() + self.pan - Vec2::new(w, h) * self.zoom * 0.5;
        let c = (at - origin) / self.zoom;
        let z2 = (self.zoom * factor).clamp(ZOOM_LADDER[0] / 100.0, ZOOM_LADDER[ZOOM_LADDER.len() - 1] / 100.0);
        let origin2 = at - c * z2;
        self.pan = origin2 - rect.center() + Vec2::new(w, h) * z2 * 0.5;
        self.zoom = z2;
    }

    /// Dibuja la escena del fotograma `f` con la esquina de la cámara en `origin` y escala `z`.
    /// Con `live`, añade el papel cebolla y el trazo que se está dibujando.
    /// `view`: es el visor (mesa de luz: cuadrícula de transparencia, luz sobre el papel y opacidad de la vista).
    pub(super) fn paint_scene(&self, painter: &egui::Painter, origin: Pos2, z: f32, f: u32, live: bool, view: bool) {
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        let cam = Rect::from_min_size(origin, Vec2::new(w, h) * z);
        let op = if view { (self.view_op / 100.0).clamp(0.1, 1.0) } else { 1.0 };
        if view && self.show_alpha {
            // Cuadrícula de transparencia: blanco con cuadros más oscuros según el contraste.
            painter.rect_filled(cam, 0.0, Color32::WHITE);
            let s = self.chk_size.max(2.0);
            let g = (255.0 * (1.0 - self.chk_con / 100.0)).round() as u8;
            let dark = Color32::from_gray(g);
            let mut y = cam.min.y;
            let mut row = 0;
            while y < cam.max.y {
                let mut x = cam.min.x + if row % 2 == 0 { 0.0 } else { s };
                while x < cam.max.x {
                    painter.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::splat(s)).intersect(cam), 0.0, dark);
                    x += 2.0 * s;
                }
                y += s;
                row += 1;
            }
        } else {
            painter.rect_filled(cam, 0.0, c32(self.project.background));
            if view {
                // La luz de la mesa se refleja en el papel.
                let a = self.light_amt / 100.0 * 0.45;
                glow(painter, cam.center(), 10.0, w * z * 0.7, with_alpha(light_color(self.light_temp), a));
            }
        }
        for (li, layer) in self.project.layers.iter().enumerate().rev() {
            if !layer.visible {
                continue;
            }
            let t = layer.transform_at(f);
            let map = move |p: [f32; 2]| {
                let q = t.apply(w, h, p);
                Pos2::new(origin.x + q[0] * z, origin.y + q[1] * z)
            };
            let zs = z * t.s.abs();
            if live && self.onion && li == self.cl {
                let cur = layer.key_at_or_before(f);
                let mut shown: Vec<u32> = cur.into_iter().collect();
                for (dir, n, tint) in [(-1i64, self.onion_before, ONION_BEFORE), (1, self.onion_after, ONION_AFTER)] {
                    for d in (1..=n).rev() {
                        let g = f as i64 + dir * d as i64;
                        if g < 0 || g >= self.project.length as i64 {
                            continue;
                        }
                        let Some(k) = layer.key_at_or_before(g as u32) else { continue };
                        if shown.contains(&k) {
                            continue;
                        }
                        shown.push(k);
                        let tg = layer.transform_at(g as u32);
                        let mapg = move |p: [f32; 2]| {
                            let q = tg.apply(w, h, p);
                            Pos2::new(origin.x + q[0] * z, origin.y + q[1] * z)
                        };
                        if let Some(dr) = layer.keys.get(&k) {
                            let wgt = self.onion_weight(d, n) * op;
                            paint_raster(painter, dr, &mapg, wgt, if dir < 0 { 1 } else { 2 });
                            paint_drawing(painter, dr, &mapg, z * tg.s.abs(), Some(tint), wgt);
                        }
                    }
                }
            }
            let alpha = layer.opacity * t.o * op;
            if layer.kind == LayerKind::Solid {
                let cam = Rect::from_min_size(origin, Vec2::new(w, h) * z);
                painter.rect_filled(cam, 0.0, with_alpha(c32(layer.solid), alpha));
            }
            if let Some(d) = layer.drawing_at(f) {
                paint_raster(painter, d, &map, alpha, 0);
                paint_drawing(painter, d, &map, zs, None, alpha);
            }
            if live && li == self.cl {
                if let (Some(ls), Some(tex)) = (&self.brush_live, &self.brush_tex) {
                    // Trazo del motor de pinceles en curso, con la opacidad del trazo.
                    paint_tex_quad(painter, tex.id(), ls.buf.w, ls.buf.h, &map, alpha * ls.spec.opacity);
                } else if let Some(Gesture::Paint { stroke, .. }) | Some(Gesture::Shape { stroke, .. }) = &self.gesture {
                    paint_stroke(painter, stroke, &map, zs, None, alpha);
                }
            }
        }
    }

    /// Guías del prototipo: áreas seguras de animación (90 %) y texto (80 %), tercios,
    /// diagonales, sección áurea (0,382 / 0,618) y cruz de centro.
    fn paint_guides(&self, p: &egui::Painter, cam: Rect) {
        let col = Color32::from_rgba_unmultiplied(0x5f, 0x8f, 0xb4, 190);
        let st = stk(1.0, col);
        if self.guides[0] {
            p.rect_stroke(Rect::from_center_size(cam.center(), cam.size() * 0.9), 0.0, st, EStrokeKind::Middle);
        }
        if self.guides[1] {
            p.rect_stroke(Rect::from_center_size(cam.center(), cam.size() * 0.8), 0.0, stk(1.0, with_alpha(ACCENT2, 0.7)), EStrokeKind::Middle);
        }
        if self.guides[2] {
            for k in [1.0 / 3.0, 2.0 / 3.0] {
                p.vline(cam.min.x + cam.width() * k, cam.y_range(), st);
                p.hline(cam.x_range(), cam.min.y + cam.height() * k, st);
            }
        }
        if self.guides[3] {
            p.line_segment([cam.left_top(), cam.right_bottom()], st);
            p.line_segment([cam.right_top(), cam.left_bottom()], st);
        }
        if self.guides[4] {
            for k in [0.382, 0.618] {
                p.vline(cam.min.x + cam.width() * k, cam.y_range(), stk(1.0, with_alpha(KEY_GOLD, 0.7)));
                p.hline(cam.x_range(), cam.min.y + cam.height() * k, stk(1.0, with_alpha(KEY_GOLD, 0.7)));
            }
        }
        if self.guides[5] {
            let c = cam.center();
            p.line_segment([c - Vec2::new(16.0, 0.0), c + Vec2::new(16.0, 0.0)], stk(1.5, col));
            p.line_segment([c - Vec2::new(0.0, 16.0), c + Vec2::new(0.0, 16.0)], stk(1.5, col));
        }
    }
}

fn mix_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> Color32 {
    let m = |i: usize| (a[i] as f32 * (1.0 - t) + b[i] as f32 * t).round() as u8;
    Color32::from_rgb(m(0), m(1), m(2))
}

/// Color de la luz según la temperatura (0 cálida … 50 neutra … 100 fría), como en el prototipo.
pub(super) fn light_color(temp: f32) -> Color32 {
    let t = (temp / 100.0).clamp(0.0, 1.0);
    if t < 0.5 {
        mix_rgb([0xff, 0xcf, 0x8f], [0xff, 0xf4, 0xe2], t * 2.0)
    } else {
        mix_rgb([0xff, 0xf4, 0xe2], [0xdf, 0xea, 0xff], (t - 0.5) * 2.0)
    }
}

/// Degradado radial: color pleno hasta `r0` y se desvanece hasta transparente en `r1`.
pub(super) fn glow(p: &egui::Painter, c: Pos2, r0: f32, r1: f32, col: Color32) {
    const N: u32 = 64;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, col);
    let dir = |i: u32| {
        let a = i as f32 / N as f32 * std::f32::consts::TAU;
        Vec2::new(a.cos(), a.sin())
    };
    for i in 0..N {
        mesh.colored_vertex(c + dir(i) * r0, col);
    }
    for i in 0..N {
        mesh.colored_vertex(c + dir(i) * r1, Color32::TRANSPARENT);
    }
    for i in 0..N {
        let j = (i + 1) % N;
        mesh.add_triangle(0, 1 + i, 1 + j);
        mesh.add_triangle(1 + i, 1 + N + i, 1 + N + j);
        mesh.add_triangle(1 + i, 1 + N + j, 1 + j);
    }
    p.add(Shape::mesh(mesh));
}

// ---------- Capas de mapa de bits en pantalla ----------

thread_local! {
    /// Texturas de los mapas de bits: (versión, tinte) → (textura, último pase en que se usó).
    static RASTER_TEX: std::cell::RefCell<std::collections::HashMap<(u64, u8), (egui::TextureHandle, u64)>> = Default::default();
}

/// Textura de un mapa de bits. `tint`: 0 sin tinte, 1 papel cebolla anterior, 2 posterior.
fn raster_tex_id(ctx: &egui::Context, r: &crate::raster::Raster, tint: u8) -> egui::TextureId {
    let now = ctx.cumulative_pass_nr();
    RASTER_TEX.with(|c| {
        let mut c = c.borrow_mut();
        let e = c.entry((r.ver, tint)).or_insert_with(|| {
            let size = [r.w as usize, r.h as usize];
            let img = if tint == 0 {
                egui::ColorImage::from_rgba_premultiplied(size, &r.data)
            } else {
                let t = if tint == 1 { ONION_BEFORE } else { ONION_AFTER };
                let mut px = Vec::with_capacity(r.data.len());
                for p in r.data.chunks_exact(4) {
                    let a = p[3] as u32;
                    px.extend_from_slice(&[(t.r() as u32 * a / 255) as u8, (t.g() as u32 * a / 255) as u8, (t.b() as u32 * a / 255) as u8, p[3]]);
                }
                egui::ColorImage::from_rgba_premultiplied(size, &px)
            };
            (ctx.load_texture("raster", img, egui::TextureOptions::LINEAR), now)
        });
        e.1 = now;
        e.0.id()
    })
}

/// Libera las texturas de mapas de bits que hace rato que no se muestran.
pub(super) fn prune_raster_tex(now: u64) {
    RASTER_TEX.with(|c| c.borrow_mut().retain(|_, v| now.saturating_sub(v.1) < 600));
}

/// Dibuja los píxeles de un dibujo con la transformación `map` (2D, 3D, miniaturas y papel cebolla).
pub(super) fn paint_raster(p: &egui::Painter, d: &Drawing, map: &dyn Fn([f32; 2]) -> Pos2, alpha: f32, tint: u8) {
    let Some(r) = &d.raster else { return };
    if r.is_blank() || alpha <= 0.0 {
        return;
    }
    let id = raster_tex_id(p.ctx(), r, tint);
    paint_tex_quad(p, id, r.w, r.h, map, alpha);
}

/// Una textura del tamaño de la escena (w×h px) dibujada con la transformación `map`.
pub(super) fn paint_tex_quad(p: &egui::Painter, id: egui::TextureId, w: u32, h: u32, map: &dyn Fn([f32; 2]) -> Pos2, alpha: f32) {
    let (w, h) = (w as f32, h as f32);
    let col = Color32::from_white_alpha((alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
    let mut mesh = egui::Mesh::with_texture(id);
    for (pt, uv) in [([0.0, 0.0], Pos2::new(0.0, 0.0)), ([w, 0.0], Pos2::new(1.0, 0.0)), ([w, h], Pos2::new(1.0, 1.0)), ([0.0, h], Pos2::new(0.0, 1.0))] {
        mesh.vertices.push(egui::epaint::Vertex { pos: map(pt), uv, color: col });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(Shape::mesh(mesh));
}

impl LeonApp {
    /// Mantiene al día la textura del trazo del motor de pinceles (solo sube la zona nueva).
    pub(super) fn sync_brush_tex(&mut self, ctx: &egui::Context) {
        let Some(ls) = &mut self.brush_live else {
            self.brush_tex = None;
            return;
        };
        let (w, h) = (ls.buf.w as usize, ls.buf.h as usize);
        if self.brush_tex.is_none() {
            let blank = egui::ColorImage::new([w, h], Color32::TRANSPARENT);
            self.brush_tex = Some(ctx.load_texture("brush_live", blank, egui::TextureOptions::LINEAR));
            ls.mark_all_dirty();
        }
        if let (Some(b), Some(tex)) = (ls.take_dirty(), self.brush_tex.as_mut()) {
            let (bw, bh) = ((b[2] - b[0]) as usize, (b[3] - b[1]) as usize);
            // Con los bordes húmedos aplicados, para que lo que se ve al dibujar sea lo que queda.
            let px = ls.region_rgba(b);
            let img = egui::ColorImage::from_rgba_premultiplied([bw, bh], &px);
            tex.set_partial([b[0] as usize, b[1] as usize], img, egui::TextureOptions::LINEAR);
        }
    }
}

/// Miniatura de un dibujo (en coordenadas de la escena w×h) dentro de `rect`.
pub(super) fn paint_thumb(painter: &egui::Painter, rect: Rect, d: &Drawing, w: f32, h: f32, bg: Color32) {
    painter.rect_filled(rect, 1.0, bg);
    let z = (rect.width() / w).min(rect.height() / h);
    let o = rect.center() - Vec2::new(w, h) * z * 0.5;
    let clip = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let map = |p: [f32; 2]| Pos2::new(o.x + p[0] * z, o.y + p[1] * z);
    paint_raster(&clip, d, &map, 1.0, 0);
    paint_drawing(&clip, d, &map, z, None, 1.0);
}

// ---------- Dibujo de trazos en pantalla ----------

pub(super) fn paint_drawing(
    p: &egui::Painter,
    d: &Drawing,
    to_screen: &dyn Fn([f32; 2]) -> Pos2,
    z: f32,
    tint: Option<Color32>,
    alpha: f32,
) {
    for s in &d.strokes {
        paint_stroke(p, s, to_screen, z, tint, alpha);
    }
}

pub(super) fn paint_stroke(
    p: &egui::Painter,
    s: &Stroke,
    to_screen: &dyn Fn([f32; 2]) -> Pos2,
    z: f32,
    tint: Option<Color32>,
    alpha: f32,
) {
    let col = with_alpha(tint.unwrap_or(c32(s.color)), alpha);
    let width = (s.width * z).max(0.6);
    let pts: Vec<Pos2> = s.points.iter().map(|q| to_screen(*q)).collect();
    if pts.is_empty() {
        return;
    }
    if !s.is_plain() {
        // Trazo vectorial de grosor variable (lápiz con presión, afilado, extremos o uniones propios).
        if let (Some(f), true) = (s.fill, pts.len() >= 3) {
            let fc = match tint {
                Some(t) => with_alpha(t, alpha * 0.35),
                None => with_alpha(c32(f), alpha),
            };
            fill_polygon(p, &pts, fc);
        }
        let mut mesh = egui::Mesh::default();
        for poly in s.pieces() {
            let base = mesh.vertices.len() as u32;
            for v in &poly {
                mesh.colored_vertex(to_screen(*v), col);
            }
            for i in 1..poly.len().saturating_sub(1) as u32 {
                mesh.add_triangle(base, base + i, base + i + 1);
            }
        }
        if !mesh.is_empty() {
            p.add(Shape::mesh(mesh));
        }
        return;
    }
    if pts.len() == 1 {
        p.circle_filled(pts[0], width * 0.5, col);
        return;
    }
    if let Some(f) = s.fill {
        let fc = match tint {
            Some(t) => with_alpha(t, alpha * 0.35),
            None => with_alpha(c32(f), alpha),
        };
        if pts.len() >= 3 {
            fill_polygon(p, &pts, fc);
        }
    }
    let opaque = col.a() == 255;
    if s.closed {
        p.add(Shape::closed_line(pts, stk(width, col)));
    } else {
        if width > 2.5 {
            if opaque {
                for q in &pts[1..pts.len() - 1] {
                    p.circle_filled(*q, width * 0.5, col);
                }
            }
            p.circle_filled(pts[0], width * 0.5, col);
            p.circle_filled(*pts.last().unwrap(), width * 0.5, col);
        }
        p.add(Shape::line(pts, stk(width, col)));
    }
}

/// Relleno de un polígono cualquiera (también cóncavo) por franjas horizontales.
fn fill_polygon(p: &egui::Painter, pts: &[Pos2], col: Color32) {
    let convex = is_convex(pts);
    if convex {
        p.add(Shape::convex_polygon(pts.to_vec(), col, EStroke::NONE));
        return;
    }
    let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |a, q| (a.0.min(q.y), a.1.max(q.y)));
    let step = ((y1 - y0) / 400.0).max(1.0);
    let mut y = y0 + step * 0.5;
    let mut mesh = egui::Mesh::default();
    while y < y1 {
        let mut xs: Vec<f32> = Vec::new();
        let n = pts.len();
        for i in 0..n {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            if (a.y > y) != (b.y > y) {
                xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks_exact(2) {
            mesh.add_colored_rect(Rect::from_min_max(Pos2::new(pair[0], y - step * 0.5), Pos2::new(pair[1], y + step * 0.5)), col);
        }
        y += step;
    }
    p.add(Shape::mesh(mesh));
}

fn is_convex(pts: &[Pos2]) -> bool {
    let n = pts.len();
    let mut sign = 0.0f32;
    for i in 0..n {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        let cross = (b - a).x * (c - b).y - (b - a).y * (c - b).x;
        if cross.abs() > 1e-3 {
            if sign == 0.0 {
                sign = cross.signum();
            } else if cross.signum() != sign {
                return false;
            }
        }
    }
    true
}
