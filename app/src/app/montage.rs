//! Línea de tiempo de montaje (Issue #106, fase M4 de docs/editor-de-tiempo-montaje.md): la línea de tiempo de capas de la
//! sala Interpolación del prototipo v0.3, pensada para motion graphics.
//!
//! A la izquierda, una fila por capa con sus columnas (ojo, solo, bloqueo, etiqueta, número, nombre, tímida, modo de fusión,
//! mate y vínculo); a la derecha, su barra de entrada y salida sobre la regla de cuadros. Al desplegar una capa salen sus
//! propiedades (anclaje, posición, escala, rotación y opacidad), cada una con cronómetro, valor y claves en el tiempo.
//!
//! Todo lo que se cambia aquí se junta en una lista de acciones y se aplica al terminar de dibujar.

use super::icons::{draw_eye, draw_lock, draw_pause, draw_play, icon_button, tri};
use super::shell::pill;
use super::*;
use crate::model::{Blend, XKey, LABELS};
use crate::tracks::Prop;

const ROW_H: f32 = 24.0;
const HEAD_H: f32 = 24.0;
const W_EYE: f32 = 20.0;
const W_SOLO: f32 = 20.0;
const W_LOCK: f32 = 20.0;
const W_LABEL: f32 = 20.0;
const W_NUM: f32 = 24.0;
const W_NAME: f32 = 150.0;
const W_SHY: f32 = 22.0;
const W_BLEND: f32 = 104.0;
const W_MATE: f32 = 96.0;
const W_PARENT: f32 = 112.0;

/// Qué propiedades se muestran cuando no se despliega capa a capa (teclas A, P, S, R, T y U del prototipo).
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Reveal {
    /// Solo estas propiedades, de la capa elegida.
    Props(Vec<Prop>),
    /// Las propiedades que tienen claves, de todas las capas.
    Animated,
}

/// Estado de la línea de tiempo de montaje (no se guarda en el proyecto).
pub(super) struct MtState {
    open: Vec<bool>,
    pub reveal: Option<Reveal>,
    /// Columnas visibles: 0 = interruptores y modos, 1 = solo interruptores, 2 = solo modos.
    cols: u8,
    /// Ancho de un cuadro en píxeles.
    cell: f32,
    last_frame: Option<u32>,
}

impl Default for MtState {
    fn default() -> Self {
        // Para capturas de pantalla: LEONTOON_MONTAJE=1 abre las propiedades de la primera capa.
        let open = if std::env::var("LEONTOON_MONTAJE").is_ok() { vec![true] } else { Vec::new() };
        MtState { open, reveal: None, cols: 0, cell: 16.0, last_frame: None }
    }
}

/// Una fila de la línea de tiempo.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Row {
    Layer(usize),
    Group(usize),
    Prop(usize, Prop),
}

/// Lo que piden los controles de una fila; se aplica después de dibujar.
enum Act {
    Select(usize),
    Frame(u32),
    ToggleOpen(usize),
    ToggleVisible(usize),
    ToggleSolo(usize),
    ToggleLock(usize),
    ToggleShy(usize),
    CycleLabel(usize),
    SetBlend(usize, Blend),
    SetParent(usize, Option<usize>),
    Stopwatch(usize, Prop),
    EditValue(usize, XKey, XKey),
    AddKey(usize, Prop, u32),
    DeleteKey(usize, Prop, u32),
    MoveKey(usize, Prop, u32, u32),
    BarIn(usize, u32),
    BarOut(usize, u32),
    BarShift(usize, i32),
    Checkpoint,
    Fold(usize),
    Group(usize),
    MoveTo(usize, usize),
    Leave(usize),
    Ungroup(usize),
}

/// Lo que se necesita saber de una capa para dibujar su fila sin tener el proyecto prestado.
struct Info {
    name: String,
    kind: LayerKind,
    visible: bool,
    solo: bool,
    locked: bool,
    shy: bool,
    label: u8,
    blend: Blend,
    parent: Option<usize>,
    in_f: u32,
    out_f: u32,
    animated: [bool; 5],
    key_frames: Vec<u32>,
    is_col: bool,
    member: bool,
    collapsed: bool,
}

/// Qué filas salen. `hidden[li]`: la capa está en una colección plegada. `animated[li][p]`: la propiedad `p` de la capa `li` tiene claves. Las capas tímidas se ocultan con el botón
/// maestro (menos la elegida).
pub(super) fn build_rows(shy: &[bool], hidden: &[bool], animated: &[[bool; 5]], cl: usize, hide_shy: bool, open: &[bool], reveal: &Option<Reveal>) -> Vec<Row> {
    let mut out = Vec::new();
    for li in 0..shy.len() {
        if (hide_shy && shy[li] && li != cl) || hidden.get(li).copied().unwrap_or(false) {
            continue;
        }
        out.push(Row::Layer(li));
        let props: Vec<Prop> = match reveal {
            Some(Reveal::Animated) => Prop::ALL.iter().copied().filter(|p| animated[li][*p as usize]).collect(),
            Some(Reveal::Props(ks)) => {
                if li == cl {
                    ks.clone()
                } else {
                    Vec::new()
                }
            }
            None => {
                if open.get(li).copied().unwrap_or(false) {
                    Prop::ALL.to_vec()
                } else {
                    Vec::new()
                }
            }
        };
        if !props.is_empty() {
            if reveal.is_none() {
                out.push(Row::Group(li));
            }
            out.extend(props.into_iter().map(|p| Row::Prop(li, p)));
        }
    }
    out
}

