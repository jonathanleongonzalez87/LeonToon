//! Xsheet (hoja de exposición): cada capa es una columna y cada fotograma una fila
//! (o al revés, con «Columnas»). Celdas: número de dibujo donde empieza, «│» mientras se mantiene.

use super::*;

const NUM_W: f32 = 40.0;
const COL_W: f32 = 90.0;
const HEAD_H: f32 = 24.0;
const NAME_W: f32 = 150.0;
const CELL_W_BASE: f32 = 26.0;
const LANE_H: f32 = 28.0;

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    Corner,
    Frame(u32),
    Header(usize, u8),
    Cell(usize, u32),
    None,
}

impl LeonApp {
    /// Operaciones de celdas desde el menú Animación > Celdas.
    pub(super) fn cell_op_pub(&mut self, op: u8) {
        self.cell_op(op);
    }

    pub(super) fn xsheet(&mut self, ui: &mut egui::Ui) {
        let avail = ui.available_rect_before_wrap();
        // Ctrl + rueda: zoom del Xsheet, un nivel por muesca (de 5 a 16).
        let hovered = ui.rect_contains_pointer(avail);
        let (ctrl, dy) = ui.input(|i| (i.modifiers.command, i.raw_scroll_delta.y));
        if hovered && ctrl && dy != 0.0 {
            self.wheel_acc += dy;
            while self.wheel_acc.abs() >= WHEEL_NOTCH {
                let s = self.wheel_acc.signum();
                self.xs_zoom = (self.xs_zoom + s).clamp(XS_ZOOM_MIN, XS_ZOOM_MAX);
                self.wheel_acc -= WHEEL_NOTCH * s;
            }
            ui.input_mut(|i| {
                i.smooth_scroll_delta = Vec2::ZERO;
                i.raw_scroll_delta = Vec2::ZERO;
            });
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(avail));
        egui::ScrollArea::both()
            .id_salt(("xs", self.xs_horizontal))
            .auto_shrink([false, false])
            .show(&mut child, |ui| self.xsheet_grid(ui));
        ui.allocate_rect(avail, Sense::hover());
    }

    /// Operaciones de celdas: 0 = ×2, 1 = invertir, 2 = ida y vuelta, 3 = vaciar.
    fn cell_op(&mut self, op: u8) {
        if self.locked_layer() {
            return;
        }
        let last = self.project.length - 1;
        let (l, a, b) = match self.sel {
            Some((l, a, b)) if l == self.cl => (l, a, b),
            _ => (self.cl, 0, last),
        };
        self.checkpoint();
        let len = self.project.length;
        let layer = &mut self.project.layers[l];
        match op {
            0 => {
                layer.double_timing();
                self.project.length = (len * 2).min(100_000);
                self.say("Tiempos duplicados: la escena dura el doble");
            }
            1 => {
                layer.reverse_range(a, b);
                self.say("Orden de los dibujos invertido");
            }
            2 => {
                let n = layer.swing_range(a, b, len);
                self.project.length = n.min(100_000);
                self.say("Ida y vuelta añadida");
            }
            _ => {
                layer.clear_range(a, b);
                self.say("Celdas vaciadas");
            }
        }
    }

    fn locked_layer(&mut self) -> bool {
        if self.layer().locked {
            self.say("La columna está bloqueada");
            true
        } else {
            false
        }
    }

    /// Orden de dibujo de las columnas: en vertical, la de más atrás a la izquierda.
    fn col_order(&self) -> Vec<usize> {
        let n = self.project.layers.len();
        if self.xs_horizontal {
            (0..n).collect()
        } else {
            (0..n).rev().collect()
        }
    }

