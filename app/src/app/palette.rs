//! Paneles de dibujos y color: Level Strip, Sustitución de dibujo y Paletas con colores con nombre.

use super::canvas::paint_thumb;
use super::shell::{muted, pill};
use super::*;
use crate::model::{NamedPalette, Swatch};
use std::sync::Arc;

impl LeonApp {
    pub(super) fn level_strip(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if pill(ui, "+ Dibujo", false).on_hover_text("Dibujo vacío en este fotograma o el siguiente (N)").clicked() {
                self.add_key(false);
            }
            if pill(ui, "Duplicar", false).on_hover_text("Copia el dibujo actual al siguiente fotograma (D)").clicked() {
                self.add_key(true);
            }
            if pill(ui, "Borrar", false).on_hover_text("Elimina el dibujo que empieza aquí (Supr)").clicked() {
                self.delete_key();
            }
        });
        let layer = self.layer();
        let cur = layer.key_at_or_before(self.frame).and_then(|k| layer.keys.get(&k).cloned());
        let drawings = layer.unique_drawings();
        ui.horizontal(|ui| {
            ui.label(RichText::new(&layer.name).strong());
            muted(ui, format!("{} dibujos", drawings.len()));
        });
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        let mut go = None;
        egui::ScrollArea::vertical().id_salt("lvstrip").max_height(230.0).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                for (i, (k, d)) in drawings.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(70.0, 50.0), Sense::click());
                    let thumb = Rect::from_min_size(rect.min, Vec2::new(70.0, 38.0));
                    paint_thumb(ui.painter(), thumb, d, w, h, Color32::WHITE);
                    let on = cur.as_ref().map_or(false, |c| Arc::ptr_eq(c, d));
                    ui.painter().rect_stroke(thumb, 2.0, if on { stk(2.0, ACCENT) } else { stk(1.0, Color32::BLACK) }, EStrokeKind::Outside);
                    ui.painter().text(
                        Pos2::new(rect.center().x, rect.max.y - 5.0),
                        Align2::CENTER_CENTER,
                        format!("{}  · f{}", i + 1, k + 1),
                        FontId::proportional(10.0),
                        if on { ACCENT2 } else { MU },
                    );
                    let resp = resp.on_hover_text(format!("Dibujo {} (aparece por primera vez en el fotograma {})\nClic: ir allí · Clic derecho: ponerlo en el fotograma actual", i + 1, k + 1));
                    if resp.clicked() {
                        go = Some(*k);
                    }
                    if resp.secondary_clicked() {
                        self.substitute(i);
                    }
                }
            });
        });
        if let Some(k) = go {
            self.playing = false;
            self.frame = k.min(self.project.length - 1);
        }
    }

    /// Pone el dibujo número `idx` de la capa en la celda del fotograma actual.
    pub(super) fn substitute(&mut self, idx: usize) {
        if self.layer().locked {
            self.say("La capa está bloqueada");
            return;
        }
        let drawings = self.layer().unique_drawings();
        let Some((_, d)) = drawings.get(idx) else { return };
        let f = self.frame;
        let cur = self.layer().key_at_or_before(f).and_then(|k| self.layer().keys.get(&k).cloned());
        if cur.as_ref().map_or(false, |c| Arc::ptr_eq(c, d)) {
            return;
        }
        self.checkpoint();
        let layer = &mut self.project.layers[self.cl];
        // Si el dibujo seguía en el fotograma siguiente, que siga allí.
        let next = f + 1;
        if next < self.project.length && !layer.keys.contains_key(&next) {
            if let Some(c) = cur {
                layer.keys.insert(next, c);
            }
        }
        layer.keys.insert(f, d.clone());
        self.say(format!("Fotograma {}: dibujo {}", f + 1, idx + 1));
    }

    pub(super) fn substitution_panel(&mut self, ui: &mut egui::Ui) {
        let layer = self.layer();
        let drawings = layer.unique_drawings();
        let cur_key = layer.key_at_or_before(self.frame);
        let cur = cur_key.and_then(|k| layer.drawing_number(k));
        let name = layer.name.clone();
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        ui.horizontal(|ui| {
            ui.label(RichText::new(&name).color(MU));
            let mut n = cur.map_or(0, |c| c + 1);
            let max = drawings.len().max(1);
            if ui.add_enabled(!drawings.is_empty(), egui::DragValue::new(&mut n).range(1..=max)).changed() {
                self.substitute(n - 1);
            }
            muted(ui, format!("de {}", drawings.len()));
        });
        let tw = (ui.available_width() * 0.62).min(150.0);
        let th = tw * h / w;
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(tw, th), Sense::hover());
            match cur.and_then(|c| drawings.get(c)) {
                Some((_, d)) => paint_thumb(ui.painter(), r, d, w, h, Color32::WHITE),
                None => {
                    ui.painter().rect_filled(r, 1.0, Color32::WHITE);
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, "celda vacía", FontId::proportional(11.0), MU);
                }
            }
            ui.painter().rect_stroke(r, 1.0, stk(1.0, Color32::BLACK), EStrokeKind::Outside);
        });
        if drawings.len() > 1 {
            let mut v = cur.unwrap_or(0) + 1;
            ui.spacing_mut().slider_width = ui.available_width() - 50.0;
            if ui.add(egui::Slider::new(&mut v, 1..=drawings.len()).show_value(true)).changed() {
                self.substitute(v - 1);
            }
        }
        muted(ui, "Cambia qué dibujo se ve en esta celda sin redibujar (útil para bocas, ojos, manos…).");
    }

    pub(super) fn palette_panel(&mut self, ui: &mut egui::Ui) {
        if self.project.palettes.is_empty() {
            self.project.palettes = crate::model::default_palettes();
        }
        self.pal_sel = self.pal_sel.min(self.project.palettes.len() - 1);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Trazo").color(MU));
            egui::color_picker::color_edit_button_srgba(ui, &mut self.color, egui::color_picker::Alpha::OnlyBlend);
            ui.label(RichText::new("Relleno").color(MU));
            egui::color_picker::color_edit_button_srgba(ui, &mut self.fill_color, egui::color_picker::Alpha::OnlyBlend);
            ui.label(RichText::new(hex(self.color)).monospace().size(11.0).color(MU));
        });

        // Lista de paletas
        ui.horizontal(|ui| {
            ui.label(RichText::new("Paletas de escena").strong().size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if ui.small_button("−").on_hover_text("Quitar esta paleta").clicked() && self.project.palettes.len() > 1 {
                    self.project.palettes.remove(self.pal_sel);
                    self.pal_sel = self.pal_sel.saturating_sub(1);
                }
                if ui.small_button("+").on_hover_text("Nueva paleta").clicked() {
                    let n = self.project.palettes.len() + 1;
                    self.project.palettes.push(NamedPalette { name: format!("Paleta {n}"), colors: vec![] });
                    self.pal_sel = n - 1;
                }
            });
        });
        let mut pick_pal = None;
        ui.spacing_mut().item_spacing.y = 2.0;
        for (i, p) in self.project.palettes.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::click());
            let on = i == self.pal_sel;
            ui.painter().rect_filled(r, 2.0, if on { ACCENT } else if resp.hovered() { BG4 } else { Color32::from_rgb(0x1a, 0x1a, 0x1a) });
            ui.painter().text(r.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, if on { "v" } else { " " }, FontId::proportional(11.0), Color32::WHITE);
            ui.painter().text(r.left_center() + Vec2::new(22.0, 0.0), Align2::LEFT_CENTER, &p.name, FontId::proportional(12.0), if on { Color32::WHITE } else { TX });
            ui.painter().text(r.right_center() - Vec2::new(6.0, 0.0), Align2::RIGHT_CENTER, format!("{}", p.colors.len()), FontId::proportional(10.5), if on { Color32::WHITE } else { MU });
            if resp.clicked() {
                pick_pal = Some(i);
            }
        }
        if let Some(i) = pick_pal {
            self.pal_sel = i;
            self.swatch_sel = None;
        }
        let pi = self.pal_sel;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Nombre").color(MU));
            ui.add(egui::TextEdit::singleline(&mut self.project.palettes[pi].name).desired_width(140.0));
        });

        // Colores de la paleta elegida
        let mut set = None;
        let mut set_fill = None;
        let mut start_rename = None;
        let colors = self.project.palettes[pi].colors.clone();
        egui::ScrollArea::vertical().id_salt("swatches").max_height(190.0).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            for (i, s) in colors.iter().enumerate() {
                let renaming = matches!(&self.swatch_rename, Some((j, _)) if *j == i);
                let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::click());
                let on = self.swatch_sel == Some(i) || (self.swatch_sel.is_none() && c32(s.color) == self.color);
                ui.painter().rect_filled(r, 2.0, if on { SEL_BG } else if resp.hovered() { BG4 } else { Color32::TRANSPARENT });
                let chip = Rect::from_min_size(r.min + Vec2::new(4.0, 3.0), Vec2::new(40.0, 14.0));
                checker(ui.painter(), chip);
                ui.painter().rect_filled(chip, 1.0, c32(s.color));
                ui.painter().rect_stroke(chip, 1.0, stk(1.0, Color32::from_gray(150)), EStrokeKind::Outside);
                if renaming {
                    if let Some((_, buf)) = &mut self.swatch_rename {
                        let er = Rect::from_min_max(Pos2::new(chip.max.x + 8.0, r.min.y + 1.0), r.max - Vec2::new(2.0, 1.0));
                        let e = ui.put(er, egui::TextEdit::singleline(buf).font(FontId::proportional(12.0)));
                        if !e.has_focus() && !e.lost_focus() {
                            e.request_focus();
                        }
                        if e.lost_focus() {
                            let name = buf.trim().to_string();
                            if !name.is_empty() {
                                self.project.palettes[pi].colors[i].name = name;
                            }
                            self.swatch_rename = None;
                        }
                    }
                } else {
                    ui.painter().text(Pos2::new(chip.max.x + 10.0, r.center().y), Align2::LEFT_CENTER, &s.name, FontId::proportional(12.0), TX);
                }
                let resp = resp.on_hover_text(format!("{} {}\nClic: trazo · Clic derecho: relleno · Doble clic: renombrar", s.name, hex(c32(s.color))));
                if resp.clicked() {
                    set = Some(i);
                }
                if resp.secondary_clicked() {
                    set_fill = Some(i);
                }
                if resp.double_clicked() {
                    start_rename = Some(i);
                }
            }
        });
        if let Some(i) = set {
            self.color = c32(colors[i].color);
            self.swatch_sel = Some(i);
        }
        if let Some(i) = set_fill {
            self.fill_color = c32(colors[i].color);
        }
        if let Some(i) = start_rename {
            self.swatch_rename = Some((i, colors[i].name.clone()));
        }
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if pill(ui, "+ Color", false).on_hover_text("Guarda el color de trazo actual en esta paleta").clicked() {
                let n = self.project.palettes[pi].colors.len() + 1;
                self.project.palettes[pi].colors.push(Swatch { name: format!("Nuevo {n}"), color: rgba(self.color) });
                self.swatch_sel = Some(n - 1);
            }
            if pill(ui, "Quitar", false).on_hover_text("Quita el color elegido de la paleta").clicked() {
                if let Some(i) = self.swatch_sel.filter(|i| *i < self.project.palettes[pi].colors.len()) {
                    self.project.palettes[pi].colors.remove(i);
                    self.swatch_sel = None;
                }
            }
            if pill(ui, "Cambiar en la escena", false)
                .on_hover_text("El color elegido de la paleta pasa a ser el color de trazo actual, y cambia en todos los dibujos que lo usan")
                .clicked()
            {
                self.recolor_swatch(pi);
            }
        });
    }

    /// Paleta por grupos (prototipo v0.3): cada grupo con sus muestras; abajo, el color elegido.
    pub(super) fn palette_groups(&mut self, ui: &mut egui::Ui) {
        if self.project.palettes.is_empty() {
            self.project.palettes = crate::model::default_palettes();
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if ui.small_button("+ Grupo").clicked() {
                let n = self.project.palettes.len() + 1;
                self.project.palettes.push(NamedPalette { name: format!("Grupo {n}"), colors: vec![] });
            }
            if ui.small_button("+ Color").on_hover_text("Guarda el color actual en el último grupo elegido").clicked() {
                let pi = self.pal_sel.min(self.project.palettes.len() - 1);
                let n = self.project.palettes[pi].colors.len() + 1;
                self.project.palettes[pi].colors.push(Swatch { name: format!("Nuevo {n}"), color: rgba(self.color) });
            }
        });
        let cur = rgba(self.color);
        let mut set = None;
        let mut set_fill = None;
        for (pi, pal) in self.project.palettes.iter().enumerate() {
            egui::Frame::default().fill(BG3).corner_radius(6.0).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&pal.name).size(12.0).color(TX));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(pal.colors.len().to_string()).size(10.5).color(INK3));
                    });
                });
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    for s in &pal.colors {
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(32.0, 26.0), Sense::click());
                        ui.painter().rect_filled(r, 4.0, c32(s.color));
                        let on = s.color == cur;
                        ui.painter().rect_stroke(r, 4.0, if on { stk(2.0, ACCENT) } else { stk(1.0, LINE_STRONG) }, EStrokeKind::Outside);
                        let resp = resp.on_hover_text(format!("{}  {}\nClic: trazo · Clic derecho: relleno", s.name, hex(c32(s.color))));
                        if resp.clicked() {
                            set = Some((pi, s.color));
                        }
                        if resp.secondary_clicked() {
                            set_fill = Some(s.color);
                        }
                    }
                });
            });
        }
        if let Some((pi, c)) = set {
            self.color = c32(c);
            self.pal_sel = pi;
            self.swatch_sel = None;
        }
        if let Some(c) = set_fill {
            self.fill_color = c32(c);
        }
        egui::Frame::default().fill(BG3).corner_radius(6.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                egui::color_picker::color_edit_button_srgba(ui, &mut self.color, egui::color_picker::Alpha::OnlyBlend);
                ui.vertical(|ui| {
                    ui.label(RichText::new(self.color_name_pub()).strong().color(TX));
                    ui.label(RichText::new(hex(self.color)).size(11.0).color(MU));
                });
            });
            muted(ui, "Abre la sala Color para editar las paletas. Con «Cambiar en la escena», todos los trazos que usan un color se actualizan.");
        });
    }

    fn color_name_pub(&self) -> String {
        let c = rgba(self.color);
        self.project.palettes.iter().flat_map(|p| p.colors.iter()).find(|s| s.color == c).map(|s| s.name.clone()).unwrap_or_else(|| "Color libre".into())
    }

    /// Cambia un color de la paleta y todos los trazos y rellenos de la escena que lo usaban.
    fn recolor_swatch(&mut self, pi: usize) {
        let Some(i) = self.swatch_sel.filter(|i| *i < self.project.palettes[pi].colors.len()) else {
            self.say("Elige primero un color de la paleta");
            return;
        };
        let old = self.project.palettes[pi].colors[i].color;
        let new = rgba(self.color);
        if old == new {
            self.say("Elige un color de trazo distinto con el selector de arriba");
            return;
        }
        self.checkpoint();
        self.project.palettes[pi].colors[i].color = new;
        let mut n = 0;
        for l in &mut self.project.layers {
            for d in l.keys.values_mut() {
                if d.strokes.iter().any(|s| s.color == old || s.fill == Some(old)) {
                    for s in &mut Arc::make_mut(d).strokes {
                        if s.color == old {
                            s.color = new;
                            n += 1;
                        }
                        if s.fill == Some(old) {
                            s.fill = Some(new);
                            n += 1;
                        }
                    }
                }
            }
        }
        self.say(format!("Color cambiado en {n} trazos y rellenos de la escena"));
    }
}