/// El estado de «mostrar propiedades» después de pulsar la tecla de `p` (con Mayús suma o quita).
pub(super) fn reveal_after(cur: &Option<Reveal>, p: Prop, add: bool) -> Option<Reveal> {
    match (cur, add) {
        (Some(Reveal::Props(ks)), true) => {
            let mut ks = ks.clone();
            match ks.iter().position(|k| *k == p) {
                Some(i) => {
                    ks.remove(i);
                }
                None => {
                    ks.push(p);
                    ks.sort();
                }
            }
            if ks.is_empty() { None } else { Some(Reveal::Props(ks)) }
        }
        (Some(Reveal::Props(ks)), false) if ks.len() == 1 && ks[0] == p => None,
        _ => Some(Reveal::Props(vec![p])),
    }
}

fn prop_key(p: Prop) -> &'static str {
    match p {
        Prop::Anchor => "A",
        Prop::Position => "P",
        Prop::Scale => "S",
        Prop::Rotation => "R",
        Prop::Opacity => "T",
    }
}

impl LeonApp {
    fn mt_left_w(&self) -> f32 {
        let mut w = W_EYE + W_SOLO + W_LOCK + W_LABEL + W_NUM + W_NAME;
        if self.mt.cols != 2 {
            w += W_SHY;
        }
        if self.mt.cols != 1 {
            w += W_BLEND + W_MATE + W_PARENT;
        }
        w
    }

    fn mt_rows(&mut self) -> Vec<Row> {
        let n = self.project.layers.len();
        self.mt.open.resize(n, false);
        let shy: Vec<bool> = self.project.layers.iter().map(|l| l.shy).collect();
        let animated: Vec<[bool; 5]> = self.project.layers.iter().map(|l| Prop::ALL.map(|p| l.tracks.has(p))).collect();
        let hidden: Vec<bool> = (0..n).map(|li| self.project.collection_of(li).map_or(false, |ci| self.project.layers[ci].collapsed)).collect();
        build_rows(&shy, &hidden, &animated, self.cl, self.hide_shy, &self.mt.open, &self.mt.reveal)
    }

