//! Edición: fotogramas, capas, claves de animación, gestos sobre el lienzo, atajos y reproducción.

use super::*;
use std::sync::Arc;

impl LeonApp {
    // ---------- Fotogramas y capas ----------

    pub(super) fn locked(&mut self) -> bool {
        if self.layer().locked {
            self.say("La capa está bloqueada: desbloquéala con el candado del Xsheet");
            true
        } else {
            false
        }
    }

    pub(super) fn add_key(&mut self, copy: bool) {
        if self.locked() {
            return;
        }
        let f = self.frame;
        let layer = self.layer();
        let target = if layer.keys.contains_key(&f) { f + 1 } else { f };
        if layer.keys.contains_key(&target) {
            self.say(format!("Ya hay un dibujo en el fotograma {}", target + 1));
            self.frame = target.min(self.project.length - 1);
            return;
        }
        let d = if copy { layer.drawing_at(f).cloned().unwrap_or_default() } else { Drawing::default() };
        self.checkpoint();
        if target >= self.project.length {
            self.project.length = target + 1;
        }
        self.project.layers[self.cl].keys.insert(target, Arc::new(d));
        self.frame = target;
        self.say(if copy {
            format!("Dibujo duplicado en el fotograma {}", target + 1)
        } else {
            format!("Dibujo nuevo en el fotograma {}", target + 1)
        });
    }

    pub(super) fn delete_key(&mut self) {
        if self.locked() {
            return;
        }
        let f = self.frame;
        if self.layer().keys.contains_key(&f) {
            self.checkpoint();
            self.project.layers[self.cl].keys.remove(&f);
            self.say(format!("Dibujo del fotograma {} eliminado", f + 1));
        } else if let Some(k) = self.layer().key_at_or_before(f) {
            self.say(format!("Este fotograma muestra el dibujo del fotograma {}: ve allí para eliminarlo", k + 1));
        } else {
            self.say("No hay ningún dibujo aquí");
        }
    }

    pub(super) fn clear_drawing(&mut self) {
        if self.locked() {
            return;
        }
        let f = self.frame;
        if self.layer().drawing_at(f).map_or(true, |d| d.is_empty()) {
            self.say("El dibujo ya está vacío");
            return;
        }
        self.checkpoint();
        if let Some(d) = self.project.layers[self.cl].drawing_at_mut(f) {
            d.strokes.clear();
            d.raster = None;
        }
        self.say("Dibujo vaciado");
    }

    pub(super) fn insert_frame(&mut self) {
        if self.locked() {
            return;
        }
        self.checkpoint();
        let f = self.frame;
        self.project.layers[self.cl].insert_frame(f);
        self.say("Fotograma insertado: el dibujo dura un fotograma más");
    }

    pub(super) fn remove_frame(&mut self) {
        if self.locked() {
            return;
        }
        self.checkpoint();
        let f = self.frame;
        self.project.layers[self.cl].remove_frame(f);
        self.say("Fotograma quitado");
    }

    pub(super) fn copy_drawing(&mut self) {
        match self.layer().drawing_at(self.frame) {
            Some(d) => {
                self.clipboard = Some(d.clone());
                self.say("Dibujo copiado");
            }
            None => self.say("No hay dibujo que copiar"),
        }
    }

    pub(super) fn paste_drawing(&mut self) {
        let Some(d) = self.clipboard.clone() else {
            self.say("Primero copia un dibujo");
            return;
        };
        if self.locked() {
            return;
        }
        self.checkpoint();
        let f = self.frame;
        self.project.layers[self.cl].keys.insert(f, Arc::new(d));
        self.say(format!("Dibujo pegado en el fotograma {}", f + 1));
    }

    fn unique_layer_name(&self, base: &str) -> String {
        let mut n = self.project.layers.len() + 1;
        loop {
            let name = format!("{base} {n}");
            if !self.project.layers.iter().any(|l| l.name == name) {
                return name;
            }
            n += 1;
        }
    }

    pub(super) fn add_layer(&mut self) {
        self.checkpoint();
        let name = self.unique_layer_name("Capa");
        self.project.layers.insert(self.cl, Layer::new(name.clone()));
        self.say(format!("Capa «{name}» creada"));
    }

