//! Capa Stop Motion (Issue #59, Fase 1 del épico #57): fotos como recursos originales y una secuencia que las ordena.
//!
//! Tres componentes, tal como pide el Issue y recoge `docs/stop-motion-estudio.md`:
//! - **Recursos originales** (`SourceFrame`): cada foto es un archivo independiente en `Sources/` que nunca se modifica ni
//!   se renombra; se guardan su identificador, tamaño, formato, fecha, dispositivo, EXIF y un hash de integridad.
//! - **Datos de secuencia** (`Sequence`, en `sequence.json`): el orden y la exposición de cada foto. Una misma foto puede
//!   estar en varias posiciones sin duplicar el archivo; cambiar la exposición o el orden nunca toca los originales.
//! - **Recursos derivados** (miniaturas, en `Thumbnails/`): se pueden regenerar y nunca sustituyen al original.
//!
//! La carpeta de cada secuencia la elige la persona. El proyecto (`.ltoon`) solo guarda una referencia (`SmRef`) y puede
//! volver a localizar la carpeta si se mueve. Este módulo no depende de la interfaz ni de ninguna cámara.
// Parte de esta interfaz (editar posiciones, capturar, limpiar temporales) la usan las pruebas y las fases siguientes (#60 a #62).
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

mod edit;
mod playback;
mod reference;
mod store;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use edit::{History, Snapshot, MARKER, POSE};
pub use playback::{load_proxy, load_thumb, proxy_path, Decoded, Key, Loader, Photo, SmCache, PROXY_SIDE};
#[allow(unused_imports)]
pub use reference::{relative_path, resolve_relative, Located, SmRef};
#[allow(unused_imports)]
pub use store::{AddOutcome, ImportReport, OpenNote, Problem, SeqFolder};

pub const FORMAT: &str = "leontoon-stopmotion";
pub const VERSION: u32 = 1;
pub const SEQUENCE_FILE: &str = "sequence.json";
pub const SOURCES: &str = "Sources";
pub const THUMBNAILS: &str = "Thumbnails";
pub const PROXIES: &str = "Proxies";
pub const JOURNAL: &str = "Journal";
/// Máximo de cuadros que puede durar una foto.
pub const MAX_EXPOSURE: u32 = 10_000;

/// Dispositivo que hizo la foto (si se capturó con una cámara).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub name: String,
    pub provider: String,
}

/// Datos EXIF que se muestran (el archivo original conserva todos los suyos, porque no se modifica).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExifInfo {
    pub make: Option<String>,
    pub model: Option<String>,
    pub date_time: Option<String>,
}

/// Una foto original.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceFrame {
    pub id: String,
    /// Ruta relativa a la carpeta de la secuencia, con `/`.
    pub file: String,
    pub width: u32,
    pub height: u32,
    /// `jpeg` o `png`.
    pub format: String,
    pub bytes: u64,
    /// `sha256:` y el hash en hexadecimal.
    pub hash: String,
    pub captured_at: Option<String>,
    pub device: Option<Device>,
    pub exif: Option<ExifInfo>,
}

/// Una posición de la secuencia: una foto durante `exposure` cuadros.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub source: String,
    pub exposure: u32,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub note: Option<String>,
}

fn yes() -> bool {
    true
}

/// Marca temporal (por ejemplo, una pose importante).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub at: u32,
    pub kind: String,
    pub label: String,
}

/// Datos de una secuencia (`sequence.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sequence {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub name: String,
    pub fps: u32,
    pub default_exposure: u32,
    pub sources: Vec<SourceFrame>,
    pub items: Vec<Item>,
    pub markers: Vec<Marker>,
    /// Siguiente número de foto: nunca se reutiliza, aunque se borre una.
    pub next_source: u32,
    pub next_item: u32,
}

impl Default for Sequence {
    fn default() -> Self {
        Sequence { format: FORMAT.into(), version: VERSION, id: String::new(), name: String::new(), fps: 12, default_exposure: 1, sources: Vec::new(), items: Vec::new(), markers: Vec::new(), next_source: 1, next_item: 1 }
    }
}

impl Sequence {
    pub fn new(name: &str, fps: u32) -> Sequence {
        Sequence { id: new_id(name), name: name.trim().to_string(), fps: fps.clamp(1, 120), ..Default::default() }
    }

    pub fn source(&self, id: &str) -> Option<&SourceFrame> {
        self.sources.iter().find(|s| s.id == id)
    }

