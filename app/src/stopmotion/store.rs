//! Almacenamiento en disco de una secuencia Stop Motion: la carpeta elegida por la persona, la escritura segura de
//! fotos y de `sequence.json`, la verificación de integridad, la recuperación de fotos huérfanas y las miniaturas.
//!
//! Reglas que se cumplen aquí:
//! - Una foto se escribe a un archivo temporal, se vuelve a leer y se comprueba, y solo entonces se renombra y se
//!   registra. Un fallo nunca deja una referencia válida a un archivo incompleto.
//! - Los originales no se modifican ni se renombran, y su número no se reutiliza.
//! - `sequence.json` se escribe de forma atómica y conserva la versión anterior (`.bak`).

use super::*;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

/// Una secuencia abierta: su carpeta y sus datos.
#[derive(Clone, Debug)]
pub struct SeqFolder {
    pub dir: PathBuf,
    pub seq: Sequence,
}

/// Cómo se abrió la secuencia.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OpenNote {
    Ok,
    /// `sequence.json` estaba dañado: se abrió la copia anterior (`.bak`).
    FromBackup,
}

/// Resultado de añadir una foto.
#[derive(Clone, Debug, PartialEq)]
pub struct AddOutcome {
    pub id: String,
    /// La foto ya estaba registrada (mismo contenido): se reutiliza su archivo, no se copia otra vez.
    pub reused: bool,
}

/// Qué pasó al importar varias fotos.
#[derive(Clone, Debug, Default)]
pub struct ImportReport {
    pub added: Vec<(PathBuf, AddOutcome)>,
    pub failed: Vec<(PathBuf, String)>,
}

/// Problema encontrado al verificar la secuencia.
#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    Missing { source: String, file: String },
    Damaged { source: String, file: String, reason: String },
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("sha256:{}", hex(&Sha256::digest(bytes)))
}

/// Formato, ancho y alto de una foto JPEG o PNG, comprobando que el archivo está completo (no cortado a la mitad).
pub fn probe_image(bytes: &[u8]) -> Result<(&'static str, u32, u32), String> {
    let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| e.to_string())?;
    let (format, complete) = match reader.format() {
        Some(image::ImageFormat::Jpeg) => ("jpeg", ends_like_jpeg(bytes)),
        Some(image::ImageFormat::Png) => ("png", ends_like_png(bytes)),
        _ => return Err("no es una foto JPEG ni PNG".into()),
    };
    if !complete {
        return Err("la foto está incompleta (el archivo termina antes de tiempo)".into());
    }
    let (w, h) = reader.into_dimensions().map_err(|e| format!("no se pudo leer la foto: {e}"))?;
    if w == 0 || h == 0 {
        return Err("la foto no tiene tamaño".into());
    }
    Ok((format, w, h))
}

fn ends_like_jpeg(b: &[u8]) -> bool {
    // El JPEG termina en FF D9 (algunas cámaras añaden ceros después).
    let end = b.iter().rposition(|&x| x != 0).map_or(0, |p| p + 1);
    end >= 2 && b[end - 2] == 0xFF && b[end - 1] == 0xD9
}

fn ends_like_png(b: &[u8]) -> bool {
    // El PNG termina con el fragmento IEND (12 bytes: longitud, «IEND» y CRC).
    b.len() >= 12 && &b[b.len() - 8..b.len() - 4] == b"IEND"
}

/// Datos EXIF que se muestran. Devuelve `None` si la foto no trae EXIF.
pub fn read_exif(bytes: &[u8]) -> Option<ExifInfo> {
    let ex = exif::Reader::new().read_from_container(&mut Cursor::new(bytes)).ok()?;
    let text = |tag: exif::Tag| {
        ex.get_field(tag, exif::In::PRIMARY).and_then(|f| match &f.value {
            exif::Value::Ascii(v) => v.first().map(|s| String::from_utf8_lossy(s).trim_matches(char::from(0)).trim().to_string()),
            _ => None,
        })
    };
    let date_time = text(exif::Tag::DateTimeOriginal).or_else(|| text(exif::Tag::DateTime)).and_then(|d| {
        // «2026:11:22 20:14:03» → «2026-11-22T20:14:03»
        let (date, time) = d.split_once(' ')?;
        Some(format!("{}T{}", date.replace(':', "-"), time))
    });
    let info = ExifInfo { make: text(exif::Tag::Make).filter(|s| !s.is_empty()), model: text(exif::Tag::Model).filter(|s| !s.is_empty()), date_time };
    (info != ExifInfo::default()).then_some(info)
}