    /// Crea una capa del tipo elegido en el menú Mayús+A y la inserta sobre la actual.
    pub(super) fn add_layer_kind(&mut self, kind: LayerKind) {
        self.checkpoint();
        let name = self.unique_layer_name(kind.label());
        let mut l = Layer::new(name.clone());
        l.kind = kind;
        if kind == LayerKind::Solid {
            // Como el prototipo: color crema al 40 % de opacidad.
            l.opacity = 0.4;
        }
        match kind {
            LayerKind::Camera => {
                l.is3d = true;
                l.depth = -crate::model::CAM_DIST;
            }
            LayerKind::Light => {
                l.is3d = true;
                l.depth = -600.0;
                l.tracks.set(Prop::Position, 0, [0.0, -300.0], Interp::Linear);
            }
            _ => {}
        }
        self.project.layers.insert(self.cl, l);
        self.sel = None;
        if kind == LayerKind::Camera {
            self.say(format!("Cámara «{name}» creada: se ve y se mueve en la vista 3D (Ctrl+3)"));
        } else if kind == LayerKind::StopMotion {
            // Abre el panel N en la pestaña Ítem, donde se elige la carpeta de la secuencia.
            self.show_n = true;
            self.n_tab = 0;
            self.say(format!("Capa Stop Motion «{name}» creada: elige la carpeta de la secuencia en el panel N"));
        } else if kind.pending() {
            self.say(format!("Capa «{name}» ({}) creada. Su efecto en la escena queda pendiente.", kind.label()));
        } else {
            self.say(format!("Capa agregada: «{name}» ({})", kind.label()));
        }
    }

    pub(super) fn duplicate_layer(&mut self) {
        self.checkpoint();
        let mut l = self.layer().clone();
        l.name = format!("{} copia", l.name);
        l.uid = 0; // la copia es otra capa: se le dará su propio identificador si hace falta
        self.project.layers.insert(self.cl, l);
        self.say("Capa duplicada");
    }

    pub(super) fn delete_layer(&mut self) {
        if self.project.layers.len() <= 1 {
            self.say("El proyecto necesita al menos una capa");
            return;
        }
        self.checkpoint();
        self.project.release_children(self.cl);
        let l = self.project.layers.remove(self.cl);
        self.cl = self.cl.min(self.project.layers.len() - 1);
        self.sel = None;
        self.say(format!("Capa «{}» eliminada", l.name));
    }

    /// Mueve la capa `from` al sitio `to` (0 = la de más arriba).
    pub(super) fn move_layer_to(&mut self, from: usize, to: usize) {
        let n = self.project.layers.len();
        if from >= n || to >= n || from == to {
            return;
        }
        self.checkpoint();
        let l = self.project.layers.remove(from);
        self.project.layers.insert(to, l);
        self.cl = to;
        self.sel = None;
    }

    pub(super) fn move_layer(&mut self, up: bool) {
        let to = if up { self.cl.checked_sub(1) } else { Some(self.cl + 1) };
        if let Some(to) = to {
            self.move_layer_to(self.cl, to);
        }
    }

    // ---------- Claves de animación ----------

    /// Fija claves en el fotograma actual. Con `t`, solo en las propiedades que cambian respecto a lo que se ve ahora (así
    /// mover una capa anima solo la posición); sin él, en todas las propiedades con los valores de ahora.
    pub(super) fn set_xkey(&mut self, t: Option<XKey>) {
        if self.locked() {
            return;
        }
        let f = self.frame;
        let had = self.layer().tracks.has_key_at(f);
        let now = self.layer().transform_at(f);
        match t {
            Some(t) => self.project.layers[self.cl].tracks.set_changed(f, &t, &now),
            None => self.project.layers[self.cl].tracks.set_all(f, &now),
        }
        if !had {
            self.say(format!("Clave de animación en el fotograma {}", f + 1));
        }
    }

    /// Cronómetro de una propiedad: si está animada la deja sin animar; si no, la anima con una clave en el cuadro actual.
    pub(super) fn toggle_stopwatch(&mut self, p: Prop) {
        if self.locked() {
            return;
        }
        let f = self.frame;
        let now = self.layer().transform_at(f);
        let l = &mut self.project.layers[self.cl];
        if l.tracks.has(p) {
            l.tracks.clear_prop(p);
            self.say(format!("{}: sin animar", p.name()));
        } else {
            l.tracks.set(p, f, p.of(&now), now.i);
            self.say(format!("{}: animada desde el fotograma {}", p.name(), f + 1));
        }
    }