    /// Cuadros que dura toda la secuencia (la suma de las exposiciones).
    pub fn total_frames(&self) -> u32 {
        self.items.iter().map(|i| i.exposure).sum()
    }

    /// Primer cuadro de la posición `index`.
    pub fn start_frame(&self, index: usize) -> u32 {
        self.items.iter().take(index).map(|i| i.exposure).sum()
    }

    /// Posición que se ve en el cuadro `frame` (empezando en 0).
    pub fn item_at_frame(&self, frame: u32) -> Option<usize> {
        let mut start = 0;
        for (i, it) in self.items.iter().enumerate() {
            if frame < start + it.exposure {
                return Some(i);
            }
            start += it.exposure;
        }
        None
    }

    /// Posición de la secuencia que se ve en un cuadro del **proyecto**. La secuencia empieza en el cuadro start del
    /// proyecto y cada cuadro de la secuencia dura project_fps / fps cuadros del proyecto, de modo que ninguna foto se
    /// pierde cuando la secuencia va a menos fotogramas por segundo que el proyecto (por ejemplo, 12 en un proyecto de 24).
    /// Si va a más, se muestrean los cuadros que caen en cada instante del proyecto (las fotos siguen en la secuencia).
    pub fn item_for_project_frame(&self, start: u32, project_fps: u32, frame: u32) -> Option<usize> {
        if frame < start || project_fps == 0 {
            return None;
        }
        let seq_frame = ((frame - start) as u64 * self.fps.max(1) as u64 / project_fps as u64) as u32;
        self.item_at_frame(seq_frame)
    }

    fn take_item_id(&mut self) -> String {
        let id = format!("i{:06}", self.next_item);
        self.next_item += 1;
        id
    }

    /// Añade al final una posición con una foto que ya está registrada.
    pub fn push_item(&mut self, source: &str, exposure: u32) -> Result<String, String> {
        if self.source(source).is_none() {
            return Err(format!("La foto «{source}» no está en la secuencia"));
        }
        let id = self.take_item_id();
        self.items.push(Item { id: id.clone(), source: source.into(), exposure: exposure.clamp(1, MAX_EXPOSURE), visible: true, note: None });
        Ok(id)
    }

    pub fn set_exposure(&mut self, item: &str, exposure: u32) -> bool {
        match self.items.iter_mut().find(|i| i.id == item) {
            Some(i) => {
                i.exposure = exposure.clamp(1, MAX_EXPOSURE);
                true
            }
            None => false,
        }
    }

    /// Repite una posición justo después de ella (la misma foto, sin copiar el archivo).
    pub fn duplicate_item(&mut self, item: &str) -> Option<String> {
        let at = self.items.iter().position(|i| i.id == item)?;
        let mut copy = self.items[at].clone();
        copy.id = self.take_item_id();
        let id = copy.id.clone();
        self.items.insert(at + 1, copy);
        Some(id)
    }

    /// Quita una posición. La foto sigue registrada y su archivo no se toca.
    pub fn remove_item(&mut self, item: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != item);
        self.items.len() != before
    }

    /// Deja la secuencia válida (por si viene de un archivo editado o dañado).
    pub fn sanitize(&mut self) {
        self.fps = self.fps.clamp(1, 120);
        self.default_exposure = self.default_exposure.clamp(1, MAX_EXPOSURE);
        let ids: Vec<String> = self.sources.iter().map(|s| s.id.clone()).collect();
        self.items.retain(|i| i.source.is_empty() || ids.contains(&i.source));
        for i in &mut self.items {
            i.exposure = i.exposure.clamp(1, MAX_EXPOSURE);
        }
        // Los contadores nunca pueden quedar por debajo de un número ya usado.
        let num = |s: &str| s.get(1..).and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
        self.next_source = self.next_source.max(self.sources.iter().map(|s| num(&s.id)).max().unwrap_or(0) + 1).max(1);
        self.next_item = self.next_item.max(self.items.iter().map(|i| num(&i.id)).max().unwrap_or(0) + 1).max(1);
    }
}

/// Identificador único de una secuencia (16 caracteres hexadecimales).
fn new_id(name: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut h = Sha256::new();
    h.update(nanos.to_le_bytes());
    h.update(std::process::id().to_le_bytes());
    h.update(COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    h.update(name.as_bytes());
    hex(&h.finalize()[..8])
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Fecha y hora UTC en formato `2026-11-22T20:14:03Z`.
pub(crate) fn iso_utc(secs: u64) -> String {
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
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

pub(crate) fn iso_from_time(t: std::time::SystemTime) -> String {
    iso_utc(t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
}
