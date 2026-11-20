//! Panel contextual de edición de trazos vectoriales (Issue #31, Fase 3): con la Selección, el panel N muestra las
//! propiedades de los trazos elegidos y permite cambiarlas, asignar otro pincel de la biblioteca, copiar y pegar
//! propiedades y restaurar el aspecto original. Solo en capas vectoriales; no toca la geometría ni rasteriza.

use super::panels::{param_row, pct_slider};
use super::shell::muted;
use super::*;
use crate::vector::edit::{self, Prop};
use crate::vector::library::VPreset;
use crate::vector::{Cap, Join, Look, Profile};

/// Estado del panel de edición de trazos.
pub(super) struct StrokeUi {
    /// Propiedades copiadas de un trazo.
    pub clip: Option<Look>,
    pub paste_width: bool,
    pub paste_style: bool,
    pub paste_color: bool,
    /// Al asignar un pincel, usar también su grosor.
    pub brush_width: bool,
    /// Último aviso al asignar un pincel.
    pub notes: Vec<String>,
    /// El puntero está pulsado y ya se guardó el paso de deshacer de este arrastre.
    pub down: bool,
    pub open: bool,
}

impl Default for StrokeUi {
    fn default() -> Self {
        StrokeUi { clip: None, paste_width: true, paste_style: true, paste_color: false, brush_width: false, notes: Vec::new(), down: false, open: false }
    }
}

const MIXED: &str = "Mixto";

impl LeonApp {
    /// Aplica `f` a los trazos elegidos. Devuelve cuántos cambió. `undo`: guardar antes un paso de deshacer.
    pub(super) fn edit_picked(&mut self, undo: bool, f: impl Fn(&mut Stroke)) -> usize {
        self.validate_pick();
        if self.picked.is_empty() || self.layer().locked {
            return 0;
        }
        if undo {
            self.checkpoint();
        }
        let (fr, picked) = (self.frame, self.picked.clone());
        let mut n = 0;
        if let Some(d) = self.project.layers[self.cl].drawing_at_mut(fr) {
            for i in picked {
                if let Some(s) = d.strokes.get_mut(i) {
                    f(s);
                    n += 1;
                }
            }
        }
        self.dirty = true;
        n
    }

    /// Cambia una propiedad de los trazos elegidos. `undo` es true en el primer cambio de un arrastre.
    fn set_picked_prop(&mut self, _undo: bool, p: Prop) {
        // Un paso de deshacer por cada arrastre (no uno por fotograma) y uno por cada cambio suelto.
        let undo = if self.sui.down { !self.sui.open } else { true };
        if self.sui.down {
            self.sui.open = true;
        }
        self.edit_picked(undo, |s| edit::set_prop(s, &p));
    }

    /// Los trazos elegidos del dibujo actual.
    pub(super) fn picked_strokes(&self) -> Vec<Stroke> {
        let Some(d) = self.layer().drawing_at(self.frame) else { return Vec::new() };
        self.picked.iter().filter_map(|i| d.strokes.get(*i).cloned()).collect()
    }