    pub(super) fn delete_xkey(&mut self) {
        let f = self.frame;
        if self.project.layers[self.cl].tracks.remove_at(f) {
            self.say("Clave borrada");
        } else {
            self.say("No hay clave en este fotograma");
        }
    }
    pub(super) fn jump_xkey(&mut self, next: bool) {
        let f = self.frame;
        let l = self.layer();
        let k = if next { l.tracks.next_frame(f) } else { l.tracks.prev_frame(f) };
        match k {
            Some(k) => {
                self.playing = false;
                self.frame = k.min(self.project.length - 1);
            }
            None => self.say("No hay más claves en esa dirección"),
        }
    }

    pub(super) fn mark_range(&mut self, start: bool) {
        let f = self.frame;
        let last = self.project.length - 1;
        let (a, b) = self.project.range.unwrap_or((0, last));
        self.project.range = Some(if start { (f, b.max(f)) } else { (a.min(f), f) });
        let (a, b) = self.project.range.unwrap();
        self.say(format!("Rango de reproducción: fotogramas {} a {}", a + 1, b + 1));
    }

    // ---------- Reproducción ----------

    #[allow(dead_code)]
    pub(super) fn set_frame(&mut self, f: i64) {
        let len = self.project.length as i64;
        self.frame = f.clamp(0, len - 1) as u32;
    }

    pub(super) fn step(&mut self, d: i64) {
        self.playing = false;
        let len = self.project.length as i64;
        self.frame = (self.frame as i64 + d).rem_euclid(len) as u32;
    }

    fn play_bounds(&self) -> (u32, u32) {
        let last = self.project.length - 1;
        match self.sm_play_range.or(self.project.range) {
            Some((a, b)) => (a.min(last), b.min(last)),
            None => (0, last),
        }
    }

    pub(super) fn toggle_play(&mut self) {
        self.playing = !self.playing;
        self.play_accum = 0.0;
        let (a, b) = self.play_bounds();
        if self.playing && (self.frame >= b || self.frame < a) {
            self.frame = a;
        }
    }

    /// Un paso de la escalera de zoom del original, centrado en la vista (SHORTCUT_ZOOM_ON_MOUSE = false).
    pub(super) fn zoom_step(&mut self, dir: i32) {
        let f = zoom_ladder_step(self.zoom, dir) / self.zoom;
        self.zoom_by(f);
    }

    pub(super) fn zoom_by(&mut self, f: f32) {
        let z = (self.zoom * f).clamp(ZOOM_LADDER[0] / 100.0, ZOOM_LADDER[ZOOM_LADDER.len() - 1] / 100.0);
        let real = z / self.zoom;
        self.zoom = z;
        self.pan *= real;
    }

    pub(super) fn advance_playback(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let dt = ctx.input(|i| i.stable_dt).min(0.1) as f64;
        self.play_accum += dt;
        let step = 1.0 / self.project.fps.max(1) as f64;
        let (a, b) = self.play_bounds();
        while self.play_accum >= step {
            self.play_accum -= step;
            if self.frame >= b {
                if self.loop_play {
                    self.frame = a;
                } else {
                    self.playing = false;
                    break;
                }
            } else {
                self.frame += 1;
            }
        }
        ctx.request_repaint();
    }

    // ---------- Gestos sobre el lienzo ----------

    /// Punto de la escena → punto dentro del dibujo de la capa actual (deshace su animación).
    fn local(&self, p: [f32; 2]) -> [f32; 2] {
        let t = self.project.effective_at(self.cl, self.frame);
        if t.is_identity() {
            p
        } else {
            t.invert(self.project.width as f32, self.project.height as f32, p)
        }
    }

