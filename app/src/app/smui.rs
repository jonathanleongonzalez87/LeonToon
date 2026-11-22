//! Panel de la capa Stop Motion (Issue #59): elegir la carpeta de la secuencia, importar fotos, verificar la integridad,
//! reincorporar fotos huérfanas, volver a localizar la carpeta y ver las miniaturas. Es la interfaz mínima de la Fase 1: la
//! sala completa, la edición de la secuencia, la captura y la composición llegan en las fases siguientes (#60 a #62).

use super::shell::{muted, panel_header};
use super::*;
use crate::stopmotion::{Located, OpenNote, Problem, SeqFolder, SmRef};
use std::path::PathBuf;

/// Una secuencia cargada en memoria (la carpeta y lo que se sabe de su estado).
pub(super) struct SmView {
    pub folder: SeqFolder,
    pub note: OpenNote,
    pub problems: Option<Vec<Problem>>,
    pub orphans: usize,
    /// Resultado de la última importación o recuperación, para mostrarlo en el panel.
    pub last: String,
    /// Los fotogramas por segundo cambiaron y falta guardarlos en sequence.json (se guardan al soltar el control).
    pub dirty: bool,
}

enum SmState {
    Ready,
    Missing(String),
}

/// Miniaturas que se cargan en cada fotograma como máximo (cada una tarda unos milisegundos).
const THUMBS_PER_FRAME: usize = 2;

