//! Sala StoryBoard: cuadrícula de planos con zoom (de miniaturas a detalle con notas),
//! editor de detalle con pincel y borrador, tira de planos y «Animatic → Xsheet».

use super::canvas::{paint_drawing, paint_stroke, paint_thumb};
use super::shell::{muted, pill};
use super::*;
use crate::model::Panel;
use std::sync::Arc;

const FIELDS: [&str; 5] = ["Acción", "Diálogo", "Cámara", "Transición", "Notas"];

fn field_mut(p: &mut Panel, i: usize) -> &mut String {
    match i {
        0 => &mut p.action,
        1 => &mut p.dialogue,
        2 => &mut p.camera,
        3 => &mut p.trans,
        _ => &mut p.notes,
    }
}

impl LeonApp {
    pub(super) fn storyboard_room(&mut self, ui: &mut egui::Ui) {
        self.sb_sel = self.sb_sel.min(self.project.storyboard.len().saturating_sub(1));
        if self.project.storyboard.is_empty() {
            self.sb_detail = false;
        }
        self.sb_bar(ui);
        if self.sb_detail {
            self.sb_detail_view(ui);
        } else {
            self.sb_grid(ui);
        }
    }

    fn sb_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::default().fill(BG2).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if self.sb_detail && pill(ui, "< Cuadrícula", false).clicked() {
                    self.sb_detail = false;
                }
                if pill(ui, "+ Plano", false).clicked() {
                    self.checkpoint();
                    let at = (self.sb_sel + 1).min(self.project.storyboard.len());
                    let n = self.project.storyboard.len() + 1;
                    self.project.storyboard.insert(at, Panel::new(n));
                    self.sb_sel = at;
                }
                let has = !self.project.storyboard.is_empty();
                if pill(ui, "Duplicar", false).clicked() && has {
                    self.checkpoint();
                    let mut p = self.project.storyboard[self.sb_sel].clone();
                    p.title = format!("{} copia", p.title);
                    self.project.storyboard.insert(self.sb_sel + 1, p);
                    self.sb_sel += 1;
                }
                if pill(ui, "Eliminar", false).clicked() && has {
                    self.checkpoint();
                    self.project.storyboard.remove(self.sb_sel);
                    self.sb_sel = self.sb_sel.saturating_sub(1);
                }
                if pill(ui, "Usar fotograma actual", false)
                    .on_hover_text("Copia lo que se ve en el fotograma actual de la escena dentro del plano")
                    .clicked()
                    && has
                {
                    self.checkpoint();
                    let d = self.project.flatten(self.frame);
                    self.project.storyboard[self.sb_sel].drawing = Arc::new(d);
                    self.say(format!("Plano {} = fotograma {}", self.sb_sel + 1, self.frame + 1));
                }
                if pill(ui, "Animatic > Xsheet", true)
                    .on_hover_text("Crea una columna «Animatic» con cada plano durante su duración")
                    .clicked()
                {
                    self.animatic();
                }
                ui.separator();
                muted(ui, "Miniaturas");
                ui.add(egui::Slider::new(&mut self.sb_zoom, 80.0..=640.0).show_value(false));
                muted(ui, "Detalle");
                let total: f32 = self.project.storyboard.iter().map(|p| p.dur).sum();
                muted(ui, format!("· {} planos · {:.1} s", self.project.storyboard.len(), total));
            });
        });
        let r = ui.cursor();
        ui.painter().hline(r.x_range(), r.top(), stk(1.0, LINE));
    }

    fn animatic(&mut self) {
        if self.project.storyboard.is_empty() {
            self.say("No hay planos");
            return;
        }
        self.checkpoint();
        let fps = self.project.fps as f32;
        let mut layer = Layer::new("Animatic");
        let mut f = 0u32;
        for p in &self.project.storyboard {
            layer.keys.insert(f, p.drawing.clone());
            f += ((p.dur * fps).round() as u32).max(1);
        }
        self.project.length = self.project.length.max(f);
        self.project.layers.insert(0, layer);
        self.cl = 0;
        self.say(format!("Columna «Animatic» creada: {f} fotogramas. Mírala en la sala Xsheet o Timing."));
    }

    fn sb_grid(&mut self, ui: &mut egui::Ui) {
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        let cw = self.sb_zoom;
        let th = cw * h / w;
        let compact = cw < 110.0;
        let detail = cw >= 300.0;
        let mut select = None;
        let mut open = None;
        let mut drop_on: Option<usize> = None;
        egui::ScrollArea::vertical().id_salt("sb_grid").auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::default().inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                let total = self.project.storyboard.len();
                let per_row = (((ui.available_width() + 12.0) / (cw + 28.0)).floor() as usize).max(1);
                for row in (0..total).step_by(per_row) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0);
                    for i in row..(row + per_row).min(total) {
                        let on = i == self.sb_sel;
                        let frame = egui::Frame::default()
                            .fill(BG3)
                            .corner_radius(6.0)
                            .inner_margin(egui::Margin::same(6))
                            .stroke(stk(2.0, if on { ACCENT } else { BG3 }));
                        let card = frame.show(ui, |ui| ui.vertical(|ui| {
                            ui.set_width(cw);
                            ui.spacing_mut().item_spacing.y = 5.0;
                            let (tr, tresp) = ui.allocate_exact_size(Vec2::new(cw, th), Sense::click_and_drag());
                            let p = &self.project.storyboard[i];
                            paint_thumb(ui.painter(), tr, &p.drawing, w, h, Color32::WHITE);
                            ui.painter().rect_stroke(tr, 2.0, stk(1.0, Color32::BLACK), EStrokeKind::Outside);
                            let badge = Rect::from_min_size(tr.min + Vec2::new(4.0, 4.0), Vec2::new(22.0, 16.0));
                            ui.painter().rect_filled(badge, 3.0, ACCENT);
                            ui.painter().text(badge.center(), Align2::CENTER_CENTER, format!("{}", i + 1), FontId::proportional(11.0), Color32::WHITE);
                            if tresp.clicked() {
                                select = Some(i);
                            }
                            if tresp.double_clicked() {
                                open = Some(i);
                            }
                            if tresp.drag_started() {
                                self.sb_drag = Some(i);
                            }
                            if !compact {
                                let p = &mut self.project.storyboard[i];
                                ui.add(egui::TextEdit::singleline(&mut p.title).desired_width(cw).font(FontId::proportional(12.5)));
                                ui.horizontal(|ui| {
                                    ui.add(egui::DragValue::new(&mut p.dur).range(0.1..=600.0).speed(0.05).max_decimals(1));
                                    ui.label(RichText::new("s").color(MU));
                                });
                            }
                            if detail {
                                let p = &mut self.project.storyboard[i];
                                for (k, name) in FIELDS.iter().enumerate() {
                                    ui.add(egui::TextEdit::multiline(field_mut(p, k)).hint_text(*name).desired_rows(2).desired_width(cw));
                                }
                            }
                        }));
                        if card.response.contains_pointer() && self.sb_drag.is_some() && ui.input(|i| i.pointer.any_released()) {
                            drop_on = Some(i);
                        }
                        if self.sb_drag.is_some() && card.response.contains_pointer() {
                            ui.painter().rect_stroke(card.response.rect, 6.0, stk(2.0, Color32::WHITE), EStrokeKind::Outside);
                        }
                    }
                });
                ui.add_space(12.0);
                }
                ui.label(RichText::new("Doble clic en un plano para editarlo en detalle · arrastra la imagen sobre otro plano para reordenar · mueve el deslizador para pasar de miniaturas a detalle con notas.").size(11.5).color(MU));
            });
        });
        if let (Some(from), Some(to)) = (self.sb_drag, drop_on) {
            if from != to {
                self.checkpoint();
                let p = self.project.storyboard.remove(from);
                self.project.storyboard.insert(to, p);
                self.sb_sel = to;
            }
        }
        if ui.input(|i| i.pointer.any_released()) {
            self.sb_drag = None;
        }
        if let Some(i) = select {
            self.sb_sel = i;
        }
        if let Some(i) = open {
            self.sb_sel = i;
            self.sb_detail = true;
        }
        if self.project.storyboard.is_empty() {
            ui.label(RichText::new("No hay planos todavía: pulsa «+ Plano».").color(MU));
        }
    }

    fn sb_detail_view(&mut self, ui: &mut egui::Ui) {
        let i = self.sb_sel;
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        // Tira de planos abajo
        egui::TopBottomPanel::bottom("sb_strip")
            .frame(egui::Frame::default().fill(BG2).inner_margin(egui::Margin::same(6)))
            .show_inside(ui, |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for j in 0..self.project.storyboard.len() {
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(96.0, 96.0 * h / w), Sense::click());
                            paint_thumb(ui.painter(), r, &self.project.storyboard[j].drawing, w, h, Color32::WHITE);
                            ui.painter().rect_stroke(r, 2.0, if j == i { stk(2.0, ACCENT) } else { stk(1.0, Color32::BLACK) }, EStrokeKind::Outside);
                            if resp.clicked() {
                                self.sb_sel = j;
                            }
                        }
                    });
                });
            });
        // Datos del plano a la derecha
        egui::SidePanel::right("sb_side")
            .exact_width(260.0)
            .resizable(false)
            .frame(egui::Frame::default().fill(BG2).inner_margin(egui::Margin::same(8)))
            .show_inside(ui, |ui| {
                let p = &mut self.project.storyboard[i];
                ui.label(RichText::new(format!("Plano {}", i + 1)).strong().color(ACCENT2));
                ui.add(egui::TextEdit::singleline(&mut p.title).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Duración (s)").color(MU));
                    ui.add(egui::DragValue::new(&mut p.dur).range(0.1..=600.0).speed(0.05).max_decimals(1));
                });
                for (k, name) in FIELDS.iter().enumerate() {
                    ui.label(RichText::new(*name).color(MU));
                    ui.add(egui::TextEdit::multiline(field_mut(p, k)).desired_rows(2).desired_width(f32::INFINITY));
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if pill(ui, "Pincel", !self.sb_erase).clicked() {
                        self.sb_erase = false;
                    }
                    if pill(ui, "Borrador", self.sb_erase).clicked() {
                        self.sb_erase = true;
                    }
                    egui::color_picker::color_edit_button_srgba(ui, &mut self.color, egui::color_picker::Alpha::OnlyBlend);
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Tamaño").color(MU));
                    ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=120.0).logarithmic(true).max_decimals(0));
                });
                ui.checkbox(&mut self.sb_onion, "Ver el plano anterior en transparencia");
                if pill(ui, "Vaciar dibujo", false).clicked() {
                    self.checkpoint();
                    self.project.storyboard[i].drawing = Arc::new(Drawing::default());
                }
            });
        // Lienzo del plano
        egui::CentralPanel::default().frame(egui::Frame::default().fill(VIEWER_BG)).show_inside(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            let resp = ui.allocate_rect(rect, Sense::click_and_drag());
            let z = ((rect.width() - 40.0) / w).min((rect.height() - 40.0) / h).max(0.05);
            let origin = rect.center() - Vec2::new(w, h) * z * 0.5;
            let cam = Rect::from_min_size(origin, Vec2::new(w, h) * z);
            let painter = ui.painter_at(rect);
            painter.rect_filled(cam, 0.0, Color32::WHITE);
            let map = |p: [f32; 2]| Pos2::new(origin.x + p[0] * z, origin.y + p[1] * z);
            let clip = painter.with_clip_rect(cam);
            if self.sb_onion && i > 0 {
                paint_drawing(&clip, &self.project.storyboard[i - 1].drawing.clone(), &map, z, None, 0.25);
            }
            paint_drawing(&clip, &self.project.storyboard[i].drawing.clone(), &map, z, None, 1.0);
            if let Some(s) = &self.sb_stroke {
                paint_stroke(&clip, s, &map, z, None, 1.0);
            }
            painter.rect_stroke(cam, 0.0, stk(1.0, Color32::BLACK), EStrokeKind::Outside);
            let to_scene = |p: Pos2| [(p.x - origin.x) / z, (p.y - origin.y) / z];
            let down = resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
            if down {
                if let Some(pp) = resp.interact_pointer_pos() {
                    let q = to_scene(pp);
                    if self.pre.is_none() {
                        self.pre = Some(self.project.clone());
                        self.changed = false;
                    }
                    if self.sb_erase {
                        let last = self.sb_last.unwrap_or(q);
                        let r = self.eraser_size.max(self.brush_size) * 0.5;
                        let d = Arc::make_mut(&mut self.project.storyboard[i].drawing);
                        for t in 0..=4 {
                            let t = t as f32 / 4.0;
                            if d.erase([last[0] + (q[0] - last[0]) * t, last[1] + (q[1] - last[1]) * t], r) {
                                self.changed = true;
                            }
                        }
                    } else {
                        let s = self.sb_stroke.get_or_insert_with(|| Stroke {
                            kind: ShapeKind::Free,
                            points: vec![],
                            color: rgba(self.color),
                            width: self.brush_size,
                            closed: false,
                            fill: None,
                            pressure: Vec::new(),
                            style: None,
                            original: None,
                        });
                        if s.points.last().map_or(true, |l| (l[0] - q[0]).abs() + (l[1] - q[1]).abs() > 0.8 / z) {
                            s.points.push(q);
                        }
                    }
                    self.sb_last = Some(q);
                }
            } else if self.pre.is_some() {
                if let Some(mut s) = self.sb_stroke.take() {
                    s.points = crate::model::simplify(&s.points, 0.3);
                    Arc::make_mut(&mut self.project.storyboard[i].drawing).strokes.push(s);
                    self.changed = true;
                }
                if let Some(pre) = self.pre.take() {
                    if self.changed {
                        self.push_undo(pre);
                        self.meta_skip = true;
                    }
                }
                self.changed = false;
                self.sb_last = None;
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        });
    }

    /// Panel derecho de la sala Storyboard: datos del plano elegido y exportación a PDF.
    pub(super) fn story_side(&mut self, ui: &mut egui::Ui) {
        if self.project.storyboard.is_empty() {
            muted(ui, "No hay planos todavía.");
            return;
        }
        let i = self.sb_sel.min(self.project.storyboard.len() - 1);
        let total: f32 = self.project.storyboard.iter().map(|p| p.dur).sum();
        muted(ui, format!("{} planos · {:.1} s", self.project.storyboard.len(), total));
        let p = &mut self.project.storyboard[i];
        ui.label(RichText::new(format!("Plano {}", i + 1)).strong().color(ACCENT2));
        ui.add(egui::TextEdit::singleline(&mut p.title).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            ui.label(RichText::new("Duración (s)").color(MU));
            ui.add(egui::DragValue::new(&mut p.dur).range(0.1..=600.0).speed(0.05).max_decimals(1));
        });
        for (k, name) in FIELDS.iter().enumerate() {
            ui.label(RichText::new(*name).color(MU));
            ui.add(egui::TextEdit::multiline(field_mut(p, k)).desired_rows(2).desired_width(f32::INFINITY));
        }
        ui.separator();
        ui.add_enabled(false, egui::Button::new("Exportar storyboard a PDF (pendiente)"));
        muted(ui, "Doble clic en un plano para dibujarlo en detalle.");
    }

    /// Panel derecho de la sala Animatic: planos con su duración y paso a la línea de tiempo.
    pub(super) fn animatic_side(&mut self, ui: &mut egui::Ui) {
        let fps = self.project.fps.max(1) as f32;
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        let mut start = 0.0f32;
        let mut go = None;
        for (i, p) in self.project.storyboard.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(64.0, 64.0 * h / w), Sense::click());
                paint_thumb(ui.painter(), r, &p.drawing, w, h, Color32::WHITE);
                if resp.clicked() {
                    go = Some((i, start));
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("{}  {}", i + 1, p.title)).color(TX));
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut p.dur).range(0.1..=600.0).speed(0.05).max_decimals(1).suffix(" s"));
                        ui.label(RichText::new(format!("cuadro {}", (start * fps).round() as u32 + 1)).size(11.0).color(INK3));
                    });
                });
            });
            start += p.dur;
        }
        if let Some((i, s)) = go {
            self.sb_sel = i;
            self.frame = ((s * fps).round() as u32).min(self.project.length - 1);
        }
        ui.separator();
        if pill(ui, "Animatic > línea de tiempo", true)
            .on_hover_text("Crea una capa «Animatic» con cada plano durante su duración")
            .clicked()
        {
            self.animatic();
        }
        muted(ui, "Las transiciones entre planos están pendientes.");
    }
}
