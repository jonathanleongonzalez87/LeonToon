//! Red de seguridad: escritura segura de archivos, copia automática en segundo plano,
//! detección de cierres inesperados, guardado de emergencia y registro de errores.

use super::*;
use std::io::Write as _;
use std::sync::Mutex;
use std::thread::JoinHandle;

/// Cada cuántos segundos se hace la copia automática si hay cambios.
pub(super) const AUTOSAVE_SECS: f64 = 30.0;

/// Último error interno, para enseñarlo en la ventana de aviso.
static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))?;
    Some(base.join("LeonToon"))
}

fn lock_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("sesion.lock"))
}

fn log_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("errores.log"))
}

fn stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Fecha y hora UTC sencillas, sin dependencias.
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3600, rem % 3600 / 60, rem % 60);
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if mo <= 2 { 1 } else { 0 };
    format!("{y:04}-{mo:02}-{d:02}_{h:02}-{m:02}-{s:02}")
}

pub fn log_line(msg: &str) {
    let Some(p) = log_path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Que el registro no crezca sin límite.
    if std::fs::metadata(&p).map_or(false, |m| m.len() > 512 * 1024) {
        let _ = std::fs::rename(&p, p.with_extension("log.old"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "[{}] {}", stamp(), msg);
    }
}

/// Anota cualquier fallo interno en el registro de errores (además del aviso normal).
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "error desconocido".into());
        let place = info.location().map(|l| format!(" ({}:{})", l.file(), l.line())).unwrap_or_default();
        let text = format!("{msg}{place}");
        log_line(&format!("Error interno: {text}"));
        if let Ok(mut g) = LAST_PANIC.lock() {
            *g = Some(text);
        }
        default(info);
    }));
}

/// Escribe el proyecto de forma segura: archivo temporal, comprobación, copia de la
/// versión anterior (.bak) y sustitución final. Si algo falla, el archivo anterior queda intacto.
pub fn write_project(p: &Project, path: &Path, backup: bool) -> Result<(), String> {
    // Siempre se guarda la composición principal, aunque se esté editando una precomposición.
    let root;
    let p = if p.in_comp() {
        root = p.root_clone();
        &root
    } else {
        p
    };
    let json = serde_json::to_vec(p).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
    }
    let tmp = path.with_extension("ltoon.tmp");
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(&json).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    // Comprobar que lo escrito se puede volver a leer.
    let back = std::fs::read(&tmp).map_err(|e| e.to_string())?;
    if back != json || serde_json::from_slice::<Project>(&back).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err("el archivo escrito no se pudo comprobar (¿disco lleno?)".into());
    }
    if backup && path.exists() {
        let _ = std::fs::copy(path, path.with_extension("ltoon.bak"));
    }
    if std::fs::rename(&tmp, path).is_err() {
        // Algunas carpetas sincronizadas (OneDrive…) bloquean un momento el archivo: copiar en su lugar.
        std::fs::copy(&tmp, path).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(())
}

pub fn read_project(path: &Path) -> Result<Project, String> {
    let s = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut proj: Project = serde_json::from_slice(&s).map_err(|e| format!("el archivo no es un proyecto válido ({e})"))?;
    proj.sanitize();
    Ok(proj)
}

pub(super) struct Safety {
    /// Copia automática que se está escribiendo en segundo plano.
    job: Option<JoinHandle<Result<(), String>>>,
    /// Había una copia automática de una sesión que no se cerró bien.
    pub offer_recovery: bool,
    /// Texto del último fallo interno, para la ventana de aviso.
    pub crash: Option<String>,
    pub crash_saved: Option<PathBuf>,
    last_rescue: f64,
    owns_lock: bool,
}

impl Safety {
    pub fn start(active: bool) -> Self {
        let mut s = Safety { job: None, offer_recovery: false, crash: None, crash_saved: None, last_rescue: -1e9, owns_lock: false };
        let active = active || std::env::var_os("LEONTOON_PRUEBA_FALLO").is_some();
        if !active {
            return s;
        }
        if let (Some(lock), Some(auto)) = (lock_path(), LeonApp::autosave_path()) {
            let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
            if let (Some(l), Some(a)) = (mtime(&lock), mtime(&auto)) {
                // La copia se hizo durante una sesión que no llegó a cerrarse bien.
                s.offer_recovery = a >= l;
                log_line("La sesión anterior no se cerró correctamente");
            }
            if let Some(dir) = lock.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            s.owns_lock = std::fs::write(&lock, std::process::id().to_string()).is_ok();
        }
        s
    }

    pub fn busy(&self) -> bool {
        self.job.as_ref().map_or(false, |j| !j.is_finished())
    }

    /// Recoge el resultado de la copia en segundo plano, si ya terminó.
    pub fn poll(&mut self) -> Option<Result<(), String>> {
        if self.job.as_ref().map_or(false, |j| j.is_finished()) {
            let j = self.job.take()?;
            return Some(j.join().unwrap_or_else(|_| Err("fallo interno al escribir".into())));
        }
        None
    }

    pub fn spawn(&mut self, project: Project, path: PathBuf) {
        self.job = Some(std::thread::spawn(move || write_project(&project, &path, false)));
    }

    pub fn finish(&mut self) {
        if let Some(j) = self.job.take() {
            let _ = j.join();
        }
        if self.owns_lock {
            if let Some(l) = lock_path() {
                let _ = std::fs::remove_file(l);
            }
            self.owns_lock = false;
        }
    }
}

