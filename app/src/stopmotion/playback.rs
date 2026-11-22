//! Lectura de las fotos para verlas y exportarlas (Issue #60): proxies de 1280 px, carga en segundo plano para el visor
//! y caché de la carpeta abierta y de las fotos descodificadas para la exportación.
//!
//! Los originales nunca se modifican: el proxy es un recurso derivado (`Proxies/`) que se regenera si falta. Las fotos
//! pequeñas (hasta 1280 px) se leen directamente del original y no generan proxy.

use super::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// Lado mayor del proxy que se usa para ver la secuencia en el visor.
pub const PROXY_SIDE: u32 = 1280;

/// Identifica una foto: identificador de la secuencia y de la foto.
pub type Key = (String, String);

/// Una foto descodificada lista para subir como textura (RGBA, opaca).
pub struct Decoded {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

fn to_decoded(img: image::DynamicImage) -> Decoded {
    let rgba = img.to_rgba8();
    Decoded { w: rgba.width(), h: rgba.height(), rgba: rgba.into_raw() }
}

/// Reduce una imagen para que su lado mayor sea `max_side` (si ya es menor o igual, no la toca).
fn shrink(img: image::DynamicImage, max_side: u32) -> image::DynamicImage {
    let (w, h) = (img.width(), img.height());
    if w.max(h) <= max_side {
        return img;
    }
    let k = max_side as f32 / w.max(h) as f32;
    let (nw, nh) = (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1));
    image::DynamicImage::ImageRgb8(image::imageops::resize(&img.to_rgb8(), nw, nh, image::imageops::FilterType::Triangle))
}

pub fn proxy_path(dir: &Path, source: &str) -> PathBuf {
    dir.join(PROXIES).join(format!("{source}.jpg"))
}

/// Foto lista para ver: si es grande se usa el proxy (que se crea la primera vez); si es pequeña, el propio original.
pub fn load_proxy(dir: &Path, s: &SourceFrame, max_side: u32) -> Result<Decoded, String> {
    let original = dir.join(&s.file);
    if s.width.max(s.height) > max_side {
        let proxy = proxy_path(dir, &s.id);
        if let Ok(bytes) = std::fs::read(&proxy) {
            if let Ok(img) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg) {
                if img.width().max(img.height()) <= max_side {
                    return Ok(to_decoded(img));
                }
            }
        }
        let bytes = std::fs::read(&original).map_err(|e| format!("no se pudo leer {}: {e}", s.file))?;
        let img = shrink(image::load_from_memory(&bytes).map_err(|e| format!("no se pudo decodificar {}: {e}", s.file))?, max_side);
        // El proxy es un derivado: si no se puede escribir, no pasa nada (se vuelve a calcular la próxima vez).
        if std::fs::create_dir_all(dir.join(PROXIES)).is_ok() {
            let tmp = proxy.with_extension("tmp");
            if image::DynamicImage::ImageRgb8(img.to_rgb8()).save_with_format(&tmp, image::ImageFormat::Jpeg).is_ok() && std::fs::rename(&tmp, &proxy).is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
        }
        Ok(to_decoded(img))
    } else {
        let bytes = std::fs::read(&original).map_err(|e| format!("no se pudo leer {}: {e}", s.file))?;
        Ok(to_decoded(image::load_from_memory(&bytes).map_err(|e| format!("no se pudo decodificar {}: {e}", s.file))?))
    }
}

struct Request {
    key: Key,
    dir: PathBuf,
    source: SourceFrame,
}

/// Carga de fotos en un hilo aparte: la interfaz pide y recoge, nunca espera.
pub struct Loader {
    tx: Option<Sender<Request>>,
    rx: Receiver<(Key, Result<Decoded, String>)>,
    pending: HashSet<Key>,
}

impl Loader {
    pub fn new() -> Loader {
        let (tx, worker_rx) = channel::<Request>();
        let (done_tx, rx) = channel();
        std::thread::spawn(move || {
            while let Ok(req) = worker_rx.recv() {
                let res = load_proxy(&req.dir, &req.source, PROXY_SIDE);
                if done_tx.send((req.key, res)).is_err() {
                    break;
                }
            }
        });
        Loader { tx: Some(tx), rx, pending: HashSet::new() }
    }

    /// Pide una foto (si ya está pedida, no hace nada). Devuelve true si se pidió ahora.
    pub fn request(&mut self, key: Key, dir: PathBuf, source: SourceFrame) -> bool {
        if self.pending.contains(&key) {
            return false;
        }
        self.pending.insert(key.clone());
        match &self.tx {
            Some(tx) if tx.send(Request { key: key.clone(), dir, source }).is_ok() => true,
            _ => {
                self.pending.remove(&key);
                false
            }
        }
    }