    fn xsheet_grid(&mut self, ui: &mut egui::Ui) {
        // Zoom del Xsheet como el original: fila de 17 px a zoom 10 (de 5 a 16); fuente = fila − 5 px.
        let row_h = XS_ROW_BASE * self.xs_zoom / XS_ZOOM_DEFAULT;
        let cell_w = CELL_W_BASE * self.xs_zoom / XS_ZOOM_DEFAULT;
        let order = self.col_order();
        let nl = order.len();
        let len = self.project.length;
        let horiz = self.xs_horizontal;
        let size = if horiz {
            Vec2::new(NAME_W + len as f32 * cell_w + 40.0, HEAD_H + nl as f32 * LANE_H)
        } else {
            Vec2::new((NUM_W + nl as f32 * COL_W).max(ui.available_width()), HEAD_H + len as f32 * row_h + 20.0)
        };
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let o = resp.rect.min;
        let clip = ui.clip_rect();
        // Coordenadas "pegajosas": cabeceras y números siguen a la vista al desplazarse.
        let sx = clip.min.x.max(o.x);
        let sy = clip.min.y.max(o.y);
        let fps = self.project.fps.max(1);

        // Celdas visibles
        let (f0, f1) = if horiz {
            let a = ((clip.min.x - o.x - NAME_W) / cell_w).floor().max(0.0) as u32;
            (a, (((clip.max.x - o.x - NAME_W) / cell_w).ceil().max(0.0) as u32 + 1).min(len))
        } else {
            let a = ((clip.min.y - o.y - HEAD_H) / row_h).floor().max(0.0) as u32;
            (a, (((clip.max.y - o.y - HEAD_H) / row_h).ceil().max(0.0) as u32 + 1).min(len))
        };
        let cell_rect = |c: usize, f: u32| -> Rect {
            if horiz {
                Rect::from_min_size(Pos2::new(o.x + NAME_W + f as f32 * cell_w, o.y + HEAD_H + c as f32 * LANE_H), Vec2::new(cell_w, LANE_H))
            } else {
                Rect::from_min_size(Pos2::new(o.x + NUM_W + c as f32 * COL_W, o.y + HEAD_H + f as f32 * row_h), Vec2::new(COL_W, row_h))
            }
        };
        painter.rect_filled(resp.rect, 0.0, BG);
        let (sa, sb) = match self.sel {
            Some((l, a, b)) => (Some(l), (a, b)),
            None => (None, (1, 0)),
        };
        for (c, &li) in order.iter().enumerate() {
            let layer = &self.project.layers[li];
            let numbers: Vec<(u32, usize)> = layer.keys.keys().map(|k| (*k, layer.drawing_number(*k).unwrap_or(0))).collect();
            for f in f0..f1 {
                let r = cell_rect(c, f);
                let key_idx = if layer.keys.contains_key(&f) { numbers.iter().position(|(k, _)| *k == f).map(|i| numbers[i].1) } else { None };
                let shown = layer.key_at_or_before(f);
                let empty = shown.map_or(true, |k| layer.keys[&k].is_empty());
                let mut bg = if shown.is_some() && !empty { Color32::from_gray(0x3a) } else { BG };
                if key_idx.is_none() && shown.is_some() && !empty {
                    bg = Color32::from_gray(0x33);
                }
                if sa == Some(li) && f >= sb.0 && f <= sb.1 {
                    bg = CELL_SEL;
                }
                painter.rect_filled(r, 0.0, bg);
                let (edge_a, edge_b) = if horiz { (r.right_top(), r.right_bottom()) } else { (r.left_bottom(), r.right_bottom()) };
                painter.line_segment([edge_a, edge_b], stk(1.0, Color32::from_black_alpha(80)));
                if f % fps == 0 && f != 0 {
                    let (a, b) = if horiz { (r.left_top(), r.left_bottom()) } else { (r.left_top(), r.right_top()) };
                    painter.line_segment([a, b], stk(1.0, Color32::from_gray(0x88)));
                }
                let txt_col = if !layer.visible { Color32::from_gray(0x66) } else { TX };
                if let Some(i) = key_idx {
                    let label = if layer.keys[&f].is_empty() { "×".to_string() } else { format!("{}", i + 1) };
                    painter.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional((row_h - 5.0).clamp(8.0, 16.0)), txt_col);
                } else if shown.is_some() && !empty {
                    let (a, b) = if horiz {
                        (r.left_center(), r.right_center())
                    } else {
                        (r.center_top() + Vec2::new(0.0, 3.0), r.center_bottom() - Vec2::new(0.0, 3.0))
                    };
                    painter.line_segment([a, b], stk(1.0, Color32::from_gray(0x77)));
                }
                if layer.tracks.has_key_at(f) {
                    painter.rect_stroke(r.shrink(1.0), 0.0, stk(2.0, KEY_GOLD), EStrokeKind::Inside);
                }
                if f == self.frame {
                    painter.rect_stroke(r, 0.0, stk(1.0, ACCENT), EStrokeKind::Inside);
                }
            }
            // Separador de columna
            let a = cell_rect(c, f0);
            let b = cell_rect(c, f1.max(1) - 1);
            if horiz {
                painter.hline(a.min.x..=b.max.x, a.max.y, stk(1.0, Color32::from_black_alpha(90)));
            } else {
                painter.vline(a.max.x, a.min.y..=b.max.y, stk(1.0, Color32::from_black_alpha(90)));
            }
        }

