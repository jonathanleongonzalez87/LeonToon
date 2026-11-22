//! Disposición de la sala Stop Motion al estilo de Dragonframe: la X-Sheet a la izquierda, el escenario (Live View o
//! visor) con su barra de transporte y captura al centro, la cámara y sus herramientas a la derecha, y la tira de
//! fotos abajo. Todo a la vista, sin pestañas para lo que se usa mientras se anima.
//!
//! Si la capa todavía no tiene secuencia, la sala muestra el visor y, debajo, el panel con los botones para crearla.

use super::panelsize::{resize_handle, Resize};
use super::shell::{area_header, in_area};
use super::smui::SmState;
use super::*;

const LEFT_DEFAULT: f32 = 270.0;
const RIGHT_DEFAULT: f32 = 250.0;
const BOTTOM_DEFAULT: f32 = 190.0;
const BAR_H: f32 = 72.0;

/// Tamaños de los paneles (se arrastran los bordes; doble clic vuelve al de por defecto). 0 = el de por defecto.
#[derive(Default)]
pub(super) struct DragonSizes {
    left: f32,
    right: f32,
    bottom: f32,
}

/// Cuánto ocupa cada panel dentro de `area` (respetando un mínimo útil y dejando sitio al escenario).
fn sizes(ds: &DragonSizes, area: Rect) -> (f32, f32, f32) {
    let pick = |v: f32, d: f32| if v <= 0.0 { d } else { v };
    let side_max = (area.width() * 0.3).max(160.0);
    (
        pick(ds.left, LEFT_DEFAULT).clamp(160.0, side_max),
        pick(ds.right, RIGHT_DEFAULT).clamp(180.0, side_max),
        pick(ds.bottom, BOTTOM_DEFAULT).clamp(110.0, (area.height() * 0.45).max(110.0)),
    )
}

impl LeonApp {
    /// ¿Hay una capa Stop Motion con su secuencia abierta, lista para usar la disposición completa?
    fn sm_dragon_ready(&mut self) -> bool {
        let Some(li) = self.sm_room_layer() else { return false };
        let Some(r) = self.project.layers[li].stopmotion.clone() else { return false };
        !matches!(self.sm_state(&r), SmState::Missing(_)) && self.sm.contains_key(&r.id)
    }

