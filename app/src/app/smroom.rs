//! La sala Stop Motion (Issue #61): secuencia con fotos y exposiciones, Frame Browser con miniaturas, X-Sheet y propiedades
//! de la secuencia o de la foto elegida. Todo lo que se hace aquí cambia solo la secuencia (`sequence.json`), nunca los
//! archivos de las fotos, y se puede deshacer y rehacer.

use super::shell::{area_header, muted, panel_header, pill};
use super::smui::SmState;
use super::*;
use crate::stopmotion::{Item, Key, Loader, Sequence, SmRef, SourceFrame, MARKER, POSE};
use std::collections::{HashMap, HashSet};

/// Estado de la interfaz de la sala (no se guarda en el proyecto).
#[derive(Default)]
pub(super) struct SmUi {
    tab: u8,
    /// Posiciones elegidas en la secuencia (ids).
    sel: Vec<String>,
    anchor: Option<String>,
    /// Fotos elegidas en el Frame Browser (ids).
    photo_sel: Vec<String>,
    photo_anchor: Option<String>,
    search: String,
    unused_only: bool,
    thumb: f32,
    hold: u32,
    /// El puntero está sobre el panel de la sala: solo entonces valen sus atajos.
    pub over_panel: bool,
    /// Lo último que se editó fue la secuencia (Ctrl+Z la deshace en vez del proyecto).
    pub last_was_seq: bool,
    drag: Option<Vec<String>>,
    /// Posición bajo el cabezal la última vez, para llevar la franja hasta ella cuando cambia.
    last_cur: Option<usize>,
    /// Captura con cámara (Issue #62).
    pub(super) cap: super::smcap::CapState,
}

impl SmUi {
    /// Deja elegida solo esta posición.
    pub(super) fn sel_one(&mut self, id: String) {
        self.sel = vec![id.clone()];
        self.anchor = Some(id);
    }

    /// Para capturas: LEONTOON_SMTAB=0|1|2 abre esa pestaña del panel de la sala.
    pub(super) fn from_env() -> SmUi {
        SmUi { tab: std::env::var("LEONTOON_SMTAB").ok().and_then(|v| v.parse().ok()).unwrap_or(0), cap: super::smcap::CapState::from_env(), ..Default::default() }
    }
}