    /// Teclas de la línea de tiempo de montaje, con el puntero encima: A, P, S, R y T muestran esa propiedad (Mayús suma),
    /// U las animadas y F4 alterna las columnas. Devuelve true si se usó alguna.
    pub(super) fn mt_keys(&mut self, ctx: &egui::Context) -> bool {
        if self.hover_area != 2 || self.slot_editor(Slot::Time) != Editor::Montage || self.room_kind().slots().iter().all(|s| *s != Slot::Time) {
            return false;
        }
        use egui::{Key, Modifiers};
        let table = [(Key::A, Prop::Anchor), (Key::P, Prop::Position), (Key::S, Prop::Scale), (Key::R, Prop::Rotation), (Key::T, Prop::Opacity)];
        for (key, p) in table {
            if ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, key)) {
                self.mt.reveal = reveal_after(&self.mt.reveal, p, true);
                return true;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key)) {
                self.mt.reveal = reveal_after(&self.mt.reveal, p, false);
                return true;
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::U)) {
            self.mt.reveal = if self.mt.reveal == Some(Reveal::Animated) { None } else { Some(Reveal::Animated) };
            return true;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F4)) {
            self.mt.cols = (self.mt.cols + 1) % 3;
            return true;
        }
        false
    }

    /// El editor «Línea de tiempo de montaje».
    pub(super) fn montage_area(&mut self, ui: &mut egui::Ui) {
        let hr = self.slot_head(ui);
        let mut h = ui.new_child(egui::UiBuilder::new().max_rect(hr).layout(egui::Layout::left_to_right(egui::Align::Center)));
        h.spacing_mut().item_spacing.x = 4.0;
        if pill(&mut h, "Tímidas", self.hide_shy).on_hover_text("Oculta de las listas las capas marcadas como tímidas").clicked() {
            self.hide_shy = !self.hide_shy;
        }
        let cols = ["Interruptores y modos", "Interruptores", "Modos"][self.mt.cols as usize % 3];
        if pill(&mut h, cols, false).on_hover_text("Alterna las columnas (F4 con el puntero encima)").clicked() {
            self.mt.cols = (self.mt.cols + 1) % 3;
        }
        h.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(RichText::new(format!("{} / {}", self.frame + 1, self.project.length)).size(11.5).color(TX).strong());
            let playing = self.playing;
            if icon_button(ui, 20.0, playing, "Reproducir / pausa (Espacio)", move |p, r, c| if playing { draw_pause(p, r, c) } else { draw_play(p, r, c) }).clicked() {
                self.toggle_play();
            }
        });
        let rows = self.mt_rows();
        let left_w = self.mt_left_w();
        let avail = ui.available_rect_before_wrap();
        // Ctrl + rueda: zoom del tiempo.
        let (ctrl, dy) = ui.input(|i| (i.modifiers.command, i.raw_scroll_delta.y));
        if ui.rect_contains_pointer(avail) && ctrl && dy != 0.0 {
            self.mt.cell = (self.mt.cell * (1.0 + dy * 0.002)).clamp(6.0, 48.0);
            ui.input_mut(|i| {
                i.smooth_scroll_delta = Vec2::ZERO;
                i.raw_scroll_delta = Vec2::ZERO;
            });
        }
        let cell = self.mt.cell;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(avail));
        let mut acts: Vec<Act> = Vec::new();
        egui::ScrollArea::both().id_salt("montage").auto_shrink([false, false]).show(&mut child, |ui| self.mt_grid(ui, &rows, left_w, cell, &mut acts));
        ui.allocate_rect(avail, Sense::hover());
        for a in acts {
            self.mt_apply(a);
        }
    }

    fn mt_grid(&mut self, ui: &mut egui::Ui, rows: &[Row], left_w: f32, cell: f32, acts: &mut Vec<Act>) {
        let len = self.project.length;
        let n = self.project.layers.len();
        let infos: Vec<Info> = (0..n)
            .map(|li| {
                let l = &self.project.layers[li];
                Info {
                    name: l.name.clone(),
                    kind: l.kind,
                    visible: l.visible,
                    solo: l.solo,
                    locked: l.locked,
                    shy: l.shy,
                    label: l.label,
                    blend: l.blend,
                    parent: l.parent.and_then(|u| self.project.index_of_uid(u)),
                    in_f: l.in_f,
                    out_f: l.out_frame(len),
                    animated: Prop::ALL.map(|p| l.tracks.has(p)),
                    key_frames: l.tracks.frames().into_iter().collect(),
                    is_col: l.kind == LayerKind::Collection,
                    member: self.project.collection_of(li).is_some(),
                    collapsed: l.collapsed,
                }
            })
            .collect();
        let size = Vec2::new(left_w + len as f32 * cell + 60.0, HEAD_H + rows.len() as f32 * ROW_H + 10.0);
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let o = resp.rect.min;
        let clip = ui.clip_rect();
        let (sx, sy) = (clip.min.x.max(o.x), clip.min.y.max(o.y));
        let tx = |f: u32| o.x + left_w + f as f32 * cell;
        let frame = self.frame;
        let fps = self.project.fps.max(1);
        let (f0, f1) = {
            let a = ((clip.min.x - o.x - left_w) / cell).floor().max(0.0) as u32;
            (a, (((clip.max.x - o.x - left_w) / cell).ceil().max(0.0) as u32 + 1).min(len))
        };
        let row_y = |ri: usize| o.y + HEAD_H + ri as f32 * ROW_H;
        painter.rect_filled(resp.rect, 0.0, BG);
        // Qué capas tienen sus propiedades abiertas (si no, se dibujan sus claves en su propia fila).
        let mut has_props = vec![false; n];
        for r in rows {
            if let Row::Prop(li, _) = r {
                has_props[*li] = true;
            }
        }
        // ---- zona de tiempo: fondo de filas, líneas de cuadros, barras y claves ----
        for (ri, row) in rows.iter().enumerate() {
            let y = row_y(ri);
            let li = match row {
                Row::Layer(l) | Row::Group(l) | Row::Prop(l, _) => *l,
            };
            let bg = match row {
                Row::Layer(_) if li == self.cl => SEL_BG,
                Row::Layer(_) => if ri % 2 == 0 { BG2 } else { BG3 },
                _ => Color32::from_rgb(0x20, 0x1f, 0x1e),
            };
            painter.rect_filled(Rect::from_min_size(Pos2::new(o.x + left_w, y), Vec2::new(len as f32 * cell + 60.0, ROW_H)), 0.0, bg);
        }
        for f in f0..f1 {
            let x = tx(f);
            let strong = f % fps == 0 && f != 0;
            painter.vline(x, (o.y + HEAD_H)..=(o.y + HEAD_H + rows.len() as f32 * ROW_H), stk(1.0, Color32::from_black_alpha(if strong { 110 } else { 45 })));
        }
        let mut drag_state: Option<(usize, Prop, u32)> = ui.data(|d| d.get_temp(ui.id().with("mt_key_drag")));
        for (ri, row) in rows.iter().enumerate() {
            let y = row_y(ri);
            match *row {
                Row::Layer(li) => {
                    let inf = &infos[li];
                    let (x0, x1) = (tx(inf.in_f), tx(inf.out_f + 1));
                    let col = if inf.label == 0 { c32(inf.kind.tint()) } else { c32(LABELS[(inf.label as usize).min(LABELS.len() - 1)]) };
                    let bar = Rect::from_min_max(Pos2::new(x0, y + 4.0), Pos2::new(x1, y + ROW_H - 4.0));
                    painter.rect_filled(bar, 3.0, with_alpha(col, if inf.visible { 0.62 } else { 0.25 }));
                    if li == self.cl {
                        painter.rect_stroke(bar, 3.0, stk(1.0, with_alpha(col, 0.95)), EStrokeKind::Inside);
                    }
                    // Si no tiene propiedades abiertas, sus claves se ven sobre su barra.
                    if !has_props[li] {
                        for &kf in &inf.key_frames {
                            if kf >= f0.saturating_sub(1) && kf <= f1 {
                                diamond(&painter, Pos2::new(tx(kf) + cell / 2.0, y + ROW_H / 2.0), 4.0, KEY_GOLD);
                            }
                        }
                    }
                    // Arrastrar la barra: los extremos recortan, el cuerpo mueve la capa en el tiempo.
                    let id = ui.id().with(("mt_bar", li));
                    let body = Rect::from_min_max(Pos2::new(x0 + 6.0, bar.min.y), Pos2::new((x1 - 6.0).max(x0 + 6.0), bar.max.y));
                    let r_in = ui.interact(Rect::from_min_max(Pos2::new(x0 - 2.0, bar.min.y), Pos2::new(x0 + 6.0, bar.max.y)), id.with("in"), Sense::click_and_drag());
                    let r_out = ui.interact(Rect::from_min_max(Pos2::new(x1 - 6.0, bar.min.y), Pos2::new(x1 + 2.0, bar.max.y)), id.with("out"), Sense::click_and_drag());
                    let r_body = ui.interact(body, id.with("body"), Sense::click_and_drag());
                    for (r, tip) in [(&r_in, "Arrastra para cambiar la entrada"), (&r_out, "Arrastra para cambiar la salida")] {
                        if r.hovered() || r.dragged() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            r.clone().on_hover_text(tip);
                        }
                    }
                    if r_body.hovered() || r_body.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                    let frame_at = |p: Pos2| ((p.x - o.x - left_w) / cell).round().max(0.0) as u32;
                    if r_in.drag_started() || r_out.drag_started() || r_body.drag_started() {
                        acts.push(Act::Checkpoint);
                        acts.push(Act::Select(li));
                    }
                    if r_in.dragged() {
                        if let Some(p) = r_in.interact_pointer_pos() {
                            acts.push(Act::BarIn(li, frame_at(p)));
                        }
                    }
                    if r_out.dragged() {
                        if let Some(p) = r_out.interact_pointer_pos() {
                            acts.push(Act::BarOut(li, frame_at(p).saturating_sub(1)));
                        }
                    }
                    if r_body.dragged() {
                        let acc_id = ui.id().with(("mt_acc", li));
                        let acc: f32 = ui.data(|d| d.get_temp(acc_id)).unwrap_or(0.0) + r_body.drag_delta().x;
                        let d = (acc / cell).trunc();
                        if d != 0.0 {
                            acts.push(Act::BarShift(li, d as i32));
                        }
                        ui.data_mut(|dd| dd.insert_temp(acc_id, acc - d * cell));
                    }
                    if r_body.drag_stopped() {
                        ui.data_mut(|dd| dd.insert_temp(ui.id().with(("mt_acc", li)), 0.0f32));
                    }
                }
                Row::Prop(li, p) => {
                    let frames = self.project.layers[li].tracks.prop_frames(p);
                    for kf in frames {
                        if kf + 1 < f0 || kf > f1 {
                            continue;
                        }
                        let c = Pos2::new(tx(kf) + cell / 2.0, y + ROW_H / 2.0);
                        let r = ui.interact(Rect::from_center_size(c, Vec2::splat(12.0)), ui.id().with(("mt_key", li, p, kf)), Sense::click_and_drag());
                        let dragging = drag_state == Some((li, p, kf)) && r.dragged();
                        diamond(&painter, c, if r.hovered() { 5.5 } else { 4.5 }, if dragging { with_alpha(KEY_GOLD, 0.4) } else { KEY_GOLD });
                        r.clone().on_hover_text("Clic: ir a este cuadro · arrastrar: mover la clave · clic derecho: borrarla");
                        if r.clicked() {
                            acts.push(Act::Select(li));
                            acts.push(Act::Frame(kf));
                        }
                        if r.secondary_clicked() {
                            acts.push(Act::Checkpoint);
                            acts.push(Act::DeleteKey(li, p, kf));
                        }
                        if r.drag_started() {
                            acts.push(Act::Checkpoint);
                            drag_state = Some((li, p, kf));
                            ui.data_mut(|d| d.insert_temp(ui.id().with("mt_key_drag"), (li, p, kf)));
                        }
                        if dragging {
                            if let Some(pos) = r.interact_pointer_pos() {
                                let f = ((pos.x - o.x - left_w) / cell).floor().max(0.0) as u32;
                                diamond(&painter, Pos2::new(tx(f) + cell / 2.0, y + ROW_H / 2.0), 4.5, KEY_GOLD);
                            }
                        }
                        if r.drag_stopped() {
                            if let (Some((dl, dp, df)), Some(pos)) = (drag_state, r.interact_pointer_pos()) {
                                let f = ((pos.x - o.x - left_w) / cell).floor().max(0.0) as u32;
                                if f != df {
                                    acts.push(Act::MoveKey(dl, dp, df, f));
                                }
                            }
                            drag_state = None;
                            ui.data_mut(|d| d.remove::<(usize, Prop, u32)>(ui.id().with("mt_key_drag")));
                        }
                    }
                }
                Row::Group(_) => {}
            }
        }
        // Clic en la zona de tiempo: elige la capa y lleva el cabezal; doble clic en una fila de propiedad crea una clave.
        if let Some(p) = resp.interact_pointer_pos() {
            if p.x >= o.x + left_w && p.y >= o.y + HEAD_H && (resp.clicked() || resp.double_clicked()) {
                let ri = ((p.y - o.y - HEAD_H) / ROW_H).floor();
                let f = ((p.x - o.x - left_w) / cell).floor().max(0.0) as u32;
                if ri >= 0.0 && (ri as usize) < rows.len() && f < len {
                    let row = rows[ri as usize];
                    let li = match row {
                        Row::Layer(l) | Row::Group(l) | Row::Prop(l, _) => l,
                    };
                    acts.push(Act::Select(li));
                    acts.push(Act::Frame(f));
                    if let (true, Row::Prop(_, pr)) = (resp.double_clicked(), row) {
                        acts.push(Act::Checkpoint);
                        acts.push(Act::AddKey(li, pr, f));
                    }
                }
            }
        }
        // ---- columnas de la izquierda (pegajosas: se quedan a la vista al desplazarse en horizontal) ----
        let lx = sx;
        for (ri, row) in rows.iter().enumerate() {
            let y = row_y(ri);
            let full = Rect::from_min_size(Pos2::new(lx, y), Vec2::new(left_w, ROW_H));
            match *row {
                Row::Layer(li) => {
                    let inf = &infos[li];
                    let sel = li == self.cl;
                    painter.rect_filled(full, 0.0, if sel { SEL_BG } else if ri % 2 == 0 { BG2 } else { BG3 });
                    let mut x = lx;
                    let mut cellr = |w: f32| {
                        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, ROW_H));
                        x += w;
                        r
                    };
                    let toggle = |ui: &mut egui::Ui, r: Rect, salt: &str| ui.interact(r, ui.id().with((salt, li)), Sense::click());
                    let r = cellr(W_EYE);
                    draw_eye(&painter, r, inf.visible, false);
                    if toggle(ui, r, "mt_eye").on_hover_text("Visible").clicked() {
                        acts.push(Act::ToggleVisible(li));
                    }
                    let r = cellr(W_SOLO);
                    if inf.solo {
                        painter.circle_filled(r.center(), 4.0, KEY_GOLD);
                    } else {
                        painter.circle_stroke(r.center(), 3.5, stk(1.0, INK3));
                    }
                    if toggle(ui, r, "mt_solo").on_hover_text("Solo: si alguna capa está en solo, únicamente se ven las que lo están").clicked() {
                        acts.push(Act::ToggleSolo(li));
                    }
                    let r = cellr(W_LOCK);
                    draw_lock(&painter, r, inf.locked, false);
                    if toggle(ui, r, "mt_lock").on_hover_text("Bloquear").clicked() {
                        acts.push(Act::ToggleLock(li));
                    }
                    let r = cellr(W_LABEL);
                    let lc = if inf.label == 0 { BG4 } else { c32(LABELS[(inf.label as usize).min(LABELS.len() - 1)]) };
                    painter.rect_filled(Rect::from_center_size(r.center(), Vec2::splat(11.0)), 3.0, lc);
                    if toggle(ui, r, "mt_label").on_hover_text("Etiqueta de color: clic para cambiarla").clicked() {
                        acts.push(Act::CycleLabel(li));
                    }
                    let r = cellr(W_NUM);
                    painter.text(r.center(), Align2::CENTER_CENTER, format!("{}", li + 1), FontId::proportional(11.0), MU);
                    // Nombre: flecha para desplegar, tipo y nombre. Las capas de una colección van sangradas; la flecha de una
                    // colección pliega sus capas y la de las demás despliega sus propiedades.
                    let r = cellr(W_NAME);
                    let open = has_props[li];
                    let ind = if inf.member { 14.0 } else { 0.0 };
                    let arrow = Rect::from_min_size(r.min + Vec2::new(ind, 0.0), Vec2::new(16.0, ROW_H));
                    let c = arrow.center();
                    let down = if inf.is_col { !inf.collapsed } else { open };
                    if down {
                        tri(&painter, c + Vec2::new(-3.5, -2.0), c + Vec2::new(3.5, -2.0), c + Vec2::new(0.0, 3.0), MU);
                    } else {
                        tri(&painter, c + Vec2::new(-2.0, -3.5), c + Vec2::new(-2.0, 3.5), c + Vec2::new(3.0, 0.0), MU);
                    }
                    if ui.interact(arrow, ui.id().with(("mt_arrow", li)), Sense::click()).on_hover_text(if inf.is_col { "Plegar o desplegar la colección" } else { "Mostrar u ocultar sus propiedades" }).clicked() {
                        acts.push(if inf.is_col { Act::Fold(li) } else { Act::ToggleOpen(li) });
                    }
                    painter.text(Pos2::new(r.min.x + ind + 20.0, y + ROW_H / 2.0), Align2::LEFT_CENTER, inf.kind.badge(), FontId::proportional(9.5), c32(inf.kind.tint()));
                    let name_r = Rect::from_min_max(Pos2::new(r.min.x + ind + 32.0, y), r.max);
                    painter.with_clip_rect(name_r).text(Pos2::new(name_r.min.x, y + ROW_H / 2.0), Align2::LEFT_CENTER, &inf.name, semi(11.5), if inf.visible { TX } else { INK3 });
                    let name_resp = ui.interact(Rect::from_min_max(r.min + Vec2::new(ind + 16.0, 0.0), r.max), ui.id().with(("mt_name", li)), Sense::click());
                    if name_resp.clicked() {
                        acts.push(Act::Select(li));
                    }
                    name_resp.context_menu(|ui| {
                        ui.label(RichText::new(&inf.name).strong());
                        ui.separator();
                        if !inf.is_col && !inf.member && ui.button("Agrupar en colección nueva   Ctrl+G").clicked() {
                            acts.push(Act::Group(li));
                            ui.close_menu();
                        }
                        if !inf.is_col {
                            for (ci, other) in infos.iter().enumerate().filter(|(_, o)| o.is_col) {
                                if self.project.collection_of(li) != Some(ci) && ui.button(format!("Mover a «{}»", other.name)).clicked() {
                                    acts.push(Act::MoveTo(li, ci));
                                    ui.close_menu();
                                }
                            }
                        }
                        if inf.member && ui.button("Sacar de la colección").clicked() {
                            acts.push(Act::Leave(li));
                            ui.close_menu();
                        }
                        if inf.is_col {
                            if ui.button(if inf.collapsed { "Desplegar" } else { "Plegar" }).clicked() {
                                acts.push(Act::Fold(li));
                                ui.close_menu();
                            }
                            if ui.button("Desagrupar   Ctrl+Mayús+G").clicked() {
                                acts.push(Act::Ungroup(li));
                                ui.close_menu();
                            }
                        }
                    });                    if self.mt.cols != 2 {
                        let r = cellr(W_SHY);
                        if inf.shy {
                            painter.circle_stroke(r.center(), 4.5, stk(1.3, KEY_GOLD));
                            painter.line_segment([r.center() + Vec2::new(-3.0, 3.0), r.center() + Vec2::new(3.0, -3.0)], stk(1.3, KEY_GOLD));
                        } else {
                            painter.circle_stroke(r.center(), 4.5, stk(1.0, with_alpha(INK3, 0.6)));
                        }
                        if toggle(ui, r, "mt_shy").on_hover_text("Tímida: se puede ocultar de las listas con el botón «Tímidas»").clicked() {
                            acts.push(Act::ToggleShy(li));
                        }
                    }
                    if self.mt.cols != 1 {
                        let r = cellr(W_BLEND);
                        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(r.shrink2(Vec2::new(2.0, 2.0))), |ui| {
                            egui::ComboBox::from_id_salt(("mt_blend", li)).selected_text(inf.blend.label()).width(r.width() - 8.0).show_ui(ui, |ui| {
                                for b in Blend::ALL {
                                    if ui.selectable_label(inf.blend == b, b.label()).clicked() {
                                        acts.push(Act::SetBlend(li, b));
                                    }
                                }
                            });
                        });
                        let r = cellr(W_MATE);
                        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(r.shrink2(Vec2::new(2.0, 2.0))), |ui| {
                            ui.add_enabled_ui(false, |ui| {
                                egui::ComboBox::from_id_salt(("mt_mate", li)).selected_text("Ninguno").width(r.width() - 8.0).show_ui(ui, |_| {});
                            })
                            .response
                            .on_hover_text("Los mates de seguimiento llegan en una fase aparte");
                        });
                        let r = cellr(W_PARENT);
                        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(r.shrink2(Vec2::new(2.0, 2.0))), |ui| {
                            let cur = inf.parent.map_or("Ninguno".to_string(), |p| format!("{}. {}", p + 1, infos[p].name));
                            egui::ComboBox::from_id_salt(("mt_parent", li)).selected_text(cur).width(r.width() - 8.0).show_ui(ui, |ui| {
                                if ui.selectable_label(inf.parent.is_none(), "Ninguno").clicked() {
                                    acts.push(Act::SetParent(li, None));
                                }
                                for (j, other) in infos.iter().enumerate() {
                                    if j == li {
                                        continue;
                                    }
                                    let cyc = self.project.would_cycle(li, j);
                                    let r = ui.add_enabled(!cyc, egui::SelectableLabel::new(inf.parent == Some(j), format!("{}. {}", j + 1, other.name)));
                                    if cyc {
                                        r.on_disabled_hover_text("Crearía un ciclo: esa capa ya depende de esta");
                                    } else if r.clicked() {
                                        acts.push(Act::SetParent(li, Some(j)));
                                    }
                                }
                            });
                        });
                    }
                }
                Row::Group(_) => {
                    painter.rect_filled(full, 0.0, Color32::from_rgb(0x20, 0x1f, 0x1e));
                    painter.text(Pos2::new(lx + W_EYE + W_SOLO + W_LOCK + W_LABEL + 28.0, y + ROW_H / 2.0), Align2::LEFT_CENTER, "Transformar", semi(11.0), MU);
                }
                Row::Prop(li, p) => {
                    painter.rect_filled(full, 0.0, Color32::from_rgb(0x20, 0x1f, 0x1e));
                    let animated = infos[li].animated[p as usize];
                    let ix = lx + W_EYE + W_SOLO + W_LOCK + W_LABEL + 36.0;
                    // Cronómetro
                    let sw = Rect::from_min_size(Pos2::new(ix, y), Vec2::new(20.0, ROW_H));
                    let resp_sw = ui.interact(sw, ui.id().with(("mt_sw", li, p)), Sense::click());
                    let col = if animated { ACCENT } else if resp_sw.hovered() { TX } else { MU };
                    let c = sw.center() + Vec2::new(0.0, 1.0);
                    painter.circle_stroke(c, 5.0, stk(1.4, col));
                    painter.line_segment([c, c + Vec2::new(2.2, -2.2)], stk(1.4, col));
                    painter.line_segment([c + Vec2::new(-1.4, -6.5), c + Vec2::new(1.4, -6.5)], stk(1.4, col));
                    if resp_sw.on_hover_text(if animated { "Cronómetro activo: pulsa para dejar de animar esta propiedad (borra sus claves)" } else { "Pulsa para animar esta propiedad desde el cuadro actual" }).clicked() {
                        acts.push(Act::Stopwatch(li, p));
                    }
                    painter.text(Pos2::new(ix + 26.0, y + ROW_H / 2.0), Align2::LEFT_CENTER, p.name(), FontId::proportional(11.5), TX);
                    painter.text(Pos2::new(ix + 26.0 + 100.0, y + ROW_H / 2.0), Align2::LEFT_CENTER, prop_key(p), FontId::proportional(9.5), INK3);
                    // Valor en este cuadro, editable.
                    let now = self.project.layers[li].transform_at(frame);
                    let mut v = p.of(&now);
                    let two = matches!(p, Prop::Anchor | Prop::Position);
                    let vr = Rect::from_min_size(Pos2::new(lx + left_w - if two { 150.0 } else { 82.0 }, y + 2.0), Vec2::new(if two { 144.0 } else { 78.0 }, ROW_H - 4.0));
                    let mut changed = false;
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(vr).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        match p {
                            Prop::Anchor | Prop::Position => {
                                changed |= ui.add(egui::DragValue::new(&mut v[0]).speed(1.0).max_decimals(1).prefix("x ")).changed();
                                changed |= ui.add(egui::DragValue::new(&mut v[1]).speed(1.0).max_decimals(1).prefix("y ")).changed();
                            }
                            Prop::Scale => changed |= ui.add(egui::DragValue::new(&mut v[0]).speed(0.01).max_decimals(2)).changed(),
                            Prop::Rotation => changed |= ui.add(egui::DragValue::new(&mut v[0]).speed(1.0).max_decimals(1).suffix("°")).changed(),
                            Prop::Opacity => changed |= ui.add(egui::DragValue::new(&mut v[0]).speed(0.01).max_decimals(2).range(0.0..=1.0)).changed(),
                        }
                    });
                    if changed {
                        let mut new = now;
                        match p {
                            Prop::Anchor => (new.ax, new.ay) = (v[0], v[1]),
                            Prop::Position => (new.x, new.y) = (v[0], v[1]),
                            Prop::Scale => new.s = v[0],
                            Prop::Rotation => new.r = v[0],
                            Prop::Opacity => new.o = v[0].clamp(0.0, 1.0),
                        }
                        acts.push(Act::EditValue(li, new, now));
                    }
                }
            }
            painter.hline(lx..=(lx + left_w), y + ROW_H, stk(1.0, Color32::from_black_alpha(90)));
        }
        // ---- cabecera y regla (pegajosas) ----
        let head = Rect::from_min_size(Pos2::new(lx, sy), Vec2::new(left_w, HEAD_H));
        painter.rect_filled(head, 0.0, BG3);
        painter.text(Pos2::new(lx + W_EYE + W_SOLO + W_LOCK + W_LABEL + 4.0, sy + HEAD_H / 2.0), Align2::LEFT_CENTER, "#", FontId::proportional(10.5), MU);
        let mut hx = lx + W_EYE + W_SOLO + W_LOCK + W_LABEL + W_NUM + 18.0;
        painter.text(Pos2::new(hx, sy + HEAD_H / 2.0), Align2::LEFT_CENTER, "Nombre de la capa", FontId::proportional(10.5), MU);
        hx = lx + W_EYE + W_SOLO + W_LOCK + W_LABEL + W_NUM + W_NAME;
        if self.mt.cols != 2 {
            painter.text(Pos2::new(hx + 4.0, sy + HEAD_H / 2.0), Align2::LEFT_CENTER, "~", FontId::proportional(11.0), MU);
            hx += W_SHY;
        }
        if self.mt.cols != 1 {
            for (t, w) in [("Modo", W_BLEND), ("Mate", W_MATE), ("Primario y vínculo", W_PARENT)] {
                painter.text(Pos2::new(hx + 6.0, sy + HEAD_H / 2.0), Align2::LEFT_CENTER, t, FontId::proportional(10.5), MU);
                hx += w;
            }
        }
        let ruler = Rect::from_min_size(Pos2::new(o.x + left_w, sy), Vec2::new(len as f32 * cell + 60.0, HEAD_H));
        painter.rect_filled(ruler, 0.0, BG3);
        for f in f0..f1 {
            let x = tx(f);
            let maj = f % 4 == 0;
            painter.vline(x, (sy + HEAD_H - if maj { 8.0 } else { 4.0 })..=(sy + HEAD_H), stk(1.0, if maj { Color32::from_gray(0x88) } else { Color32::from_gray(0x55) }));
            if f % 2 == 0 && f != frame && cell >= 9.0 {
                painter.text(Pos2::new(x + 2.0, sy + 8.0), Align2::LEFT_CENTER, format!("{}", f + 1), FontId::proportional(9.5), MU);
            }
        }
        // Cabezal
        let hxp = tx(frame) + cell / 2.0;
        painter.vline(hxp, (sy)..=(o.y + HEAD_H + rows.len() as f32 * ROW_H), stk(1.5, ACCENT));
        let lab = Rect::from_center_size(Pos2::new(hxp, sy + HEAD_H / 2.0), Vec2::new(24.0, 15.0));
        painter.rect_filled(lab, 3.0, ACCENT);
        painter.text(lab.center(), Align2::CENTER_CENTER, format!("{}", frame + 1), FontId::proportional(10.0), ACCENT_INK);
        // Clic o arrastre en la regla: mueve el cabezal.
        let rr = ui.interact(ruler.intersect(Rect::from_min_max(Pos2::new(clip.min.x.max(o.x + left_w), sy), Pos2::new(clip.max.x, sy + HEAD_H))), ui.id().with("mt_ruler"), Sense::click_and_drag());
        if rr.clicked() || rr.dragged() {
            if let Some(p) = rr.interact_pointer_pos() {
                let f = (((p.x - o.x - left_w) / cell).floor().max(0.0) as u32).min(len.saturating_sub(1));
                acts.push(Act::Frame(f));
            }
        }
        // Mantiene el cabezal a la vista cuando cambia de cuadro por otro lado.
        if self.mt.last_frame != Some(frame) {
            ui.scroll_to_rect(Rect::from_min_size(Pos2::new(tx(frame), o.y), Vec2::new(cell, 10.0)).expand2(Vec2::new(cell * 4.0, 0.0)), None);
            self.mt.last_frame = Some(frame);
        }
    }

    fn mt_apply(&mut self, a: Act) {
        let n = self.project.layers.len();
        let frame = self.frame;
        match a {
            Act::Checkpoint => self.checkpoint(),
            Act::Select(li) if li < n => {
                self.cl = li;
                self.sel = None;
            }
            Act::Frame(f) => {
                self.playing = false;
                self.frame = f.min(self.project.length.saturating_sub(1));
            }
            Act::ToggleOpen(li) => {
                self.mt.reveal = None;
                if let Some(o) = self.mt.open.get_mut(li) {
                    *o = !*o;
                }
                self.cl = li.min(n.saturating_sub(1));
            }
            Act::ToggleVisible(li) => self.project.layers[li].visible = !self.project.layers[li].visible,
            Act::ToggleSolo(li) => self.project.layers[li].solo = !self.project.layers[li].solo,
            Act::ToggleLock(li) => self.project.layers[li].locked = !self.project.layers[li].locked,
            Act::ToggleShy(li) => self.project.layers[li].shy = !self.project.layers[li].shy,
            Act::CycleLabel(li) => self.project.layers[li].label = (self.project.layers[li].label + 1) % LABELS.len() as u8,
            Act::SetBlend(li, b) => {
                self.checkpoint();
                self.project.layers[li].blend = b;
            }
            Act::SetParent(li, p) => {
                self.checkpoint();
                if !self.project.set_parent(li, p) {
                    self.say("Ese vínculo crearía un ciclo");
                }
            }
            Act::Stopwatch(li, p) => {
                self.cl = li;
                self.toggle_stopwatch(p);
            }
            Act::EditValue(li, new, old) => {
                if !self.project.layers[li].locked {
                    self.project.layers[li].tracks.set_changed(frame, &new, &old);
                }
            }
            Act::AddKey(li, p, f) => {
                let l = &mut self.project.layers[li];
                let (v, i) = l.tracks.value_at(p, f);
                l.tracks.set(p, f, v, i);
            }
            Act::DeleteKey(li, p, f) => {
                self.project.layers[li].tracks.remove_prop_at(p, f);
            }
            Act::MoveKey(li, p, from, to) => {
                self.project.layers[li].tracks.move_key(p, from, to);
            }
            Act::BarIn(li, f) => {
                let len = self.project.length;
                let l = &mut self.project.layers[li];
                l.in_f = f.min(l.out_frame(len));
            }
            Act::BarOut(li, f) => {
                let len = self.project.length;
                let last = len.saturating_sub(1);
                let l = &mut self.project.layers[li];
                let o = f.clamp(l.in_f, last);
                l.out_f = if o >= last { None } else { Some(o) };
            }
            Act::BarShift(li, d) => {
                self.project.layers[li].shift_time(d);
            }
            Act::Fold(li) => {
                if let Some(l) = self.project.layers.get_mut(li) {
                    l.collapsed = !l.collapsed;
                }
                self.cl = li.min(n.saturating_sub(1));
            }
            Act::Group(li) => {
                self.cl = li.min(n.saturating_sub(1));
                self.group_current();
            }
            Act::MoveTo(li, ci) => self.move_to_collection(li, ci),
            Act::Leave(li) => self.leave_collection(li),
            Act::Ungroup(ci) => {
                self.cl = ci.min(n.saturating_sub(1));
                self.ungroup_current();
            }
            _ => {}
        }
    }
}