impl Drop for LeonApp {
    fn drop(&mut self) {
        self.safety.finish();
    }
}

impl LeonApp {
    pub(super) fn autosave(&mut self) {
        if self.shot.is_some() {
            return;
        }
        if let Some(res) = self.safety.poll() {
            if let Err(e) = res {
                log_line(&format!("Copia automática fallida: {e}"));
                self.say(format!("No se pudo hacer la copia automática: {e}"));
            }
        }
        if self.dirty && self.gesture.is_none() && !self.safety.busy() && self.now - self.last_autosave > AUTOSAVE_SECS {
            self.last_autosave = self.now;
            if let Some(p) = Self::autosave_path() {
                // Clonar es barato: los dibujos se comparten.
                self.safety.spawn(self.project.clone(), p);
            }
        }
    }

    /// Llamado cuando algo falla dentro de un fotograma: se guarda todo y se sigue trabajando.
    pub(super) fn recover_from_panic(&mut self, ctx: &egui::Context) {
        let msg = LAST_PANIC.lock().ok().and_then(|mut g| g.take()).unwrap_or_else(|| "error desconocido".into());
        // Volver a un estado tranquilo.
        if let Some(pre) = self.pre.take() {
            if self.gesture.is_some() {
                self.project = pre;
            }
        }
        self.gesture = None;
        self.changed = false;
        self.playing = false;
        self.renaming = None;
        self.pending = None;
        self.show_new = false;
        self.project.sanitize();
        self.cl = self.cl.min(self.project.layers.len() - 1);
        self.frame = self.frame.min(self.project.length - 1);
        // Guardado de emergencia (como mucho uno cada 10 s, por si el fallo se repite).
        if self.now - self.safety.last_rescue > 10.0 {
            self.safety.last_rescue = self.now;
            if let Some(dir) = data_dir() {
                let p = dir.join(format!("rescate_{}.ltoon", stamp()));
                match write_project(&self.project, &p, false) {
                    Ok(()) => {
                        log_line(&format!("Proyecto guardado de emergencia en {}", p.display()));
                        self.safety.crash_saved = Some(p);
                    }
                    Err(e) => log_line(&format!("No se pudo guardar de emergencia: {e}")),
                }
            }
        }
        self.safety.crash = Some(msg);
        ctx.request_repaint();
    }

    pub(super) fn safety_dialogs(&mut self, ctx: &egui::Context) {
        if self.safety.offer_recovery {
            let mut choice = None;
            egui::Window::new("Recuperar tu trabajo")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("La última vez LeonToon se cerró de forma inesperada.");
                    ui.label("Hay una copia automática con tu trabajo más reciente.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Recuperar").strong()).clicked() {
                            choice = Some(true);
                        }
                        if ui.button("Empezar sin recuperar").clicked() {
                            choice = Some(false);
                        }
                    });
                    ui.label(
                        RichText::new("Puedes recuperarla más tarde en Archivo > Recuperar copia automática.")
                            .size(11.5)
                            .color(Color32::from_gray(160)),
                    );
                });
            if let Some(c) = choice {
                self.safety.offer_recovery = false;
                if c {
                    self.perform(ctx, Pending::Recover);
                }
            }
        }
        if let Some(msg) = self.safety.crash.clone() {
            let mut close = false;
            egui::Window::new("Algo salió mal")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("LeonToon encontró un error interno, pero tu trabajo está a salvo.");
                    if let Some(p) = &self.safety.crash_saved {
                        ui.label(format!("Se guardó una copia de emergencia en:\n{}", p.display()));
                    }
                    ui.label("Puedes seguir trabajando. Te recomendamos guardar ahora (Ctrl+S).");
                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("Detalle: {msg}")).size(11.0).color(Color32::from_gray(150)));
                    ui.add_space(6.0);
                    if ui.button(RichText::new("Entendido").strong()).clicked() {
                        close = true;
                    }
                });
            if close {
                self.safety.crash = None;
            }
        }
    }

    /// Abre la versión anterior (.bak) del proyecto actual.
    pub(super) fn open_backup(&mut self) {
        let Some(path) = self.path.clone() else {
            self.say("Primero guarda o abre un proyecto");
            return;
        };
        let bak = path.with_extension("ltoon.bak");
        if !bak.exists() {
            self.say("Todavía no hay versión anterior de este proyecto");
            return;
        }
        match read_project(&bak) {
            Ok(p) => {
                self.checkpoint();
                self.project = p;
                self.clamp_state();
                self.dirty = true;
                self.say("Versión anterior cargada. Ctrl+Z para volver, Ctrl+S para quedártela.");
            }
            Err(e) => self.say(format!("No se pudo abrir la versión anterior: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_write_keeps_previous_version() {
        let dir = std::env::temp_dir().join(format!("leontoon_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("prueba.ltoon");
        let mut p = Project::demo();
        write_project(&p, &path, true).unwrap();
        p.fps = 12;
        write_project(&p, &path, true).unwrap();
        assert_eq!(read_project(&path).unwrap().fps, 12);
        assert_eq!(read_project(&path.with_extension("ltoon.bak")).unwrap().fps, 24);
        assert!(!path.with_extension("ltoon.tmp").exists());
        std::fs::write(dir.join("roto.ltoon"), b"{ esto no es un proyecto").unwrap();
        assert!(read_project(&dir.join("roto.ltoon")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stamp_looks_like_a_date() {
        let s = stamp();
        assert_eq!(s.len(), 19);
        assert!(s.starts_with("20"));
    }
}