/// Escribe un archivo de forma segura: temporal, comprobación y renombrado. No pisa un archivo que ya existe.
fn write_verified(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        return Err(format!("ya existe un archivo con ese nombre ({})", path.display()));
    }
    let tmp = path.with_extension("tmp");
    let result = (|| {
        let mut f = std::fs::File::create(&tmp).map_err(|e| format!("no se pudo crear el archivo temporal: {e}"))?;
        f.write_all(bytes).map_err(|e| format!("no se pudo escribir la foto: {e}"))?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        let back = std::fs::read(&tmp).map_err(|e| e.to_string())?;
        if back != bytes {
            return Err("lo escrito no coincide con la foto (¿disco lleno?)".to_string());
        }
        std::fs::rename(&tmp, path).map_err(|e| format!("no se pudo guardar la foto: {e}"))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn number_in_name(name: &str) -> Option<u32> {
    name.strip_prefix("frame_")?.split('.').next()?.parse().ok()
}

impl SeqFolder {
    /// Crea una secuencia nueva en `dir` (la carpeta se crea si no existe). No pisa una secuencia que ya esté ahí.
    pub fn create(dir: &Path, name: &str, fps: u32) -> Result<SeqFolder, String> {
        if dir.join(SEQUENCE_FILE).exists() {
            return Err("Esa carpeta ya tiene una secuencia Stop Motion: ábrela en lugar de crear otra".into());
        }
        for sub in [SOURCES, THUMBNAILS, PROXIES, JOURNAL] {
            std::fs::create_dir_all(dir.join(sub)).map_err(|e| format!("no se pudo crear la carpeta «{sub}»: {e}"))?;
        }
        let f = SeqFolder { dir: dir.to_path_buf(), seq: Sequence::new(name, fps) };
        f.save()?;
        f.log("Secuencia creada");
        Ok(f)
    }

    /// Abre la secuencia de `dir`. Si `sequence.json` está dañado, intenta con la copia anterior.
    pub fn open(dir: &Path) -> Result<(SeqFolder, OpenNote), String> {
        let path = dir.join(SEQUENCE_FILE);
        if !path.exists() {
            return Err("No hay una secuencia Stop Motion en esa carpeta (falta sequence.json)".into());
        }
        let read = |p: &Path| -> Result<Sequence, String> {
            let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
            let mut s: Sequence = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if s.format != FORMAT {
                return Err("el archivo no es una secuencia de LeonToon".into());
            }
            if s.version > VERSION {
                return Err(format!("la secuencia es de una versión más nueva ({}): actualiza LeonToon para abrirla", s.version));
            }
            if s.id.is_empty() {
                return Err("a la secuencia le falta su identificador".into());
            }
            s.sanitize();
            Ok(s)
        };
        match read(&path) {
            Ok(seq) => Ok((SeqFolder { dir: dir.to_path_buf(), seq }, OpenNote::Ok)),
            // Una versión más nueva no se «arregla» con la copia vieja: se dice tal cual y no se toca nada.
            Err(e) if e.contains("versión más nueva") => Err(e),
            Err(e) => match read(&path.with_extension("json.bak")) {
                Ok(seq) => Ok((SeqFolder { dir: dir.to_path_buf(), seq }, OpenNote::FromBackup)),
                Err(_) => Err(format!("El archivo sequence.json está dañado y no hay copia anterior que sirva ({e})")),
            },
        }
    }

    /// Guarda `sequence.json` de forma atómica y conserva la versión anterior como `.bak`.
    pub fn save(&self) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(&self.seq).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let path = self.dir.join(SEQUENCE_FILE);
        let tmp = path.with_extension("json.tmp");
        {
            let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
            f.write_all(&json).map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
        }
        let back = std::fs::read(&tmp).map_err(|e| e.to_string())?;
        if back != json || serde_json::from_slice::<Sequence>(&back).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return Err("sequence.json no se pudo comprobar tras escribirlo (¿disco lleno?)".into());
        }
        if path.exists() {
            let _ = std::fs::copy(&path, path.with_extension("json.bak"));
        }
        if std::fs::rename(&tmp, &path).is_err() {
            // Carpetas sincronizadas (OneDrive…) a veces bloquean el archivo un momento.
            std::fs::copy(&tmp, &path).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(&tmp);
        }
        Ok(())
    }

    /// Anota una línea en el diario de la carpeta (si no se puede, no pasa nada).
    pub fn log(&self, msg: &str) {
        let dir = self.dir.join(JOURNAL);
        if std::fs::create_dir_all(&dir).is_ok() {
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("registro.txt")) {
                let _ = writeln!(f, "[{}] {msg}", iso_from_time(std::time::SystemTime::now()));
            }
        }
    }

    pub fn sources_dir(&self) -> PathBuf {
        self.dir.join(SOURCES)
    }

    /// Siguiente número de foto libre: supera al contador, a los ids registrados y a cualquier archivo que ya esté en `Sources/`.
    fn next_number(&self) -> u32 {
        let on_disk = std::fs::read_dir(self.sources_dir()).into_iter().flatten().flatten().filter_map(|e| number_in_name(&e.file_name().to_string_lossy())).max().unwrap_or(0);
        self.seq.next_source.max(on_disk + 1)
    }

    /// Registra una foto: la valida, calcula su hash, la guarda en `Sources/` de forma segura y la añade a la lista de
    /// recursos. **No** la pone en la línea de tiempo ni guarda `sequence.json` (eso lo hace quien llama).
    pub fn add_photo_bytes(&mut self, bytes: &[u8], device: Option<Device>, captured: Option<std::time::SystemTime>) -> Result<AddOutcome, String> {
        let (format, width, height) = probe_image(bytes)?;
        let hash = sha256_hex(bytes);
        if let Some(s) = self.seq.sources.iter().find(|s| s.hash == hash && self.dir.join(&s.file).is_file()) {
            return Ok(AddOutcome { id: s.id.clone(), reused: true });
        }
        std::fs::create_dir_all(self.sources_dir()).map_err(|e| e.to_string())?;
        let n = self.next_number();
        let ext = if format == "jpeg" { "jpg" } else { "png" };
        let file = format!("{SOURCES}/frame_{n:06}.{ext}");
        write_verified(&self.dir.join(&file), bytes)?;
        let exif = read_exif(bytes);
        let captured_at = exif.as_ref().and_then(|e| e.date_time.clone()).or_else(|| captured.map(iso_from_time));
        let id = format!("s{n:06}");
        self.seq.next_source = n + 1;
        self.seq.sources.push(SourceFrame { id: id.clone(), file, width, height, format: format.into(), bytes: bytes.len() as u64, hash, captured_at, device, exif });
        Ok(AddOutcome { id, reused: false })
    }

    /// Registra una foto, la añade al final de la secuencia (con la exposición predeterminada) y guarda. Es lo que usará la captura.
    pub fn append_photo(&mut self, bytes: &[u8], device: Option<Device>) -> Result<AddOutcome, String> {
        let out = self.add_photo_bytes(bytes, device, Some(std::time::SystemTime::now()))?;
        let exposure = self.seq.default_exposure;
        self.seq.push_item(&out.id, exposure)?;
        self.save()?;
        Ok(out)
    }

    /// Importa fotos existentes: las **copia** a `Sources/` (el original de la persona no se toca) y las añade al final.
    /// Una foto que no se puede importar no impide las demás; el informe dice cuáles fallaron y por qué.
    pub fn import_files(&mut self, paths: &[PathBuf]) -> ImportReport {
        let mut report = ImportReport::default();
        for p in paths {
            let outcome = std::fs::read(p).map_err(|e| format!("no se pudo leer el archivo: {e}")).and_then(|bytes| {
                let modified = std::fs::metadata(p).and_then(|m| m.modified()).ok();
                self.add_photo_bytes(&bytes, None, modified)
            });
            match outcome {
                Ok(out) => {
                    let exposure = self.seq.default_exposure;
                    match self.seq.push_item(&out.id, exposure) {
                        Ok(_) => report.added.push((p.clone(), out)),
                        Err(e) => report.failed.push((p.clone(), e)),
                    }
                }
                Err(e) => report.failed.push((p.clone(), e)),
            }
        }
        if !report.added.is_empty() {
            match self.save() {
                Ok(()) => self.log(&format!("Importadas {} fotos ({} con problemas)", report.added.len(), report.failed.len())),
                Err(e) => {
                    // Las fotos están en Sources/ pero la lista no se guardó: se pueden reincorporar como huérfanas.
                    let n = report.added.len();
                    report.failed.extend(report.added.drain(..).map(|(p, _)| (p, format!("la foto se guardó pero no se pudo actualizar la secuencia: {e}"))));
                    self.log(&format!("No se pudo guardar sequence.json tras importar {n} fotos: {e}"));
                }
            }
        }
        report
    }

    /// Revisa que cada foto registrada exista y no haya cambiado. `deep` lee todo el archivo para comprobar el hash.
    pub fn verify(&self, deep: bool) -> Vec<Problem> {
        let mut out = Vec::new();
        for s in &self.seq.sources {
            let path = self.dir.join(&s.file);
            let source = s.id.clone();
            let file = s.file.clone();
            match std::fs::metadata(&path) {
                Err(_) => out.push(Problem::Missing { source, file }),
                Ok(m) if !m.is_file() => out.push(Problem::Missing { source, file }),
                Ok(m) if m.len() != s.bytes => out.push(Problem::Damaged { source, file, reason: format!("el tamaño cambió ({} bytes en vez de {})", m.len(), s.bytes) }),
                Ok(_) if deep => match std::fs::read(&path) {
                    Ok(b) if sha256_hex(&b) != s.hash => out.push(Problem::Damaged { source, file, reason: "el contenido cambió".into() }),
                    Ok(_) => {}
                    Err(e) => out.push(Problem::Damaged { source, file, reason: format!("no se pudo leer: {e}") }),
                },
                Ok(_) => {}
            }
        }
        out
    }

    /// Fotos de `Sources/` que no están registradas (por ejemplo, tras un cierre inesperado entre guardar la foto y
    /// guardar la secuencia). Ignora los archivos temporales.
    pub fn orphans(&self) -> Vec<PathBuf> {
        let known: Vec<&str> = self.seq.sources.iter().map(|s| s.file.as_str()).collect();
        let mut out: Vec<PathBuf> = std::fs::read_dir(self.sources_dir())
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .filter(|p| {
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                !name.ends_with(".tmp") && !name.starts_with('.') && !known.contains(&format!("{SOURCES}/{name}").as_str())
            })
            .collect();
        out.sort();
        out
    }

    /// Registra las fotos huérfanas válidas y las añade al final de la secuencia. Devuelve cuántas se recuperaron y cuáles no.
    pub fn recover_orphans(&mut self) -> Result<(usize, Vec<(PathBuf, String)>), String> {
        let mut skipped = Vec::new();
        let mut recovered = 0;
        for p in self.orphans() {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let res = std::fs::read(&p).map_err(|e| e.to_string()).and_then(|bytes| {
                let (format, width, height) = probe_image(&bytes)?;
                // Conserva el número del archivo si su identificador está libre; si no, toma el siguiente.
                let n = number_in_name(&name).filter(|n| self.seq.source(&format!("s{n:06}")).is_none()).unwrap_or(self.seq.next_source);
                let id = format!("s{n:06}");
                let captured = std::fs::metadata(&p).and_then(|m| m.modified()).ok().map(iso_from_time);
                let exif = read_exif(&bytes);
                self.seq.next_source = self.seq.next_source.max(n + 1);
                let exposure = self.seq.default_exposure;
                self.seq.sources.push(SourceFrame {
                    id: id.clone(),
                    file: format!("{SOURCES}/{name}"),
                    width,
                    height,
                    format: format.into(),
                    bytes: bytes.len() as u64,
                    hash: sha256_hex(&bytes),
                    captured_at: exif.as_ref().and_then(|e| e.date_time.clone()).or(captured),
                    device: None,
                    exif,
                });
                self.seq.push_item(&id, exposure).map(|_| ())
            });
            match res {
                Ok(()) => recovered += 1,
                Err(e) => skipped.push((p, e)),
            }
        }
        if recovered > 0 {
            self.save()?;
            self.log(&format!("Recuperadas {recovered} fotos huérfanas"));
        }
        Ok((recovered, skipped))
    }

    /// Borra los archivos temporales que dejó un cierre inesperado a mitad de una escritura.
    pub fn clean_temp(&self) -> usize {
        let mut n = 0;
        for dir in [self.sources_dir(), self.dir.join(THUMBNAILS)] {
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                if e.path().extension().map_or(false, |x| x == "tmp") && std::fs::remove_file(e.path()).is_ok() {
                    n += 1;
                }
            }
        }
        n
    }

    pub fn thumbnail_path(&self, source: &str) -> PathBuf {
        self.dir.join(THUMBNAILS).join(format!("{source}.jpg"))
    }

    /// Miniatura de 320 px de ancho de una foto. Es un recurso derivado: si falta, se regenera; nunca toca el original.
    pub fn ensure_thumbnail(&self, source: &str) -> Result<PathBuf, String> {
        let path = self.thumbnail_path(source);
        if path.is_file() {
            return Ok(path);
        }
        let s = self.seq.source(source).ok_or_else(|| format!("la foto «{source}» no está en la secuencia"))?;
        let bytes = std::fs::read(self.dir.join(&s.file)).map_err(|e| format!("no se pudo leer el original: {e}"))?;
        let img = image::load_from_memory(&bytes).map_err(|e| format!("no se pudo decodificar la foto: {e}"))?.to_rgb8();
        let w = 320.min(img.width()).max(1);
        let h = ((img.height() as f32 * w as f32 / img.width() as f32).round() as u32).max(1);
        let thumb = image::imageops::thumbnail(&img, w, h);
        std::fs::create_dir_all(self.dir.join(THUMBNAILS)).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("tmp");
        thumb.save_with_format(&tmp, image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            e.to_string()
        })?;
        Ok(path)
    }
}