/// Lo que piden los controles; se aplica al terminar de dibujar, cuando ya no se está leyendo la secuencia.
enum Act {
    Goto(usize),
    Move(Vec<String>, usize),
    Duplicate,
    Delete,
    Blank,
    Exposure(u32),
    ExposureMerge(u32),
    AddExposure(i32),
    Hold,
    Freeze,
    Marker(&'static str),
    Undo,
    Redo,
    AddPhotos(Vec<String>, AddWhere),
    SetVisible(bool),
    Import,
    Verify,
}

#[derive(Clone, Copy, PartialEq)]
enum AddWhere {
    End,
    AfterSelection,
    Replace,
}

const THUMB_DEFAULT: f32 = 96.0;
const HOLD_DEFAULT: u32 = 12;
/// Tamaños que se piden a la vez al hilo de miniaturas.
const MAX_PENDING_THUMBS: usize = 48;

/// Selección con clic, Ctrl (alternar) y Mayús (rango), sobre `order`.
fn click_select(sel: &mut Vec<String>, anchor: &mut Option<String>, order: &[String], id: &str, ctrl: bool, shift: bool) {
    if shift {
        if let Some(a) = anchor.as_ref().and_then(|a| order.iter().position(|x| x == a)) {
            let b = order.iter().position(|x| x == id).unwrap_or(a);
            let (lo, hi) = (a.min(b), a.max(b));
            if !ctrl {
                sel.clear();
            }
            for x in &order[lo..=hi] {
                if !sel.contains(x) {
                    sel.push(x.clone());
                }
            }
            return;
        }
    }
    if ctrl {
        if let Some(i) = sel.iter().position(|x| x == id) {
            sel.remove(i);
        } else {
            sel.push(id.to_string());
        }
    } else {
        sel.clear();
        sel.push(id.to_string());
    }
    *anchor = Some(id.to_string());
}

/// Miniatura de una foto: la que ya está, o la pide al hilo (sin esperar).
fn thumb_for(loader: &mut Loader, thumbs: &HashMap<Key, egui::TextureHandle>, failed: &HashSet<Key>, seq: &str, dir: &std::path::Path, s: &SourceFrame) -> Option<egui::TextureId> {
    let key: Key = (seq.to_string(), s.id.clone());
    if let Some(t) = thumbs.get(&key) {
        return Some(t.id());
    }
    if !failed.contains(&key) && loader.pending() < MAX_PENDING_THUMBS {
        loader.request(key, dir.to_path_buf(), s.clone());
    }
    None
}

/// Pinta una miniatura dentro de `rect` sin deformarla, o su sustituto si falta o aún no llegó.
fn paint_thumb(p: &egui::Painter, rect: Rect, tex: Option<egui::TextureId>, failed: bool, src: Option<&SourceFrame>) {
    p.rect_filled(rect, 3.0, UI0);
    match (tex, src) {
        (Some(t), Some(s)) => {
            let ar = s.width.max(1) as f32 / s.height.max(1) as f32;
            let fit = if rect.width() / rect.height() > ar { Vec2::new(rect.height() * ar, rect.height()) } else { Vec2::new(rect.width(), rect.width() / ar) };
            p.image(t, Rect::from_center_size(rect.center(), fit), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        _ if failed => {
            p.text(rect.center(), Align2::CENTER_CENTER, "falta", FontId::proportional(11.0), ACCENT2);
        }
        _ => {
            p.text(rect.center(), Align2::CENTER_CENTER, "…", FontId::proportional(12.0), INK3);
        }
    }
}

/// Cuadro del proyecto en el que empieza la posición `idx` de la secuencia.
pub(super) fn project_frame_of(seq: &Sequence, r: &SmRef, pfps: u32, idx: usize) -> u32 {
    r.start + (seq.start_frame(idx) as u64 * pfps as u64).div_ceil(seq.fps.max(1) as u64) as u32
}

impl LeonApp {
    /// La capa Stop Motion con la que trabaja la sala: la actual si lo es, o la primera del proyecto.
    pub(super) fn sm_room_layer(&self) -> Option<usize> {
        let ok = |i: usize| self.project.layers[i].kind == LayerKind::StopMotion;
        if ok(self.cl) { Some(self.cl) } else { (0..self.project.layers.len()).find(|&i| ok(i)) }
    }

    /// Recoge las miniaturas que terminaron de cargar.
    fn sm_poll_thumbs(&mut self, ctx: &egui::Context) {
        for (key, res) in self.sm_tloader.poll() {
            match res {
                Ok(d) => {
                    let img = egui::ColorImage::from_rgba_unmultiplied([d.w as usize, d.h as usize], &d.rgba);
                    let tex = ctx.load_texture(format!("smt_{}_{}", key.0, key.1), img, egui::TextureOptions::LINEAR);
                    self.sm_thumbs.insert(key, tex);
                }
                Err(_) => {
                    self.sm_failed.insert(key);
                }
            }
        }
        if self.sm_tloader.pending() > 0 {
            ctx.request_repaint();
        }
    }

    /// Panel inferior de la sala: pestañas Secuencia, Frame Browser y X-Sheet, con las operaciones de edición.
    pub(super) fn sm_bottom(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.sm_poll_thumbs(&ctx);
        self.sm_ui.over_panel = ui.rect_contains_pointer(ui.max_rect());
        let hr = area_header(ui, "");
        let mut h = ui.new_child(egui::UiBuilder::new().max_rect(hr.shrink2(Vec2::new(6.0, 2.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        h.spacing_mut().item_spacing.x = 3.0;
        for (i, name) in ["Secuencia", "Frame Browser", "X-Sheet", "Captura"].into_iter().enumerate() {
            if pill(&mut h, name, self.sm_ui.tab as usize == i).clicked() {
                self.sm_ui.tab = i as u8;
            }
        }
        let live = self.sm_ui.cap.live;
        if pill(&mut h, "● Live View", live).on_hover_text("Muestra la cámara en el visor para capturar fotos").clicked() {
            self.sm_ui.cap.live = !live;
            if !live && self.sm_ui.cap.session_is_none() {
                self.sm_cap_connect();
            }
        }
        if self.sm_ui.tab == 3 {
            self.sm_cap_panel(ui);
            return;
        }
        let Some(li) = self.sm_room_layer() else {
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                muted(ui, "Esta sala trabaja con una capa Stop Motion y el proyecto todavía no tiene ninguna.");
                if ui.button("Agregar una capa Stop Motion").clicked() {
                    self.add_layer_kind(LayerKind::StopMotion);
                }
            });
            return;
        };
        let Some(r) = self.project.layers[li].stopmotion.clone() else {
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                muted(ui, "La capa Stop Motion todavía no tiene secuencia. Elige la carpeta donde se guardarán las fotos.");
                ui.horizontal(|ui| {
                    if ui.button("Crear una secuencia nueva…").clicked() {
                        self.cl = li;
                        self.sm_create();
                    }
                    if ui.button("Usar una secuencia que ya existe…").clicked() {
                        self.cl = li;
                        self.sm_open_existing();
                    }
                });
            });
            return;
        };
        if let SmState::Missing(msg) = self.sm_state(&r) {
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(format!("⚠ {msg}")).size(12.0).color(ACCENT2));
                if ui.button("Buscar la carpeta…").clicked() {
                    self.cl = li;
                    self.sm_relocate(&r);
                }
            });
            return;
        }
        let id = r.id.clone();
        let mut acts: Vec<Act> = Vec::new();
        self.sm_toolbar(ui, &id, &mut acts);
        match self.sm_ui.tab {
            0 => self.sm_strip(ui, &id, &r, &mut acts),
            1 => self.sm_browser(ui, &id, &mut acts),
            _ => self.sm_xsheet(ui, &id, &r, &mut acts),
        }
        for a in acts {
            self.sm_act(&id, &r, a);
        }
    }

