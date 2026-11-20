//! Panel de pinceles vectoriales del panel N (Issue #31, Fase 2): lista por categorías con muestras, favoritos,
//! buscador, guardar, actualizar, duplicar, renombrar, eliminar, restaurar, importar y exportar.
//! Solo aparece con el Lápiz vectorial; no comparte lista, archivo ni formato con los pinceles de píxeles.

use super::shell::muted;
use super::*;
use crate::vector::library::{self as vlib, VPreset, USER_CATEGORY};
use crate::vector::VStyle;

#[derive(Default, PartialEq, Clone, Copy)]
pub(super) enum VEdit {
    #[default]
    None,
    SaveAs,
    Rename,
    Category,
}

#[derive(Default, PartialEq, Clone, Copy)]
pub(super) enum VConfirm {
    #[default]
    None,
    Delete,
    RestoreAll,
}

/// Estado de la interfaz de la biblioteca vectorial.
#[derive(Default)]
pub(super) struct VUi {
    pub edit: VEdit,
    pub confirm: VConfirm,
    pub name: String,
    pub category: String,
    pub search: String,
    pub only_fav: bool,
}

/// Trazo de muestra con el motor real: una onda con una presión que sube y baja.
pub(super) fn sample_stroke(r: Rect, size: f32, style: &VStyle) -> Stroke {
    let n = 40;
    let points: Vec<[f32; 2]> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            [8.0 + t * (r.width() - 16.0), r.height() * 0.5 + (t * std::f32::consts::TAU).sin() * r.height() * 0.2]
        })
        .collect();
    let pressure: Vec<f32> = (0..=n).map(|i| ((i as f32 / n as f32) * std::f32::consts::PI).sin().clamp(0.05, 1.0)).collect();
    Stroke { kind: ShapeKind::Free, points, color: [232, 232, 232, 255], width: size.clamp(1.0, r.height() * 0.45), closed: false, fill: None, pressure, style: Some(style.clone().clamped()), original: None, nodes: None }
}

fn paint_sample(painter: &egui::Painter, r: Rect, size: f32, opacity: f32, style: &VStyle) {
    let s = sample_stroke(r, size, style);
    super::canvas::paint_stroke(&painter.with_clip_rect(r), &s, &|p| Pos2::new(r.min.x + p[0], r.min.y + p[1]), 1.0, None, opacity);
}

impl LeonApp {
    /// Receta del lápiz vectorial tal como está en pantalla.
    fn vrecipe_now(&self) -> VPreset {
        VPreset { size: self.pencil_size, opacity: self.stroke_opacity, smoothing: self.smoothing, style: self.vstyle.clone(), ..Default::default() }
    }

    /// Pone el pincel en pantalla. Cada trazo nuevo copiará este estilo y recordará el identificador del pincel.
    pub(super) fn apply_vpreset(&mut self, p: &VPreset) {
        let p = p.clone().clamped();
        self.pencil_size = p.size;
        self.stroke_opacity = p.opacity;
        self.smoothing = p.smoothing;
        self.vstyle = VStyle { brush: Some(p.id()), ..p.style };
        self.vpreset = Some(p.id());
    }

    /// Guarda la biblioteca en el equipo (las capturas de prueba no la tocan).
    fn save_vlib(&mut self) {
        if self.shot.is_some() {
            return;
        }
        if let Some(dir) = safety::data_dir() {
            if let Err(e) = vlib::save(&dir, &self.vlib) {
                safety::log_line(&format!("No se pudo guardar la biblioteca de pinceles vectoriales: {e}"));
                self.say(format!("No se pudo guardar la biblioteca de pinceles vectoriales: {e}"));
            }
        }
    }

    fn commit_vedit(&mut self) {
        let cur = self.vpreset.clone();
        let (name, cat) = (self.vui.name.clone(), self.vui.category.clone());
        let need = || "Elige primero un pincel de la lista".to_string();
        let res: Result<(String, &str), String> = match self.vui.edit {
            VEdit::SaveAs => {
                let mut r = self.vrecipe_now();
                r.name = name;
                r.category = if cat.trim().is_empty() { USER_CATEGORY.into() } else { cat };
                Ok((self.vlib.save_new(r), "guardado"))
            }
            VEdit::Rename => cur.ok_or_else(need).and_then(|k| self.vlib.rename(&k, &name)).map(|k| (k, "renombrado")),
            VEdit::Category => cur.ok_or_else(need).and_then(|k| self.vlib.set_category(&k, &cat)).map(|k| (k, "movido de categoría")),
            VEdit::None => return,
        };
        match res {
            Ok((id, what)) => {
                self.say(format!("Pincel vectorial «{}» {what}", id.rsplit('/').next().unwrap_or(&id)));
                // El pincel en pantalla pasa a ser el guardado (así el estilo ya lleva su identificador).
                if let Some(p) = self.vlib.find(&id) {
                    self.vstyle.brush = Some(p.id());
                }
                self.vpreset = Some(id);
                self.vui.edit = VEdit::None;
                self.save_vlib();
            }
            Err(e) => self.say(e),
        }
    }