impl LeonApp {
    fn project_dir(&self) -> Option<PathBuf> {
        self.path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf())
    }

    /// Carga la secuencia de la referencia (o usa la que ya está en memoria).
    fn sm_state(&mut self, r: &SmRef) -> SmState {
        if self.sm.contains_key(&r.id) {
            return SmState::Ready;
        }
        match r.locate(self.project_dir().as_deref()) {
            Located::Found(dir) => match SeqFolder::open(&dir) {
                Ok((folder, note)) => {
                    let orphans = folder.orphans().len();
                    let last = if note == OpenNote::FromBackup { "sequence.json estaba dañado: se abrió la copia anterior. Se guardará de nuevo al importar o recuperar.".to_string() } else { String::new() };
                    self.sm.insert(r.id.clone(), SmView { folder, note, problems: None, orphans, last, dirty: false });
                    SmState::Ready
                }
                Err(e) => SmState::Missing(e),
            },
            Located::WrongSequence(name) => SmState::Missing(format!("La carpeta guardada tiene otra secuencia («{name}»), no «{}».", r.name)),
            Located::Missing => SmState::Missing(format!("No se encuentra la carpeta de la secuencia «{}».", r.name)),
        }
    }

    /// Guarda la referencia de la capa actual (y marca el proyecto como cambiado).
    fn sm_set_ref(&mut self, r: Option<SmRef>) {
        let cl = self.cl;
        self.project.layers[cl].stopmotion = r;
        self.dirty = true;
    }

    fn sm_adopt(&mut self, folder: SeqFolder, note: OpenNote) {
        let pdir = self.project_dir();
        let r = SmRef::new(&folder, pdir.as_deref());
        let orphans = folder.orphans().len();
        self.checkpoint();
        self.sm_set_ref(Some(r.clone()));
        self.sm.insert(r.id, SmView { folder, note, problems: None, orphans, last: String::new(), dirty: false });
    }

    /// Crea una secuencia nueva en una carpeta que elige la persona.
    fn sm_create(&mut self) {
        let Some(dir) = rfd::FileDialog::new().set_title("Elige la carpeta donde se guardará la secuencia (se crea si no existe)").pick_folder() else { return };
        let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Secuencia".into());
        match SeqFolder::create(&dir, &name, self.project.fps) {
            Ok(f) => {
                self.sm_adopt(f, OpenNote::Ok);
                self.say(format!("Secuencia «{name}» creada en {}", dir.display()));
            }
            Err(e) => self.say(e),
        }
    }

    /// Usa una secuencia que ya existe en una carpeta.
    fn sm_open_existing(&mut self) {
        let Some(dir) = rfd::FileDialog::new().set_title("Elige la carpeta de una secuencia Stop Motion que ya existe").pick_folder() else { return };
        match SeqFolder::open(&dir) {
            Ok((f, note)) => {
                // Dos capas del proyecto con la misma secuencia pisarían su sequence.json.
                if self.project.layers.iter().any(|l| l.stopmotion.as_ref().map_or(false, |r| r.id == f.seq.id)) {
                    self.say("Esa secuencia ya está en otra capa de este proyecto: dos capas no pueden compartir la misma carpeta");
                    return;
                }
                let name = f.seq.name.clone();
                self.sm_adopt(f, note);
                self.say(format!("Secuencia «{name}» abierta"));
            }
            Err(e) => self.say(e),
        }
    }

    fn sm_relocate(&mut self, r: &SmRef) {
        let Some(dir) = rfd::FileDialog::new().set_title(format!("Elige la carpeta de la secuencia «{}»", r.name)).pick_folder() else { return };
        let mut nr = r.clone();
        match nr.relocate(&dir, self.project_dir().as_deref()) {
            Ok(()) => {
                self.sm.remove(&r.id);
                self.sm_set_ref(Some(nr));
                self.say(format!("Carpeta localizada: {}", dir.display()));
            }
            Err(e) => self.say(e),
        }
    }

    fn sm_import(&mut self, id: &str) {
        let Some(files) = rfd::FileDialog::new().set_title("Importar fotos a la secuencia").add_filter("Fotos (JPEG o PNG)", &["jpg", "jpeg", "png"]).pick_files() else { return };
        let pdir = self.project_dir();
        let Some(view) = self.sm.get_mut(id) else { return };
        let report = view.folder.import_files(&files);
        let reused = report.added.iter().filter(|(_, o)| o.reused).count();
        let mut text = format!("{} fotos importadas", report.added.len());
        if reused > 0 {
            text += &format!(" ({reused} ya estaban: se reutilizó su archivo)");
        }
        if !report.failed.is_empty() {
            text += &format!("\n{} no se pudieron importar:", report.failed.len());
            for (p, e) in report.failed.iter().take(6) {
                text += &format!("\n• {}: {e}", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
            }
            if report.failed.len() > 6 {
                text += &format!("\n… y {} más", report.failed.len() - 6);
            }
        }
        view.last = text.clone();
        view.problems = None;
        let mut r = SmRef::new(&view.folder, pdir.as_deref());
        r.update(&view.folder, pdir.as_deref());
        self.sm_set_ref(Some(r));
        self.say(text.lines().next().unwrap_or("").to_string());
    }

    fn sm_verify(&mut self, id: &str) {
        if let Some(v) = self.sm.get_mut(id) {
            v.problems = Some(v.folder.verify(true));
            v.orphans = v.folder.orphans().len();
        }
    }

    fn sm_recover(&mut self, id: &str) {
        let pdir = self.project_dir();
        let Some(v) = self.sm.get_mut(id) else { return };
        match v.folder.recover_orphans() {
            Ok((n, skipped)) => {
                v.last = format!("{n} fotos reincorporadas al final de la secuencia{}", if skipped.is_empty() { String::new() } else { format!("; {} no eran fotos válidas", skipped.len()) });
                v.orphans = v.folder.orphans().len();
                v.problems = None;
                let r = SmRef::new(&v.folder, pdir.as_deref());
                let msg = v.last.clone();
                self.sm_set_ref(Some(r));
                self.say(msg);
            }
            Err(e) => self.say(format!("No se pudieron recuperar las fotos: {e}")),
        }
    }

    /// Miniatura de una foto como textura (se genera en disco la primera vez y se reutiliza).
    fn sm_thumb(&mut self, ctx: &egui::Context, id: &str, source: &str) -> Option<egui::TextureHandle> {
        let key = (id.to_string(), source.to_string());
        if let Some(t) = self.sm_thumbs.get(&key) {
            return Some(t.clone());
        }
        let path = self.sm.get(id)?.folder.ensure_thumbnail(source).ok()?;
        let img = image::open(&path).ok()?.to_rgb8();
        let color = egui::ColorImage::from_rgb([img.width() as usize, img.height() as usize], img.as_raw());
        let tex = ctx.load_texture(format!("sm_{id}_{source}"), color, egui::TextureOptions::LINEAR);
        self.sm_thumbs.insert(key, tex.clone());
        Some(tex)
    }

    /// Sección «Secuencia Stop Motion» del panel N, para las capas de ese tipo.
    pub(super) fn stopmotion_panel_ui(&mut self, ui: &mut egui::Ui) {
        panel_header(ui, "Secuencia Stop Motion");
        let r = self.layer().stopmotion.clone();
        let Some(r) = r else {
            muted(ui, "Esta capa todavía no tiene secuencia. Elige la carpeta donde se guardarán las fotos: es una carpeta tuya, y el proyecto solo recuerda dónde está.");
            if ui.button("Crear una secuencia nueva…").on_hover_text("Elige una carpeta (se crea si no existe)").clicked() {
                self.sm_create();
            }
            if ui.button("Usar una secuencia que ya existe…").on_hover_text("Elige la carpeta de una secuencia Stop Motion anterior").clicked() {
                self.sm_open_existing();
            }
            return;
        };
        match self.sm_state(&r) {
            SmState::Missing(msg) => {
                ui.label(RichText::new(format!("⚠ {msg}")).size(11.5).color(ACCENT2));
                muted(ui, &format!("Se guardó en:\n{}", r.abs));
                if let Some(rel) = &r.rel {
                    muted(ui, &format!("Relativa al proyecto: {rel}"));
                }
                muted(ui, &format!("{} fotos · {} cuadros · {} fps (resumen guardado)", r.photos, r.frames, r.fps));
                if ui.button("Buscar la carpeta…").on_hover_text("Elige la carpeta donde está ahora esa secuencia: se acepta solo si es la misma").clicked() {
                    self.sm_relocate(&r);
                }
            }
            SmState::Ready => self.sm_ready_ui(ui, &r),
        }
    }

    /// Dónde empieza la secuencia en el proyecto, a cuántos fotogramas por segundo va y si cabe en la duración del proyecto.
    fn sm_timing_ui(&mut self, ui: &mut egui::Ui, r: &SmRef, id: &str, frames: u32, fps: u32) {
        let (pfps, plen) = (self.project.fps, self.project.length);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Empieza en el cuadro").size(11.5).color(MU));
            let mut start = r.start + 1;
            let resp = ui.add(egui::DragValue::new(&mut start).range(1..=plen.max(1)).speed(0.2));
            if resp.drag_started() || resp.gained_focus() {
                self.checkpoint();
            }
            if resp.changed() && start - 1 != r.start {
                let mut nr = r.clone();
                nr.start = start - 1;
                self.sm_set_ref(Some(nr));
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Fotogramas por segundo").size(11.5).color(MU));
            let mut f = fps;
            let resp = ui.add(egui::DragValue::new(&mut f).range(1..=120).speed(0.1));
            if resp.changed() && f != fps {
                if let Some(v) = self.sm.get_mut(id) {
                    v.folder.seq.fps = f;
                    v.dirty = true;
                }
            }
            if resp.drag_stopped() || resp.lost_focus() {
                self.sm_save_fps(id);
            }
        });
        // Cuántos cuadros del proyecto ocupa la secuencia (cada cuadro suyo dura ps proyecto / fps secuencia cuadros del proyecto).
        let need = r.start + (frames as u64 * pfps as u64).div_ceil(fps.max(1) as u64) as u32;
        if need > plen {
            ui.label(RichText::new(format!("⚠ La secuencia llega hasta el cuadro {need} y el proyecto dura {plen}: los últimos no se verán ni se exportarán.")).size(11.0).color(ACCENT2));
        }
        if frames > 0 && need != plen && ui.small_button("Ajustar la duración del proyecto a la secuencia").on_hover_text(format!("Pone el proyecto en {need} cuadros")).clicked() {
            self.checkpoint();
            self.project.length = need.clamp(1, 100_000);
            self.dirty = true;
            self.say(format!("El proyecto ahora dura {} cuadros", self.project.length));
        }
    }

    /// Guarda en sequence.json el cambio de fotogramas por segundo y actualiza el resumen de la capa.
    fn sm_save_fps(&mut self, id: &str) {
        let pdir = self.project_dir();
        let mut r = self.layer().stopmotion.clone();
        let Some(v) = self.sm.get_mut(id) else { return };
        if !v.dirty {
            return;
        }
        v.dirty = false;
        let saved = v.folder.save();
        if saved.is_ok() {
            if let Some(r) = r.as_mut() {
                r.update(&v.folder, pdir.as_deref());
            }
        }
        match saved {
            Ok(()) => {
                self.sm_set_ref(r);
                self.say("Fotogramas por segundo de la secuencia guardados");
            }
            Err(e) => self.say(format!("No se pudo guardar sequence.json: {e}")),
        }
    }

    fn sm_ready_ui(&mut self, ui: &mut egui::Ui, r: &SmRef) {
        let id = r.id.clone();
        let (name, dir, photos, items, frames, fps, note, orphans, last, sources, problems) = {
            let v = &self.sm[&id];
            (
                v.folder.seq.name.clone(),
                v.folder.dir.clone(),
                v.folder.seq.sources.len(),
                v.folder.seq.items.len(),
                v.folder.seq.total_frames(),
                v.folder.seq.fps,
                v.note,
                v.orphans,
                v.last.clone(),
                v.folder.seq.sources.iter().take(12).map(|s| s.id.clone()).collect::<Vec<_>>(),
                v.problems.clone(),
            )
        };
        ui.label(RichText::new(&name).strong().color(TX));
        muted(ui, &format!("Carpeta: {}", dir.display()));
        muted(ui, &format!("{photos} fotos · {items} posiciones · {frames} cuadros · {fps} fps"));
        self.sm_timing_ui(ui, r, &id, frames, fps);
        if note == OpenNote::FromBackup {
            ui.label(RichText::new("Se abrió la copia anterior de sequence.json").size(11.0).color(ACCENT2));
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Importar fotos…").on_hover_text("Copia fotos JPEG o PNG a la carpeta de la secuencia; las tuyas no se tocan").clicked() {
                self.sm_import(&id);
            }
            if ui.button("Verificar").on_hover_text("Comprueba que todas las fotos existen y no han cambiado").clicked() {
                self.sm_verify(&id);
            }
            if ui.button("Abrir carpeta").clicked() {
                let _ = std::process::Command::new("explorer").arg(&dir).spawn();
            }
        });
        if !last.is_empty() {
            muted(ui, &last);
        }
        if let Some(p) = &problems {
            if p.is_empty() {
                ui.label(RichText::new("✔ Todas las fotos están y no han cambiado").size(11.5).color(MU));
            } else {
                ui.label(RichText::new(format!("⚠ {} fotos con problemas", p.len())).size(11.5).color(ACCENT2));
                for pr in p.iter().take(8) {
                    let line = match pr {
                        Problem::Missing { file, .. } => format!("• Falta {file}"),
                        Problem::Damaged { file, reason, .. } => format!("• {file}: {reason}"),
                    };
                    muted(ui, &line);
                }
            }
        }
        if orphans > 0 {
            ui.label(RichText::new(format!("Hay {orphans} fotos en la carpeta que no están en la secuencia (por ejemplo, tras un cierre inesperado).")).size(11.5).color(MU));
            if ui.button("Reincorporarlas al final").clicked() {
                self.sm_recover(&id);
            }
        }
        if photos > 0 {
            ui.add_space(4.0);
            let ctx = ui.ctx().clone();
            let mut loaded = 0;
            let mut pending = false;
            ui.horizontal_wrapped(|ui| {
                for s in &sources {
                    let have = self.sm_thumbs.contains_key(&(id.clone(), s.clone()));
                    if !have && loaded >= THUMBS_PER_FRAME {
                        pending = true;
                        let (rr, _) = ui.allocate_exact_size(Vec2::new(64.0, 38.0), Sense::hover());
                        ui.painter().rect_filled(rr, 3.0, UI0);
                        continue;
                    }
                    if !have {
                        loaded += 1;
                    }
                    let (rr, resp) = ui.allocate_exact_size(Vec2::new(64.0, 38.0), Sense::hover());
                    match self.sm_thumb(&ctx, &id, s) {
                        Some(t) => {
                            ui.painter().image(t.id(), rr, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                        }
                        None => {
                            ui.painter().rect_filled(rr, 3.0, UI0);
                            ui.painter().text(rr.center(), Align2::CENTER_CENTER, "?", FontId::proportional(14.0), ACCENT2);
                        }
                    }
                    resp.on_hover_text(s.as_str());
                }
            });
            if pending {
                ctx.request_repaint();
            }
            if photos > sources.len() {
                muted(ui, &format!("… y {} más", photos - sources.len()));
            }
        }
        muted(ui, "Las fotos son recursos originales: reordenar, repetir o cambiar su exposición nunca las modifica. Ver la secuencia en la escena y exportarla llega en la Fase 2 (#60).");
    }
}