        // Números de fotograma (pegajosos)
        for f in f0..f1 {
            let r = if horiz {
                Rect::from_min_size(Pos2::new(o.x + NAME_W + f as f32 * cell_w, sy), Vec2::new(cell_w, HEAD_H))
            } else {
                Rect::from_min_size(Pos2::new(sx, o.y + HEAD_H + f as f32 * row_h), Vec2::new(NUM_W, row_h))
            };
            let cur = f == self.frame;
            painter.rect_filled(r, 0.0, if cur { ACCENT } else { BG3 });
            if f % fps == 0 && f != 0 {
                painter.line_segment(if horiz { [r.left_top(), r.left_bottom()] } else { [r.left_top(), r.right_top()] }, stk(1.0, Color32::from_gray(0x88)));
            }
            let show = !horiz || cur || f == 0 || (f + 1) % 5 == 0;
            if show {
                let (pos, al) = if horiz { (r.center(), Align2::CENTER_CENTER) } else { (r.right_center() - Vec2::new(6.0, 0.0), Align2::RIGHT_CENTER) };
                painter.text(pos, al, format!("{}", f + 1), FontId::proportional(10.5), if cur { Color32::WHITE } else { MU });
            }
        }

        // Cabeceras de columna (pegajosas)
        for (c, &li) in order.iter().enumerate() {
            let r = header_rect(horiz, c, o, sx, sy);
            let layer = &self.project.layers[li];
            let on = li == self.cl;
            painter.rect_filled(r, 0.0, if on { SEL_BG } else { BG3 });
            painter.rect_stroke(r, 0.0, stk(1.0, LINE), EStrokeKind::Inside);
            let eye = Rect::from_min_size(r.right_top() - Vec2::new(44.0, 0.0), Vec2::new(20.0, r.height()));
            let lock = Rect::from_min_size(r.right_top() - Vec2::new(22.0, 0.0), Vec2::new(20.0, r.height()));
            draw_eye(&painter, eye, layer.visible, false);
            draw_lock(&painter, lock, layer.locked, false);
            let renaming = matches!(&self.renaming, Some((i, _, _)) if *i == li);
            if !renaming {
                let name_r = Rect::from_min_max(r.min + Vec2::new(6.0, 0.0), Pos2::new(eye.min.x - 2.0, r.max.y));
                let g = painter.layout_no_wrap(layer.name.clone(), FontId::proportional(12.0), if on { Color32::WHITE } else { TX });
                painter.with_clip_rect(name_r).galley(Pos2::new(name_r.min.x, name_r.center().y - g.size().y / 2.0), g, TX);
            }
        }
        let corner = Rect::from_min_size(Pos2::new(sx, sy), Vec2::new(if horiz { NAME_W } else { NUM_W }, HEAD_H));
        painter.rect_filled(corner, 0.0, BG3);
        painter.rect_stroke(corner, 0.0, stk(1.0, LINE), EStrokeKind::Inside);
        if horiz {
            painter.text(corner.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, "Columnas", FontId::proportional(11.0), MU);
        }

