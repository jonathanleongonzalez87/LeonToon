//! Exportaciones en segundo plano (Issue #155): el programa sigue respondiendo mientras se exporta, el panel muestra una barra con el avance
//! y la fase, se puede cancelar, y al terminar dice con claridad si salió bien o mal, con el archivo, su tamaño, lo que tardó y botones
//! para abrir la carpeta o el archivo.

use super::shell::{muted, pill};
use super::*;
use crate::audio;
use crate::render::{self, Progress};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Qué se exporta y adónde.
pub(super) enum ExportKind {
    Png { path: PathBuf },
    Sequence { dir: PathBuf },
    Sheet { path: PathBuf, cols: u32 },
    Gif { path: PathBuf },
    /// Vídeo MP4 con ffmpeg; con audios, su mezcla se une al vídeo.
    Mp4 { path: PathBuf, ffmpeg: PathBuf, audio: Option<(Vec<audio::Item>, audio::Cache)> },
}

/// Cómo terminó una exportación.
#[derive(Clone)]
pub(super) struct Outcome {
    pub ok: bool,
    pub title: String,
    pub detail: String,
    /// El archivo o la carpeta que se creó (para abrirla).
    pub path: Option<PathBuf>,
}

/// Una exportación en marcha: su avance compartido con el hilo que trabaja y el canal por el que llega el resultado.
pub(super) struct Job {
    label: String,
    progress: Arc<Progress>,
    started: Instant,
    rx: mpsc::Receiver<Outcome>,
}

#[derive(Default)]
pub(super) enum ExportState {
    #[default]
    Idle,
    Running(Job),
    Done(Outcome),
}

fn human_size(bytes: u64) -> String {
    if bytes >= 1 << 20 {
        format!("{:.1} MB", bytes as f64 / (1u64 << 20) as f64)
    } else {
        format!("{:.0} KB", (bytes as f64 / 1024.0).max(1.0))
    }
}

fn file_size(p: &Path) -> u64 {
    if p.is_dir() {
        std::fs::read_dir(p).map(|rd| rd.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()).unwrap_or(0)
    } else {
        std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
    }
}

/// Hace la exportación (en el hilo de trabajo) y devuelve su resultado.
fn run(kind: ExportKind, root: &Project, a: u32, b: u32, scale: f32, transparent: bool, progress: &Progress) -> Outcome {
    let started = Instant::now();
    let fps = root.fps.max(1) as f32;
    let frames = b.saturating_sub(a) + 1;
    // El archivo (o la carpeta) que sale y lo que se cuenta al terminar.
    let (target, result): (PathBuf, Result<(String, String), String>) = match kind {
        ExportKind::Png { path } => {
            progress.set_phase("Dibujando el fotograma");
            let r = render::export_png_opts(root, a, scale, transparent, &path).map(|_| ("Imagen PNG guardada".to_string(), format!("fotograma {}", a + 1)));
            (path, r)
        }
        ExportKind::Sequence { dir } => {
            let r = render::export_sequence_range(root, &dir, a, b, scale, transparent, Some(progress)).map(|n| (format!("Secuencia PNG guardada: {n} imágenes"), format!("fotogramas {} a {}", a + 1, b + 1)));
            (dir, r)
        }
        ExportKind::Sheet { path, cols } => {
            let r = render::export_sprite_sheet(root, &path, a, b, scale, cols, transparent, Some(progress)).map(|n| (format!("Hoja de sprites guardada: {n} fotogramas"), "con su archivo .json".to_string()));
            (path, r)
        }
        ExportKind::Gif { path } => {
            let maxw = (root.width as f32 * scale).round() as u32;
            let r = render::export_gif_range(root, &path, a, b, maxw.min(1920), Some(progress)).map(|_| (format!("GIF animado guardado: {frames} fotogramas"), format!("{:.1} s de animación", frames as f32 / fps)));
            (path, r)
        }
        ExportKind::Mp4 { path, ffmpeg, audio } => {
            let wav = std::env::temp_dir().join(format!("leontoon_audio_{}_{}.wav", std::process::id(), started.elapsed().as_nanos()));
            let mut with_audio = false;
            let mut audio_err = None;
            if let Some((items, cache)) = &audio {
                progress.set_phase("Mezclando el audio");
                let m = audio::mix(items, cache, 48_000, a as f32 / fps, (frames as f32 / fps).min(480.0));
                match audio::write_wav(&wav, 48_000, &m) {
                    Ok(()) => with_audio = true,
                    Err(e) => audio_err = Some(e),
                }
            }
            let r = match audio_err {
                Some(e) => Err(format!("No se pudo preparar el audio: {e}")),
                None => render::export_video(root, &ffmpeg, &path, a, b, scale, with_audio.then_some(wav.as_path()), Some(progress)).map(|n| {
                    (format!("Vídeo MP4 listo: {n} fotogramas{}", if with_audio { " con sonido" } else { "" }), format!("{:.1} s de vídeo", n as f32 / fps))
                }),
            };
            let _ = std::fs::remove_file(&wav);
            (path, r)
        }
    };
    let secs = started.elapsed().as_secs_f32();
    match result {
        Ok((title, what)) => Outcome { ok: true, title, detail: format!("{what} · {} · tardó {secs:.1} s", human_size(file_size(&target))), path: Some(target) },
        Err(e) => {
            // Un archivo a medias no sirve: se quita (las carpetas de fotogramas se dejan, por si algo de lo hecho sirve).
            if target.is_file() {
                let _ = std::fs::remove_file(&target);
            }
            let cancelled = progress.cancelled();
            Outcome {
                ok: false,
                title: if cancelled { "Exportación cancelada".into() } else { "No se pudo exportar".into() },
                detail: if cancelled { "No se guardó nada.".into() } else { format!("{e}. Lo que ya estaba en el proyecto no se tocó.") },
                path: None,
            }
        }
    }
}