    /// Panel contextual: propiedades de los trazos elegidos con la Selección.
    pub(super) fn stroke_props_ui(&mut self, ui: &mut egui::Ui) {
        self.validate_pick();
        self.sui.down = ui.input(|i| i.pointer.primary_down());
        if !self.sui.down {
            self.sui.open = false;
        }
        panel_header_local(ui, "Trazos elegidos");
        let strokes = self.picked_strokes();
        if strokes.is_empty() {
            muted(ui, "Elige uno o varios trazos con la Selección (clic, Mayús+clic o recuadro) para cambiar su grosor, color, opacidad, perfil, extremos y uniones, o asignarles otro pincel, sin volver a dibujarlos.");
            return;
        }
        if self.layer().locked {
            muted(ui, "La capa está bloqueada: desbloquéala para editar sus trazos.");
        }
        let refs: Vec<&Stroke> = strokes.iter().collect();
        let c = edit::common(&refs);
        let mixed_note = |m: bool| if m { " (mixto)" } else { "" };
        ui.label(RichText::new(format!("{} {}{}", c.n, if c.n == 1 { "trazo" } else { "trazos" }, if c.edited > 0 { format!(" · {} editado{}", c.edited, if c.edited == 1 { "" } else { "s" }) } else { String::new() })).size(11.5).color(MU));
        match &c.brush {
            Some(Some(id)) => muted(ui, &format!("Pincel: {}", id.replace('/', " · "))),
            Some(None) => muted(ui, "Pincel: ninguno (trazo propio)"),
            None => muted(ui, "Pincel: varios"),
        }

        // Grosor
        let mut w = c.width.or(strokes.first().map(|s| s.width)).unwrap_or(4.0);
        param_row(ui, &format!("Grosor{}", mixed_note(c.width.is_none())), |ui| {
            let r = ui.add(egui::Slider::new(&mut w, 0.5..=300.0).logarithmic(true).max_decimals(1).suffix(" px"));
            if r.changed() {
                let undo = r.drag_started() || !r.dragged();
                self.set_picked_prop(undo, Prop::Width(w));
            }
        });
        // Opacidad
        let mut a = c.alpha.or(strokes.first().map(|s| s.color[3])).unwrap_or(255) as f32 / 255.0;
        param_row(ui, &format!("Opacidad{}", mixed_note(c.alpha.is_none())), |ui| {
            let before = a;
            pct_slider(ui, &mut a, 5.0, 100.0);
            if (a - before).abs() > 1e-4 {
                let dragging = ui.input(|i| i.pointer.primary_down());
                self.set_picked_prop(!dragging, Prop::Alpha((a * 255.0).round().clamp(0.0, 255.0) as u8));
            }
        });
        // Color
        let mut rgb = c.rgb.or(strokes.first().map(|s| [s.color[0], s.color[1], s.color[2]])).unwrap_or([0, 0, 0]);
        param_row(ui, &format!("Color{}", mixed_note(c.rgb.is_none())), |ui| {
            if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).changed() {
                let dragging = ui.input(|i| i.pointer.primary_down());
                self.set_picked_prop(!dragging, Prop::Rgb(rgb));
            }
        });