    pub(super) fn begin_gesture(&mut self, scene: [f32; 2], shift: bool, command: bool) {
        if self.tool.pending() {
            self.say(format!("{}: pendiente.", self.tool.name()));
            return;
        }
        if self.tool == Tool::Picker {
            self.gesture = Some(Gesture::Pick);
            self.pick(scene);
            return;
        }
        if self.tool.is_view() {
            return;
        }
        if let Some(why) = self.tool.layer_block(self.layer().kind) {
            self.say(why);
            return;
        }
        let (locked, visible, base, shown) = {
            let l = self.layer();
            (l.locked, l.visible, l.transform_at(self.frame), l.key_at_or_before(self.frame))
        };
        if locked {
            self.say("La capa está bloqueada: desbloquéala con el candado del Xsheet");
            return;
        }
        if !visible {
            self.say("La capa está oculta: muéstrala con el ojo del Xsheet");
            return;
        }
        if (self.obj_mode && self.tool == Tool::Select) || self.tool == Tool::RotateObj {
            self.pre = Some(self.project.clone());
            self.changed = false;
            let mode = if self.tool == Tool::RotateObj || shift { 1 } else if command { 2 } else { 0 };
            self.gesture = Some(Gesture::Animate { start: scene, base, mode });
            return;
        }
        if let Some(k) = shown {
            if k != self.frame && matches!(self.tool, Tool::Brush | Tool::Vector | Tool::Shape) {
                self.say(format!("Estás dibujando en el dibujo del fotograma {}. Pulsa N si quieres un dibujo nuevo aquí.", k + 1));
            }
        }
        let p = self.local(scene);
        self.pre = Some(self.project.clone());
        self.changed = false;
        let mut color = rgba(self.color);
        color[3] = (color[3] as f32 * self.stroke_opacity).round() as u8;
        let g = match self.tool {
            Tool::Brush | Tool::Vector => {
                let width = if self.tool == Tool::Brush { self.brush_size } else { self.pencil_size };
                // El Lápiz vectorial guarda la presión de cada punto y su propio estilo en el trazo (edición no destructiva).
                let pencil = self.tool == Tool::Vector;
                let (pressure, style) = if pencil { (vec![self.cur_pen().pressure], Some(self.vstyle.clone().clamped())) } else { (Vec::new(), None) };
                Gesture::Paint {
                    stroke: Stroke { kind: ShapeKind::Free, points: vec![p], color, width, closed: false, fill: None, pressure, style, original: None, nodes: None },
                    smooth: p,
                    last_raw: p,
                }
            }
            Tool::Shape => {
                let kind = self.shape_kind;
                let fill = if self.fill_shapes { Some(rgba(self.fill_color)) } else { None };
                Gesture::Shape { kind, start: p, stroke: Stroke::shape(kind, p, p, color, self.shape_width, fill) }
            }
            Tool::Eraser => {
                self.erase_segment(p, p);
                Gesture::Erase { last: p }
            }
            Tool::Fill => {
                self.fill_at(p);
                self.finish_gesture();
                return;
            }
            Tool::Pen => match self.pen_begin(p) {
                Some(g) => g,
                None => {
                    self.pre = None;
                    return;
                }
            },
            Tool::Select => self.select_begin(p, shift, command),
            _ => {
                self.pre = None;
                return;
            }
        };
        // Pincel sobre una capa de mapa de bits: lo pinta el motor de pinceles (marcas con dureza,
        // espaciado, flujo, redondez y ángulo) en lugar de estampar el trazo vectorial.
        self.brush_live = None;
        self.brush_tex = None;
        if self.tool == Tool::Brush && self.layer().kind == LayerKind::Bitmap {
            let seed = (self.now * 1000.0) as u64 ^ self.frame_no.wrapping_mul(2_654_435_761);
            let spec = self.brush_spec([color[0], color[1], color[2]], self.brush_size, seed);
            let mut ls = crate::brush::LiveStroke::new(self.project.width, self.project.height, spec);
            ls.push_with(p, self.cur_pen());
            self.brush_live = Some(ls);
        }
        self.gesture = Some(g);
    }

    /// Lee la presión del lápiz de los eventos táctiles (egui no da inclinación). Sin lápiz, 1.
    pub(super) fn read_pen(&mut self, ctx: &egui::Context) {
        let mut pressure = self.pen_pressure;
        ctx.input(|i| {
            for e in &i.events {
                if let egui::Event::Touch { phase, force, .. } = e {
                    match phase {
                        egui::TouchPhase::Start | egui::TouchPhase::Move => {
                            if let Some(f) = force {
                                pressure = f.clamp(0.0, 1.0);
                            }
                        }
                        _ => pressure = 1.0,
                    }
                }
            }
        });
        self.pen_pressure = pressure;
    }

    /// Entrada del lápiz para el motor de pinceles (con el interruptor «Presión»).
    pub(super) fn cur_pen(&self) -> crate::brush::Pen {
        crate::brush::Pen { pressure: if self.pressure { self.pen_pressure } else { 1.0 }, tilt: 1.0 }
    }

