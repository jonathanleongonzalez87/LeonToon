//! Edición de la secuencia (Issue #61): reordenar, duplicar, eliminar, reemplazar, insertar fotogramas vacíos, exposiciones,
//! congelar, pausas y marcadores. **Solo cambia los datos de la secuencia** (posiciones y marcadores): los archivos de las
//! fotos nunca se tocan, ni siquiera al eliminar una posición (la foto sigue registrada en `Sources/`).
//!
//! Cada cambio se puede deshacer y rehacer con `History`, que guarda copias de las posiciones y los marcadores.

use super::*;

/// Lo que cambia al editar. Las fotos registradas no entran: nunca se quitan ni se modifican.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub items: Vec<Item>,
    pub markers: Vec<Marker>,
}

/// Historial de deshacer y rehacer de una secuencia.
#[derive(Default, Debug)]
pub struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl History {
    /// Pasos que se recuerdan como máximo.
    pub const LIMIT: usize = 200;

    /// Guarda el estado **antes** de un cambio. Rehacer se vacía, porque el futuro cambió.
    pub fn record(&mut self, seq: &Sequence) {
        self.undo.push(seq.snapshot());
        if self.undo.len() > Self::LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Descarta el último estado guardado (cuando el cambio no llegó a hacerse).
    pub fn forget_last(&mut self) {
        self.undo.pop();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, seq: &mut Sequence) -> bool {
        let Some(s) = self.undo.pop() else { return false };
        self.redo.push(seq.snapshot());
        seq.restore(s);
        true
    }

    pub fn redo(&mut self, seq: &mut Sequence) -> bool {
        let Some(s) = self.redo.pop() else { return false };
        self.undo.push(seq.snapshot());
        seq.restore(s);
        true
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

/// Clases de marcador.
pub const MARKER: &str = "marker";
pub const POSE: &str = "pose";

impl Sequence {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot { items: self.items.clone(), markers: self.markers.clone() }
    }

    pub fn restore(&mut self, s: Snapshot) {
        self.items = s.items;
        self.markers = s.markers;
        self.sanitize();
    }

    pub fn index_of(&self, item: &str) -> Option<usize> {
        self.items.iter().position(|i| i.id == item)
    }

    /// Mueve las posiciones `ids` (en su orden actual) al hueco `slot`: 0 es antes de la primera y `len` después de la última.
    /// Devuelve false si el orden no cambia.
    pub fn move_items(&mut self, ids: &[String], slot: usize) -> bool {
        let slot = slot.min(self.items.len());
        let moved_before = self.items.iter().take(slot).filter(|i| ids.contains(&i.id)).count();
        let old: Vec<String> = self.items.iter().map(|i| i.id.clone()).collect();
        let (moving, mut rest): (Vec<Item>, Vec<Item>) = std::mem::take(&mut self.items).into_iter().partition(|i| ids.contains(&i.id));
        let at = (slot - moved_before).min(rest.len());
        for (k, m) in moving.into_iter().enumerate() {
            rest.insert(at + k, m);
        }
        self.items = rest;
        self.items.iter().map(|i| &i.id).ne(old.iter())
    }
    /// Inserta un fotograma vacío (sin foto) en la posición `at`.
    pub fn insert_blank(&mut self, at: usize, exposure: u32) -> String {
        let id = self.take_item_id();
        let at = at.min(self.items.len());
        self.items.insert(at, Item { id: id.clone(), source: String::new(), exposure: exposure.clamp(1, MAX_EXPOSURE), visible: true, note: None });
        id
    }

    /// Inserta una foto ya registrada en la posición `at` (desde el Frame Browser).
    pub fn insert_source(&mut self, at: usize, source: &str, exposure: u32) -> Result<String, String> {
        if self.source(source).is_none() {
            return Err(format!("La foto «{source}» no está en la secuencia"));
        }
        let id = self.take_item_id();
        let at = at.min(self.items.len());
        self.items.insert(at, Item { id: id.clone(), source: source.into(), exposure: exposure.clamp(1, MAX_EXPOSURE), visible: true, note: None });
        Ok(id)
    }

    /// Cambia la foto de las posiciones `ids` por otra ya registrada (conservan su exposición). Devuelve cuántas cambiaron.
    pub fn replace_source(&mut self, ids: &[String], source: &str) -> Result<usize, String> {
        if self.source(source).is_none() {
            return Err(format!("La foto «{source}» no está en la secuencia"));
        }
        let mut n = 0;
        for it in self.items.iter_mut().filter(|i| ids.contains(&i.id) && i.source != source) {
            it.source = source.into();
            n += 1;
        }
        Ok(n)
    }

    /// Quita posiciones de la secuencia. Las fotos siguen registradas y sus archivos no se tocan.
    pub fn remove_items(&mut self, ids: &[String]) -> usize {
        let before = self.items.len();
        self.items.retain(|i| !ids.contains(&i.id));
        before - self.items.len()
    }

    /// Repite cada posición elegida justo después de ella (la misma foto, sin copiar el archivo). Devuelve los ids nuevos.
    pub fn duplicate_items(&mut self, ids: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        for id in ids {
            if let Some(n) = self.duplicate_item(id) {
                out.push(n);
            }
        }
        out
    }

    pub fn set_exposures(&mut self, ids: &[String], exposure: u32) -> usize {
        let mut n = 0;
        for it in self.items.iter_mut().filter(|i| ids.contains(&i.id)) {
            let e = exposure.clamp(1, MAX_EXPOSURE);
            if it.exposure != e {
                it.exposure = e;
                n += 1;
            }
        }
        n
    }

    /// Suma o resta cuadros de exposición (una pausa es sumar; nunca baja de 1 ni pasa del máximo).
    pub fn add_exposure(&mut self, ids: &[String], delta: i32) -> usize {
        let mut n = 0;
        for it in self.items.iter_mut().filter(|i| ids.contains(&i.id)) {
            let e = (it.exposure as i64 + delta as i64).clamp(1, MAX_EXPOSURE as i64) as u32;
            if it.exposure != e {
                it.exposure = e;
                n += 1;
            }
        }
        n
    }

    /// Congela una posición: pone justo después otra con la misma foto durante `frames` cuadros.
    pub fn freeze(&mut self, item: &str, frames: u32) -> Option<String> {
        let id = self.duplicate_item(item)?;
        self.set_exposure(&id, frames);
        Some(id)
    }

    pub fn set_visible(&mut self, ids: &[String], visible: bool) -> usize {
        let mut n = 0;
        for it in self.items.iter_mut().filter(|i| ids.contains(&i.id) && i.visible != visible) {
            it.visible = visible;
            n += 1;
        }
        n
    }

    pub fn set_note(&mut self, item: &str, note: &str) -> bool {
        match self.items.iter_mut().find(|i| i.id == item) {
            Some(i) => {
                let n = note.trim();
                i.note = if n.is_empty() { None } else { Some(n.to_string()) };
                true
            }
            None => false,
        }
    }

    /// Pone un marcador en el cuadro `at` de la secuencia. Si ya hay uno de esa clase ahí, cambia su texto.
    pub fn add_marker(&mut self, at: u32, kind: &str, label: &str) {
        let kind = if kind == POSE { POSE } else { MARKER };
        match self.markers.iter_mut().find(|m| m.at == at && m.kind == kind) {
            Some(m) => m.label = label.to_string(),
            None => self.markers.push(Marker { at, kind: kind.into(), label: label.to_string() }),
        }
        self.markers.sort_by_key(|m| (m.at, m.kind.clone()));
    }

    pub fn remove_marker(&mut self, index: usize) -> bool {
        if index < self.markers.len() {
            self.markers.remove(index);
            true
        } else {
            false
        }
    }

    /// Marcadores que caen dentro de la posición `index`.
    pub fn markers_in(&self, index: usize) -> Vec<&Marker> {
        let (a, b) = (self.start_frame(index), self.start_frame(index) + self.items.get(index).map_or(0, |i| i.exposure));
        self.markers.iter().filter(|m| m.at >= a && m.at < b).collect()
    }
}

impl SeqFolder {
    /// Pone una foto capturada al final de la secuencia (con la exposición predeterminada) como un cambio que se puede
    /// deshacer. Si algo falla (foto incompleta, disco lleno), la secuencia y el historial quedan exactamente como estaban.
    /// No guarda `sequence.json`: lo hace quien llama, igual que con las demás ediciones.
    pub fn capture_to_end(&mut self, hist: &mut History, bytes: &[u8], device: Option<Device>) -> Result<String, String> {
        hist.record(&self.seq);
        let added = self.add_photo_bytes(bytes, device, Some(std::time::SystemTime::now())).and_then(|out| {
            let exposure = self.seq.default_exposure;
            self.seq.push_item(&out.id, exposure)
        });
        if added.is_err() {
            hist.forget_last();
        }
        added
    }
}