/// Rombo de clave.
fn diamond(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    p.add(Shape::convex_polygon(vec![c + Vec2::new(0.0, -r), c + Vec2::new(r, 0.0), c + Vec2::new(0.0, r), c + Vec2::new(-r, 0.0)], col, EStroke::NONE));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animated(n: usize) -> Vec<[bool; 5]> {
        vec![[false; 5]; n]
    }

    #[test]
    fn sin_desplegar_hay_una_fila_por_capa_y_al_desplegar_salen_el_grupo_y_las_cinco_propiedades() {
        let shy = [false, false, false];
        let mut open = vec![false; 3];
        let rows = build_rows(&shy, &[false; 3], &animated(3), 0, false, &open, &None);
        assert_eq!(rows, vec![Row::Layer(0), Row::Layer(1), Row::Layer(2)]);
        open[1] = true;
        let rows = build_rows(&shy, &[false; 3], &animated(3), 0, false, &open, &None);
        assert_eq!(rows.len(), 3 + 1 + 5);
        assert_eq!(rows[1], Row::Layer(1));
        assert_eq!(rows[2], Row::Group(1));
        assert_eq!(rows[3], Row::Prop(1, Prop::Anchor));
        assert_eq!(rows[7], Row::Prop(1, Prop::Opacity));
        assert_eq!(rows[8], Row::Layer(2));
    }

    #[test]
    fn las_capas_de_una_coleccion_plegada_no_salen_y_las_demas_siguen() {
        let shy = [false; 4];
        // La capa 1 y la 2 están en una colección plegada (la 0).
        let hidden = [false, true, true, false];
        let rows = build_rows(&shy, &hidden, &animated(4), 0, false, &[false; 4], &None);
        assert_eq!(rows, vec![Row::Layer(0), Row::Layer(3)]);
    }

    #[test]
    fn las_capas_timidas_se_ocultan_menos_la_elegida() {
        let shy = [false, true, true];
        let rows = build_rows(&shy, &[false; 3], &animated(3), 2, true, &[false; 3], &None);
        assert_eq!(rows, vec![Row::Layer(0), Row::Layer(2)]);
        let all = build_rows(&shy, &[false; 3], &animated(3), 2, false, &[false; 3], &None);
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn las_teclas_de_propiedad_muestran_solo_esa_y_u_las_animadas() {
        let shy = [false, false];
        let mut an = animated(2);
        an[0][Prop::Position as usize] = true;
        an[1][Prop::Opacity as usize] = true;
        // P en la capa elegida (1): solo su posición, ninguna fila de la otra capa.
        let rows = build_rows(&shy, &[false; 2], &an, 1, false, &[false; 2], &Some(Reveal::Props(vec![Prop::Position])));
        assert_eq!(rows, vec![Row::Layer(0), Row::Layer(1), Row::Prop(1, Prop::Position)]);
        // U: las propiedades con claves de todas las capas, sin la fila «Transformar».
        let rows = build_rows(&shy, &[false; 2], &an, 0, false, &[false; 2], &Some(Reveal::Animated));
        assert_eq!(rows, vec![Row::Layer(0), Row::Prop(0, Prop::Position), Row::Layer(1), Row::Prop(1, Prop::Opacity)]);
    }

    #[test]
    fn pulsar_la_misma_tecla_otra_vez_la_oculta_y_mayus_suma_y_quita() {
        let a = reveal_after(&None, Prop::Position, false);
        assert_eq!(a, Some(Reveal::Props(vec![Prop::Position])));
        assert_eq!(reveal_after(&a, Prop::Position, false), None);
        let b = reveal_after(&a, Prop::Opacity, true);
        assert_eq!(b, Some(Reveal::Props(vec![Prop::Position, Prop::Opacity])));
        let c = reveal_after(&b, Prop::Position, true);
        assert_eq!(c, Some(Reveal::Props(vec![Prop::Opacity])));
        assert_eq!(reveal_after(&c, Prop::Opacity, true), None);
        // Otra tecla sin Mayús sustituye.
        assert_eq!(reveal_after(&b, Prop::Scale, false), Some(Reveal::Props(vec![Prop::Scale])));
    }
}