    fn sm_toolbar(&mut self, ui: &mut egui::Ui, id: &str, acts: &mut Vec<Act>) {
        let (can_undo, can_redo, n_sel) = {
            let v = &self.sm[id];
            (v.hist.can_undo(), v.hist.can_redo(), self.sm_ui.sel.len())
        };
        let has_sel = n_sel > 0;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
            ui.add_enabled_ui(can_undo, |ui| {
                if ui.small_button("Deshacer").on_hover_text("Deshace el último cambio de la secuencia (Ctrl+Z con el puntero sobre este panel)").clicked() {
                    acts.push(Act::Undo);
                }
            });
            ui.add_enabled_ui(can_redo, |ui| {
                if ui.small_button("Rehacer").clicked() {
                    acts.push(Act::Redo);
                }
            });
            ui.separator();
            ui.add_enabled_ui(has_sel, |ui| {
                if ui.small_button("Duplicar").on_hover_text("Repite la foto justo después (Ctrl+D). No copia el archivo").clicked() {
                    acts.push(Act::Duplicate);
                }
                if ui.small_button("Eliminar").on_hover_text("Quita la posición de la secuencia (Supr). La foto y su archivo siguen en la carpeta").clicked() {
                    acts.push(Act::Delete);
                }
            });
            if ui.small_button("Vacío").on_hover_text("Inserta un fotograma vacío (B)").clicked() {
                acts.push(Act::Blank);
            }
            ui.separator();
            ui.add_enabled_ui(has_sel, |ui| {
                if ui.small_button("−1").on_hover_text("Un cuadro menos de exposición (−)").clicked() {
                    acts.push(Act::AddExposure(-1));
                }
                if ui.small_button("+1").on_hover_text("Un cuadro más de exposición (+)").clicked() {
                    acts.push(Act::AddExposure(1));
                }
                for n in [1u32, 2, 3, 4] {
                    if ui.small_button(format!("{n}")).on_hover_text(format!("Exposición de {n} cuadro(s)")).clicked() {
                        acts.push(Act::Exposure(n));
                    }
                }
            });
            let mut hold = if self.sm_ui.hold == 0 { HOLD_DEFAULT } else { self.sm_ui.hold };
            ui.add(egui::DragValue::new(&mut hold).range(1..=1000).suffix(" cuadros"));
            self.sm_ui.hold = hold;
            ui.add_enabled_ui(has_sel, |ui| {
                if ui.small_button("Pausa").on_hover_text("Suma esos cuadros a la exposición de la foto").clicked() {
                    acts.push(Act::Hold);
                }
                if ui.small_button("Congelar").on_hover_text("Pone después otra vez la misma foto durante esos cuadros").clicked() {
                    acts.push(Act::Freeze);
                }
                if ui.small_button("Ocultar").on_hover_text("La posición sigue en la secuencia pero no se ve ni se exporta").clicked() {
                    acts.push(Act::SetVisible(false));
                }
                if ui.small_button("Mostrar").clicked() {
                    acts.push(Act::SetVisible(true));
                }
            });
            ui.separator();
            if ui.small_button("◆ Pose").on_hover_text("Marca el cuadro actual como pose importante (P)").clicked() {
                acts.push(Act::Marker(POSE));
            }
            if ui.small_button("▼ Marcador").on_hover_text("Pone un marcador en el cuadro actual (M)").clicked() {
                acts.push(Act::Marker(MARKER));
            }
        });
    }

    /// Franja de la secuencia: una celda por posición, más ancha cuanto más dura. Se arrastra para reordenar.
    fn sm_strip(&mut self, ui: &mut egui::Ui, id: &str, r: &SmRef, acts: &mut Vec<Act>) {
        let avail = ui.available_size();
        let th = (avail.y - 40.0).clamp(36.0, 110.0);
        let tw = th * 16.0 / 9.0;
        let (pfps, frame) = (self.project.fps, self.frame);
        let ctx = ui.ctx().clone();
        let v = &self.sm[id];
        let seq = &v.folder.seq;
        if seq.items.is_empty() {
            muted(ui, "La secuencia está vacía. Importa fotos (panel de la derecha) y añádelas desde el Frame Browser.");
            return;
        }
        let cur = seq.item_for_project_frame(r.start, pfps, frame);
        let cell_w = |it: &Item| tw + (it.exposure.saturating_sub(1)).min(8) as f32 * 10.0;
        let gap = 4.0;
        let mut xs = Vec::with_capacity(seq.items.len() + 1);
        let mut x = 0.0;
        for it in &seq.items {
            xs.push(x);
            x += cell_w(it) + gap;
        }
        xs.push(x);
        let total_w = x + 20.0;
        let dir = v.folder.dir.clone();
        let drag_active = self.sm_ui.drag.is_some();
        let mut drop_slot: Option<usize> = None;
        let mut edge = 0.0f32;
        egui::ScrollArea::horizontal().auto_shrink([false, false]).id_salt("sm_strip").show_viewport(ui, |ui, vp| {
            let (area, _) = ui.allocate_exact_size(Vec2::new(total_w, th + 34.0), Sense::hover());
            let p = ui.painter_at(area);
            // Si el cabezal pasa a otra posición que no se ve, la franja se desplaza hasta ella.
            if cur != self.sm_ui.last_cur && !drag_active {
                if let Some(c) = cur {
                    if xs[c] < vp.min.x || xs[c] + cell_w(&seq.items[c]) > vp.max.x {
                        let r = Rect::from_min_size(Pos2::new(area.min.x + xs[c], area.min.y), Vec2::new(cell_w(&seq.items[c]), th));
                        ui.scroll_to_rect(r, Some(egui::Align::Center));
                    }
                }
                self.sm_ui.last_cur = cur;
            }
            let first = xs.partition_point(|&x| x + tw < vp.min.x).saturating_sub(1);
            let ptr = ui.input(|i| i.pointer.hover_pos());
            for idx in first..seq.items.len() {
                if xs[idx] > vp.max.x {
                    break;
                }
                let it = &seq.items[idx];
                let cell = Rect::from_min_size(Pos2::new(area.min.x + xs[idx], area.min.y + 16.0), Vec2::new(cell_w(it), th));
                let sel = self.sm_ui.sel.contains(&it.id);
                p.rect_filled(cell.expand(2.0), 4.0, if sel { ACCENT.gamma_multiply(0.45) } else { BG3 });
                let thumb_rect = Rect::from_min_size(cell.min, Vec2::new(tw, th));
                let src = seq.source(&it.source);
                let key: Key = (seq.id.clone(), it.source.clone());
                let failed = self.sm_failed.contains(&key);
                if it.source.is_empty() {
                    p.rect_filled(thumb_rect, 3.0, UI0);
                    p.text(thumb_rect.center(), Align2::CENTER_CENTER, "vacío", FontId::proportional(11.0), INK3);
                } else {
                    let tex = src.and_then(|s| thumb_for(&mut self.sm_tloader, &self.sm_thumbs, &self.sm_failed, &seq.id, &dir, s));
                    paint_thumb(&p, thumb_rect, tex, failed, src);
                }
                // Las posiciones largas muestran su exposición como una cinta a la derecha de la foto.
                if cell.width() > tw {
                    p.rect_filled(Rect::from_min_max(Pos2::new(thumb_rect.max.x + 1.0, cell.min.y), cell.max), 3.0, BG4);
                }
                if !it.visible {
                    p.rect_filled(cell, 3.0, Color32::from_black_alpha(150));
                }
                p.text(Pos2::new(cell.min.x + 4.0, cell.max.y + 3.0), Align2::LEFT_TOP, format!("{}", idx + 1), FontId::proportional(10.5), MU);
                p.text(Pos2::new(cell.max.x - 2.0, cell.max.y + 3.0), Align2::RIGHT_TOP, format!("×{}", it.exposure), FontId::proportional(10.5), TX);
                if Some(idx) == cur {
                    p.rect_filled(Rect::from_min_size(Pos2::new(cell.min.x, cell.min.y - 5.0), Vec2::new(cell.width(), 3.0)), 1.0, ACCENT);
                }
                for m in seq.markers_in(idx) {
                    let (icon, col) = if m.kind == POSE { ("◆", ACCENT) } else { ("▼", Color32::from_rgb(0x6d, 0xb3, 0xe8)) };
                    p.text(Pos2::new(cell.min.x + 10.0 + (m.at.saturating_sub(seq.start_frame(idx))) as f32 * 10.0, area.min.y + 1.0), Align2::CENTER_TOP, icon, FontId::proportional(11.0), col);
                }
                let resp = ui.interact(cell, egui::Id::new(("sm_cell", &it.id)), Sense::click_and_drag());
                let tip = format!("Posición {} · {} · {} cuadro(s){}", idx + 1, if it.source.is_empty() { "fotograma vacío".to_string() } else { it.source.clone() }, it.exposure, it.note.as_ref().map(|n| format!("\n{n}")).unwrap_or_default());
                let resp = resp.on_hover_text(tip);
                if resp.clicked() {
                    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                    let order: Vec<String> = seq.items.iter().map(|i| i.id.clone()).collect();
                    click_select(&mut self.sm_ui.sel, &mut self.sm_ui.anchor, &order, &it.id, ctrl, shift);
                    acts.push(Act::Goto(idx));
                }
                if resp.drag_started() {
                    if !self.sm_ui.sel.contains(&it.id) {
                        self.sm_ui.sel = vec![it.id.clone()];
                        self.sm_ui.anchor = Some(it.id.clone());
                    }
                    self.sm_ui.drag = Some(order_of(seq, &self.sm_ui.sel));
                }
            }
            if drag_active {
                if let Some(pt) = ptr {
                    let rel = pt.x - area.min.x;
                    let slot = (0..seq.items.len()).find(|&i| rel < xs[i] + (xs[i + 1] - gap - xs[i]) / 2.0).unwrap_or(seq.items.len());
                    drop_slot = Some(slot);
                    let lx = area.min.x + if slot == 0 { 0.0 } else { xs[slot] - gap / 2.0 };
                    p.vline(lx, area.min.y + 10.0..=area.max.y - 14.0, stk(3.0, ACCENT));
                    // Cerca de un borde, la franja se desplaza sola para poder llegar a posiciones lejanas.
                    let clip = ui.clip_rect();
                    if pt.x < clip.min.x + 40.0 {
                        edge = 14.0;
                    } else if pt.x > clip.max.x - 40.0 {
                        edge = -14.0;
                    }
                    if edge != 0.0 {
                        ui.scroll_with_delta(Vec2::new(edge, 0.0));
                    }
                }
            }
        });
        if edge != 0.0 {
            ctx.request_repaint();
        }
        if drag_active {
            if ui.input(|i| i.pointer.any_released()) {
                if let (Some(ids), Some(slot)) = (self.sm_ui.drag.take(), drop_slot) {
                    acts.push(Act::Move(ids, slot));
                }
                self.sm_ui.drag = None;
            } else if !ui.input(|i| i.pointer.any_down()) {
                self.sm_ui.drag = None;
            }
        }
        let _ = ctx;
    }

    /// Frame Browser: todas las fotos de la secuencia con su miniatura (solo se cargan las que se ven).
    fn sm_browser(&mut self, ui: &mut egui::Ui, id: &str, acts: &mut Vec<Act>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add(egui::TextEdit::singleline(&mut self.sm_ui.search).hint_text("Buscar por número o archivo").desired_width(170.0));
            ui.checkbox(&mut self.sm_ui.unused_only, "Solo las que no se usan");
            if self.sm_ui.thumb == 0.0 {
                self.sm_ui.thumb = THUMB_DEFAULT;
            }
            ui.add(egui::Slider::new(&mut self.sm_ui.thumb, 56.0..=200.0).show_value(false)).on_hover_text("Tamaño de las miniaturas");
            let n = self.sm_ui.photo_sel.len();
            ui.add_enabled_ui(n > 0, |ui| {
                if ui.small_button("Añadir al final").on_hover_text("También: doble clic sobre una foto").clicked() {
                    acts.push(Act::AddPhotos(self.sm_ui.photo_sel.clone(), AddWhere::End));
                }
                if ui.small_button("Insertar tras la selección").on_hover_text("Después de la posición elegida en la secuencia").clicked() {
                    acts.push(Act::AddPhotos(self.sm_ui.photo_sel.clone(), AddWhere::AfterSelection));
                }
                if ui.small_button("Reemplazar").on_hover_text("Cambia la foto de las posiciones elegidas en la secuencia por esta (conservan su exposición)").clicked() {
                    acts.push(Act::AddPhotos(self.sm_ui.photo_sel.clone(), AddWhere::Replace));
                }
            });
            if n > 0 {
                muted(ui, format!("{n} elegidas"));
            }
        });
        let v = &self.sm[id];
        let seq = &v.folder.seq;
        let dir = v.folder.dir.clone();
        let q = self.sm_ui.search.trim().to_lowercase();
        let used: HashMap<&str, u32> = seq.items.iter().fold(HashMap::new(), |mut m, i| {
            *m.entry(i.source.as_str()).or_insert(0) += 1;
            m
        });
        let list: Vec<&SourceFrame> = seq
            .sources
            .iter()
            .filter(|s| (q.is_empty() || s.id.to_lowercase().contains(&q) || s.file.to_lowercase().contains(&q) || s.id.trim_start_matches('s').trim_start_matches('0') == q) && (!self.sm_ui.unused_only || !used.contains_key(s.id.as_str())))
            .collect();
        if list.is_empty() {
            muted(ui, if seq.sources.is_empty() { "La secuencia no tiene fotos. Usa «Importar fotos…» en el panel de la derecha." } else { "Ninguna foto coincide con la búsqueda." });
            return;
        }
        let cell = self.sm_ui.thumb.max(56.0);
        let (cw, chh) = (cell, cell * 9.0 / 16.0 + 16.0);
        let gap = 6.0;
        let cols = (((ui.available_width() - 8.0) / (cw + gap)).floor() as usize).max(1);
        let rows = list.len().div_ceil(cols);
        egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt("sm_browser").show_rows(ui, chh + gap, rows, |ui, range| {
            for row in range {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for s in list.iter().skip(row * cols).take(cols) {
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(cw, chh), Sense::click());
                        let p = ui.painter_at(rect);
                        let sel = self.sm_ui.photo_sel.contains(&s.id);
                        p.rect_filled(rect, 4.0, if sel { ACCENT.gamma_multiply(0.5) } else { BG3 });
                        let tr = Rect::from_min_size(rect.min + Vec2::splat(2.0), Vec2::new(cw - 4.0, chh - 18.0));
                        let key: Key = (seq.id.clone(), s.id.clone());
                        let failed = self.sm_failed.contains(&key);
                        let tex = thumb_for(&mut self.sm_tloader, &self.sm_thumbs, &self.sm_failed, &seq.id, &dir, s);
                        paint_thumb(&p, tr, tex, failed, Some(s));
                        let n = used.get(s.id.as_str()).copied().unwrap_or(0);
                        p.text(Pos2::new(rect.min.x + 4.0, rect.max.y - 2.0), Align2::LEFT_BOTTOM, s.id.trim_start_matches('s').trim_start_matches('0'), FontId::proportional(10.5), MU);
                        if n > 0 {
                            p.text(Pos2::new(rect.max.x - 4.0, rect.max.y - 2.0), Align2::RIGHT_BOTTOM, format!("usada ×{n}"), FontId::proportional(10.0), ACCENT);
                        }
                        if failed {
                            p.rect_stroke(rect, 4.0, stk(1.5, ACCENT2), egui::StrokeKind::Inside);
                        }
                        let resp = resp.on_hover_text(format!("{} · {}×{} · {}", s.file, s.width, s.height, s.captured_at.clone().unwrap_or_default()));
                        if resp.clicked() {
                            let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                            let order: Vec<String> = list.iter().map(|x| x.id.clone()).collect();
                            click_select(&mut self.sm_ui.photo_sel, &mut self.sm_ui.photo_anchor, &order, &s.id, ctrl, shift);
                        }
                        if resp.double_clicked() {
                            acts.push(Act::AddPhotos(vec![s.id.clone()], AddWhere::End));
                        }
                    }
                });
                ui.add_space(gap);
            }
        });
    }

    /// X-Sheet: una fila por posición con sus cuadros, su foto, su exposición y su nota.
    fn sm_xsheet(&mut self, ui: &mut egui::Ui, id: &str, r: &SmRef, acts: &mut Vec<Act>) {
        let (pfps, frame) = (self.project.fps, self.frame);
        let v = &self.sm[id];
        let seq = &v.folder.seq;
        if seq.items.is_empty() {
            muted(ui, "La secuencia está vacía.");
            return;
        }
        let cur = seq.item_for_project_frame(r.start, pfps, frame);
        let mut exp_edit: Option<(String, u32)> = None;
        let mut note_edit: Option<(String, String)> = None;
        let row_h = 22.0;
        let widths = [46.0, 96.0, 90.0, 80.0, 40.0];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (w, t) in widths.iter().zip(["Pos.", "Cuadros", "Foto", "Exposición", "Vis."]) {
                ui.add_sized([*w, 16.0], egui::Label::new(RichText::new(t).size(11.0).color(MU)));
            }
            ui.label(RichText::new("Nota").size(11.0).color(MU));
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt("sm_xsheet").show_rows(ui, row_h, seq.items.len(), |ui, range| {
            for idx in range {
                let it = &seq.items[idx];
                let start = seq.start_frame(idx);
                let sel = self.sm_ui.sel.contains(&it.id);
                let (row, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                let bg = if sel { ACCENT.gamma_multiply(0.4) } else if Some(idx) == cur { BG4 } else if idx % 2 == 0 { BG3 } else { BG2 };
                ui.painter().rect_filled(row, 2.0, bg);
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
                child.spacing_mut().item_spacing.x = 4.0;
                child.add_sized([widths[0], row_h - 2.0], egui::Label::new(RichText::new(format!("{}", idx + 1)).size(11.5).color(TX)));
                child.add_sized([widths[1], row_h - 2.0], egui::Label::new(RichText::new(format!("{}–{}", start + 1, start + it.exposure)).size(11.5).color(MU)));
                child.add_sized([widths[2], row_h - 2.0], egui::Label::new(RichText::new(if it.source.is_empty() { "vacío".to_string() } else { it.source.trim_start_matches('s').trim_start_matches('0').to_string() }).size(11.5).color(TX)));
                let mut e = it.exposure;
                let rr = child.add_sized([widths[3], row_h - 4.0], egui::DragValue::new(&mut e).range(1..=crate::stopmotion::MAX_EXPOSURE).speed(0.1));
                if rr.changed() && e != it.exposure {
                    exp_edit = Some((it.id.clone(), e));
                }
                child.add_sized([widths[4], row_h - 2.0], egui::Label::new(RichText::new(if it.visible { "✔" } else { "–" }).size(11.5).color(MU)));
                let mut note = it.note.clone().unwrap_or_default();
                let nr = child.add(egui::TextEdit::singleline(&mut note).desired_width(160.0).font(FontId::proportional(11.5)));
                if nr.changed() {
                    note_edit = Some((it.id.clone(), note));
                }
                if resp.clicked() && !rr.hovered() && !nr.hovered() {
                    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                    let order: Vec<String> = seq.items.iter().map(|i| i.id.clone()).collect();
                    click_select(&mut self.sm_ui.sel, &mut self.sm_ui.anchor, &order, &it.id, ctrl, shift);
                    acts.push(Act::Goto(idx));
                }
            }
        });
        if let Some((item, e)) = exp_edit {
            self.sm_edit_merge(id, |s| s.set_exposure(&item, e));
        }
        if let Some((item, n)) = note_edit {
            self.sm_edit_merge(id, |s| s.set_note(&item, &n));
        }
    }

    /// Hace un cambio en la secuencia: lo registra para deshacer, lo guarda y avisa al visor y al resumen de la capa.
    fn sm_edit(&mut self, id: &str, f: impl FnOnce(&mut Sequence) -> bool) -> bool {
        self.sm_edit_inner(id, false, f)
    }

    /// Como sm_edit, pero varios cambios seguidos del mismo control (arrastrar una exposición, escribir una nota) son un
    /// solo paso de deshacer.
    fn sm_edit_merge(&mut self, id: &str, f: impl FnOnce(&mut Sequence) -> bool) -> bool {
        self.sm_edit_inner(id, true, f)
    }

    fn sm_edit_inner(&mut self, id: &str, merging: bool, f: impl FnOnce(&mut Sequence) -> bool) -> bool {
        let pdir = self.project_dir();
        let Some(v) = self.sm.get_mut(id) else { return false };
        let now = self.now;
        let merge = merging && now - v.last_edit < 0.8 && v.hist.can_undo();
        if !merge {
            v.hist.record(&v.folder.seq);
        }
        v.last_edit = now;
        if !f(&mut v.folder.seq) {
            if !merge {
                v.hist.forget_last();
            }
            return false;
        }
        self.sm_ui.last_was_seq = true;
        self.sm_after_edit(id, pdir)
    }

    pub(super) fn sm_after_edit(&mut self, id: &str, pdir: Option<std::path::PathBuf>) -> bool {
        let Some(v) = self.sm.get_mut(id) else { return false };
        let saved = v.folder.save();
        v.problems = None;
        self.project.sm.forget(id);
        for l in self.project.layers.iter_mut() {
            if let Some(r) = l.stopmotion.as_mut().filter(|r| r.id == id) {
                r.update(&v.folder, pdir.as_deref());
            }
        }
        self.dirty = true;
        match saved {
            Ok(()) => true,
            Err(e) => {
                self.say(format!("No se pudo guardar sequence.json: {e}"));
                false
            }
        }
    }

    /// Deshace o rehace en la secuencia. Devuelve false si no había nada que deshacer ahí.
    pub(super) fn sm_history(&mut self, undo: bool) -> bool {
        if self.room_kind() != RoomKind::StopMotion || !(self.sm_ui.over_panel || self.sm_ui.last_was_seq) {
            return false;
        }
        let Some(li) = self.sm_room_layer() else { return false };
        let Some(id) = self.project.layers[li].stopmotion.as_ref().map(|r| r.id.clone()) else { return false };
        let pdir = self.project_dir();
        let Some(v) = self.sm.get_mut(&id) else { return false };
        let done = if undo { v.hist.undo(&mut v.folder.seq) } else { v.hist.redo(&mut v.folder.seq) };
        if !done {
            return false;
        }
        self.sm_ui.sel.retain(|s| self.sm[&id].folder.seq.index_of(s).is_some());
        self.sm_ui.last_was_seq = true;
        self.sm_after_edit(&id, pdir);
        self.say(if undo { "Cambio de la secuencia deshecho" } else { "Cambio de la secuencia rehecho" });
        true
    }

    fn sm_act(&mut self, id: &str, r: &SmRef, a: Act) {
        let sel = self.sm_ui.sel.clone();
        let hold = if self.sm_ui.hold == 0 { HOLD_DEFAULT } else { self.sm_ui.hold };
        let pfps = self.project.fps;
        // Posición donde se inserta algo nuevo: después de la última elegida, o al final.
        let after = |s: &Sequence, sel: &[String]| sel.iter().filter_map(|i| s.index_of(i)).max().map(|i| i + 1).unwrap_or(s.items.len());
        match a {
            Act::Goto(idx) => {
                if let Some(v) = self.sm.get(id) {
                    let f = project_frame_of(&v.folder.seq, r, pfps, idx);
                    self.frame = f.min(self.project.length.saturating_sub(1));
                    self.playing = false;
                }
            }
            Act::Move(ids, slot) => {
                if self.sm_edit(id, |s| s.move_items(&ids, slot)) {
                    self.say("Posiciones reordenadas");
                }
            }
            Act::Duplicate => {
                let mut new = Vec::new();
                if self.sm_edit(id, |s| {
                    new = s.duplicate_items(&sel);
                    !new.is_empty()
                }) {
                    self.sm_ui.sel = new;
                    self.say("Posiciones repetidas (la foto no se copia)");
                }
            }
            Act::Delete => {
                let mut n = 0;
                if self.sm_edit(id, |s| {
                    n = s.remove_items(&sel);
                    n > 0
                }) {
                    self.sm_ui.sel.clear();
                    self.say(format!("{n} posiciones quitadas de la secuencia; las fotos siguen en la carpeta"));
                }
            }
            Act::Blank => {
                let mut new = String::new();
                let exposure = self.sm.get(id).map_or(1, |v| v.folder.seq.default_exposure);
                if self.sm_edit(id, |s| {
                    let at = after(s, &sel);
                    new = s.insert_blank(at, exposure);
                    true
                }) {
                    self.sm_ui.sel = vec![new];
                    self.say("Fotograma vacío insertado");
                }
            }
            Act::Exposure(n) => {
                self.sm_edit(id, |s| s.set_exposures(&sel, n) > 0);
            }
            Act::ExposureMerge(n) => {
                self.sm_edit_merge(id, |s| s.set_exposures(&sel, n) > 0);
            }
            Act::AddExposure(d) => {
                self.sm_edit(id, |s| s.add_exposure(&sel, d) > 0);
            }
            Act::Hold => {
                self.sm_edit(id, |s| s.add_exposure(&sel, hold as i32) > 0);
            }
            Act::Freeze => {
                let mut new = Vec::new();
                if self.sm_edit(id, |s| {
                    // Cada una se congela justo después de sí misma.
                    for i in &sel {
                        if let Some(n) = s.freeze(i, hold) {
                            new.push(n);
                        }
                    }
                    !new.is_empty()
                }) {
                    self.sm_ui.sel = new;
                    self.say(format!("Foto congelada {hold} cuadros"));
                }
            }
            Act::SetVisible(v) => {
                self.sm_edit(id, |s| s.set_visible(&sel, v) > 0);
            }
            Act::Marker(kind) => {
                let at = self.sm.get(id).and_then(|v| {
                    let s = &v.folder.seq;
                    // El cuadro de la secuencia que está bajo el cabezal; si está fuera, el inicio de la posición elegida.
                    let f = self.frame;
                    (f >= r.start).then(|| ((f - r.start) as u64 * s.fps.max(1) as u64 / pfps.max(1) as u64) as u32).filter(|c| *c < s.total_frames()).or_else(|| sel.first().and_then(|i| s.index_of(i)).map(|i| s.start_frame(i)))
                });
                match at {
                    Some(at) => {
                        let label = if kind == POSE { format!("Pose {}", at + 1) } else { String::new() };
                        self.sm_edit(id, |s| {
                            s.add_marker(at, kind, &label);
                            true
                        });
                        self.say(if kind == POSE { "Pose marcada" } else { "Marcador puesto" });
                    }
                    None => self.say("Pon el cabezal dentro de la secuencia o elige una posición para marcar"),
                }
            }
            Act::Undo => {
                self.sm_ui.over_panel = true;
                self.sm_history(true);
            }
            Act::Redo => {
                self.sm_ui.over_panel = true;
                self.sm_history(false);
            }
            Act::AddPhotos(photos, how) => {
                if photos.is_empty() {
                    return;
                }
                let exposure = self.sm.get(id).map_or(1, |v| v.folder.seq.default_exposure);
                let mut added = Vec::new();
                let mut res: Result<usize, String> = Ok(0);
                let changed = self.sm_edit(id, |s| {
                    match how {
                        AddWhere::Replace => match s.replace_source(&sel, &photos[0]) {
                            Ok(n) => {
                                res = Ok(n);
                                return n > 0;
                            }
                            Err(e) => {
                                res = Err(e);
                                return false;
                            }
                        },
                        AddWhere::End | AddWhere::AfterSelection => {
                            let mut at = if how == AddWhere::End { s.items.len() } else { after(s, &sel) };
                            for p in &photos {
                                match s.insert_source(at, p, exposure) {
                                    Ok(n) => {
                                        added.push(n);
                                        at += 1;
                                    }
                                    Err(e) => res = Err(e),
                                }
                            }
                        }
                    }
                    !added.is_empty()
                });
                match (changed, how) {
                    (true, AddWhere::Replace) => self.say(format!("{} posiciones con otra foto", res.unwrap_or(0))),
                    (true, _) => {
                        self.say(format!("{} fotos añadidas a la secuencia", added.len()));
                        self.sm_ui.sel = added;
                    }
                    (false, AddWhere::Replace) if sel.is_empty() => self.say("Elige primero las posiciones de la secuencia cuya foto quieres reemplazar"),
                    _ => {
                        if let Err(e) = res {
                            self.say(e);
                        }
                    }
                }
            }
            Act::Import => self.sm_import(id),
            Act::Verify => self.sm_verify(id),
        }
    }

    /// Atajos de la sala: solo valen con el puntero sobre su panel, así que no chocan con los de las demás salas.
    /// Devuelve true si se usó alguno.
    pub(super) fn sm_room_keys(&mut self, ctx: &egui::Context) -> bool {
        if self.room_kind() != RoomKind::StopMotion || !self.sm_ui.over_panel {
            return false;
        }
        let Some(li) = self.sm_room_layer() else { return false };
        let Some(r) = self.project.layers[li].stopmotion.clone() else { return false };
        if !self.sm.contains_key(&r.id) {
            return false;
        }
        use egui::{Key, Modifiers};
        let k = |m: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(m, key));
        let none = Modifiers::NONE;
        let mut act = None;
        if k(none, Key::Delete) || k(none, Key::Backspace) {
            act = Some(Act::Delete);
        } else if k(Modifiers::COMMAND, Key::D) {
            act = Some(Act::Duplicate);
        } else if k(none, Key::Plus) || k(none, Key::Equals) {
            act = Some(Act::AddExposure(1));
        } else if k(none, Key::Minus) {
            act = Some(Act::AddExposure(-1));
        } else if k(none, Key::B) {
            act = Some(Act::Blank);
        } else if k(none, Key::M) {
            act = Some(Act::Marker(MARKER));
        } else if k(none, Key::P) {
            act = Some(Act::Marker(POSE));
        } else if k(Modifiers::COMMAND, Key::A) {
            self.sm_ui.sel = self.sm[&r.id].folder.seq.items.iter().map(|i| i.id.clone()).collect();
            return true;
        } else if k(none, Key::Escape) {
            self.sm_ui.sel.clear();
            self.sm_ui.photo_sel.clear();
            return true;
        } else if k(Modifiers::ALT, Key::ArrowLeft) || k(Modifiers::ALT, Key::ArrowRight) {
            // Mover la selección un lugar.
            let right = ctx.input(|i| i.key_down(Key::ArrowRight));
            let sel = self.sm_ui.sel.clone();
            let (min, max) = {
                let s = &self.sm[&r.id].folder.seq;
                (sel.iter().filter_map(|i| s.index_of(i)).min(), sel.iter().filter_map(|i| s.index_of(i)).max())
            };
            if let (Some(lo), Some(hi)) = (min, max) {
                let slot = if right { hi + 2 } else { lo.saturating_sub(1) };
                act = Some(Act::Move(sel, slot));
            }
        }
        match act {
            Some(a) => {
                self.sm_act(&r.id.clone(), &r, a);
                true
            }
            None => false,
        }
    }

    /// Propiedades de la sala: la secuencia, la foto o posición elegida y los marcadores.
    pub(super) fn sm_room_props(&mut self, ui: &mut egui::Ui) {
        let Some(li) = self.sm_room_layer() else {
            muted(ui, "Agrega una capa Stop Motion para ver aquí sus propiedades.");
            return;
        };
        let Some(r) = self.project.layers[li].stopmotion.clone() else {
            self.cl = li;
            self.stopmotion_panel_ui(ui);
            return;
        };
        if let SmState::Missing(_) = self.sm_state(&r) {
            self.cl = li;
            self.stopmotion_panel_ui(ui);
            return;
        }
        let id = r.id.clone();
        let mut acts: Vec<Act> = Vec::new();
        let (name, dir, photos, frames, fps, items) = {
            let v = &self.sm[&id];
            (v.folder.seq.name.clone(), v.folder.dir.clone(), v.folder.seq.sources.len(), v.folder.seq.total_frames(), v.folder.seq.fps, v.folder.seq.items.len())
        };
        panel_header(ui, "Secuencia");
        ui.label(RichText::new(&name).strong().color(TX));
        muted(ui, format!("{photos} fotos · {items} posiciones · {frames} cuadros · {:.1} s a {fps} fps", frames as f32 / fps.max(1) as f32));
        muted(ui, format!("Carpeta: {}", dir.display()));
        self.sm_timing_ui(ui, &r, &id, frames, fps);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Importar fotos…").on_hover_text("Copia fotos JPEG o PNG a la carpeta de la secuencia; las tuyas no se tocan").clicked() {
                acts.push(Act::Import);
            }
            if ui.button("Verificar").on_hover_text("Comprueba que todas las fotos existen y no han cambiado").clicked() {
                acts.push(Act::Verify);
            }
        });
        if let Some(v) = self.sm.get(&id) {
            if !v.last.is_empty() {
                muted(ui, v.last.clone());
            }
            if let Some(p) = &v.problems {
                if p.is_empty() {
                    muted(ui, "✔ Todas las fotos están y no han cambiado");
                } else {
                    ui.label(RichText::new(format!("⚠ {} fotos con problemas", p.len())).size(11.5).color(ACCENT2));
                }
            }
        }
        ui.add_space(6.0);
        panel_header(ui, "Posición elegida");
        let sel = self.sm_ui.sel.clone();
        let seq = &self.sm[&id].folder.seq;
        match sel.as_slice() {
            [] => muted(ui, "Elige una posición en la secuencia o en la X-Sheet."),
            [one] => {
                if let Some(idx) = seq.index_of(one) {
                    let it = &seq.items[idx];
                    let start = seq.start_frame(idx);
                    ui.label(RichText::new(format!("Posición {} · cuadros {}–{}", idx + 1, start + 1, start + it.exposure)).color(TX));
                    match seq.source(&it.source) {
                        Some(s) => {
                            muted(ui, format!("Foto {} · {}", s.id.trim_start_matches('s').trim_start_matches('0'), s.file));
                            muted(ui, format!("{}×{} · {} · {:.1} MB", s.width, s.height, s.format, s.bytes as f32 / 1_048_576.0));
                            if let Some(t) = &s.captured_at {
                                muted(ui, format!("Tomada: {t}"));
                            }
                            if let Some(d) = &s.device {
                                muted(ui, format!("Cámara: {} ({})", d.name, d.provider));
                            }
                            if let Some(e) = &s.exif {
                                let line = [e.make.clone(), e.model.clone()].into_iter().flatten().collect::<Vec<_>>().join(" ");
                                if !line.is_empty() {
                                    muted(ui, format!("EXIF: {line}"));
                                }
                            }
                            muted(ui, format!("Huella: {}…", s.hash.chars().take(15).collect::<String>()));
                        }
                        None if it.source.is_empty() => muted(ui, "Fotograma vacío (sin foto)"),
                        None => {
                            ui.label(RichText::new("⚠ La foto de esta posición no está registrada").size(11.5).color(ACCENT2));
                        }
                    }
                    let mut e = it.exposure;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Exposición").size(11.5).color(MU));
                        if ui.add(egui::DragValue::new(&mut e).range(1..=crate::stopmotion::MAX_EXPOSURE).suffix(" cuadros")).changed() && e != it.exposure {
                            acts.push(Act::ExposureMerge(e));
                        }
                    });
                    let mut vis = it.visible;
                    if ui.checkbox(&mut vis, "Visible (se ve y se exporta)").changed() {
                        acts.push(Act::SetVisible(vis));
                    }
                }
            }
            many => {
                let total: u32 = many.iter().filter_map(|i| seq.index_of(i)).map(|i| seq.items[i].exposure).sum();
                ui.label(RichText::new(format!("{} posiciones elegidas · {total} cuadros", many.len())).color(TX));
                muted(ui, "Los botones de la barra se aplican a todas.");
            }
        }
        ui.add_space(6.0);
        panel_header(ui, "Marcadores y poses");
        let markers = seq.markers.clone();
        if markers.is_empty() {
            muted(ui, "Sin marcadores. P marca una pose y M un marcador en el cuadro actual (con el puntero sobre el panel de la sala).");
        }
        let mut del: Option<usize> = None;
        let mut goto: Option<u32> = None;
        let mut relabel: Option<(usize, String)> = None;
        for (i, m) in markers.iter().enumerate() {
            ui.horizontal(|ui| {
                let (icon, col) = if m.kind == POSE { ("◆", ACCENT) } else { ("▼", Color32::from_rgb(0x6d, 0xb3, 0xe8)) };
                if ui.add(egui::Label::new(RichText::new(format!("{icon} {}", m.at + 1)).color(col)).sense(Sense::click())).on_hover_text("Ir a este cuadro").clicked() {
                    goto = Some(m.at);
                }
                let mut label = m.label.clone();
                if ui.add(egui::TextEdit::singleline(&mut label).desired_width(110.0).font(FontId::proportional(11.5))).changed() {
                    relabel = Some((i, label));
                }
                if ui.small_button("✕").clicked() {
                    del = Some(i);
                }
            });
        }
        if let Some(at) = goto {
            let f = r.start + (at as u64 * self.project.fps as u64).div_ceil(fps.max(1) as u64) as u32;
            self.frame = f.min(self.project.length.saturating_sub(1));
            self.playing = false;
        }
        if let Some(i) = del {
            self.sm_edit(&id, |s| s.remove_marker(i));
        }
        if let Some((i, label)) = relabel {
            self.sm_edit_merge(&id, |s| match s.markers.get_mut(i) {
                Some(m) => {
                    m.label = label;
                    true
                }
                None => false,
            });
        }
        ui.add_space(6.0);
        muted(ui, "Todo lo que se hace en esta sala cambia solo la secuencia: los archivos de las fotos nunca se modifican.");
        for a in acts {
            self.sm_act(&id, &r, a);
        }
    }
}

fn order_of(seq: &Sequence, sel: &[String]) -> Vec<String> {
    seq.items.iter().filter(|i| sel.contains(&i.id)).map(|i| i.id.clone()).collect()
}