    /// Fotos que ya terminaron de cargar desde la última vez.
    pub fn poll(&mut self) -> Vec<(Key, Result<Decoded, String>)> {
        let mut out = Vec::new();
        while let Ok((key, res)) = self.rx.try_recv() {
            self.pending.remove(&key);
            out.push((key, res));
        }
        out
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    pub fn is_pending(&self, key: &Key) -> bool {
        self.pending.contains(key)
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        // Al soltar el emisor, el hilo termina al vaciar la cola.
        self.tx = None;
    }
}

/// Foto descodificada para dibujarla en un fotograma exportado (píxeles premultiplicados, opacos).
pub struct Photo {
    pub pixmap: tiny_skia::Pixmap,
}

struct Entry {
    dir: PathBuf,
    stamp: (u128, u64),
    folder: Arc<SeqFolder>,
    abs: String,
    rel: Option<String>,
    base: Option<PathBuf>,
}

#[derive(Default)]
struct Inner {
    folders: HashMap<String, Entry>,
    /// Fotos descodificadas recientes (la más reciente al final).
    photos: Vec<((String, String, u32), Arc<Photo>)>,
}

/// Caché que viaja con el proyecto (no se guarda): evita volver a leer `sequence.json` y las fotos en cada fotograma.
#[derive(Clone, Default)]
pub struct SmCache(Arc<Mutex<Inner>>);

impl std::fmt::Debug for SmCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SmCache")
    }
}

/// Fotos descodificadas que se conservan para exportar (con varios cuadros seguidos suelen repetirse).
const KEEP_PHOTOS: usize = 6;

fn stamp(dir: &Path) -> Option<(u128, u64)> {
    let m = std::fs::metadata(dir.join(SEQUENCE_FILE)).ok()?;
    let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    Some((t, m.len()))
}

impl SmCache {
    /// La secuencia de una referencia, abierta (con la caché al día: se vuelve a leer si `sequence.json` cambió).
    pub fn folder(&self, r: &SmRef, base: Option<&Path>) -> Option<Arc<SeqFolder>> {
        let mut g = self.0.lock().ok()?;
        if let Some(e) = g.folders.get(&r.id) {
            if e.abs == r.abs && e.rel == r.rel && e.base.as_deref() == base && stamp(&e.dir) == Some(e.stamp) {
                return Some(e.folder.clone());
            }
        }
        g.folders.remove(&r.id);
        let Located::Found(dir) = r.locate(base) else { return None };
        let st = stamp(&dir)?;
        let (folder, _) = SeqFolder::open(&dir).ok()?;
        let folder = Arc::new(folder);
        g.folders.insert(r.id.clone(), Entry { dir, stamp: st, folder: folder.clone(), abs: r.abs.clone(), rel: r.rel.clone(), base: base.map(|b| b.to_path_buf()) });
        Some(folder)
    }

    /// El original descodificado y reducido a `max_side` como mucho (para exportar sin leer una foto enorme por cada cuadro).
    pub fn photo(&self, folder: &SeqFolder, s: &SourceFrame, max_side: u32) -> Option<Arc<Photo>> {
        let key = (folder.seq.id.clone(), s.id.clone(), max_side);
        if let Ok(mut g) = self.0.lock() {
            if let Some(i) = g.photos.iter().position(|(k, _)| *k == key) {
                let hit = g.photos.remove(i);
                let photo = hit.1.clone();
                g.photos.push(hit);
                return Some(photo);
            }
        }
        // La lectura se hace sin tener el candado cogido.
        let bytes = std::fs::read(folder.dir.join(&s.file)).ok()?;
        let img = shrink(image::load_from_memory(&bytes).ok()?, max_side.max(16));
        let rgba = img.to_rgba8();
        let mut pixmap = tiny_skia::Pixmap::new(rgba.width(), rgba.height())?;
        for (dst, src) in pixmap.data_mut().chunks_exact_mut(4).zip(rgba.as_raw().chunks_exact(4)) {
            dst.copy_from_slice(&[src[0], src[1], src[2], 255]);
        }
        let photo = Arc::new(Photo { pixmap });
        if let Ok(mut g) = self.0.lock() {
            g.photos.push((key, photo.clone()));
            while g.photos.len() > KEEP_PHOTOS {
                g.photos.remove(0);
            }
        }
        Some(photo)
    }
}
