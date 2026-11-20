//! Pinceles guardados en la interfaz: estado del pincel dentro del proyecto, biblioteca del usuario y el
//! panel de presets del panel N (guardar, actualizar, duplicar, renombrar, mover de categoría y eliminar).

use super::shell::muted;
use super::*;
use crate::brush::Gray;
use crate::presets::{self, BrushImage, BrushPreset, BrushSetup, BrushState, PresetKey, USER_CATEGORY};
use std::sync::Arc;

/// Imagen de punta o de textura en uso: identificador (huella), nombre del archivo y la máscara.
#[derive(Clone)]
pub(super) struct BrushImg {
    pub id: String,
    pub name: String,
    pub gray: Arc<Gray>,
}

#[derive(Default, PartialEq, Clone, Copy)]
pub(super) enum PresetEdit {
    #[default]
    None,
    SaveAs,
    Rename,
    Category,
}

/// Estado del editor de presets del panel N.
#[derive(Default)]
pub(super) struct PresetUi {
    pub edit: PresetEdit,
    pub name: String,
    pub category: String,
    pub confirm_delete: bool,
    /// Último preset elegido que la lista ya mostró (para llevar la lista hasta él una sola vez).
    pub seen: Option<PresetKey>,
}

impl LeonApp {
    /// Configuración del pincel tal como está ahora en pantalla.
    pub(super) fn brush_setup_now(&self) -> BrushSetup {
        BrushSetup {
            params: self.brush,
            size: self.brush_size,
            opacity: self.stroke_opacity,
            smoothing: self.smoothing,
            tip: self.brush_tip_img.as_ref().map(|i| i.id.clone()),
            texture: self.brush_tex_img.as_ref().map(|i| i.id.clone()),
        }
    }

    /// Copia el pincel en uso al proyecto, para que se guarde con él. No cuenta como un cambio del proyecto.
    pub(super) fn sync_brush_to_project(&mut self) {
        let st = BrushState { setup: self.brush_setup_now(), preset: self.brush_preset.clone() };
        if self.project.brush_state.as_ref() != Some(&st) {
            self.project.brush_state = Some(st);
        }
    }

    /// Los pinceles guardados y sus imágenes no se deshacen con Ctrl+Z: se llevan al proyecto restaurado.
    pub(super) fn brush_data(&self) -> (Vec<BrushPreset>, Vec<BrushImage>) {
        (self.project.brush_presets.clone(), self.project.brush_images.clone())
    }

    pub(super) fn restore_brush_data(&mut self, d: (Vec<BrushPreset>, Vec<BrushImage>)) {
        self.project.brush_presets = d.0;
        self.project.brush_images = d.1;
    }

    /// Máscara de una imagen guardada en el proyecto (se decodifica una vez y se guarda en memoria).
    pub(super) fn gray_by_id(&mut self, id: &str) -> Option<Arc<Gray>> {
        if let Some(g) = self.brush_img_cache.get(id) {
            return Some(g.clone());
        }
        let im = self.project.brush_images.iter().find(|i| i.id == id)?;
        match im.to_gray() {
            Ok(g) => {
                let g = Arc::new(g);
                self.brush_img_cache.insert(id.to_string(), g.clone());
                Some(g)
            }
            Err(e) => {
                let name = im.name.clone();
                self.say(format!("No se pudo leer la imagen «{name}» del pincel: {e}"));
                None
            }
        }
    }

    fn img_by_id(&mut self, id: &str) -> Option<BrushImg> {
        let name = self.project.brush_images.iter().find(|i| i.id == id)?.name.clone();
        let gray = self.gray_by_id(id)?;
        Some(BrushImg { id: id.to_string(), name, gray })
    }