        // Estilo
        param_row(ui, "Con presión", |ui| {
            let mut on = c.pressure.unwrap_or(false);
            let r = ui.add(egui::Checkbox::new(&mut on, "Grosor variable").indeterminate(c.pressure.is_none()));
            if r.changed() {
                self.set_picked_prop(true, Prop::Pressure(on));
            }
        });
        if c.pressure != Some(false) {
            let mut m = c.min_width.unwrap_or(0.2);
            param_row(ui, &format!("Mínimo{}", mixed_note(c.min_width.is_none())), |ui| {
                let before = m;
                pct_slider(ui, &mut m, 0.0, 100.0);
                if (m - before).abs() > 1e-4 {
                    let dragging = ui.input(|i| i.pointer.primary_down());
                    self.set_picked_prop(!dragging, Prop::MinWidth(m));
                }
            });
            let mut k = c.curve.unwrap_or(1.0);
            param_row(ui, &format!("Curva{}", mixed_note(c.curve.is_none())), |ui| {
                let r = ui.add(egui::Slider::new(&mut k, 0.2..=4.0).logarithmic(true).max_decimals(2));
                if r.changed() {
                    self.set_picked_prop(r.drag_started() || !r.dragged(), Prop::Curve(k));
                }
            });
        }
        param_row(ui, "Perfil", |ui| {
            let mut chosen: Option<Profile> = None;
            egui::ComboBox::from_id_salt("sp_profile").selected_text(c.profile.map_or(MIXED, |p| p.label())).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                for k in Profile::ALL {
                    if ui.selectable_label(c.profile == Some(k), k.label()).clicked() {
                        chosen = Some(k);
                    }
                }
            });
            if let Some(k) = chosen {
                self.set_picked_prop(true, Prop::Profile(k));
            }
        });
        if c.profile == Some(Profile::Custom) {
            let mut shape = c.shape.clone().or_else(|| strokes.first().and_then(|s| s.style.as_ref().map(|st| st.shape.clone()))).unwrap_or_default();
            if super::vectorui::shape_editor(ui, &mut shape) {
                self.set_picked_prop(true, Prop::Shape(shape));
            }
        }
        if c.profile.map_or(true, |p| p.is_taper()) {
            let mut t = c.taper.unwrap_or(0.25);
            param_row(ui, &format!("Afilado{}", mixed_note(c.taper.is_none())), |ui| {
                let before = t;
                pct_slider(ui, &mut t, 2.0, 50.0);
                if (t - before).abs() > 1e-4 {
                    let dragging = ui.input(|i| i.pointer.primary_down());
                    self.set_picked_prop(!dragging, Prop::Taper(t));
                }
            });
        }
        param_row(ui, "Extremo", |ui| {
            let mut chosen: Option<Cap> = None;
            egui::ComboBox::from_id_salt("sp_cap").selected_text(c.cap.map_or(MIXED, |p| p.label())).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                for k in Cap::ALL {
                    if ui.selectable_label(c.cap == Some(k), k.label()).clicked() {
                        chosen = Some(k);
                    }
                }
            });
            if let Some(k) = chosen {
                self.set_picked_prop(true, Prop::Cap(k));
            }
        });
        param_row(ui, "Unión", |ui| {
            let mut chosen: Option<Join> = None;
            egui::ComboBox::from_id_salt("sp_join").selected_text(c.join.map_or(MIXED, |p| p.label())).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                for k in Join::ALL {
                    if ui.selectable_label(c.join == Some(k), k.label()).clicked() {
                        chosen = Some(k);
                    }
                }
            });
            if let Some(k) = chosen {
                self.set_picked_prop(true, Prop::Join(k));
            }
        });

        // Asignar otro pincel de la biblioteca (conserva trayectoria, presión, color y opacidad).
        param_row(ui, "Asignar pincel", |ui| {
            let mut chosen: Option<VPreset> = None;
            egui::ComboBox::from_id_salt("sp_brush").selected_text("Elegir…").width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                let all = self.vlib.all();
                for cat in self.vlib.categories() {
                    let list: Vec<&VPreset> = all.iter().filter(|p| p.category.eq_ignore_ascii_case(&cat)).collect();
                    if list.is_empty() {
                        continue;
                    }
                    ui.label(RichText::new(&cat).size(11.0).color(INK3));
                    for p in list {
                        if ui.selectable_label(c.brush == Some(Some(p.id())), &p.name).clicked() {
                            chosen = Some(p.clone());
                        }
                    }
                }
            });
            if let Some(p) = chosen {
                self.assign_brush_to_picked(&p);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.sui.brush_width, "Usar también su grosor").on_hover_text("Si no, cada trazo conserva su grosor");
            if ui.small_button("Con el pincel del lápiz").on_hover_text("Asigna el pincel que tienes configurado en el Lápiz vectorial").clicked() {
                let p = VPreset { size: self.pencil_size, opacity: self.stroke_opacity, smoothing: self.smoothing, style: self.vstyle.clone(), name: "Lápiz".into(), ..Default::default() };
                self.assign_brush_to_picked(&p);
            }
        });
        for n in self.sui.notes.clone() {
            ui.label(RichText::new(format!("⚠ {n}")).size(11.0).color(ACCENT2));
        }

        // Copiar, pegar y restaurar
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(c.n == 1 || !strokes.is_empty(), egui::Button::new("Copiar propiedades").small()).on_hover_text("Copia grosor, color y estilo del primer trazo elegido").clicked() {
                self.sui.clip = strokes.first().map(|s| s.look());
                self.say("Propiedades copiadas");
            }
            if ui.add_enabled(self.sui.clip.is_some(), egui::Button::new("Pegar propiedades").small()).clicked() {
                if let Some(look) = self.sui.clip.clone() {
                    let (w, s, col) = (self.sui.paste_width, self.sui.paste_style, self.sui.paste_color);
                    if w || s || col {
                        let n = self.edit_picked(true, |t| edit::paste_look(t, &look, w, s, col));
                        self.say(format!("Propiedades pegadas en {n} trazos"));
                    }
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Pegar:").size(11.0).color(MU));
            ui.checkbox(&mut self.sui.paste_width, "Grosor");
            ui.checkbox(&mut self.sui.paste_style, "Estilo");
            ui.checkbox(&mut self.sui.paste_color, "Color y opacidad");
        });
        if ui.add_enabled(c.edited > 0, egui::Button::new("Restaurar originales").small()).on_hover_text("Devuelve los trazos elegidos al aspecto con el que se dibujaron").clicked() {
            let n = self.edit_picked(true, |s| {
                edit::restore(s);
            });
            self.sui.notes.clear();
            self.say(format!("{n} trazos restaurados a su aspecto original"));
        }
        muted(ui, "Los cambios afectan solo a la propiedad que tocas y se ven al instante. La trayectoria y la presión capturada no se modifican, y puedes deshacer con Ctrl+Z.");
    }

    fn assign_brush_to_picked(&mut self, p: &VPreset) {
        let strokes = self.picked_strokes();
        let refs: Vec<&Stroke> = strokes.iter().collect();
        self.sui.notes = edit::brush_notes(&refs, p);
        let with_width = self.sui.brush_width;
        let n = self.edit_picked(true, |s| edit::apply_brush(s, p, with_width));
        self.say(format!("Pincel «{}» asignado a {n} trazos", p.name));
    }
}

fn panel_header_local(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).font(semi(12.0)).color(TX));
}