    /// Dibuja toda la sala Stop Motion dentro de `area`.
    pub(super) fn sm_dragon(&mut self, ui: &mut egui::Ui, area: Rect) {
        let gap = 2.0;
        if !self.playing {
            self.sm_play_range = None;
        }
        let pointer = ui.input(|i| i.pointer.hover_pos());
        if !self.sm_dragon_ready() {
            // Sin secuencia: visor arriba y, abajo, los avisos y botones para crearla o buscarla.
            let h = BOTTOM_DEFAULT.min(area.height() * 0.5);
            let view = Rect::from_min_max(area.min, Pos2::new(area.max.x, area.max.y - h - gap));
            let bottom = Rect::from_min_max(Pos2::new(area.min.x, area.max.y - h), area.max);
            in_area(ui, view, |ui| self.viewer(ui));
            in_area(ui, bottom, |ui| self.sm_bottom(ui));
            if let Some(p) = pointer {
                if view.contains(p) {
                    self.hover_area = 1;
                }
            }
            return;
        }
        let r = self.sm_room_layer().and_then(|li| self.project.layers[li].stopmotion.clone()).expect("comprobado en sm_dragon_ready");
        let id = r.id.clone();
        let (lw, rw, bh) = sizes(&self.sm_ui.dw, area);
        let bottom = Rect::from_min_max(Pos2::new(area.min.x, area.max.y - bh), area.max);
        let upper = Rect::from_min_max(area.min, Pos2::new(area.max.x, bottom.min.y - gap));
        let left = Rect::from_min_max(upper.min, Pos2::new(upper.min.x + lw, upper.max.y));
        let right = Rect::from_min_max(Pos2::new(upper.max.x - rw, upper.min.y), upper.max);
        let mid = Rect::from_min_max(Pos2::new(left.max.x + gap, upper.min.y), Pos2::new(right.min.x - gap, upper.max.y));
        let bar = Rect::from_min_max(Pos2::new(mid.min.x, (mid.max.y - BAR_H).max(mid.min.y)), mid.max);
        let stage = Rect::from_min_max(mid.min, Pos2::new(mid.max.x, (bar.min.y - gap).max(mid.min.y)));

        // X-Sheet (columna izquierda).
        in_area(ui, left, |ui| {
            area_header(ui, "X-Sheet");
            let mut acts = Vec::new();
            self.sm_xsheet(ui, &id, &r, &mut acts, true);
            for a in acts {
                self.sm_act(&id, &r, a);
            }
        });
        // Escenario: el Live View mientras se anima; al reproducir, lo capturado (la cámara sigue conectada).
        if self.sm_ui.cap.live && !self.playing {
            in_area(ui, stage, |ui| self.sm_live_view(ui));
        } else {
            in_area(ui, stage, |ui| self.viewer(ui));
        }
        in_area(ui, bar, |ui| self.sm_capture_bar(ui));
        in_area(ui, right, |ui| self.sm_cam_panel(ui));
        in_area(ui, bottom, |ui| self.sm_bottom(ui));
        if let Some(p) = pointer {
            if stage.contains(p) {
                self.hover_area = 1;
            } else if bottom.contains(p) || left.contains(p) {
                self.hover_area = 2;
            }
            // Los atajos de edición de la sala (Supr, Ctrl+D, +/−…) valen sobre la X-Sheet y la tira de fotos.
            self.sm_ui.over_panel = bottom.contains(p) || left.contains(p);
        }

        // Bordes arrastrables.
        let mut sz = std::mem::take(&mut self.sm_ui.dw);
        let v = Rect::from_center_size(Pos2::new(left.max.x + gap * 0.5, upper.center().y), Vec2::new(8.0, upper.height()));
        match resize_handle(ui, "sm_left", v, true) {
            Resize::Drag(p) => sz.left = p.x - area.min.x - gap * 0.5,
            Resize::Reset => sz.left = 0.0,
            _ => {}
        }
        let v = Rect::from_center_size(Pos2::new(right.min.x - gap * 0.5, upper.center().y), Vec2::new(8.0, upper.height()));
        match resize_handle(ui, "sm_right", v, true) {
            Resize::Drag(p) => sz.right = area.max.x - p.x - gap * 0.5,
            Resize::Reset => sz.right = 0.0,
            _ => {}
        }
        let hbar = Rect::from_center_size(Pos2::new(area.center().x, bottom.min.y - gap * 0.5), Vec2::new(area.width(), 8.0));
        match resize_handle(ui, "sm_bottom", hbar, false) {
            Resize::Drag(p) => sz.bottom = area.max.y - p.y - gap * 0.5,
            Resize::Reset => sz.bottom = 0.0,
            _ => {}
        }
        self.sm_ui.dw = sz;
    }

    /// La X-Sheet de Stop Motion como editor suelto (versión compacta). Sin secuencia, dice qué falta.
    pub(super) fn sm_xsheet_editor(&mut self, ui: &mut egui::Ui) {
        if !self.sm_dragon_ready() {
            ui.add_space(8.0);
            super::shell::muted(ui, "Este editor necesita una capa Stop Motion con su secuencia. Elige una capa Stop Motion o crea la secuencia en la sala Stop Motion.");
            return;
        }
        let r = self.sm_room_layer().and_then(|li| self.project.layers[li].stopmotion.clone()).expect("comprobado arriba");
        let mut acts = Vec::new();
        self.sm_xsheet(ui, &r.id, &r, &mut acts, true);
        for a in acts {
            self.sm_act(&r.id, &r, a);
        }
    }}