        // ---- interacción ----
        let hit = |p: Pos2| -> Hit {
            if corner.contains(p) {
                return Hit::Corner;
            }
            for (c, &li) in order.iter().enumerate() {
                let r = header_rect(horiz, c, o, sx, sy);
                if r.contains(p) {
                    let part = if p.x >= r.max.x - 22.0 { 2 } else if p.x >= r.max.x - 44.0 { 1 } else { 0 };
                    return Hit::Header(li, part);
                }
            }
            let (num_hit, f, c) = if horiz {
                (p.y < sy + HEAD_H, ((p.x - o.x - NAME_W) / cell_w).floor(), ((p.y - o.y - HEAD_H) / LANE_H).floor())
            } else {
                (p.x < sx + NUM_W, ((p.y - o.y - HEAD_H) / row_h).floor(), ((p.x - o.x - NUM_W) / COL_W).floor())
            };
            if f < 0.0 || f as u32 >= len {
                return Hit::None;
            }
            if num_hit {
                return Hit::Frame(f as u32);
            }
            if c >= 0.0 && (c as usize) < nl {
                return Hit::Cell(order[c as usize], f as u32);
            }
            Hit::None
        };
        let ptr = resp.interact_pointer_pos();
        let hp = ptr.map(hit).unwrap_or(Hit::None);
        let drag_id = ui.id().with("xs_drag_col");
        if resp.drag_started() {
            if let Hit::Header(li, 0) = hp {
                ui.data_mut(|d| d.insert_temp(drag_id, li));
            }
        }
        let dragging_col: Option<usize> = ui.data(|d| d.get_temp(drag_id));
        if let (Some(from), Some(p)) = (dragging_col, ptr) {
            if resp.dragged() {
                painter.text(p + Vec2::new(12.0, 0.0), Align2::LEFT_CENTER, "Mover columna aquí", FontId::proportional(11.0), ACCENT2);
            }
            if resp.drag_stopped() {
                ui.data_mut(|d| d.remove::<usize>(drag_id));
                if let Hit::Header(to, _) | Hit::Cell(to, _) = hit(p) {
                    self.move_layer_to(from, to);
                }
            }
        } else if resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down()) {
            let pressed = ui.input(|i| i.pointer.primary_pressed());
            let shift = ui.input(|i| i.modifiers.shift);
            match hp {
                Hit::Cell(li, f) => {
                    self.playing = false;
                    if pressed {
                        if shift && self.sel.map_or(false, |s| s.0 == li) {
                            let (_, a, _) = self.sel.unwrap();
                            self.sel = Some((li, a.min(f), a.max(f)));
                        } else {
                            self.sel = Some((li, f, f));
                            ui.data_mut(|d| d.insert_temp(ui.id().with("xs_anchor"), f));
                        }
                        self.cl = li;
                    } else if let Some((l, _, _)) = self.sel {
                        if l == li {
                            let anchor: u32 = ui.data(|d| d.get_temp(ui.id().with("xs_anchor"))).unwrap_or(f);
                            self.sel = Some((li, anchor.min(f), anchor.max(f)));
                        }
                    }
                    self.frame = f;
                }
                Hit::Frame(f) => {
                    self.playing = false;
                    self.frame = f;
                }
                _ => {}
            }
        }
        if resp.clicked() {
            match hp {
                Hit::Header(li, 1) => self.project.layers[li].visible = !self.project.layers[li].visible,
                Hit::Header(li, 2) => self.project.layers[li].locked = !self.project.layers[li].locked,
                Hit::Header(li, _) => {
                    self.cl = li;
                    self.sel = None;
                }
                _ => {}
            }
        }
        if resp.double_clicked() {
            match hp {
                Hit::Header(li, 0) => self.renaming = Some((li, self.project.layers[li].name.clone(), true)),
                Hit::Cell(li, f) if !self.project.layers[li].keys.contains_key(&f) => {
                    self.cl = li;
                    self.frame = f;
                    self.add_key(false);
                }
                _ => {}
            }
        }
        if let Some(Hit::Header(_, part)) = resp.hover_pos().map(hit) {
            let tip = ["Clic: elegir · doble clic: renombrar · arrastrar: reordenar", "Visible", "Bloquear"][part as usize];
            resp.clone().on_hover_text(tip);
        }
        if self.last_scroll_frame != Some(self.frame) {
            if let Some(c) = order.iter().position(|l| *l == self.cl) {
                let r = cell_rect(c, self.frame);
                ui.scroll_to_rect(r.expand2(if horiz { Vec2::new(cell_w * 3.0, 0.0) } else { Vec2::new(0.0, row_h * 3.0) }), None);
            }
            self.last_scroll_frame = Some(self.frame);
        }
        self.rename_box(ui, &order, o, sx, sy);
        resp.context_menu(|ui| {
            ui.label(RichText::new(format!("Fotograma {} · {}", self.frame + 1, self.layer().name)).strong());
            ui.separator();
            for (label, act) in [
                ("Nuevo dibujo   N", 0),
                ("Duplicar dibujo   D", 1),
                ("Eliminar dibujo   Supr", 2),
                ("Copiar dibujo", 3),
                ("Pegar dibujo", 4),
                ("Insertar fotograma   F5", 5),
                ("Quitar fotograma   Mayús+F5", 6),
                ("Fijar clave de animación   K", 7),
            ] {
                if ui.button(label).clicked() {
                    ui.close_menu();
                    match act {
                        0 => self.add_key(false),
                        1 => self.add_key(true),
                        2 => self.delete_key(),
                        3 => self.copy_drawing(),
                        4 => self.paste_drawing(),
                        5 => self.insert_frame(),
                        6 => self.remove_frame(),
                        _ => self.set_xkey(None),
                    }
                }
            }
        });
    }

    fn rename_box(&mut self, ui: &mut egui::Ui, order: &[usize], o: Pos2, sx: f32, sy: f32) {
        let Some((li, mut buf, first)) = self.renaming.take() else { return };
        let Some(c) = order.iter().position(|l| *l == li) else { return };
        let r = header_rect(self.xs_horizontal, c, o, sx, sy);
        let edit = Rect::from_min_max(r.min + Vec2::new(2.0, 2.0), Pos2::new(r.max.x - 46.0, r.max.y - 2.0));
        let resp = ui.put(edit, egui::TextEdit::singleline(&mut buf).font(FontId::proportional(12.0)));
        if first {
            resp.request_focus();
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            return;
        }
        if resp.lost_focus() && !first {
            let name = buf.trim();
            if !name.is_empty() {
                self.project.layers[li].name = name.to_string();
            }
        } else {
            self.renaming = Some((li, buf, false));
        }
    }
}

fn header_rect(horiz: bool, c: usize, o: Pos2, sx: f32, sy: f32) -> Rect {
    if horiz {
        Rect::from_min_size(Pos2::new(sx, o.y + HEAD_H + c as f32 * LANE_H), Vec2::new(NAME_W, LANE_H))
    } else {
        Rect::from_min_size(Pos2::new(o.x + NUM_W + c as f32 * COL_W, sy), Vec2::new(COL_W, HEAD_H))
    }
}