    /// Una fila de la lista: muestra del trazo, nombre y estrella de favorito. Devuelve (aplicar, alternar favorito).
    fn vpreset_row(&self, ui: &mut egui::Ui, p: &VPreset, on: bool, edited: bool) -> (bool, bool) {
        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
        if on {
            ui.painter().rect_filled(r, 4.0, Color32::from_rgba_unmultiplied(255, 122, 26, 36));
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 4.0, Color32::from_white_alpha(10));
        }
        let sample = Rect::from_min_size(r.min + Vec2::new(4.0, 2.0), Vec2::new(70.0, 22.0));
        paint_sample(ui.painter(), sample, p.size * 0.6, p.opacity, &p.style);
        let name = if edited { format!("{} (editado)", p.name) } else { p.name.clone() };
        ui.painter().text(Pos2::new(sample.max.x + 8.0, r.center().y), Align2::LEFT_CENTER, name, FontId::proportional(12.0), if on { TX } else { MU });
        let star = Rect::from_center_size(Pos2::new(r.max.x - 14.0, r.center().y), Vec2::splat(20.0));
        let fav = self.vlib.is_favorite(&p.id());
        ui.painter().text(star.center(), Align2::CENTER_CENTER, if fav { "★" } else { "☆" }, FontId::proportional(14.0), if fav { ACCENT2 } else { INK3 });
        let resp = resp.on_hover_text(p.summary().iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("\n"));
        let clicked = resp.clicked();
        let click_pos = ui.input(|i| i.pointer.interact_pos());
        let star_hit = clicked && click_pos.map_or(false, |c| star.contains(c));
        (clicked && !star_hit, star_hit)
    }

    /// Lista, buscador y acciones de la biblioteca de pinceles vectoriales.
    pub(super) fn vector_presets_ui(&mut self, ui: &mut egui::Ui) {
        let cur = self.vpreset.clone();
        let selected = cur.as_ref().and_then(|k| self.vlib.find(k));
        let modified = selected.as_ref().map_or(false, |p| !p.same_recipe(self.pencil_size, self.stroke_opacity, self.smoothing, &self.vstyle));
        let user_sel = selected.as_ref().map_or(false, |p| !p.builtin);
        let edited_sel = cur.as_ref().map_or(false, |k| self.vlib.is_edited(k));

        egui::CollapsingHeader::new(RichText::new("Pinceles vectoriales").font(semi(12.0)).color(TX)).id_salt("fold_vpresets").default_open(true).show(ui, |ui| {
            let label = match &selected {
                Some(p) => format!("{} · {}{}", p.category, p.name, if modified { "   (modificado)" } else { "" }),
                None => "Sin preset: configuración propia".to_string(),
            };
            ui.label(RichText::new(label).size(11.5).color(if modified { ACCENT2 } else { MU }));

            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.vui.search).hint_text("Buscar pincel").desired_width((ui.available_width() - 34.0).max(40.0)));
                if ui.selectable_label(self.vui.only_fav, "★").on_hover_text("Mostrar solo los favoritos").clicked() {
                    self.vui.only_fav = !self.vui.only_fav;
                }
            });

            let all = self.vlib.search(&self.vui.search);
            let searching = !self.vui.search.trim().is_empty() || self.vui.only_fav;
            let mut apply: Option<VPreset> = None;
            let mut fav: Option<String> = None;
            egui::ScrollArea::vertical().id_salt("vpreset_list").max_height(230.0).auto_shrink([false, true]).show(ui, |ui| {
                let mut shown = 0;
                let mut section = |this: &Self, ui: &mut egui::Ui, title: &str, list: Vec<&VPreset>| {
                    if list.is_empty() {
                        return;
                    }
                    ui.label(RichText::new(title).size(11.0).color(INK3));
                    for p in list {
                        let id = p.id();
                        let (a, f) = this.vpreset_row(ui, p, cur.as_ref() == Some(&id), p.builtin && this.vlib.is_edited(&id));
                        if a {
                            apply = Some(p.clone());
                        }
                        if f {
                            fav = Some(id);
                        }
                        shown += 1;
                    }
                };
                let visible: Vec<&VPreset> = all.iter().filter(|p| !self.vui.only_fav || self.vlib.is_favorite(&p.id())).collect();
                if !searching {
                    let favs: Vec<&VPreset> = visible.iter().copied().filter(|p| self.vlib.is_favorite(&p.id())).collect();
                    section(self, ui, "★ Favoritos", favs);
                }
                for cat in self.vlib.categories() {
                    let list: Vec<&VPreset> = visible.iter().copied().filter(|p| p.category.eq_ignore_ascii_case(&cat)).collect();
                    section(self, ui, &cat, list);
                }
                if shown == 0 {
                    muted(ui, if self.vui.only_fav && self.vui.search.trim().is_empty() { "Aún no tienes favoritos: marca la estrella de un pincel." } else { "Ningún pincel coincide con la búsqueda." });
                }
            });
            if let Some(id) = fav {
                self.vlib.toggle_favorite(&id);
                self.save_vlib();
            }
            if let Some(p) = apply {
                self.apply_vpreset(&p);
                self.vui.edit = VEdit::None;
                self.vui.confirm = VConfirm::None;
                self.say(format!("Pincel vectorial «{}» aplicado", p.name));
            }

            // Acciones
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Guardar como…").on_hover_text("Guarda el lápiz actual como un pincel nuevo").clicked() {
                    self.vui.edit = VEdit::SaveAs;
                    self.vui.name = selected.as_ref().map_or("Pincel nuevo".to_string(), |p| format!("{} (nuevo)", p.name));
                    self.vui.category = selected.as_ref().filter(|p| !p.builtin).map_or(USER_CATEGORY.to_string(), |p| p.category.clone());
                }
                if ui.add_enabled(selected.is_some() && modified, egui::Button::new("Actualizar").small()).on_hover_text("Guarda los cambios en el pincel elegido. En uno incluido, se guarda como edición y se puede restaurar").clicked() {
                    if let Some(k) = cur.clone() {
                        match self.vlib.update(&k, self.pencil_size, self.stroke_opacity, self.smoothing, &self.vstyle) {
                            Ok(()) => {
                                self.vstyle.brush = Some(k.clone());
                                self.say("Pincel vectorial actualizado. Los trazos ya dibujados no cambian");
                                self.save_vlib();
                            }
                            Err(e) => self.say(e),
                        }
                    }
                }
                if ui.add_enabled(selected.is_some(), egui::Button::new("Duplicar").small()).clicked() {
                    if let Some(k) = cur.clone() {
                        match self.vlib.duplicate(&k) {
                            Ok(id) => {
                                self.say(format!("Pincel duplicado como «{}»", id.rsplit('/').next().unwrap_or(&id)));
                                if let Some(p) = self.vlib.find(&id) {
                                    self.apply_vpreset(&p);
                                }
                                self.save_vlib();
                            }
                            Err(e) => self.say(e),
                        }
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(user_sel, egui::Button::new("Renombrar").small()).clicked() {
                    self.vui.edit = VEdit::Rename;
                    self.vui.name = selected.as_ref().map_or(String::new(), |p| p.name.clone());
                }
                if ui.add_enabled(user_sel, egui::Button::new("Categoría").small()).on_hover_text("Mueve el pincel a otra categoría").clicked() {
                    self.vui.edit = VEdit::Category;
                    self.vui.category = selected.as_ref().map_or(String::new(), |p| p.category.clone());
                }
                if ui.add_enabled(user_sel, egui::Button::new("Eliminar").small()).clicked() {
                    self.vui.confirm = VConfirm::Delete;
                }
                if ui.add_enabled(edited_sel, egui::Button::new("Restaurar").small()).on_hover_text("Devuelve este pincel incluido a su configuración original").clicked() {
                    if let Some(k) = cur.clone() {
                        match self.vlib.restore(&k) {
                            Ok(()) => {
                                if let Some(p) = self.vlib.find(&k) {
                                    self.apply_vpreset(&p);
                                }
                                self.say("Pincel restaurado a su configuración original");
                                self.save_vlib();
                            }
                            Err(e) => self.say(e),
                        }
                    }
                }
                if ui.add_enabled(!self.vlib.edited.is_empty(), egui::Button::new("Restaurar todos").small()).on_hover_text("Devuelve todos los pinceles incluidos a su configuración original").clicked() {
                    self.vui.confirm = VConfirm::RestoreAll;
                }
            });
            if selected.as_ref().map_or(false, |p| p.builtin) {
                muted(ui, "Pincel incluido: «Actualizar» guarda tu edición aparte y «Restaurar» vuelve al original. «Duplicar» crea uno propio.");
            }
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Importar…").on_hover_text("Lee un paquete de pinceles vectoriales (.ltvb)").clicked() {
                    self.vector_import();
                }
                if ui.add_enabled(selected.is_some(), egui::Button::new("Exportar este…").small()).on_hover_text("Guarda el pincel elegido en un paquete .ltvb").clicked() {
                    if let Some(p) = selected.clone() {
                        self.vector_export(vec![p], "pincel");
                    }
                }
                if ui.small_button("Exportar colección…").on_hover_text("Guarda en un paquete .ltvb todos tus pinceles y los incluidos que editaste").clicked() {
                    let mut list: Vec<VPreset> = self.vlib.user.clone();
                    list.extend(self.vlib.edited.clone());
                    self.vector_export(list, "pinceles_vectoriales");
                }
            });

            // Confirmaciones
            match self.vui.confirm {
                VConfirm::Delete => {
                    if let (Some(k), true) = (cur.clone(), user_sel) {
                        ui.label(RichText::new(format!("¿Eliminar «{}» de tu biblioteca? Los trazos ya dibujados con él no cambian. No se puede deshacer.", selected.as_ref().map_or("", |p| p.name.as_str()))).size(11.5).color(ACCENT2));
                        ui.horizontal(|ui| {
                            if ui.small_button("Sí, eliminar").clicked() {
                                match self.vlib.remove(&k) {
                                    Ok(()) => {
                                        self.say("Pincel vectorial eliminado de la biblioteca");
                                        self.vpreset = None;
                                        self.vstyle.brush = None;
                                        self.save_vlib();
                                    }
                                    Err(e) => self.say(e),
                                }
                                self.vui.confirm = VConfirm::None;
                            }
                            if ui.small_button("No").clicked() {
                                self.vui.confirm = VConfirm::None;
                            }
                        });
                    } else {
                        self.vui.confirm = VConfirm::None;
                    }
                }
                VConfirm::RestoreAll => {
                    ui.label(RichText::new(format!("¿Descartar tus ediciones de {} pinceles incluidos y volver a los originales? Tus pinceles propios no se tocan.", self.vlib.edited.len())).size(11.5).color(ACCENT2));
                    ui.horizontal(|ui| {
                        if ui.small_button("Sí, restaurar").clicked() {
                            let n = self.vlib.restore_all();
                            if let Some(p) = cur.as_ref().and_then(|k| self.vlib.find(k)) {
                                self.apply_vpreset(&p);
                            }
                            self.say(format!("{n} pinceles incluidos restaurados"));
                            self.save_vlib();
                            self.vui.confirm = VConfirm::None;
                        }
                        if ui.small_button("No").clicked() {
                            self.vui.confirm = VConfirm::None;
                        }
                    });
                }
                VConfirm::None => {}
            }

            // Editor de nombre y categoría
            if self.vui.edit != VEdit::None {
                let edit = self.vui.edit;
                ui.label(RichText::new(match edit {
                    VEdit::SaveAs => "Guardar pincel como",
                    VEdit::Rename => "Nuevo nombre",
                    _ => "Nueva categoría",
                }).size(11.5).color(MU));
                let mut enter = false;
                if edit != VEdit::Category {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.vui.name).hint_text("Nombre").desired_width(ui.available_width() - 4.0));
                    enter |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                }
                if edit != VEdit::Rename {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.vui.category).hint_text("Categoría").desired_width(ui.available_width() - 4.0));
                    enter |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    ui.horizontal_wrapped(|ui| {
                        for c in self.vlib.categories().into_iter().skip(8) {
                            if ui.small_button(&c).clicked() {
                                self.vui.category = c;
                            }
                        }
                    });
                }
                let mut ok = enter;
                ui.horizontal(|ui| {
                    if ui.small_button("Aceptar").clicked() {
                        ok = true;
                    }
                    if ui.small_button("Cancelar").clicked() {
                        self.vui.edit = VEdit::None;
                    }
                });
                if ok {
                    self.commit_vedit();
                }
            }

            if let Some(p) = &selected {
                egui::CollapsingHeader::new(RichText::new("Propiedades del pincel").size(11.5).color(MU)).id_salt("vpreset_info").show(ui, |ui| {
                    for (k, v) in p.summary() {
                        ui.label(RichText::new(format!("{k}: {v}")).size(11.0).color(MU));
                    }
                });
            }
        });
    }

    fn vector_import(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Pinceles vectoriales de LeonToon", &["ltvb"]).pick_file() else { return };
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| vlib::import_pack(&t)) {
            Ok(list) => {
                let n = vlib::merge(&mut self.vlib, list);
                self.say(format!("{n} pinceles vectoriales importados"));
                self.save_vlib();
            }
            Err(e) => self.say(e),
        }
    }

    fn vector_export(&mut self, list: Vec<VPreset>, file: &str) {
        let text = match vlib::export_pack(&list) {
            Ok(t) => t,
            Err(e) => return self.say(e),
        };
        let Some(path) = rfd::FileDialog::new().add_filter("Pinceles vectoriales de LeonToon", &["ltvb"]).set_file_name(format!("{file}.ltvb")).save_file() else { return };
        match std::fs::write(&path, text) {
            Ok(()) => self.say(format!("{} pinceles vectoriales exportados a {}", list.len(), path.display())),
            Err(e) => self.say(format!("No se pudo guardar el paquete: {e}")),
        }
    }
}
