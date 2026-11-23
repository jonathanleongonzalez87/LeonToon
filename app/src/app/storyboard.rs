//! Sala StoryBoard: cuadrícula de planos con zoom (de miniaturas a detalle con notas). Un plano es una composición
//! (Issue #132): con doble clic se abre en el visor, con las capas, las herramientas y las paletas del programa.
//! También «Animatic → Xsheet».

use super::canvas::paint_thumb;
use super::shell::{muted, panel_header, pill};
use super::*;
use crate::model::Panel;

/// Tamaño de las miniaturas de la cuadrícula (ancho en px): desde thumbnails diminutos hasta planos grandes con notas.
const SB_ZOOM_MIN: f32 = 60.0;
const SB_ZOOM_MAX: f32 = 800.0;
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
        // Si se salió del plano por otro lado (la ruta de navegación, deshacer), se vuelve a la cuadrícula.
        if self.sb_detail && !self.panel_canvas() {
            self.sb_detail = false;
        }
        if self.sb_detail {
            self.sb_panel_bar(ui);
            self.viewer(ui);
            return;
        }
        self.sb_bar(ui);
        self.sb_grid(ui);
    }

    fn sb_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::default().fill(BG2).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
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
                    if let Some(j) = self.project.duplicate_panel(self.sb_sel) {
                        self.sb_sel = j;
                    }
                }
                if pill(ui, "Eliminar", false).clicked() && has {
                    self.checkpoint();
                    if self.project.remove_panel(self.sb_sel) {
                        self.sb_sel = self.sb_sel.saturating_sub(1);
                    }
                }
                if pill(ui, "Usar fotograma actual", false)
                    .on_hover_text("Copia lo que se ve en el fotograma actual de la escena dentro del plano")
                    .clicked()
                    && has
                {
                    self.checkpoint();
                    let d = self.project.flatten(self.frame);
                    self.project.set_panel_drawing(self.sb_sel, d);
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
                ui.add(egui::Slider::new(&mut self.sb_zoom, SB_ZOOM_MIN..=SB_ZOOM_MAX).show_value(false));
                muted(ui, "Detalle");
                if pill(ui, "Ajustar", false).on_hover_text("El tamaño más grande en que caben todos los planos a la vez").clicked() {
                    self.sb_fit = true;
                }
                let total: f32 = self.project.storyboard.iter().map(|p| p.dur).sum();
                muted(ui, format!("· {} planos · {:.1} s", self.project.storyboard.len(), total));
            });
        });
        let r = ui.cursor();
        ui.painter().hline(r.x_range(), r.top(), stk(1.0, LINE));
    }

    /// El animatic como composición: una precomposición «Animatic» con una capa-clip por plano (barra de entrada y salida y
    /// fundidos cruzados). Se entra en ella para verla y editarla en la línea de tiempo de montaje.
    fn animatic_comp(&mut self) {
        if self.project.storyboard.is_empty() {
            self.say("No hay planos");
            return;
        }
        let built = crate::animatic::build(&self.project.storyboard, self.project.fps);
        let length = built.length;
        self.checkpoint();
        // Estando dentro de la composición del animatic no se puede regenerar: se sale primero.
        if self.project.comp_path().iter().any(|n| n == crate::animatic::NAME) {
            self.exit_to_level(0);
        }
        let existed = self.project.layers.iter().any(|l| l.kind == LayerKind::Precomp && l.precomp.and_then(|id| self.project.comp(id)).map_or(false, |c| c.name == crate::animatic::NAME));
        match self.project.set_animatic(built) {
            Some(i) => {
                self.project.length = self.project.length.max(length);
                self.cl = i;
                self.lsel.clear();
                self.say(if existed {
                    format!("Composición «Animatic» actualizada desde el storyboard: {length} fotogramas")
                } else {
                    format!("Composición «Animatic» creada: {length} fotogramas, un clip por plano. Tab para verla por dentro")
                });
            }
            None => self.say("No se puede actualizar el animatic estando dentro de su composición"),
        }
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
        // Tamaño a la medida: Ctrl+rueda sobre la cuadrícula, o «Ajustar» para que quepan todos los planos.
        if ui.rect_contains_pointer(ui.max_rect()) {
            let (cmd, dy) = ui.input(|i| (i.modifiers.command, i.raw_scroll_delta.y));
            if cmd && dy != 0.0 {
                self.sb_zoom = (self.sb_zoom * (1.0 + dy.signum() * 0.1)).clamp(SB_ZOOM_MIN, SB_ZOOM_MAX);
                ui.input_mut(|i| {
                    i.smooth_scroll_delta = Vec2::ZERO;
                    i.raw_scroll_delta = Vec2::ZERO;
                });
            }
        }
        if self.sb_fit {
            self.sb_fit = false;
            let avail = ui.available_size();
            let n = self.project.storyboard.len().max(1);
            let mut best = SB_ZOOM_MIN;
            let mut cw = SB_ZOOM_MAX;
            while cw >= SB_ZOOM_MIN {
                let per_row = (((avail.x - 28.0 + 12.0) / (cw + 28.0)).floor() as usize).max(1);
                let rows = n.div_ceil(per_row);
                let extra = if cw < 110.0 { 16.0 } else if cw < 300.0 { 62.0 } else { 62.0 + 5.0 * 52.0 };
                if rows as f32 * (cw * h / w + extra + 24.0) + 28.0 <= avail.y {
                    best = cw;
                    break;
                }
                cw -= 10.0;
            }
            self.sb_zoom = best;
        }
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
                            // Con una herramienta de dibujo, la miniatura es un lienzo (thumbnailing): se dibuja encima con el Lápiz,
                            // el Pincel, las formas, el Borrador o el Bote. El plano se abre mientras dura el gesto y se cierra al soltar.
                            let drawing_tool = matches!(self.tool, Tool::Brush | Tool::Vector | Tool::Shape | Tool::Eraser | Tool::Fill);
                            if self.sb_live == Some(i) {
                                self.paint_scene(&ui.painter_at(tr), tr.min, cw / w, 0, true, false);
                            } else {
                                paint_thumb(ui.painter(), tr, &self.project.storyboard[i].drawing, w, h, Color32::WHITE);
                            }
                            ui.painter().rect_stroke(tr, 2.0, stk(1.0, Color32::BLACK), EStrokeKind::Outside);
                            // El número es el asa del plano: clic lo abre, arrastrarlo lo reordena.
                            let badge = Rect::from_min_size(tr.min + Vec2::new(4.0, 4.0), Vec2::new(22.0, 16.0));
                            let bresp = ui.interact(badge, ui.id().with(("sb_badge", i)), Sense::click_and_drag());
                            ui.painter().rect_filled(badge, 3.0, if bresp.hovered() { ACCENT2 } else { ACCENT });
                            ui.painter().text(badge.center(), Align2::CENTER_CENTER, format!("{}", i + 1), FontId::proportional(11.0), Color32::WHITE);
                            if bresp.hovered() {
                                bresp.clone().on_hover_text("Clic: abrir el plano · arrastrar: reordenar");
                            }
                            if bresp.clicked() {
                                select = Some(i);
                                open = Some(i);
                            }
                            if bresp.drag_started() {
                                self.sb_drag = Some(i);
                            }
                            if drawing_tool {
                                let ctx = ui.ctx().clone();
                                let on_thumb = !bresp.hovered() && !bresp.dragged();
                                if tresp.is_pointer_button_down_on() && ctx.input(|i| i.pointer.primary_down()) && on_thumb {
                                    if let Some(pp) = ctx.input(|i| i.pointer.interact_pos()) {
                                        let z = cw / w;
                                        let cp = [(pp.x - tr.min.x) / z, (pp.y - tr.min.y) / z];
                                        let (shift, command) = ctx.input(|i| (i.modifiers.shift, i.modifiers.command));
                                        if self.sb_live.is_none() && ctx.input(|i| i.pointer.primary_pressed()) {
                                            self.begin_panel_stroke(i, cp, shift, command);
                                        } else if self.sb_live == Some(i) {
                                            self.continue_gesture(cp, shift);
                                        }
                                    }
                                } else if self.sb_live == Some(i) {
                                    self.end_panel_stroke();
                                }
                                if tresp.clicked() || tresp.drag_started() {
                                    select = Some(i);
                                }
                            } else {
                                if tresp.clicked() {
                                    select = Some(i);
                                }
                                if tresp.double_clicked() {
                                    open = Some(i);
                                }
                                if tresp.drag_started() {
                                    self.sb_drag = Some(i);
                                }
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
                ui.label(RichText::new("Con el Lápiz, el Pincel o las formas se dibuja directamente sobre las miniaturas (thumbnails) · clic en el número abre el plano con el visor y las capas · arrastra el número sobre otro plano para reordenar · Ctrl+rueda o el deslizador cambian el tamaño.").size(11.5).color(MU));
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
            self.open_panel(i);
        }
        if self.project.storyboard.is_empty() {
            ui.label(RichText::new("No hay planos todavía: pulsa «+ Plano».").color(MU));
        }
    }

    /// ¿Se está dibujando dentro de un plano? (La sala Storyboard muestra entonces el visor con las capas del plano.)
    pub(super) fn panel_canvas(&self) -> bool {
        self.room_kind().def().story && self.sb_detail && self.project.stack.len() > self.sb_base
    }

    /// Abre el plano `i` para editarlo con el visor, las capas y las herramientas (su composición pasa a ser la que se edita).
    pub(super) fn open_panel(&mut self, i: usize) {
        if i >= self.project.storyboard.len() {
            return;
        }
        self.sb_sel = i;
        self.sb_base = self.project.stack.len();
        self.checkpoint();
        if self.project.open_panel(i, self.cl, self.frame) {
            self.reset_after_level_change();
            self.frame = 0;
            self.sb_detail = true;
            self.need_fit = true;
            self.say(format!("Plano {}: dibuja con el visor y las capas. «Cuadrícula» vuelve a todos los planos", i + 1));
        }
    }

    /// Empieza un trazo sobre la miniatura del plano `i`: el plano se abre (sin tocar el historial) y el gesto sigue las herramientas
    /// de dibujo de siempre. `cp` está en coordenadas del lienzo del plano.
    pub(super) fn begin_panel_stroke(&mut self, i: usize, cp: [f32; 2], shift: bool, command: bool) {
        let outer = self.project.clone();
        self.sb_sel = i;
        self.sb_base = self.project.stack.len();
        if !self.project.open_panel(i, self.cl, self.frame) {
            return;
        }
        self.reset_after_level_change();
        self.frame = 0;
        self.sb_outer = Some(outer);
        self.sb_live = Some(i);
        self.meta_skip = false;
        self.begin_gesture(cp, shift, command);
        if self.gesture.is_none() {
            // Herramientas de un solo clic (Bote) o gestos que no empezaron: se cierra enseguida.
            self.end_panel_stroke();
        }
    }

    /// Termina el trazo de la miniatura: se confirma, se sale del plano (su miniatura se rehace) y Deshacer vuelve a la escena
    /// tal como estaba antes del trazo, sin entrar en el plano.
    pub(super) fn end_panel_stroke(&mut self) {
        if self.gesture.is_some() {
            self.finish_gesture();
        }
        let pushed = self.meta_skip;
        if let Some((cl, frame)) = self.project.exit_comp() {
            self.reset_after_level_change();
            self.cl = cl.min(self.project.layers.len().saturating_sub(1));
            self.frame = frame.min(self.project.length.saturating_sub(1));
        }
        if let Some(outer) = self.sb_outer.take() {
            if pushed {
                if let Some(last) = self.undo.last_mut() {
                    *last = outer;
                }
            }
        }
        self.sb_live = None;
    }

    /// Sale del plano que se edita y vuelve a la cuadrícula.
    pub(super) fn close_panel(&mut self) {
        self.exit_to_level(self.sb_base);
        self.sb_detail = false;
    }

    /// Barra del plano abierto: volver a la cuadrícula e ir al plano anterior o siguiente.
    fn sb_panel_bar(&mut self, ui: &mut egui::Ui) {
        let n = self.project.storyboard.len();
        let i = self.sb_sel.min(n.saturating_sub(1));
        let mut go: Option<usize> = None;
        let mut back = false;
        egui::Frame::default().fill(BG2).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if pill(ui, "< Cuadrícula", false).clicked() {
                    back = true;
                }
                if pill(ui, "< Plano anterior", false).clicked() && i > 0 {
                    go = Some(i - 1);
                }
                if pill(ui, "Plano siguiente >", false).clicked() && i + 1 < n {
                    go = Some(i + 1);
                }
                ui.separator();
                let title = self.project.storyboard.get(i).map(|p| p.title.clone()).unwrap_or_default();
                muted(ui, format!("Plano {} de {n} · {title}", i + 1));
            });
        });
        if back {
            self.close_panel();
        } else if let Some(j) = go {
            self.close_panel();
            self.open_panel(j);
        }
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
        if self.panel_canvas() {
            ui.add_space(6.0);
            panel_header(ui, "Paleta");
            self.palette_groups(ui);
        } else {
            muted(ui, "Dibuja sobre las miniaturas o abre un plano (clic en su número) para usar el visor y las capas.");
        }
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
                        let fade = crate::animatic::is_fade(&p.trans);
                        egui::ComboBox::from_id_salt(("plano_trans", i)).selected_text(if fade { "Fundido" } else { "Corte" }).width(76.0).show_ui(ui, |ui| {
                            if ui.selectable_label(!fade, "Corte").clicked() && fade {
                                p.trans = "Corte".into();
                            }
                            if ui.selectable_label(fade, "Fundido").clicked() && !fade {
                                p.trans = "Fundido".into();
                            }
                        });
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
            .on_hover_text("Crea (o actualiza) la composición «Animatic»: una capa-clip por plano, con su barra y sus fundidos. Se edita en la línea de tiempo de montaje")
            .clicked()
        {
            self.animatic_comp();
        }
        if pill(ui, "Animatic > Xsheet", false).on_hover_text("Crea una columna «Animatic» con cada plano durante su duración (un dibujo por exposición)").clicked() {
            self.animatic();
        }
        muted(ui, "Fundido: el plano entra en fundido cruzado sobre el anterior (hasta medio segundo). Corte: sin transición.");
    }
}