    /// Termina el trazo del motor de pinceles: lo compone sobre la capa con la opacidad del trazo.
    fn commit_live(&mut self, ls: crate::brush::LiveStroke) {
        let f = self.frame;
        let (w, h) = (self.project.width, self.project.height);
        let layer = &mut self.project.layers[self.cl];
        if layer.drawing_at(f).is_none() {
            layer.keys.insert(f, Default::default());
        }
        if let Some(d) = layer.drawing_at_mut(f) {
            let r = d.raster.get_or_insert_with(|| Arc::new(crate::raster::Raster::new(w, h)));
            ls.finish_into(Arc::make_mut(r));
            self.changed = true;
        }
    }

    /// Relleno: la forma cerrada de más arriba que contiene el punto toma el color actual.
    fn fill_at(&mut self, p: [f32; 2]) {
        let f = self.frame;
        let color = rgba(self.color);
        if self.layer().kind == LayerKind::Bitmap {
            self.say("El relleno en capas de mapa de bits está pendiente: dibuja la forma con pincel o usa una capa vectorial");
            return;
        }
        let Some(d) = self.project.layers[self.cl].drawing_at_mut(f) else {
            self.say("No hay dibujo aquí para rellenar");
            return;
        };
        let hit = d.strokes.iter_mut().rev().find(|s| s.closed && s.points.len() >= 3 && crate::model::point_in_polygon(p, &s.points));
        match hit {
            Some(s) => {
                s.fill = Some(color);
                self.changed = true;
                self.say("Forma rellenada");
            }
            None => self.say("Haz clic dentro de una forma cerrada (rectángulo, elipse o trazo que vuelva a su inicio)"),
        }
    }