/// Abre el archivo con el programa por defecto, o la carpeta (seleccionando el archivo) en el explorador.
fn open_path(p: &Path, reveal: bool) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = if reveal && p.is_file() {
            let mut c = std::process::Command::new("explorer");
            c.arg(format!("/select,{}", p.display()));
            c
        } else if reveal {
            let mut c = std::process::Command::new("explorer");
            c.arg(p);
            c
        } else {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", "start", ""]).arg(p);
            c.creation_flags(0x0800_0000);
            c
        };
        let _ = cmd.spawn();
    }
    #[cfg(not(windows))]
    {
        let target = if reveal && p.is_file() { p.parent().unwrap_or(p) } else { p };
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }
}

impl LeonApp {
    /// Empieza una exportación en segundo plano de los cuadros `a` a `b` (desde 0) de la composición principal.
    pub(super) fn start_export(&mut self, kind: ExportKind, a: u32, b: u32, scale: f32, transparent: bool) {
        if matches!(self.export, ExportState::Running(_)) {
            return;
        }
        let root = self.project.root_clone();
        let b = b.min(root.length.saturating_sub(1));
        let a = a.min(b);
        let label = match &kind {
            ExportKind::Png { .. } => "Exportando el fotograma",
            ExportKind::Sequence { .. } => "Exportando la secuencia PNG",
            ExportKind::Sheet { .. } => "Exportando la hoja de sprites",
            ExportKind::Gif { .. } => "Exportando el GIF",
            ExportKind::Mp4 { .. } => "Exportando el vídeo MP4",
        }
        .to_string();
        let progress = Arc::new(Progress::default());
        progress.set_phase("Preparando");
        let (tx, rx) = mpsc::channel();
        let p2 = progress.clone();
        std::thread::spawn(move || {
            let outcome = run(kind, &root, a, b, scale, transparent, &p2);
            let _ = tx.send(outcome);
        });
        self.export = ExportState::Running(Job { label, progress, started: Instant::now(), rx });
    }

    /// Anota un resultado sin haber exportado en segundo plano (por ejemplo, el PDF o un aviso de que falta ffmpeg).
    pub(super) fn finish_export(&mut self, ok: bool, title: impl Into<String>, detail: impl Into<String>, path: Option<PathBuf>) {
        let o = Outcome { ok, title: title.into(), detail: detail.into(), path };
        self.say(format!("{} {}", o.title, o.detail));
        self.export = ExportState::Done(o);
    }

