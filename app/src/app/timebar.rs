//! Barra de tiempo: transporte + pista con regla, indicador de fotograma, dibujos de la capa activa,
//! claves de animación, rango de reproducción y bordes del papel cebolla (rojo antes, verde después).

use super::shell::pill;
use super::*;

const H: f32 = 58.0;
const RULER: f32 = 16.0;
const BAND_Y: f32 = 20.0;
const BAND_H: f32 = 18.0;
const KEY_Y: f32 = 42.0;

const DRAG_SCRUB: u8 = 1;
const DRAG_ONION_B: u8 = 2;
const DRAG_ONION_A: u8 = 3;
const DRAG_RANGE_A: u8 = 4;
const DRAG_RANGE_B: u8 = 5;

impl LeonApp {
    /// Área de la línea de tiempo (prototipo v0.3): cabecera con el selector Línea de tiempo /
    /// Hoja de exposición, el transporte y los contadores; la barra de cuadros (con los bordes
    /// del papel cebolla y el rango) y la rejilla de capas y cuadros.
    pub(super) fn timeline_area(&mut self, ui: &mut egui::Ui) {
        // El selector de editor del hueco va a la izquierda de la cabecera; el resto de la cabecera es de la línea de tiempo.
        let hr = self.slot_head(ui);
        let mut h = ui.new_child(
            egui::UiBuilder::new().max_rect(hr).layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        h.spacing_mut().item_spacing.x = 3.0;
        let wide = hr.width() > 560.0;
        // En la sala Stop Motion la hoja de exposición es la pestaña X-Sheet de su panel: aquí solo va la línea de tiempo.
        let sala = self.room_kind() == RoomKind::StopMotion;
        if sala {
            h.label(RichText::new("Línea de tiempo").size(12.0).color(TX));
        } else {
            if pill(&mut h, if wide { "Línea de tiempo" } else { "Línea" }, self.xs_horizontal).on_hover_text("Capas en filas, cuadros en columnas (Ctrl+Tab)").clicked() {
                self.xs_horizontal = true;
            }
            if pill(&mut h, if wide { "Hoja de exposición" } else { "Hoja" }, !self.xs_horizontal)
                .on_hover_text("Cuadros en filas, capas en columnas, al lado de la mesa de luz (Ctrl+Tab)")
                .clicked()
            {
                self.xs_horizontal = false;
            }
        }
        h.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let n = self.project.length;
            let (a, b) = self.project.range.map_or((1, n), |r| (r.0 + 1, r.1 + 1));
            if self.xs_horizontal {
                ui.label(RichText::new(format!("Inicio {a}  Fin {b}")).size(11.5).color(MU));
            }
            if wide {
                ui.add(egui::DragValue::new(&mut self.project.fps).range(1..=120).suffix(" fps"));
            }
            ui.label(RichText::new(format!("{} / {}", self.frame + 1, n)).size(12.0).color(TX).strong());
            ui.spacing_mut().item_spacing.x = 1.0;
            let auto = self.obj_mode && matches!(self.tool, Tool::Select | Tool::RotateObj);
            let (dot, dot_resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
            ui.painter().circle_filled(dot.center(), 5.0, if auto { Color32::from_rgb(0xe0, 0x40, 0x30) } else { MU });
            if dot_resp
                .on_hover_text("Animación automática: en modo objeto, mover, girar o escalar crea claves en el cuadro actual")
                .clicked()
            {
                if !self.obj_mode {
                    self.toggle_mode();
                }
                self.tool = Tool::Select;
            }
            if icon_button(ui, 22.0, self.loop_play, "Bucle", draw_loop).clicked() {
                self.loop_play = !self.loop_play;
            }
            if icon_button(ui, 22.0, false, "Último cuadro (Fin)", draw_last).clicked() {
                self.playing = false;
                self.frame = self.project.range.map_or(n - 1, |r| r.1);
            }
            if icon_button(ui, 22.0, false, "Cuadro siguiente ( . )", draw_next).clicked() {
                self.step(1);
            }
            let playing = self.playing;
            let play = icon_button(ui, 22.0, playing, "Reproducir / pausa (Espacio)", move |p, r, c| {
                if playing {
                    draw_pause(p, r, c)
                } else {
                    draw_play(p, r, c)
                }
            });
            if play.clicked() {
                self.toggle_play();
            }
            if icon_button(ui, 22.0, false, "Cuadro anterior ( , )", draw_prev).clicked() {
                self.step(-1);
            }
            if icon_button(ui, 22.0, false, "Primer cuadro (Inicio)", draw_first).clicked() {
                self.playing = false;
                self.frame = self.project.range.map_or(0, |r| r.0);
            }
        });
        // Barra de cuadros
        let avail = ui.available_rect_before_wrap();
        let tr = Rect::from_min_size(avail.min + Vec2::new(4.0, 3.0), Vec2::new(avail.width() - 8.0, H));
        let resp = ui.interact(tr, ui.id().with("navbar"), Sense::click_and_drag());
        self.track(ui, tr, resp);
        ui.allocate_rect(Rect::from_min_size(avail.min, Vec2::new(avail.width(), H + 6.0)), Sense::hover());
        // Rejilla de capas y cuadros
        self.xsheet(ui);
    }