    fn erase_segment(&mut self, a: [f32; 2], b: [f32; 2]) {
        let r = self.eraser_size * 0.5;
        let f = self.frame;
        let size = self.eraser_size;
        let bitmap = self.layer().kind == LayerKind::Bitmap;
        let Some(d) = self.project.layers[self.cl].drawing_at_mut(f) else { return };
        if bitmap {
            // Mapa de bits: el borrador quita píxeles.
            if let Some(ra) = d.raster.as_mut() {
                crate::render::erase_segment(Arc::make_mut(ra), a, b, size);
                self.changed = true;
            }
            return;
        }
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        let n = (len / (r * 0.5).max(0.5)).ceil().max(1.0) as usize;
        let mut changed = false;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            if d.erase([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], r) {
                changed = true;
            }
        }
        if changed {
            self.changed = true;
        }
    }

    fn pick(&mut self, scene: [f32; 2]) {
        let f = self.frame;
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        for l in &self.project.layers {
            if !l.visible {
                continue;
            }
            let p = l.transform_at(f).invert(w, h, scene);
            if let Some(c) = l.drawing_at(f).and_then(|d| d.pick(p)) {
                self.color = c32(c);
                return;
            }
        }
        self.color = c32(self.project.background);
    }

    pub(super) fn continue_gesture(&mut self, scene: [f32; 2], shift: bool) {
        let p = self.local(scene);
        let min_step = 0.8 / self.zoom;
        let smoothing = self.smoothing;
        match self.gesture.take() {
            Some(Gesture::Paint { mut stroke, mut smooth, .. }) => {
                let k = 1.0 - smoothing.clamp(0.0, 0.95);
                smooth = [smooth[0] + (p[0] - smooth[0]) * k, smooth[1] + (p[1] - smooth[1]) * k];
                let last = *stroke.points.last().unwrap();
                if ((smooth[0] - last[0]).powi(2) + (smooth[1] - last[1]).powi(2)).sqrt() >= min_step {
                    stroke.points.push(smooth);
                    let pen = self.cur_pen();
                    if stroke.style.is_some() {
                        stroke.pressure.push(pen.pressure);
                    }
                    if let Some(ls) = &mut self.brush_live {
                        ls.push_with(smooth, pen);
                    }
                }
                self.gesture = Some(Gesture::Paint { stroke, smooth, last_raw: p });
            }
            Some(Gesture::Shape { kind, start, mut stroke }) => {
                let mut end = p;
                if shift {
                    let (dx, dy) = (p[0] - start[0], p[1] - start[1]);
                    if kind == ShapeKind::Line {
                        let ang = dy.atan2(dx);
                        let snap = (ang / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
                        let len = (dx * dx + dy * dy).sqrt();
                        end = [start[0] + len * snap.cos(), start[1] + len * snap.sin()];
                    } else {
                        let s = dx.abs().max(dy.abs());
                        end = [start[0] + s * dx.signum(), start[1] + s * dy.signum()];
                    }
                }
                let (pts, closed) = crate::model::shape_points(kind, start, end);
                stroke.points = pts;
                stroke.closed = closed;
                self.gesture = Some(Gesture::Shape { kind, start, stroke });
            }
            Some(Gesture::Erase { last }) => {
                self.erase_segment(last, p);
                self.gesture = Some(Gesture::Erase { last: p });
            }
            Some(Gesture::Move { last }) => {
                let (dx, dy) = (p[0] - last[0], p[1] - last[1]);
                if (dx != 0.0 || dy != 0.0) && self.transform_picked(|s| s.translate(dx, dy)) {
                    self.changed = true;
                }
                self.gesture = Some(Gesture::Move { last: p });
            }
            Some(Gesture::PenNode { idx }) => {
                self.pen_drag(idx, p);
                self.gesture = Some(Gesture::PenNode { idx });
            }
            Some(Gesture::NodeDrag { idx, part }) => {
                self.node_drag(idx, part, p);
                self.gesture = Some(Gesture::NodeDrag { idx, part });
            }
            Some(g @ (Gesture::Marquee { .. } | Gesture::SelScale { .. } | Gesture::SelRotate { .. })) => {
                let g = self.select_continue(g, p);
                self.gesture = Some(g);
            }
            Some(Gesture::Animate { start, base, mode }) => {
                let (dx, dy) = (scene[0] - start[0], scene[1] - start[1]);
                let mut t = base;
                match mode {
                    1 => {
                        let c = [self.project.width as f32 / 2.0 + base.x, self.project.height as f32 / 2.0 + base.y];
                        let a0 = (start[1] - c[1]).atan2(start[0] - c[0]);
                        let a1 = (scene[1] - c[1]).atan2(scene[0] - c[0]);
                        t.r = base.r + (a1 - a0).to_degrees();
                    }
                    2 => t.s = (base.s * (1.0 + (dx - dy) * 0.004)).clamp(0.05, 20.0),
                    _ => {
                        t.x = base.x + dx;
                        t.y = base.y + dy;
                    }
                }
                if t != base {
                    self.project.layers[self.cl].tracks.set_changed(self.frame, &t, &base);
                    self.changed = true;
                }
                self.gesture = Some(Gesture::Animate { start, base, mode });
            }
            Some(Gesture::Pick) => {
                self.pick(scene);
                self.gesture = Some(Gesture::Pick);
            }
            None => {}
        }
    }

    fn commit_stroke(&mut self, s: Stroke) {
        let f = self.frame;
        let (w, h) = (self.project.width, self.project.height);
        let layer = &mut self.project.layers[self.cl];
        let bitmap = layer.kind == LayerKind::Bitmap;
        if layer.drawing_at(f).is_none() {
            layer.keys.insert(f, Default::default());
        }
        if let Some(d) = layer.drawing_at_mut(f) {
            if bitmap {
                // Capa de mapa de bits: el trazo se pinta en los píxeles.
                let r = d.raster.get_or_insert_with(|| Arc::new(crate::raster::Raster::new(w, h)));
                crate::render::stamp_stroke(Arc::make_mut(r), &s);
            } else {
                d.strokes.push(s);
            }
            self.changed = true;
        }
    }

    pub(super) fn finish_gesture(&mut self) {
        match self.gesture.take() {
            Some(Gesture::Paint { mut stroke, last_raw, .. }) if self.brush_live.is_some() => {
                // Capa de mapa de bits con el motor de pinceles.
                let pen = self.cur_pen();
                if let Some(mut ls) = self.brush_live.take() {
                    if stroke.points.len() > 1 {
                        ls.push_with(last_raw, pen);
                    }
                    self.commit_live(ls);
                }
                self.brush_tex = None;
                stroke.points.clear();
            }
            Some(Gesture::Paint { mut stroke, last_raw, .. }) => {
                let last = *stroke.points.last().unwrap();
                if last != last_raw && stroke.points.len() > 1 {
                    stroke.points.push(last_raw);
                    if stroke.style.is_some() {
                        stroke.pressure.push(self.cur_pen().pressure);
                    }
                }
                stroke.simplify_in_place(0.2 / self.zoom.max(0.05));
                // Un trazo que vuelve a su inicio queda cerrado: así se puede rellenar.
                let n = stroke.points.len();
                if n >= 6 {
                    let (a, b) = (stroke.points[0], stroke.points[n - 1]);
                    let gap = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
                    if gap <= (stroke.width * 1.5).max(10.0 / self.zoom.max(0.05)) {
                        stroke.closed = true;
                    }
                }
                self.commit_stroke(stroke);
            }
            Some(Gesture::Shape { stroke, .. }) => {
                let b = stroke.points.iter().fold((f32::MAX, f32::MIN, f32::MAX, f32::MIN), |a, p| {
                    (a.0.min(p[0]), a.1.max(p[0]), a.2.min(p[1]), a.3.max(p[1]))
                });
                if (b.1 - b.0).max(b.3 - b.2) >= 1.0 {
                    self.commit_stroke(stroke);
                }
            }
            Some(Gesture::Marquee { start, cur, add }) => self.select_finish(start, cur, add),
            _ => {}
        }
        if let Some(pre) = self.pre.take() {
            if self.changed {
                self.push_undo(pre);
                self.meta_skip = true;
            }
        }
        self.changed = false;
    }

    // ---------- Teclado ----------

    pub(super) fn typing(ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.focused())
            .map_or(false, |id| egui::text_edit::TextEditState::load(ctx, id).is_some())
    }

    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Proyectos soltados sobre la ventana
        let dropped: Option<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()));
        if let Some(p) = dropped {
            if p.extension().map_or(false, |e| e.eq_ignore_ascii_case("ltoon")) {
                self.request(ctx, Pending::OpenPath(p));
            } else {
                self.say("Solo se pueden abrir proyectos .ltoon");
            }
        }
        // Mientras se escribe un texto o se está dibujando, los atajos esperan.
        if Self::typing(ctx) || self.gesture.is_some() {
            self.space_down = false;
            return;
        }
        // Captura con cámara: F5 e Intro valen en toda la sala mientras el Live View está abierto.
        if self.sm_cap_keys(ctx) {
            return;
        }
        // Atajos de la sala Stop Motion (solo con el puntero sobre su panel).
        if self.sm_room_keys(ctx) {
            return;
        }
        // Línea de tiempo de montaje: A, P, S, R, T, U y F4 con el puntero encima.
        if self.mt_keys(ctx) {
            return;
        }
        // Pluma con una curva en curso: Intro termina, Esc cancela, Retroceso quita el último nodo.
        if self.pen_keys(ctx) {
            return;
        }
        // Espacio: si se pulsa y suelta sin arrastrar, reproduce / pausa (como en la versión web).
        let space = ctx.input(|i| i.key_down(Key::Space));
        if space && !self.space_down {
            self.space_panned = false;
        }
        let story = self.room_kind().def().story;
        if !space && self.space_down && !self.space_panned && !story {
            self.toggle_play();
        }
        self.space_down = space;

        // Atajos del prototipo v0.3 (estilo Blender).
        let k = |m: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(m, key));
        if k(Modifiers::COMMAND, Key::Num3) && !story {
            self.toggle_view3d();
        }
        self.keys_3d(ctx);
        let cs = Modifiers::COMMAND | Modifiers::SHIFT;
        let none = Modifiers::NONE;
        let sh = Modifiers::SHIFT;
        let cmd = Modifiers::COMMAND;
        if k(cs, Key::Z) || k(cmd, Key::Y) {
            self.do_redo();
        }
        if k(cmd, Key::Z) {
            self.do_undo();
        }
        if k(cs, Key::S) {
            self.save_as();
        }
        if k(cmd, Key::S) {
            self.save();
        }
        if k(cmd, Key::O) {
            self.request(ctx, Pending::Open);
        }
        if k(cmd, Key::N) {
            self.stage = super::start::Stage::Home;
        }
        if k(cmd, Key::Q) {
            self.request(ctx, Pending::Quit);
        }
        if k(cmd, Key::E) {
            self.goto_room_kind(RoomKind::Exportar);
        }
        if k(cmd, Key::Num1) || k(cmd, Key::Num0) {
            let f = 1.0 / self.zoom;
            self.zoom_by(f);
        }
        if k(cmd, Key::Tab) {
            if self.room_kind() != RoomKind::StopMotion {
                self.xs_horizontal = !self.xs_horizontal;
            }
        }
        if k(cmd, Key::J) {
            self.duplicate_layer();
        }
        if k(cmd, Key::T) {
            self.set_tool(Tool::Text);
        }
        let (copy, paste) = ctx.input(|i| {
            (
                i.events.iter().any(|e| matches!(e, egui::Event::Copy | egui::Event::Cut)),
                i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
            )
        });
        let selecting = self.tool == Tool::Select && !self.obj_mode && !story;
        if copy && !(selecting && self.picked_copy()) {
            self.copy_drawing();
        }
        if paste {
            if selecting {
                self.picked_paste();
            } else {
                self.paste_drawing();
            }
        }
        if selecting {
            if k(cmd, Key::A) {
                self.select_all();
            }
            if k(none, Key::Escape) {
                self.picked.clear();
            }
            if !self.picked.is_empty() && (k(none, Key::Delete) || k(none, Key::Backspace)) {
                // Con un nodo elegido en una curva, Supr quita el nodo; si no, borra los trazos.
                if !self.node_delete_key() {
                    self.picked_delete();
                }
            }
        }
        if k(sh, Key::F5) {
            self.remove_frame();
        }
        if k(none, Key::F5) {
            self.insert_frame();
        }
        if k(none, Key::F1) {
            self.show_help = !self.show_help;
        }
        if k(Modifiers::ALT, Key::I) {
            self.mark_range(true);
        }
        if k(Modifiers::ALT, Key::O) {
            self.mark_range(false);
        }
        if k(sh, Key::A) {
            // Menú «Agregar capa» en el cursor, como en el prototipo.
            let at = ctx.input(|i| i.pointer.hover_pos()).unwrap_or_else(|| ctx.screen_rect().center());
            self.add_menu = Some(at);
        }
        if k(sh, Key::N) {
            self.add_layer();
        }
        if k(sh, Key::I) {
            self.add_key(false);
        }
        if k(sh, Key::D) {
            self.add_key(true);
        }
        if k(sh, Key::Delete) {
            self.delete_key();
        }
        if k(sh, Key::ArrowLeft) {
            self.playing = false;
            self.frame = self.play_bounds().0;
        }
        if k(sh, Key::ArrowRight) {
            self.playing = false;
            self.frame = self.play_bounds().1;
        }
        if story {
            return;
        }
        if k(none, Key::Tab) {
            self.toggle_mode();
        }
        if k(none, Key::T) {
            self.show_toolbar = !self.show_toolbar;
        }
        if k(none, Key::N) {
            self.show_n = !self.show_n;
        }
        if k(none, Key::O) {
            self.onion = !self.onion;
            self.say(if self.onion { "Papel cebolla activado" } else { "Papel cebolla desactivado" });
        }
        if k(none, Key::K) {
            self.set_xkey(None);
        }
        if k(none, Key::CloseBracket) {
            self.jump_xkey(true);
        }
        if k(none, Key::OpenBracket) {
            self.jump_xkey(false);
        }
        for t in Tool::ALL {
            if let Some(key) = t.key() {
                if k(none, key) {
                    self.set_tool(t);
                }
            }
        }
        if k(none, Key::ArrowRight) || k(none, Key::Period) {
            self.step(1);
        }
        if k(none, Key::ArrowLeft) || k(none, Key::Comma) {
            self.step(-1);
        }
        if k(none, Key::Home) {
            self.need_fit = true;
        }
        if k(none, Key::End) {
            self.playing = false;
            self.frame = self.play_bounds().1;
        }
        if k(none, Key::ArrowUp) {
            self.cl = self.cl.saturating_sub(1);
        }
        if k(none, Key::ArrowDown) {
            self.cl = (self.cl + 1).min(self.project.layers.len() - 1);
        }
        if k(none, Key::Enter) {
            self.toggle_play();
        }
        if k(none, Key::Delete) || k(none, Key::Backspace) {
            self.delete_key();
        }
        if k(none, Key::Plus) || k(none, Key::Equals) {
            self.zoom_step(1);
        }
        if k(none, Key::Minus) {
            self.zoom_step(-1);
        }
        // Que la barra espaciadora no "pulse" botones con foco.
        k(none, Key::Space);    }
}
