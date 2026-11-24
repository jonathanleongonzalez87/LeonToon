//! Sala StoryBoard: cuadrícula de planos con zoom (de miniaturas a detalle con notas). Un plano es una composición
//! (Issue #132): con doble clic se abre en el visor, con las capas, las herramientas y las paletas del programa.
//! También «Crear animatic» (la composición del animatic, con un clip vivo por plano).

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
                if pill(ui, "PDF", false).on_hover_text("Exportar el storyboard a PDF").clicked() {
                    self.export_storyboard_pdf();
                }
                if pill(ui, "Crear animatic", true)
                    .on_hover_text("Crea (o actualiza) la composición «Animatic»: un clip por plano, en el orden y con la duración de la línea de tiempo del storyboard, con sus fundidos y su cámara. Tab entra en ella")
                    .clicked()
                {
                    self.create_animatic_and_open();
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
    /// Crea el animatic al entrar en la sala Animatic si hay planos y todavía no existe (y se está en la composición principal): se
    /// crea la composición y se entra en ella, para que la línea de tiempo de montaje muestre los clips.
    pub(super) fn ensure_animatic(&mut self) {
        if self.project.storyboard.is_empty() || self.project.in_comp() || self.project.has_animatic() {
            return;
        }
        self.animatic_comp();
        self.open_animatic_level();
    }

    /// Entra en la composición «Animatic» (si se está fuera de toda composición y existe).
    fn open_animatic_level(&mut self) {
        if self.project.in_comp() {
            return;
        }
        let found = self.project.layers.iter().position(|l| l.kind == LayerKind::Precomp && l.precomp.and_then(|id| self.project.comp(id)).map_or(false, |c| c.name == crate::animatic::NAME));
        if let Some(i) = found {
            self.enter_precomp(i);
        }
    }

    /// El botón «Crear animatic»: crea o actualiza la composición y abre la sala Animatic dentro de ella.
    pub(super) fn create_animatic_and_open(&mut self) {
        self.animatic_comp();
        self.goto_room_kind(RoomKind::Animatic);
        self.open_animatic_level();
    }

    fn animatic_comp(&mut self) {
        if self.project.storyboard.is_empty() {
            self.say("No hay planos");
            return;
        }
        let built = self.project.build_animatic();
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
    /// La miniatura de un plano como lienzo (la usan la cuadrícula y la columna de miniaturas): con una herramienta de dibujo se
    /// dibuja encima; el número es el asa (clic abre el plano, arrastrarlo lo reordena). Devuelve por select y open lo que se pidió.
    fn panel_thumb(&mut self, ui: &mut egui::Ui, i: usize, cw: f32, select: &mut Option<usize>, open: &mut Option<usize>) {
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        let th = cw * h / w;
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
                            let label = self.project.storyboard[i].label(i);
                            let badge = Rect::from_min_size(tr.min + Vec2::new(4.0, 4.0), Vec2::new((label.chars().count() as f32 * 6.5 + 10.0).max(22.0), 16.0));
                            let bresp = ui.interact(badge, ui.id().with(("sb_badge", i)), Sense::click_and_drag());
                            ui.painter().rect_filled(badge, 3.0, if bresp.hovered() { ACCENT2 } else { ACCENT });
                            ui.painter().text(badge.center(), Align2::CENTER_CENTER, label.clone(), FontId::proportional(11.0), Color32::WHITE);
                            if bresp.hovered() {
                                bresp.clone().on_hover_text("Clic: abrir el plano · arrastrar: reordenar");
                            }
                            if bresp.clicked() {
                                *select = Some(i);
                                *open = Some(i);
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
                                    *select = Some(i);
                                }
                            } else {
                                if tresp.clicked() {
                                    *select = Some(i);
                                }
                                if tresp.double_clicked() {
                                    *open = Some(i);
                                }
                                if tresp.drag_started() {
                                    self.sb_drag = Some(i);
                                }
                            }
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
                            self.panel_thumb(ui, i, cw, &mut select, &mut open);
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
    /// Exporta el storyboard a PDF: pide dónde guardarlo y escribe una hoja A4 por cada seis planos.
    pub(super) fn export_storyboard_pdf(&mut self) {
        let name = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| format!("{}_storyboard.pdf", s.to_string_lossy())).unwrap_or_else(|| "storyboard.pdf".into());
        if let Some(path) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(name).save_file() {
            self.write_storyboard_pdf(&path);
        }
    }

    /// Escribe el PDF del storyboard en `path` y avisa del resultado.
    pub(super) fn write_storyboard_pdf(&mut self, path: &std::path::Path) {
        let title = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Storyboard".into());
        // Se exporta la composición principal aunque se esté dentro de un plano.
        let root = self.project.root_clone();
        match crate::pdf::storyboard_pdf(&root, &title).and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string())) {
            Ok(()) => self.say(format!("Storyboard exportado: {} ({} planos)", path.display(), root.storyboard.len())),
            Err(e) => self.say(format!("No se pudo exportar el PDF: {e}")),
        }
    }

    // ---------- Línea de tiempo del storyboard (Issue #133, fase S2) ----------

    /// Duración de un plano en segundos, medida en cuadros enteros (la que usa el animatic).
    fn panel_secs(&self, i: usize) -> f32 {
        let fps = self.project.fps.max(1);
        crate::animatic::panel_frames(&self.project.storyboard[i], fps) as f32 / fps as f32
    }

    fn sbt_total(&self) -> f32 {
        (0..self.project.storyboard.len()).map(|i| self.panel_secs(i)).sum()
    }

    /// Firma de lo que cambia el animatic (duraciones, transiciones, cámara, orden y composiciones de los planos).
    fn animatic_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.project.fps, self.project.width, self.project.storyboard.len()).hash(&mut h);
        for p in &self.project.storyboard {
            (p.dur.to_bits(), &p.trans, &p.title, p.comp).hash(&mut h);
            match p.cam {
                Some(c) => {
                    for v in [c.s0, c.x0, c.y0, c.s1, c.x1, c.y1] {
                        v.to_bits().hash(&mut h);
                    }
                    c.ease.hash(&mut h);
                }
                None => 0u8.hash(&mut h),
            }
        }
        h.finish()
    }

    /// Mantiene el animatic al día: si hay una composición «Animatic» y los planos cambiaron (duración, transición, cámara, orden…),
    /// la rehace. Un proyecto recién abierto solo toma nota de cómo está: no se toca lo que la persona hizo en la composición.
    pub(super) fn sync_animatic(&mut self) {
        let sig = self.animatic_signature();
        match self.anim_sig {
            Some(s) if s == sig => {}
            Some(_) => {
                self.anim_sig = Some(sig);
                if self.project.refresh_animatic() {
                    self.frame = self.frame.min(self.project.length.saturating_sub(1));
                }
            }
            None => self.anim_sig = Some(sig),
        }
    }

    /// Botones de la línea de tiempo del storyboard: el transporte (en la sala Animatic), las operaciones de plano y el zoom del tiempo.
    fn sbt_bar(&mut self, ui: &mut egui::Ui, animatic: bool) {
        egui::Frame::default().fill(BG2).inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if animatic {
                    let n = self.project.length;
                    ui.spacing_mut().item_spacing.x = 1.0;
                    if icon_button(ui, 22.0, false, "Primer cuadro (Inicio)", draw_first).clicked() {
                        self.playing = false;
                        self.frame = 0;
                    }
                    if icon_button(ui, 22.0, false, "Cuadro anterior ( , )", draw_prev).clicked() {
                        self.step(-1);
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
                    if icon_button(ui, 22.0, false, "Cuadro siguiente ( . )", draw_next).clicked() {
                        self.step(1);
                    }
                    if icon_button(ui, 22.0, false, "Último cuadro (Fin)", draw_last).clicked() {
                        self.playing = false;
                        self.frame = n.saturating_sub(1);
                    }
                    if icon_button(ui, 22.0, self.loop_play, "Bucle", draw_loop).clicked() {
                        self.loop_play = !self.loop_play;
                    }
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let fps = self.project.fps.max(1) as f32;
                    ui.label(RichText::new(format!("{:.1} / {:.1} s · cuadro {} de {}", self.frame as f32 / fps, n as f32 / fps, self.frame + 1, n)).size(12.0).color(TX));
                    ui.separator();
                }
                let n = self.project.storyboard.len();
                let i = self.sb_sel.min(n.saturating_sub(1));
                if pill(ui, "+ Plano", false).clicked() {
                    self.checkpoint();
                    let at = if n == 0 { 0 } else { (i + 1).min(n) };
                    self.project.storyboard.insert(at, Panel::new(n + 1));
                    self.sb_sel = at;
                }
                if pill(ui, "Duplicar", false).clicked() && n > 0 {
                    self.checkpoint();
                    if let Some(j) = self.project.duplicate_panel(i) {
                        self.sb_sel = j;
                    }
                }
                if pill(ui, "Dividir", false).on_hover_text("Parte el plano en dos por la cabeza de reproducción (clic en la regla o en el plano)").clicked() && n > 0 {
                    self.split_panel(i);
                }
                if pill(ui, "Eliminar", false).clicked() && n > 0 {
                    self.checkpoint();
                    if self.project.remove_panel(i) {
                        self.sb_sel = i.saturating_sub(1);
                    }
                }
                if pill(ui, "◀ Mover", false).on_hover_text("Intercambia el plano con el anterior").clicked() && i > 0 {
                    self.checkpoint();
                    self.project.storyboard.swap(i, i - 1);
                    self.sb_sel = i - 1;
                }
                if pill(ui, "Mover ▶", false).on_hover_text("Intercambia el plano con el siguiente").clicked() && i + 1 < n {
                    self.checkpoint();
                    self.project.storyboard.swap(i, i + 1);
                    self.sb_sel = i + 1;
                }
                if pill(ui, "+ Audio", false).on_hover_text("Importa un audio (WAV, MP3, FLAC, OGG o AAC) como capa de audio, desde el cuadro actual").clicked() {
                    self.import_audio();
                }
                ui.separator();
                muted(ui, "Zoom ↔");
                ui.add(egui::Slider::new(&mut self.sbt_pps, 20.0..=400.0).show_value(false)).on_hover_text("Zoom horizontal: segundos por pantalla (Ctrl + rueda)");
                muted(ui, "↕");
                ui.add(egui::Slider::new(&mut self.sbt_vz, 0.5..=3.0).show_value(false)).on_hover_text("Zoom vertical: alto de los planos (Alt + rueda)");
                muted(ui, format!("· {} planos · {:.1} s", n, self.sbt_total()));
            });
        });
    }

    /// Parte el plano `i` en dos por la cabeza de reproducción: la copia es el trozo de la derecha.
    fn split_panel(&mut self, i: usize) {
        let fps = self.project.fps.max(1) as f32;
        if self.room_kind() == RoomKind::Animatic {
            self.sbt_t = self.frame as f32 / fps;
        }
        let start: f32 = (0..i).map(|k| self.panel_secs(k)).sum();
        let dur = self.panel_secs(i);
        let (off_f, dur_f) = (((self.sbt_t - start) * fps).round(), (dur * fps).round());
        if off_f < 1.0 || off_f > dur_f - 1.0 {
            self.say("Para dividir, pon la cabeza de reproducción dentro del plano (clic en la regla o en el plano), al menos un cuadro del borde");
            return;
        }
        self.checkpoint();
        if let Some(j) = self.project.duplicate_panel(i) {
            let title = self.project.storyboard[i].title.clone();
            self.project.storyboard[i].dur = off_f / fps;
            self.project.storyboard[j].dur = (dur_f - off_f) / fps;
            self.project.storyboard[j].title = format!("{title} (2)");
            self.sb_sel = j;
        }
    }

    /// La línea de tiempo del storyboard: una regla en segundos, la tira de planos (miniatura, número, título y duración; se
    /// arrastra el borde derecho para cambiar la duración y el cuerpo para reordenar), la pista de cámara y la de transiciones.
    /// En la sala Animatic la cabeza de reproducción es el cuadro actual (se reproduce y se arrastra por la regla) y el animatic
    /// se rehace solo al cambiar los planos.
    pub(super) fn sb_timeline_editor(&mut self, ui: &mut egui::Ui) {
        let animatic = self.room_kind() == RoomKind::Animatic;
        self.sbt_bar(ui, animatic);
        let n = self.project.storyboard.len();
        if n == 0 {
            muted(ui, "No hay planos todavía: pulsa «+ Plano».");
            return;
        }
        let fps = self.project.fps.max(1) as f32;
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        let total = self.sbt_total();
        if !animatic {
            self.sbt_t = self.sbt_t.clamp(0.0, total);
        }
        let head_t = if animatic { (self.frame as f32 / fps).min(total) } else { self.sbt_t };
        let mut select: Option<usize> = None;
        let mut open: Option<usize> = None;
        let mut cam_toggle: Option<usize> = None;
        let mut set_t: Option<f32> = None;
        let mut resize: Option<(usize, f32)> = None;
        let mut resize_start = false;
        let mut drop_to: Option<(usize, usize)> = None;
        let mut trans_set: Option<(usize, &'static str)> = None;
        let playing = self.playing;
        // Pista de audio: las capas de audio de la composición, con su onda.
        let audio_items = crate::audio::collect(&self.project);
        let audio_h = if audio_items.is_empty() { 0.0 } else { 40.0 };
        // Ctrl + rueda: zoom horizontal; Alt + rueda: zoom vertical. La rueda sola desplaza (con Mayús, de lado).
        let area = ui.available_rect_before_wrap();
        let (ctrl, alt, dy) = ui.input(|i| (i.modifiers.command, i.modifiers.alt, i.raw_scroll_delta.y));
        if ui.rect_contains_pointer(area) && (ctrl || alt) && dy != 0.0 {
            if alt {
                self.sbt_vz = (self.sbt_vz * (1.0 + dy * 0.002)).clamp(0.5, 3.0);
            } else {
                self.sbt_pps = (self.sbt_pps * (1.0 + dy * 0.002)).clamp(20.0, 400.0);
            }
            ui.input_mut(|i| {
                i.smooth_scroll_delta = Vec2::ZERO;
                i.raw_scroll_delta = Vec2::ZERO;
            });
        }
        let pps = self.sbt_pps;
        let vz = self.sbt_vz;
        egui::ScrollArea::both().id_salt("sb_timeline").auto_shrink([false, false]).show(ui, |ui| {
            let (ruler_h, cam_h, trans_h) = (20.0, 20.0, 24.0);
            let clip_h = ((ui.available_height() - ruler_h - cam_h - trans_h - 18.0).clamp(46.0, 120.0) * vz).clamp(30.0, 360.0);
            let width = (total * pps + 120.0).max(ui.available_width());
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, ruler_h + clip_h + cam_h + trans_h + audio_h + 12.0), Sense::hover());
            let painter = ui.painter_at(rect);
            let id = ui.id();
            // Regla en segundos: clic o arrastre mueven la cabeza de reproducción.
            let ruler = Rect::from_min_size(rect.min, Vec2::new(width, ruler_h));
            painter.rect_filled(ruler, 0.0, BG3);
            let step = if pps < 40.0 { 5 } else { 1 };
            let mut s = 0;
            while (s as f32) <= total + 1.0 {
                let x = rect.min.x + s as f32 * pps;
                painter.line_segment([Pos2::new(x, ruler.max.y - 7.0), Pos2::new(x, ruler.max.y)], stk(1.0, MU));
                painter.text(Pos2::new(x + 3.0, ruler.min.y + 8.0), Align2::LEFT_CENTER, format!("{s} s"), FontId::proportional(10.0), MU);
                s += step;
            }
            let rresp = ui.interact(ruler, id.with("sbt_ruler"), Sense::click_and_drag());
            if rresp.clicked() || rresp.dragged() {
                if let Some(pp) = rresp.interact_pointer_pos() {
                    set_t = Some(((pp.x - rect.min.x) / pps).clamp(0.0, total));
                }
            }
            let top = rect.min.y + ruler_h + 3.0;
            let cam_top = top + clip_h + 4.0;
            let trans_top = cam_top + cam_h + 3.0;
            let mut x = rect.min.x;
            let mut bounds: Vec<(f32, f32)> = Vec::with_capacity(n);
            for i in 0..n {
                let (drawing, title, cam, trans) = {
                    let p = &self.project.storyboard[i];
                    (p.drawing.clone(), p.title.clone(), p.cam, p.trans.clone())
                };
                let dur = self.panel_secs(i);
                let cwid = (dur * pps).max(10.0);
                let r = Rect::from_min_size(Pos2::new(x, top), Vec2::new(cwid, clip_h));
                bounds.push((r.min.x, r.max.x));
                let on = i == self.sb_sel;
                painter.rect_filled(r.shrink(1.0), 4.0, BG3);
                let (avail_w, avail_h) = ((cwid - 10.0).max(6.0), (clip_h - 26.0).max(10.0));
                let tw = (avail_h * w / h).min(avail_w);
                let tr = Rect::from_min_size(r.min + Vec2::new(5.0, 5.0), Vec2::new(tw, tw * h / w));
                paint_thumb(&painter, tr, &drawing, w, h, Color32::WHITE);
                painter.rect_stroke(tr, 1.0, stk(1.0, Color32::BLACK), EStrokeKind::Outside);
                let label = self.project.storyboard[i].label(i);
                let badge = Rect::from_min_size(tr.min + Vec2::new(2.0, 2.0), Vec2::new((label.chars().count() as f32 * 5.8 + 8.0).max(18.0), 13.0));
                painter.rect_filled(badge, 3.0, ACCENT);
                painter.text(badge.center(), Align2::CENTER_CENTER, label, FontId::proportional(10.0), Color32::WHITE);
                painter.with_clip_rect(r.shrink(2.0)).text(Pos2::new(r.min.x + 6.0, r.max.y - 11.0), Align2::LEFT_CENTER, format!("{title} · {dur:.1} s"), FontId::proportional(11.0), TX);
                painter.rect_stroke(r.shrink(1.0), 4.0, stk(2.0, if on { ACCENT } else { LINE }), EStrokeKind::Inside);
                // Cuerpo del plano (clic elige y pone la cabeza, doble clic abre, arrastrar reordena) y borde derecho (duración).
                let edge = Rect::from_min_max(Pos2::new(r.max.x - 7.0, r.min.y), r.max);
                let body = Rect::from_min_max(r.min, Pos2::new(r.max.x - 7.0, r.max.y));
                let br = ui.interact(body, id.with(("sbt_clip", i)), Sense::click_and_drag());
                let er = ui.interact(edge, id.with(("sbt_edge", i)), Sense::drag());
                if er.hovered() || er.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    painter.rect_filled(edge.shrink2(Vec2::new(1.0, 4.0)), 2.0, ACCENT);
                }
                if er.drag_started() {
                    resize_start = true;
                    select = Some(i);
                }
                if er.dragged() {
                    if let Some(pp) = er.interact_pointer_pos() {
                        let frames = (((pp.x - r.min.x) / pps) * fps).round().max(1.0);
                        resize = Some((i, frames / fps));
                    }
                }
                if br.clicked() {
                    select = Some(i);
                    if let Some(pp) = br.interact_pointer_pos() {
                        set_t = Some(((pp.x - rect.min.x) / pps).clamp(0.0, total));
                    }
                }
                if br.double_clicked() {
                    open = Some(i);
                }
                if br.drag_started() {
                    select = Some(i);
                    self.sbt_drag = Some(i);
                }
                // Pista de cámara: una barra por plano con una clave al principio y otra al final si tiene movimiento.
                let cr = Rect::from_min_size(Pos2::new(x, cam_top), Vec2::new(cwid, cam_h));
                let bar = cr.shrink2(Vec2::new(1.0, 5.0));
                if cam.is_some() {
                    painter.rect_filled(bar, 3.0, ACCENT.gamma_multiply(0.55));
                    for kx in [bar.min.x + 2.0, bar.max.x - 2.0] {
                        let c = Pos2::new(kx, bar.center().y);
                        let d = 5.0;
                        painter.add(egui::Shape::convex_polygon(vec![c + Vec2::new(0.0, -d), c + Vec2::new(d, 0.0), c + Vec2::new(0.0, d), c + Vec2::new(-d, 0.0)], Color32::from_rgb(0xff, 0xd2, 0x66), stk(1.0, Color32::BLACK)));
                    }
                } else {
                    painter.rect_filled(bar, 3.0, BG3.gamma_multiply(0.7));
                }
                let cresp = ui.interact(cr, id.with(("sbt_cam", i)), Sense::click());
                if cresp.clicked() {
                    select = Some(i);
                }
                if cresp.double_clicked() {
                    cam_toggle = Some(i);
                }
                cresp.on_hover_text("Cámara del plano. Doble clic: añadir o quitar un movimiento (zoom y desplazamiento); se edita en el panel «Plano»");
                // Pista de transiciones: la que une este plano con el anterior (el primero no tiene).
                if i > 0 && cwid >= 56.0 {
                    let chip = Rect::from_min_size(Pos2::new(x + 2.0, trans_top), Vec2::new((cwid - 4.0).min(118.0), trans_h - 2.0));
                    let cur = crate::animatic::kind(&trans);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(chip), |ui| {
                        egui::ComboBox::from_id_salt(("sbt_trans", i)).selected_text(crate::animatic::trans_name(cur)).width(chip.width() - 22.0).show_ui(ui, |ui| {
                            for (k, name) in crate::animatic::TRANSITIONS {
                                if ui.selectable_label(k == cur, name).clicked() && k != cur {
                                    trans_set = Some((i, name));
                                }
                            }
                        });
                    });
                } else if i > 0 {
                    painter.text(Pos2::new(x + 2.0, trans_top + 10.0), Align2::LEFT_CENTER, "·", FontId::proportional(11.0), MU);
                }
                x += cwid;
            }
            // Pista de audio: una barra por audio, con su onda (se mueven y se recortan en la línea de tiempo de montaje).
            for it in &audio_items {
                let bar = Rect::from_min_size(Pos2::new(rect.min.x + it.start * pps, trans_top + trans_h + 3.0), Vec2::new((it.len * pps).max(4.0), audio_h - 4.0));
                painter.rect_filled(bar, 3.0, Color32::from_rgb(0x6f, 0xc3, 0xd8).gamma_multiply(0.45));
                painter.rect_stroke(bar, 3.0, stk(1.0, Color32::from_rgb(0x6f, 0xc3, 0xd8)), EStrokeKind::Inside);
                if let Some(e) = self.audio.cache.get(&it.path) {
                    super::audioui::paint_wave(&painter, bar, e, 1.0 / pps, 0.0, Color32::from_black_alpha(170));
                }
                let name = it.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                painter.with_clip_rect(bar).text(bar.min + Vec2::new(4.0, 2.0), Align2::LEFT_TOP, name, FontId::proportional(10.0), TX);
            }
            // Reordenar: el plano arrastrado se suelta antes o después de otro, según dónde esté el cursor.
            if let Some(from) = self.sbt_drag {
                if let Some(pp) = ui.input(|i| i.pointer.interact_pos()) {
                    let mut to = 0;
                    let mut mx = bounds.first().map_or(rect.min.x, |b| b.0);
                    for (k, (x0, x1)) in bounds.iter().enumerate() {
                        if pp.x > (x0 + x1) * 0.5 {
                            to = k + 1;
                            mx = *x1;
                        }
                    }
                    painter.line_segment([Pos2::new(mx, top - 2.0), Pos2::new(mx, top + clip_h + 2.0)], stk(3.0, Color32::WHITE));
                    if ui.input(|i| i.pointer.any_released()) {
                        drop_to = Some((from, to));
                    }
                }
            }
            // Cabeza de reproducción.
            let px = rect.min.x + head_t * pps;
            painter.line_segment([Pos2::new(px, rect.min.y), Pos2::new(px, rect.max.y)], stk(1.5, ACCENT));
            painter.add(egui::Shape::convex_polygon(vec![Pos2::new(px - 5.0, rect.min.y), Pos2::new(px + 5.0, rect.min.y), Pos2::new(px, rect.min.y + 8.0)], ACCENT, egui::Stroke::NONE));
            // Al reproducir, la tira acompaña a la cabeza.
            if animatic && playing {
                ui.scroll_to_rect(Rect::from_center_size(Pos2::new(px, rect.center().y), Vec2::new(160.0, 10.0)), None);
            }
        });
        if let Some(t) = set_t {
            if animatic {
                self.playing = false;
                self.frame = ((t * fps).floor().max(0.0) as u32).min(self.project.length.saturating_sub(1));
            } else {
                self.sbt_t = t;
            }
        }
        if let Some(i) = select {
            self.sb_sel = i;
        }
        if resize_start {
            self.checkpoint();
        }
        if let Some((i, d)) = resize {
            self.project.storyboard[i].dur = d;
        }
        if let Some((i, name)) = trans_set {
            self.checkpoint();
            self.project.storyboard[i].trans = name.to_string();
            self.sb_sel = i;
        }
        if let Some(i) = cam_toggle {
            self.checkpoint();
            let p = &mut self.project.storyboard[i];
            p.cam = if p.cam.is_some() { None } else { Some(crate::model::CamMove::default()) };
            self.sb_sel = i;
        }
        if let Some((from, to)) = drop_to {
            let dest = if to > from { to - 1 } else { to };
            if dest != from {
                self.checkpoint();
                let p = self.project.storyboard.remove(from);
                self.project.storyboard.insert(dest, p);
                self.sb_sel = dest;
            }
        }
        if ui.input(|i| i.pointer.any_released()) {
            self.sbt_drag = None;
        }
        if let Some(i) = open {
            self.open_panel(i);
        }
    }
    /// Las miniaturas de los planos en una columna vertical (como en Storyboard Pro). Se dibuja sobre ellas igual que en la
    /// cuadrícula; el número abre el plano o, arrastrado, lo reordena.
    pub(super) fn sb_column_editor(&mut self, ui: &mut egui::Ui) {
        let n = self.project.storyboard.len();
        if n == 0 {
            muted(ui, "No hay planos todavía.");
            return;
        }
        let cw = (ui.available_width() - 28.0).clamp(60.0, 600.0);
        let mut select: Option<usize> = None;
        let mut open: Option<usize> = None;
        let mut drop_on: Option<usize> = None;
        egui::ScrollArea::vertical().id_salt("sb_column").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            for i in 0..n {
                let on = i == self.sb_sel;
                let frame = egui::Frame::default().fill(BG3).corner_radius(6.0).inner_margin(egui::Margin::same(6)).stroke(stk(2.0, if on { ACCENT } else { BG3 }));
                let card = frame.show(ui, |ui| {
                    ui.set_width(cw);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    self.panel_thumb(ui, i, cw, &mut select, &mut open);
                    let (title, dur) = {
                        let p = &self.project.storyboard[i];
                        (p.title.clone(), p.dur)
                    };
                    ui.label(RichText::new(format!("{title} · {dur:.1} s")).size(11.5).color(TX));
                });
                if self.sb_drag.is_some() {
                    if card.response.contains_pointer() {
                        ui.painter().rect_stroke(card.response.rect, 6.0, stk(2.0, Color32::WHITE), EStrokeKind::Outside);
                        if ui.input(|i| i.pointer.any_released()) {
                            drop_on = Some(i);
                        }
                    }
                }
                ui.add_space(6.0);
            }
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
        ui.label(RichText::new(format!("Plano {}", p.label(i))).strong().color(ACCENT2));
        ui.add(egui::TextEdit::singleline(&mut p.title).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            ui.label(RichText::new("Escena").color(MU));
            ui.add(egui::TextEdit::singleline(&mut p.scene).desired_width(52.0).hint_text("—"));
            ui.label(RichText::new("Plano").color(MU));
            ui.add(egui::TextEdit::singleline(&mut p.num).desired_width(52.0).hint_text(format!("{}", i + 1)));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Duración (s)").color(MU));
            ui.add(egui::DragValue::new(&mut p.dur).range(0.1..=600.0).speed(0.05).max_decimals(1));
        });
        for (k, name) in FIELDS.iter().enumerate() {
            ui.label(RichText::new(*name).color(MU));
            ui.add(egui::TextEdit::multiline(field_mut(p, k)).desired_rows(2).desired_width(f32::INFINITY));
        }
        ui.separator();
        ui.label(RichText::new("Cámara").color(MU));
        let mut on = p.cam.is_some();
        if ui.checkbox(&mut on, "Mover la cámara en este plano").on_hover_text("Zoom y desplazamiento de la imagen del principio al final del plano; se ve en el animatic y al exportarlo").changed() {
            p.cam = if on { Some(crate::model::CamMove::default()) } else { None };
        }
        if let Some(c) = &mut p.cam {
            let row = |ui: &mut egui::Ui, name: &str, s: &mut f32, x: &mut f32, y: &mut f32| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name).color(MU));
                    ui.add(egui::DragValue::new(s).range(0.2..=5.0).speed(0.01).max_decimals(2).prefix("zoom "));
                    ui.add(egui::DragValue::new(x).speed(1.0).max_decimals(0).prefix("x "));
                    ui.add(egui::DragValue::new(y).speed(1.0).max_decimals(0).prefix("y "));
                });
            };
            row(ui, "Inicio", &mut c.s0, &mut c.x0, &mut c.y0);
            row(ui, "Final", &mut c.s1, &mut c.x1, &mut c.y1);
            ui.checkbox(&mut c.ease, "Arranque y frenado suaves");
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if pill(ui, "Renumerar de 10 en 10", false).on_hover_text("Plano 10, 20, 30… para poder intercalar planos nuevos entre dos").clicked() {
                self.checkpoint();
                self.project.renumber_panels(10);
            }
            if pill(ui, "De 1 en 1", false).clicked() {
                self.checkpoint();
                self.project.renumber_panels(1);
            }
        });
        if pill(ui, "Exportar storyboard a PDF", true).on_hover_text("Hojas A4 con la miniatura de cada plano y sus textos").clicked() {
            self.export_storyboard_pdf();
        }

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
                        let cur = crate::animatic::kind(&p.trans);
                        egui::ComboBox::from_id_salt(("plano_trans", i)).selected_text(crate::animatic::trans_name(cur)).width(112.0).show_ui(ui, |ui| {
                            for (k, name) in crate::animatic::TRANSITIONS {
                                if ui.selectable_label(k == cur, name).clicked() && k != cur {
                                    p.trans = name.to_string();
                                }
                            }
                        });                        ui.label(RichText::new(format!("cuadro {}", (start * fps).round() as u32 + 1)).size(11.0).color(INK3));
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
        if pill(ui, "Importar audio…", false).on_hover_text("Pone un audio (WAV, MP3, FLAC, OGG o AAC) como capa de audio desde el cuadro actual: suena al reproducir y sale en el MP4").clicked() {
            self.import_audio();
        }
        if pill(ui, "Crear animatic", true)
            .on_hover_text("Crea (o actualiza) la composición «Animatic»: un clip por plano, con su barra, sus fundidos y su cámara, usando los planos vivos. Se edita en la línea de tiempo de montaje")
            .clicked()
        {
            self.create_animatic_and_open();
        }
        muted(ui, "Transiciones (hasta medio segundo): Fundido cruza los dos planos; Fundido a negro apaga uno y enciende el otro; Deslizar y Empujar mueven el plano nuevo desde la derecha (con movimiento de cámara en alguno de los dos se hace un fundido). Corte: sin transición. El animatic se rehace solo al cambiar los planos.");
    }
}