    fn track(&mut self, ui: &mut egui::Ui, rect: Rect, resp: egui::Response) {
        let n = self.project.length;
        let fps = self.project.fps.max(1);
        let w = rect.width();
        let ppf = self.tb_ppf.unwrap_or((w / (n as f32 + 1.0)).clamp(3.0, 48.0)).clamp(2.0, 80.0);
        let visible = w / ppf;
        if self.tb_ppf.is_none() {
            self.tb_start = 0.0;
        }
        // Mantener el fotograma actual a la vista
        let f = self.frame as f32;
        if f < self.tb_start || f > self.tb_start + visible - 1.0 {
            self.tb_start = (f - visible / 2.0).clamp(0.0, (n as f32 + 1.0 - visible).max(0.0));
        }
        let vs = self.tb_start;
        let fx = |k: f32| rect.min.x + (k - vs) * ppf;
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 3.0, Color32::from_rgb(0x23, 0x23, 0x23));
        let f0 = vs.floor().max(0.0) as u32;
        let f1 = ((vs + visible).ceil() as u32).min(n);
        let y = |v: f32| rect.min.y + v;
        // segundos alternados
        for k in f0..f1 {
            if (k / fps) % 2 == 1 {
                p.rect_filled(Rect::from_min_max(Pos2::new(fx(k as f32), y(RULER)), Pos2::new(fx(k as f32 + 1.0), rect.max.y)), 0.0, Color32::from_rgb(0x27, 0x27, 0x27));
            }
        }
        // fuera de la duración
        if fx(n as f32) < rect.max.x {
            p.rect_filled(Rect::from_min_max(Pos2::new(fx(n as f32), rect.min.y), rect.max), 0.0, Color32::from_rgb(0x17, 0x17, 0x17));
        }
        // regla
        p.rect_filled(Rect::from_min_max(rect.min, Pos2::new(rect.max.x, y(RULER))), 0.0, BG);
        let step = [1u32, 2, 5, 10, 20, 25, 50, 100, 200, 500, 1000].into_iter().find(|s| *s as f32 * ppf >= 38.0).unwrap_or(1000);
        for k in f0..f1 {
            let cx = fx(k as f32) + ppf / 2.0;
            if k % step == 0 {
                p.vline(cx, egui::Rangef::new(y(RULER - 6.0), y(RULER)), stk(1.0, Color32::from_gray(0x9a)));
                p.text(Pos2::new(cx + 3.0, y(7.0)), Align2::LEFT_CENTER, format!("{}", k + 1), FontId::proportional(9.5), Color32::from_gray(0x9a));
            } else if ppf >= 6.0 {
                p.vline(cx, egui::Rangef::new(y(RULER - 3.0), y(RULER)), stk(1.0, Color32::from_gray(0x55)));
            }
            if k % fps == 0 && k != 0 {
                p.vline(fx(k as f32), egui::Rangef::new(y(RULER), rect.max.y), stk(1.0, Color32::from_gray(0x6a)));
            }
        }
        // rango de reproducción
        if let Some((a, b)) = self.project.range {
            let r = Rect::from_min_max(Pos2::new(fx(a as f32), y(RULER)), Pos2::new(fx(b as f32 + 1.0), rect.max.y));
            p.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(234, 114, 33, 30));
            p.rect_filled(Rect::from_min_max(Pos2::new(r.min.x, y(KEY_Y + 14.0)), Pos2::new(r.max.x, y(KEY_Y + 16.0))), 0.0, ACCENT);
            p.rect_filled(Rect::from_min_size(Pos2::new(r.min.x, y(KEY_Y + 6.0)), Vec2::new(3.0, 12.0)), 0.0, ACCENT);
            p.rect_filled(Rect::from_min_size(Pos2::new(r.max.x - 3.0, y(KEY_Y + 6.0)), Vec2::new(3.0, 12.0)), 0.0, ACCENT);
        }
        // papel cebolla
        let cur = self.frame;
        let (ob, oa) = (self.onion_before, self.onion_after);
        for d in 1..=ob {
            if cur >= d {
                let a = if self.onion { (self.onion_weight(d, ob) + 0.12).min(1.0) } else { 0.12 };
                let k = (cur - d) as f32;
                p.rect_filled(Rect::from_min_max(Pos2::new(fx(k) + 0.5, y(BAND_Y)), Pos2::new(fx(k + 1.0) - 0.5, y(BAND_Y + BAND_H))), 0.0, with_alpha(ONION_BEFORE, a));
            }
        }
        for d in 1..=oa {
            if cur + d < n {
                let a = if self.onion { (self.onion_weight(d, oa) + 0.12).min(1.0) } else { 0.12 };
                let k = (cur + d) as f32;
                p.rect_filled(Rect::from_min_max(Pos2::new(fx(k) + 0.5, y(BAND_Y)), Pos2::new(fx(k + 1.0) - 0.5, y(BAND_Y + BAND_H))), 0.0, with_alpha(ONION_AFTER, a));
            }
        }
        let gx0 = fx(cur as f32 - ob as f32);
        let gx1 = fx(cur as f32 + 1.0 + oa as f32);
        p.rect_filled(Rect::from_min_size(Pos2::new(gx0 - 3.0, y(BAND_Y - 2.0)), Vec2::new(5.0, BAND_H + 4.0)), 0.0, ONION_BEFORE);
        p.rect_filled(Rect::from_min_size(Pos2::new(gx1 - 2.0, y(BAND_Y - 2.0)), Vec2::new(5.0, BAND_H + 4.0)), 0.0, ONION_AFTER);
        // dibujos de la capa activa
        let layer = self.layer();
        let keys: Vec<u32> = layer.keys.keys().copied().collect();
        for (i, k) in keys.iter().enumerate() {
            let end = keys.get(i + 1).copied().unwrap_or(n).min(n);
            if *k >= f1 || end <= f0 {
                continue;
            }
            for c in (*k).max(f0)..end.min(f1) {
                let x0 = fx(c as f32) + 0.5;
                let r = Rect::from_min_max(Pos2::new(x0, y(BAND_Y + 4.0)), Pos2::new(x0 + ppf - 1.0, y(BAND_Y + BAND_H - 4.0)));
                p.rect_filled(r, 0.0, if c == *k { Color32::from_gray(0x6b) } else { Color32::from_gray(0x55) });
                if c == *k {
                    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(3.0f32.min(ppf - 1.0), r.height())), 0.0, ACCENT);
                }
            }
        }
        // claves de animación
        for k in layer.tracks.frames().iter() {
            if *k < f0 || *k >= f1 {
                continue;
            }
            let cx = fx(*k as f32) + ppf / 2.0;
            let pts = vec![Pos2::new(cx, y(KEY_Y - 1.0)), Pos2::new(cx + 5.0, y(KEY_Y + 4.0)), Pos2::new(cx, y(KEY_Y + 9.0)), Pos2::new(cx - 5.0, y(KEY_Y + 4.0))];
            p.add(Shape::convex_polygon(pts, KEY_GOLD, stk(1.0, Color32::BLACK)));
        }
        // indicador de fotograma
        let px = fx(cur as f32) + ppf / 2.0;
        p.rect_filled(Rect::from_min_max(Pos2::new(fx(cur as f32), y(RULER)), Pos2::new(fx(cur as f32 + 1.0), rect.max.y)), 0.0, Color32::from_rgba_unmultiplied(234, 114, 33, 64));
        p.vline(px, egui::Rangef::new(y(RULER - 1.0), rect.max.y), stk(1.5, ACCENT));
        let label = format!("{}", cur + 1);
        let tw = label.len() as f32 * 6.5 + 10.0;
        let lx = px.clamp(rect.min.x + tw / 2.0, rect.max.x - tw / 2.0);
        let tag = Rect::from_center_size(Pos2::new(lx, y(7.5)), Vec2::new(tw, RULER - 1.0));
        p.rect_filled(tag, 3.0, ACCENT);
        p.text(tag.center(), Align2::CENTER_CENTER, label, FontId::proportional(10.5), Color32::WHITE);

        // ---- interacción ----
        let frame_at = |x: f32| (((x - rect.min.x) / ppf + vs).floor().max(0.0) as u32).min(n - 1);
        let units = |x: f32| (x - rect.min.x) / ppf + vs;
        if resp.hovered() {
            let (scroll, shift) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.shift));
            let dy = scroll.y + scroll.x;
            if dy != 0.0 {
                if shift || scroll.x.abs() > scroll.y.abs() {
                    self.tb_start = (self.tb_start - dy / ppf).clamp(0.0, (n as f32 + 1.0 - visible).max(0.0));
                } else if let Some(hp) = resp.hover_pos() {
                    let u = units(hp.x);
                    let np = (ppf * (dy * 0.004).exp()).clamp(2.0, 80.0);
                    self.tb_ppf = Some(np);
                    self.tb_start = (u - (hp.x - rect.min.x) / np).max(0.0);
                }
            }
        }
        let near = |x: f32, a: f32| (x - a).abs() <= 6.0;
        if resp.drag_started() || resp.clicked() {
            if let Some(pp) = resp.interact_pointer_pos() {
                let ly = pp.y - rect.min.y;
                self.tb_drag = DRAG_SCRUB;
                if (BAND_Y - 4.0..=BAND_Y + BAND_H + 4.0).contains(&ly) {
                    if near(pp.x, gx0) {
                        self.tb_drag = DRAG_ONION_B;
                    } else if near(pp.x, gx1) {
                        self.tb_drag = DRAG_ONION_A;
                    }
                }
                if let (Some((a, b)), true) = (self.project.range, ly >= KEY_Y + 2.0) {
                    if near(pp.x, fx(a as f32)) {
                        self.tb_drag = DRAG_RANGE_A;
                    } else if near(pp.x, fx(b as f32 + 1.0)) {
                        self.tb_drag = DRAG_RANGE_B;
                    }
                }
            }
        }
        if (resp.dragged() || resp.clicked()) && self.tb_drag != 0 {
            if let Some(pp) = resp.interact_pointer_pos() {
                let u = units(pp.x);
                match self.tb_drag {
                    DRAG_ONION_B => {
                        self.onion_before = (cur as f32 - u).round().clamp(0.0, ONION_LIMIT as f32) as u32;
                        self.onion = true;
                    }
                    DRAG_ONION_A => {
                        self.onion_after = (u - cur as f32 - 1.0).round().clamp(0.0, ONION_LIMIT as f32) as u32;
                        self.onion = true;
                    }
                    DRAG_RANGE_A => {
                        if let Some((_, b)) = self.project.range {
                            self.project.range = Some(((u.round().max(0.0) as u32).min(b), b));
                        }
                    }
                    DRAG_RANGE_B => {
                        if let Some((a, _)) = self.project.range {
                            self.project.range = Some((a, ((u.round() as i64 - 1).max(a as i64) as u32).min(n - 1)));
                        }
                    }
                    _ => {
                        self.playing = false;
                        self.frame = frame_at(pp.x);
                    }
                }
            }
        }
        if resp.drag_stopped() || resp.clicked() {
            self.tb_drag = 0;
        }
        if resp.double_clicked() {
            if let Some(pp) = resp.interact_pointer_pos() {
                if pp.y - rect.min.y >= KEY_Y + 2.0 {
                    let fr = frame_at(pp.x);
                    let inside = self.project.range.map_or(false, |(a, b)| fr >= a && fr <= b);
                    self.project.range = if inside { None } else { Some((fr, (fr + fps - 1).min(n - 1))) };
                }
            }
        }
        let hover_text = match resp.hover_pos() {
            Some(hp) => format!(
                "Fotograma {}\nArrastra para moverte · bordes rojo/verde: papel cebolla · doble clic abajo: rango de reproducción · rueda: zoom",
                frame_at(hp.x) + 1
            ),
            None => String::new(),
        };
        resp.on_hover_text(hover_text);
    }
}