    /// Guarda en el proyecto una imagen recién cargada (sin repetirla) y la deja lista para usar.
    pub(super) fn register_image(&mut self, name: &str, g: Gray) -> BrushImg {
        let id = presets::gray_id(&g);
        if !self.project.brush_images.iter().any(|i| i.id == id) {
            match BrushImage::from_gray(name, &g) {
                Ok(im) => self.project.brush_images.push(im),
                Err(e) => self.say(e),
            }
        }
        let gray = Arc::new(g);
        self.brush_img_cache.insert(id.clone(), gray.clone());
        BrushImg { id, name: name.to_string(), gray }
    }

    /// Quita del proyecto las imágenes que ya no usa ningún preset ni el pincel en uso.
    pub(super) fn prune_images(&mut self) {
        let mut used: Vec<String> = Vec::new();
        for s in self.project.brush_presets.iter().map(|p| &p.setup).chain(std::iter::once(&self.brush_setup_now())) {
            used.extend(s.tip.clone());
            used.extend(s.texture.clone());
        }
        self.project.brush_images.retain(|i| used.contains(&i.id));
        self.brush_img_cache.retain(|k, _| used.contains(k));
    }

    /// Pone el pincel en pantalla como dice la configuración (con sus imágenes).
    fn apply_setup(&mut self, setup: &BrushSetup) {
        let s = setup.clone().clamped();
        self.brush = s.params;
        self.brush_size = s.size;
        self.stroke_opacity = s.opacity;
        self.smoothing = s.smoothing;
        self.brush_tip_img = s.tip.as_deref().and_then(|id| self.img_by_id(id));
        self.brush_tex_img = s.texture.as_deref().and_then(|id| self.img_by_id(id));
        if self.brush_tip_img.is_none() {
            self.brush.use_tip_image = false;
        }
        if self.brush_tex_img.is_none() && self.brush.texture == crate::brush::TexKind::Image {
            self.brush.texture = crate::brush::TexKind::None;
        }
    }

    pub(super) fn apply_preset(&mut self, p: &BrushPreset) {
        self.apply_setup(&p.setup);
        self.brush_preset = Some(p.key());
    }

    /// Al abrir o crear un proyecto: recupera el pincel guardado. Los pinceles de la biblioteca NO se mezclan solos con
    /// el proyecto (un paquete grande lo haría crecer varios MB): se traen desde «Biblioteca…».
    pub(super) fn adopt_project_brush(&mut self) {
        self.brush_img_cache.clear();
        if let Some(st) = self.project.brush_state.clone() {
            self.apply_setup(&st.setup);
            self.brush_preset = st.preset.filter(|k| presets::find(&self.project.brush_presets, k).is_some());
        }
    }

    /// Pone en la biblioteca del usuario los pinceles del proyecto (sin pisar los demás) y quita los eliminados,
    /// para poder traerlos a otros proyectos. Las capturas de pantalla de prueba no tocan la biblioteca.
    pub(super) fn save_library_now(&mut self) {
        if self.shot.is_some() {
            self.brush_lib_removed.clear();
            return;
        }
        let removed = std::mem::take(&mut self.brush_lib_removed);
        if let Some(dir) = safety::data_dir() {
            if let Err(e) = presets::update_library(&dir, &self.project.brush_presets, &self.project.brush_images, &removed) {
                safety::log_line(&format!("No se pudo guardar la biblioteca de pinceles: {e}"));
            }
        }
    }

    /// Abre la ventana de la biblioteca con lo que hay guardado en el equipo.
    pub(super) fn open_library_window(&mut self) {
        let lib = safety::data_dir().map(|d| presets::load_library(&d)).unwrap_or_default();
        self.lib_window = Some(lib);
    }

    /// Trae al proyecto los pinceles de una categoría de la biblioteca (o de todas, con None).
    fn bring_from_library(&mut self, category: Option<&str>) {
        let Some(lib) = self.lib_window.clone() else { return };
        let filtered = presets::Library { presets: lib.presets.iter().filter(|p| category.map_or(true, |c| p.category == c)).cloned().collect(), ..lib.clone() };
        let n = presets::merge_library(&mut self.project.brush_presets, &mut self.project.brush_images, filtered);
        self.say(format!("{n} pinceles traídos de la biblioteca al proyecto"));
    }