    /// Recoge el resultado de la exportación en marcha, si ya terminó.
    pub(super) fn poll_export(&mut self, ctx: &egui::Context) {
        let ExportState::Running(job) = &self.export else { return };
        match job.rx.try_recv() {
            Ok(o) => {
                self.say(format!("{} {}", o.title, o.detail));
                self.export = ExportState::Done(o);
            }
            Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.export = ExportState::Done(Outcome { ok: false, title: "No se pudo exportar".into(), detail: "El trabajo se interrumpió de forma inesperada.".into(), path: None });
            }
        }
    }

    /// La barra de avance de la exportación en marcha o el resultado de la última, en el panel de exportación.
    pub(super) fn export_status_ui(&mut self, ui: &mut egui::Ui) {
        let mut dismiss = false;
        let mut cancel = false;
        match &self.export {
            ExportState::Idle => {
                muted(ui, "Elige un formato: aquí verás el avance de la exportación y, al terminar, si salió bien o no.");
            }
            ExportState::Running(job) => {
                egui::Frame::default().fill(BG3).corner_radius(6.0).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(&job.label).strong().color(TX));
                    let frac = job.progress.get();
                    ui.add(egui::ProgressBar::new(frac).show_percentage().animate(true).desired_height(16.0));
                    ui.horizontal(|ui| {
                        muted(ui, format!("{} · {:.0} s", job.progress.phase(), job.started.elapsed().as_secs_f32()));
                        if pill(ui, "Cancelar", false).on_hover_text("Detiene la exportación y no guarda el archivo").clicked() {
                            cancel = true;
                        }
                    });
                });
            }
            ExportState::Done(o) => {
                let (col, mark) = if o.ok { (Color32::from_rgb(0x5c, 0xc2, 0x7a), "✔") } else { (Color32::from_rgb(0xe0, 0x58, 0x48), "✖") };
                egui::Frame::default().fill(BG3).corner_radius(6.0).stroke(stk(1.5, col)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(format!("{mark} {}", o.title)).strong().size(14.0).color(col));
                    ui.label(RichText::new(&o.detail).color(TX));
                    if let Some(p) = &o.path {
                        ui.label(RichText::new(p.display().to_string()).size(11.0).color(MU));
                    }
                    ui.horizontal(|ui| {
                        if let Some(p) = &o.path {
                            if pill(ui, "Abrir carpeta", false).clicked() {
                                open_path(p, true);
                            }
                            if p.is_file() && pill(ui, "Abrir archivo", false).clicked() {
                                open_path(p, false);
                            }
                        }
                        if pill(ui, "Cerrar", false).clicked() {
                            dismiss = true;
                        }
                    });
                });
            }
        }
        if cancel {
            if let ExportState::Running(job) = &self.export {
                job.progress.cancel();
                job.progress.set_phase("Cancelando…");
            }
        }
        if dismiss {
            self.export = ExportState::Idle;
        }
    }

    /// Para capturas de pantalla: pone el panel en un estado de ejemplo.
    pub(super) fn demo_export_state(&mut self, which: &str) {
        match which {
            "progreso" => {
                let progress = Arc::new(Progress::default());
                progress.set(0.43);
                progress.set_phase("Dibujando los fotogramas");
                let (_tx, rx) = mpsc::channel();
                // El emisor se deja vivo (no se envía nada): el panel enseña la barra a medias.
                std::mem::forget(_tx);
                self.export = ExportState::Running(Job { label: "Exportando el vídeo MP4".into(), progress, started: Instant::now() - Duration::from_secs(7), rx });
            }
            "error" => self.export = ExportState::Done(Outcome { ok: false, title: "Falta ffmpeg para exportar vídeo".into(), detail: "ffmpeg es gratuito (ffmpeg.org). Instálalo con «winget install Gyan.FFmpeg» y vuelve a abrir LeonToon, o indica dónde está ffmpeg.exe.".into(), path: None }),
            _ => self.export = ExportState::Done(Outcome { ok: true, title: "Vídeo MP4 listo: 252 fotogramas con sonido".into(), detail: "10.5 s de vídeo · 1.8 MB · tardó 9.4 s".into(), path: Some(std::path::PathBuf::from("C:\\Temp\\leontoon_demo\\mi_animacion.mp4")) }),
        }
    }

    /// ¿Hay una exportación en marcha? (Los botones de exportar se desactivan mientras tanto.)
    pub(super) fn exporting(&self) -> bool {
        matches!(self.export, ExportState::Running(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("leontoon_export_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_finished_export_reports_what_it_made_how_big_and_how_long_and_the_bar_reaches_the_end() {
        let dir = temp("ok");
        let p = Project::demo();
        let progress = Progress::default();
        let o = run(ExportKind::Sequence { dir: dir.clone() }, &p, 0, 3, 0.1, false, &progress);
        assert!(o.ok, "{}: {}", o.title, o.detail);
        assert!(o.title.contains("4 imágenes"), "{}", o.title);
        assert!(o.detail.contains("fotogramas 1 a 4") && o.detail.contains("tardó"), "{}", o.detail);
        assert_eq!(o.path.as_deref(), Some(dir.as_path()));
        assert_eq!(progress.get(), 1.0, "la barra llega al final");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancelling_stops_the_export_leaves_no_file_and_says_so() {
        let dir = temp("cancel");
        let p = Project::demo();
        let progress = Progress::default();
        progress.cancel();
        let gif = dir.join("a.gif");
        let o = run(ExportKind::Gif { path: gif.clone() }, &p, 0, 20, 0.1, false, &progress);
        assert!(!o.ok);
        assert_eq!(o.title, "Exportación cancelada");
        assert!(o.detail.contains("No se guardó nada"));
        assert!(!gif.exists(), "un archivo a medias se borra");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_export_explains_the_problem_and_a_missing_ffmpeg_is_reported_as_such() {
        let dir = temp("fail");
        let p = Project::demo();
        let progress = Progress::default();
        // Una carpeta que no existe: la exportación falla con un mensaje, sin entrar en pánico.
        let o = run(ExportKind::Sequence { dir: dir.join("no").join("existe") }, &p, 0, 1, 0.1, false, &progress);
        assert!(!o.ok);
        assert_eq!(o.title, "No se pudo exportar");
        assert!(o.detail.contains("no se tocó"), "{}", o.detail);
        // ffmpeg que no existe: el vídeo falla con un mensaje claro y los fotogramas temporales se limpian.
        let mp4 = dir.join("v.mp4");
        let o = run(ExportKind::Mp4 { path: mp4.clone(), ffmpeg: dir.join("no_hay_ffmpeg.exe"), audio: None }, &p, 0, 2, 0.1, false, &Progress::default());
        assert!(!o.ok && o.detail.contains("ffmpeg"), "{}", o.detail);
        assert!(!mp4.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn progress_reports_phase_and_fraction_between_threads() {
        let pr = Arc::new(Progress::default());
        pr.set_phase("Codificando");
        pr.set(0.5);
        let p2 = pr.clone();
        let h = std::thread::spawn(move || (p2.get(), p2.phase()));
        assert_eq!(h.join().unwrap(), (0.5, "Codificando".to_string()));
        pr.set(7.0);
        assert_eq!(pr.get(), 1.0, "se limita a 1");
        assert!(!pr.cancelled());
        pr.cancel();
        assert!(pr.cancelled());
    }

    #[test]
    fn the_real_background_pipeline_finishes_with_a_result_and_a_full_bar() {
        let dir = temp("hilo");
        let p = Project::demo();
        let progress = Arc::new(Progress::default());
        let (tx, rx) = mpsc::channel();
        let (p2, d2) = (progress.clone(), dir.clone());
        std::thread::spawn(move || {
            let o = run(ExportKind::Sheet { path: d2.join("s.png"), cols: 2 }, &p, 0, 3, 0.1, false, &p2);
            let _ = tx.send(o);
        });
        let o = rx.recv_timeout(Duration::from_secs(60)).expect("la exportación termina");
        assert!(o.ok, "{}", o.detail);
        assert!(o.title.contains("4 fotogramas"));
        assert!(dir.join("s.png").exists() && dir.join("s.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
