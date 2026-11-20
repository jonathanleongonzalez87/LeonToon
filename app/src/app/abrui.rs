//! Importar y exportar pinceles de Photoshop (`.abr`): ventanas con el resumen y con todo lo que no se conserva.

use super::*;
use crate::abr::{self, AbrFile};
use crate::brush::Gray;
use crate::presets::{self, BrushPreset};

/// Ventana de importación: lo que trae el archivo y la categoría donde se guardará.
pub(super) struct ImportUi {
    pub file_name: String,
    pub abr: AbrFile,
    pub category: String,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Scope {
    One,
    Category,
    Mine,
}

/// Ventana de exportación: qué pinceles van al archivo.
pub(super) struct ExportUi {
    pub scope: Scope,
}

/// Resultado de importar o exportar, con todo lo que no cupo.
pub(super) struct ReportUi {
    pub title: String,
    pub general: Vec<String>,
    pub per: Vec<(String, Vec<String>)>,
}

/// Nombre de la propiedad a la que se refiere un aviso («Textura de patrón «X»: …» → «Textura de patrón»).
fn feature_of(note: &str) -> String {
    let head = note.split(':').next().unwrap_or(note);
    let head = head.split(" (").next().unwrap_or(head);
    let mut out = String::new();
    let mut skip = false;
    for c in head.chars() {
        match c {
            '«' => skip = true,
            '»' => skip = false,
            _ if skip => {}
            _ if c.is_ascii_digit() || c == '%' || c == '.' => {}
            _ => out.push(c),
        }
    }
    out.trim().trim_end_matches(" de").trim().to_string()
}

/// Cuántos pinceles tienen cada tipo de aviso, de más a menos.
fn summarize<'a>(lists: impl Iterator<Item = &'a Vec<String>>) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for notes in lists {
        let mut seen: Vec<String> = Vec::new();
        for n in notes {
            let f = feature_of(n);
            if seen.contains(&f) {
                continue;
            }
            match counts.iter_mut().find(|(k, _)| *k == f) {
                Some(c) => c.1 += 1,
                None => counts.push((f.clone(), 1)),
            }
            seen.push(f);
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    counts
}

impl LeonApp {
    /// Pide un archivo `.abr` y lo lee. Si no es un ABR de Photoshop, dice por qué.
    pub(super) fn abr_start_import(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Pinceles de Photoshop", &["abr"]).pick_file() else { return };
        let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        match abr::read_abr(&path) {
            Ok(f) => {
                let stem = path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                self.abr_import = Some(ImportUi { file_name, abr: f, category: stem });
            }
            Err(e) => {
                self.abr_report = Some(ReportUi { title: format!("No se pudo importar «{file_name}»"), general: vec![e], per: vec![] });
            }
        }
    }

    pub(super) fn abr_start_export(&mut self) {
        self.abr_export = Some(ExportUi { scope: if self.brush_preset.is_some() { Scope::One } else { Scope::Mine } });
    }

    /// Solo para capturas de pantalla: LEONTOON_ABR=<archivo> abre la ventana de importación; con LEONTOON_ABR_DEMO=1
    /// importa, y pinta con los primeros pinceles importados que no sean enormes para ver cómo quedan.
    pub(super) fn abr_screenshot_demo(&mut self, path: &std::path::Path) {
        let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        match abr::read_abr(path) {
            Err(e) => self.abr_report = Some(ReportUi { title: "No se pudo importar".into(), general: vec![e], per: vec![] }),
            Ok(f) => {
                self.abr_import = Some(ImportUi { file_name, abr: f, category: "Kyle Brush Presets".into() });
                if std::env::var("LEONTOON_ABR_DEMO").is_err() {
                    return;
                }
                self.abr_commit_import();
                let mut keys: Vec<presets::PresetKey> = Vec::new();
                for p in self.project.brush_presets.iter().filter(|p| p.setup.size <= 80.0 && p.setup.tip.is_some()) {
                    keys.push(p.key());
                    if keys.len() == 6 {
                        break;
                    }
                }
                let (w, h) = (self.project.width, self.project.height);
                let mut layer = Layer::new("Pinceles de Photoshop");
                layer.kind = LayerKind::Bitmap;
                let mut raster = crate::raster::Raster::new(w, h);
                for (i, k) in keys.iter().enumerate() {
                    let Some(p) = presets::find(&self.project.brush_presets, k) else { continue };
                    let tip = p.setup.tip.as_deref().and_then(|id| self.gray_by_id(id));
                    let spec = crate::brush::BrushSpec::new(p.setup.size, p.setup.opacity, [30, 60, 140], p.setup.params, 100 + i as u64).with_color2([200, 60, 60]).with_images(tip, None);
                    let mut ls = crate::brush::LiveStroke::new(w, h, spec);
                    let y = 130.0 + i as f32 * 85.0;
                    for s in 0..=80 {
                        let t = s as f32 / 80.0;
                        let pressure = (t * std::f32::consts::PI).sin().clamp(0.1, 1.0);
                        ls.push_with([260.0 + t * 620.0, y + (t * 7.0).sin() * 22.0], crate::brush::Pen { pressure, tilt: 1.0 });
                    }
                    ls.finish_into(&mut raster);
                }
                layer.keys.insert(0, std::sync::Arc::new(Drawing { strokes: vec![], raster: Some(std::sync::Arc::new(raster)) }));
                self.project.layers.insert(0, layer);
                self.cl = 0;
            }
        }
    }

    /// Guarda en el proyecto los pinceles leídos, con sus formas y con la lista de lo que no se conservó.
    fn abr_commit_import(&mut self) {
        let Some(ui) = self.abr_import.take() else { return };
        let total = ui.abr.brushes.len();
        let mut first: Option<presets::PresetKey> = None;
        for im in ui.abr.brushes {
            let mut setup = im.setup.clone();
            setup.tip = None;
            match im.tip {
                Some(g) => {
                    let name = if im.tip_name.is_empty() { im.name.clone() } else { im.tip_name.clone() };
                    setup.tip = Some(self.register_image(&name, g).id);
                }
                None => setup.params.use_tip_image = false,
            }
            let key = presets::save_new_with_notes(&mut self.project.brush_presets, &im.name, &ui.category, setup, im.notes);
            first.get_or_insert(key);
        }
        self.prune_images();
        self.save_library_now();
        if let Some(k) = first {
            if let Some(p) = presets::find(&self.project.brush_presets, &k) {
                self.apply_preset(&p);
            }
        }
        self.say(format!("Importados {total} pinceles en «{}»", ui.category));
    }

    fn collect_for_export(&mut self, scope: Scope) -> Vec<(BrushPreset, Option<Gray>)> {
        let all: Vec<BrushPreset> = presets::builtin().into_iter().chain(self.project.brush_presets.iter().cloned()).collect();
        let cur = self.brush_preset.clone();
        let chosen: Vec<BrushPreset> = match scope {
            Scope::One => cur.and_then(|k| presets::find(&self.project.brush_presets, &k)).into_iter().collect(),
            Scope::Category => match cur {
                Some(k) => all.into_iter().filter(|p| p.category == k.0).collect(),
                None => Vec::new(),
            },
            Scope::Mine => self.project.brush_presets.clone(),
        };
        chosen
            .into_iter()
            .map(|p| {
                let tip = p.setup.tip.clone().filter(|_| p.setup.params.use_tip_image).and_then(|id| self.gray_for_export(&id));
                (p, tip)
            })
            .collect()
    }

    fn gray_for_export(&mut self, id: &str) -> Option<Gray> {
        self.gray_by_id(id).map(|g| (*g).clone())
    }

    fn abr_do_export(&mut self, scope: Scope) {
        let list = self.collect_for_export(scope);
        if list.is_empty() {
            self.say("No hay pinceles que exportar con esa elección");
            return;
        }
        let suggested = match (scope, self.brush_preset.as_ref()) {
            (Scope::One, Some(k)) => k.1.clone(),
            (Scope::Category, Some(k)) => k.0.clone(),
            _ => "Mis pinceles".to_string(),
        };
        let Some(path) = rfd::FileDialog::new().add_filter("Pinceles de Photoshop (ABR versión 2)", &["abr"]).set_file_name(format!("{suggested}.abr")).save_file() else { return };
        match abr::export_v2(&list) {
            Ok((bytes, rep)) => match std::fs::write(&path, bytes) {
                Ok(()) => {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    self.say(format!("Exportados {} pinceles a {name}", rep.written));
                    self.abr_report = Some(ReportUi { title: format!("Exportados {} pinceles a «{name}»", rep.written), general: rep.general, per: rep.per_brush });
                }
                Err(e) => self.say(format!("No se pudo guardar el archivo: {e}")),
            },
            Err(e) => self.say(e),
        }
    }

    /// Ventanas de importación, exportación e informe (se dibujan una vez por fotograma, fuera de los paneles).
    pub(super) fn abr_dialogs(&mut self, ctx: &egui::Context) {
        // ---- Importar ----
        if let Some(ui) = &mut self.abr_import {
            let mut go = false;
            let mut cancel = false;
            egui::Window::new("Importar pinceles de Photoshop").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui_| {
                ui_.set_width(420.0);
                let a = &ui.abr;
                ui_.label(RichText::new(format!("{} · ABR versión {}.{}", ui.file_name, a.version.0, a.version.1)).color(MU));
                let with_tip = a.brushes.iter().filter(|b| b.tip.is_some()).count();
                ui_.label(format!("{} pinceles, {} con forma de imagen ({} formas en el archivo).", a.brushes.len(), with_tip, a.tips_found));
                let sum = summarize(a.brushes.iter().map(|b| &b.notes));
                if !a.warnings.is_empty() || !sum.is_empty() {
                    ui_.add_space(4.0);
                    ui_.label(RichText::new("Lo que no se conserva").font(semi(12.0)).color(ACCENT2));
                    egui::ScrollArea::vertical().max_height(220.0).id_salt("abr_imp_scroll").show(ui_, |ui_| {
                        for w in &a.warnings {
                            ui_.label(RichText::new(format!("• {w}")).size(11.5).color(TX));
                        }
                        for (f, n) in &sum {
                            ui_.label(RichText::new(format!("• {f}: {n} {}", if *n == 1 { "pincel" } else { "pinceles" })).size(11.5).color(MU));
                        }
                    });
                    ui_.label(RichText::new("Cada pincel guarda su lista; la verás al elegirlo.").size(11.0).color(INK3));
                }
                ui_.add_space(4.0);
                ui_.horizontal(|ui_| {
                    ui_.label("Categoría:");
                    ui_.add(egui::TextEdit::singleline(&mut ui.category).desired_width(240.0));
                });
                ui_.horizontal(|ui_| {
                    if ui_.add_enabled(!a.brushes.is_empty(), egui::Button::new(format!("Importar {} pinceles", a.brushes.len()))).clicked() {
                        go = true;
                    }
                    if ui_.button("Cancelar").clicked() {
                        cancel = true;
                    }
                });
            });
            if go {
                self.abr_commit_import();
            } else if cancel {
                self.abr_import = None;
            }
        }

        // ---- Exportar ----
        if let Some(ex) = &mut self.abr_export {
            let mut go = false;
            let mut cancel = false;
            let cur = self.brush_preset.clone();
            egui::Window::new("Exportar pinceles a Photoshop").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.set_width(400.0);
                ui.label("Qué pinceles van al archivo:");
                ui.add_enabled_ui(cur.is_some(), |ui| {
                    ui.radio_value(&mut ex.scope, Scope::One, cur.as_ref().map_or("El pincel elegido".to_string(), |k| format!("Solo «{}»", k.1)));
                    ui.radio_value(&mut ex.scope, Scope::Category, cur.as_ref().map_or("Su categoría".to_string(), |k| format!("Toda la categoría «{}»", k.0)));
                });
                ui.radio_value(&mut ex.scope, Scope::Mine, "Todos mis pinceles guardados");
                if cur.is_none() && ex.scope != Scope::Mine {
                    ex.scope = Scope::Mine;
                }
                ui.add_space(4.0);
                ui.label(RichText::new("Se escribe ABR versión 2, que Photoshop abre. Solo guarda la forma, el diámetro, la dureza, el espaciado, la redondez y el ángulo: las dinámicas, los efectos, el flujo y la opacidad no caben, y el informe te dirá cuáles se pierden en cada pincel.").size(11.5).color(MU));
                ui.horizontal(|ui| {
                    if ui.button("Elegir archivo y exportar…").clicked() {
                        go = true;
                    }
                    if ui.button("Cancelar").clicked() {
                        cancel = true;
                    }
                });
            });
            let scope = ex.scope;
            if go {
                self.abr_export = None;
                self.abr_do_export(scope);
            } else if cancel {
                self.abr_export = None;
            }
        }

        // ---- Informe ----
        if let Some(r) = &self.abr_report {
            let mut close = false;
            egui::Window::new(&r.title).collapsible(false).resizable(true).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.set_width(460.0);
                for g in &r.general {
                    ui.label(RichText::new(g).size(12.0).color(TX));
                }
                let sum = summarize(r.per.iter().map(|(_, n)| n).filter(|n| !n.is_empty()));
                if !sum.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new("No se pudo guardar").font(semi(12.0)).color(ACCENT2));
                    for (f, n) in &sum {
                        ui.label(RichText::new(format!("• {f}: {n} {}", if *n == 1 { "pincel" } else { "pinceles" })).size(11.5).color(MU));
                    }
                    egui::CollapsingHeader::new("Detalle por pincel").id_salt("abr_report_detail").show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(220.0).id_salt("abr_report_scroll").show(ui, |ui| {
                            for (name, notes) in r.per.iter().filter(|(_, n)| !n.is_empty()) {
                                ui.label(RichText::new(name).size(11.5).color(TX));
                                for n in notes {
                                    ui.label(RichText::new(format!("   no se guarda {n}")).size(11.0).color(MU));
                                }
                            }
                        });
                    });
                }
                if ui.button("Cerrar").clicked() {
                    close = true;
                }
            });
            if close {
                self.abr_report = None;
            }
        }
    }
}