    /// Ventana «Biblioteca de pinceles»: qué hay en el equipo que este proyecto todavía no tiene.
    pub(super) fn library_dialog(&mut self, ctx: &egui::Context) {
        let Some(lib) = &self.lib_window else { return };
        let offer = presets::library_offer(&self.project.brush_presets, lib);
        let total = lib.presets.len();
        let mut close = false;
        let mut bring: Option<Option<String>> = None;
        egui::Window::new("Biblioteca de pinceles").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.label(RichText::new(format!("Tu biblioteca guarda {total} pinceles en este equipo. Los pinceles que guardas o importas se copian aquí; un proyecto solo lleva los que le traes, para no crecer de más.")).size(11.5).color(MU));
            ui.add_space(4.0);
            if offer.is_empty() {
                ui.label("No hay pinceles en la biblioteca que este proyecto no tenga.");
            } else {
                egui::ScrollArea::vertical().max_height(240.0).id_salt("lib_scroll").show(ui, |ui| {
                    for (cat, n, bytes) in &offer {
                        ui.horizontal(|ui| {
                            ui.label(format!("{cat} · {n} {}", if *n == 1 { "pincel" } else { "pinceles" }));
                            ui.label(RichText::new(format!("≈ {:.1} MB", *bytes as f32 / 1_000_000.0)).size(11.0).color(INK3));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("Traer").clicked() {
                                    bring = Some(Some(cat.clone()));
                                }
                            });
                        });
                    }
                });
                if offer.len() > 1 && ui.button("Traer todo").clicked() {
                    bring = Some(None);
                }
            }
            if ui.button("Cerrar").clicked() {
                close = true;
            }
        });
        if let Some(cat) = bring {
            self.bring_from_library(cat.as_deref());
        }
        if close {
            self.lib_window = None;
        }
    }

    fn commit_preset_edit(&mut self) {
        let key = self.brush_preset.clone();
        let (name, cat) = (self.preset_ui.name.clone(), self.preset_ui.category.clone());
        let need = || "Elige primero un pincel de la lista".to_string();
        let res: Result<(PresetKey, &str), String> = match self.preset_ui.edit {
            PresetEdit::SaveAs => {
                let setup = self.brush_setup_now();
                Ok((presets::save_new(&mut self.project.brush_presets, &name, &cat, setup), "guardado"))
            }
            PresetEdit::Rename => key.clone().ok_or_else(need).and_then(|k| presets::rename(&mut self.project.brush_presets, &k, &name)).map(|k| (k, "renombrado")),
            PresetEdit::Category => key.clone().ok_or_else(need).and_then(|k| presets::set_category(&mut self.project.brush_presets, &k, &cat)).map(|k| (k, "movido de categoría")),
            PresetEdit::None => return,
        };
        match res {
            Ok((k, what)) => {
                if self.preset_ui.edit != PresetEdit::SaveAs {
                    if let Some(old) = key {
                        self.brush_lib_removed.push(old);
                    }
                }
                self.say(format!("Pincel «{}» {what} en «{}»", k.1, k.0));
                self.brush_preset = Some(k);
                self.preset_ui = PresetUi::default();
                self.prune_images();
                self.save_library_now();
            }
            Err(e) => self.say(e),
        }
    }

    /// Panel de pinceles guardados: lista por categorías y acciones.
    pub(super) fn brush_presets_ui(&mut self, ui: &mut egui::Ui) {
        let cur = self.brush_preset.clone();
        let selected = cur.as_ref().and_then(|k| presets::find(&self.project.brush_presets, k));
        let modified = selected.as_ref().map_or(false, |p| p.setup.clone().clamped() != self.brush_setup_now().clamped());
        let user_sel = selected.as_ref().map_or(false, |p| !p.builtin);

        egui::CollapsingHeader::new(RichText::new("Pinceles guardados").font(semi(12.0)).color(TX)).id_salt("fold_presets").default_open(true).show(ui, |ui| {
            let label = match &selected {
                Some(p) => format!("{} · {}{}", p.category, p.name, if modified { "   (modificado)" } else { "" }),
                None => "Sin preset: configuración propia".to_string(),
            };
            ui.label(RichText::new(label).size(11.5).color(if modified { ACCENT2 } else { MU }));

            let all: Vec<BrushPreset> = presets::builtin().into_iter().chain(self.project.brush_presets.iter().cloned()).collect();
            let mut apply: Option<BrushPreset> = None;
            let scroll_to = cur != self.preset_ui.seen;
            self.preset_ui.seen = cur.clone();
            egui::ScrollArea::vertical().id_salt("preset_list").max_height(210.0).auto_shrink([false, true]).show(ui, |ui| {
                for cat in presets::categories(&self.project.brush_presets) {
                    ui.label(RichText::new(&cat).size(11.0).color(INK3));
                    for p in all.iter().filter(|p| p.category.eq_ignore_ascii_case(&cat)) {
                        let on = cur.as_ref() == Some(&p.key());
                        let tip = if p.builtin { "Pincel incluido: no se puede modificar, pero sí duplicar" } else { "Pincel guardado" };
                        let r = ui.selectable_label(on, RichText::new(&p.name).size(12.0)).on_hover_text(tip);
                        if on && scroll_to {
                            r.scroll_to_me(Some(egui::Align::Center));
                        }
                        if r.clicked() {
                            apply = Some(p.clone());
                        }
                    }
                }
            });
            if let Some(p) = apply {
                self.apply_preset(&p);
                self.preset_ui = PresetUi::default();
                self.say(format!("Pincel «{}» aplicado", p.name));
            }

            // Acciones
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Guardar como…").on_hover_text("Guarda el pincel actual como uno nuevo").clicked() {
                    self.preset_ui = PresetUi {
                        edit: PresetEdit::SaveAs,
                        name: selected.as_ref().map_or("Pincel nuevo".to_string(), |p| format!("{} (nuevo)", p.name)),
                        category: selected.as_ref().filter(|p| !p.builtin).map_or(USER_CATEGORY.to_string(), |p| p.category.clone()),
                        confirm_delete: false,
                        seen: cur.clone(),
                    };
                }
                if ui.add_enabled(user_sel && modified, egui::Button::new("Actualizar").small()).on_hover_text("Guarda los cambios en el pincel elegido").clicked() {
                    if let Some(k) = cur.clone() {
                        let setup = self.brush_setup_now();
                        match presets::update(&mut self.project.brush_presets, &k, setup) {
                            Ok(()) => {
                                self.say(format!("Pincel «{}» actualizado", k.1));
                                self.prune_images();
                                self.save_library_now();
                            }
                            Err(e) => self.say(e),
                        }
                    }
                }
                if ui.add_enabled(selected.is_some(), egui::Button::new("Duplicar").small()).clicked() {
                    if let Some(k) = cur.clone() {
                        match presets::duplicate(&mut self.project.brush_presets, &k) {
                            Ok(nk) => {
                                self.say(format!("Pincel duplicado como «{}» en «{}»", nk.1, nk.0));
                                self.brush_preset = Some(nk);
                                self.save_library_now();
                            }
                            Err(e) => self.say(e),
                        }
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(user_sel, egui::Button::new("Renombrar").small()).clicked() {
                    self.preset_ui = PresetUi { edit: PresetEdit::Rename, name: cur.as_ref().map_or(String::new(), |k| k.1.clone()), ..Default::default() };
                }
                if ui.add_enabled(user_sel, egui::Button::new("Categoría").small()).on_hover_text("Mueve el pincel a otra categoría").clicked() {
                    self.preset_ui = PresetUi { edit: PresetEdit::Category, category: cur.as_ref().map_or(String::new(), |k| k.0.clone()), ..Default::default() };
                }
                if ui.add_enabled(user_sel, egui::Button::new("Eliminar").small()).clicked() {
                    self.preset_ui = PresetUi { confirm_delete: true, ..Default::default() };
                }
            });
            if !user_sel && selected.is_some() {
                muted(ui, "Los pinceles incluidos no se modifican: duplícalo para tener tu propia copia.");
            }
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Importar .abr…").on_hover_text("Pinceles de Photoshop: formas y parámetros que se puedan leer; avisa de lo demás").clicked() {
                    self.abr_start_import();
                }
                if ui.small_button("Biblioteca…").on_hover_text("Trae al proyecto pinceles guardados antes en este equipo").clicked() {
                    self.open_library_window();
                }
                if ui.small_button("Exportar .abr…").on_hover_text("Guarda pinceles en un archivo de Photoshop (versión 2) y avisa de lo que no cabe").clicked() {
                    self.abr_start_export();
                }
            });
            // Lo que no se conservó al importar este pincel.
            if let Some(p) = selected.as_ref().filter(|p| !p.notes.is_empty()) {
                egui::CollapsingHeader::new(RichText::new(format!("No se conservó de Photoshop ({})", p.notes.len())).size(11.5).color(ACCENT2)).id_salt("preset_notes").show(ui, |ui| {
                    for n in &p.notes {
                        ui.label(RichText::new(format!("• {n}")).size(11.0).color(MU));
                    }
                });
            }

            // Confirmar la eliminación
            if self.preset_ui.confirm_delete {
                if let Some(k) = cur.clone().filter(|_| user_sel) {
                    ui.label(RichText::new(format!("¿Eliminar «{}» de este proyecto y de tu biblioteca? No se puede deshacer.", k.1)).size(11.5).color(ACCENT2));
                    ui.horizontal(|ui| {
                        if ui.small_button("Sí, eliminar").clicked() {
                            match presets::remove(&mut self.project.brush_presets, &k) {
                                Ok(()) => {
                                    self.say(format!("Pincel «{}» eliminado del proyecto y de la biblioteca", k.1));
                                    self.brush_lib_removed.push(k.clone());
                                    self.brush_preset = None;
                                    self.preset_ui = PresetUi::default();
                                    self.prune_images();
                                    self.save_library_now();
                                }
                                Err(e) => self.say(e),
                            }
                        }
                        if ui.small_button("No").clicked() {
                            self.preset_ui = PresetUi::default();
                        }
                    });
                } else {
                    self.preset_ui.confirm_delete = false;
                }
            }

            // Editor de nombre y categoría
            if self.preset_ui.edit != PresetEdit::None {
                let edit = self.preset_ui.edit;
                let title = match edit {
                    PresetEdit::SaveAs => "Guardar pincel como",
                    PresetEdit::Rename => "Nuevo nombre",
                    _ => "Nueva categoría",
                };
                ui.label(RichText::new(title).size(11.5).color(MU));
                let mut enter = false;
                if edit != PresetEdit::Category {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.preset_ui.name).hint_text("Nombre").desired_width(ui.available_width() - 4.0));
                    enter |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                }
                if edit != PresetEdit::Rename {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.preset_ui.category).hint_text("Categoría").desired_width(ui.available_width() - 4.0));
                    enter |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    // Categorías que ya existen, para elegir con un clic.
                    ui.horizontal_wrapped(|ui| {
                        for c in presets::categories(&self.project.brush_presets) {
                            if ui.small_button(RichText::new(&c).size(10.5)).clicked() {
                                self.preset_ui.category = c;
                            }
                        }
                    });
                }
                ui.horizontal(|ui| {
                    if ui.small_button("Aceptar").clicked() || enter {
                        self.commit_preset_edit();
                    }
                    if ui.small_button("Cancelar").clicked() {
                        self.preset_ui = PresetUi::default();
                    }
                });
            }
        });
    }
}
