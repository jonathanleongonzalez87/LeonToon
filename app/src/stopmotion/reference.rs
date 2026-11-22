//! Referencia de una capa Stop Motion a la carpeta de su secuencia, guardada en el `.ltoon`.
//!
//! El proyecto no contiene las fotos: guarda el identificador de la secuencia, la ruta de su carpeta (relativa a la
//! carpeta del proyecto cuando es posible, y absoluta como respaldo) y un resumen. Al abrir el proyecto, si la carpeta no
//! está donde se guardó, se puede **volver a localizar**: la carpeta nueva se acepta solo si su `sequence.json` tiene el
//! mismo identificador.

use super::*;
use std::path::{Component, Path, PathBuf};

/// Dónde está (o no) la carpeta de una secuencia.
#[derive(Clone, Debug, PartialEq)]
pub enum Located {
    Found(PathBuf),
    /// No se encuentra ninguna carpeta con esa secuencia.
    Missing,
    /// La ruta guardada existe pero tiene **otra** secuencia (nombre de esa otra).
    WrongSequence(String),
}

/// Lo que guarda el proyecto de una secuencia Stop Motion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SmRef {
    pub id: String,
    pub name: String,
    /// Ruta de la carpeta relativa a la del proyecto, con `/` (si estaban en la misma unidad).
    #[serde(default)]
    pub rel: Option<String>,
    /// Ruta absoluta de la carpeta cuando se guardó.
    pub abs: String,
    /// Resumen para mostrarlo aunque la carpeta no esté: fotos, cuadros y fotogramas por segundo.
    #[serde(default)]
    pub photos: u32,
    #[serde(default)]
    pub frames: u32,
    #[serde(default)]
    pub fps: u32,
    /// Cuadro del proyecto (desde 0) en el que empieza la secuencia.
    #[serde(default)]
    pub start: u32,
}

fn norm(c: &Component) -> String {
    let s = c.as_os_str().to_string_lossy().to_string();
    if cfg!(windows) { s.to_lowercase() } else { s }
}

/// Ruta de `to` relativa a `from_dir`, con `/`. `None` si están en unidades distintas.
pub fn relative_path(from_dir: &Path, to: &Path) -> Option<String> {
    let a: Vec<Component> = from_dir.components().collect();
    let b: Vec<Component> = to.components().collect();
    if a.is_empty() || b.is_empty() || norm(&a[0]) != norm(&b[0]) {
        return None;
    }
    let mut i = 0;
    while i < a.len() && i < b.len() && norm(&a[i]) == norm(&b[i]) {
        i += 1;
    }
    let mut parts: Vec<String> = Vec::new();
    for _ in i..a.len() {
        parts.push("..".into());
    }
    for c in &b[i..] {
        parts.push(c.as_os_str().to_string_lossy().to_string());
    }
    Some(if parts.is_empty() { ".".to_string() } else { parts.join("/") })
}

/// Lo contrario de `relative_path`: aplica una ruta relativa (con `/` y `..`) a una carpeta base.
pub fn resolve_relative(base: &Path, rel: &str) -> PathBuf {
    let mut p = base.to_path_buf();
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                p.pop();
            }
            x => p.push(x),
        }
    }
    p
}

/// Identificador y nombre de la secuencia que hay en `dir` (sin leer las fotos), si hay una.
fn peek(dir: &Path) -> Option<(String, String)> {
    #[derive(Deserialize)]
    #[serde(default)]
    struct Head {
        id: String,
        name: String,
    }
    impl Default for Head {
        fn default() -> Self {
            Head { id: String::new(), name: String::new() }
        }
    }
    let h: Head = serde_json::from_slice(&std::fs::read(dir.join(SEQUENCE_FILE)).ok()?).ok()?;
    (!h.id.is_empty()).then_some((h.id, h.name))
}

impl SmRef {
    /// Referencia a una secuencia abierta. `project_dir` es la carpeta del proyecto, si ya se guardó.
    pub fn new(f: &SeqFolder, project_dir: Option<&Path>) -> SmRef {
        let mut r = SmRef { id: f.seq.id.clone(), name: f.seq.name.clone(), rel: None, abs: f.dir.to_string_lossy().to_string(), photos: 0, frames: 0, fps: 0, start: 0 };
        r.update(f, project_dir);
        r
    }

    /// Actualiza el resumen y las rutas con el estado actual de la secuencia.
    pub fn update(&mut self, f: &SeqFolder, project_dir: Option<&Path>) {
        self.name = f.seq.name.clone();
        self.photos = f.seq.sources.len() as u32;
        self.frames = f.seq.total_frames();
        self.fps = f.seq.fps;
        self.abs = f.dir.to_string_lossy().to_string();
        self.rel = project_dir.and_then(|p| relative_path(p, &f.dir));
    }

    /// Busca la carpeta: primero la ruta relativa (si el proyecto se movió junto a su carpeta), luego la absoluta.
    pub fn locate(&self, project_dir: Option<&Path>) -> Located {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let (Some(base), Some(rel)) = (project_dir, &self.rel) {
            candidates.push(resolve_relative(base, rel));
        }
        candidates.push(PathBuf::from(&self.abs));
        let mut other: Option<String> = None;
        for dir in candidates {
            match peek(&dir) {
                Some((id, _)) if id == self.id => return Located::Found(dir),
                Some((_, name)) => other = other.or(Some(name)),
                None => {}
            }
        }
        match other {
            Some(name) => Located::WrongSequence(name),
            None => Located::Missing,
        }
    }

    /// Apunta la referencia a otra carpeta. Solo se acepta si ahí está **la misma secuencia** (mismo identificador).
    pub fn relocate(&mut self, dir: &Path, project_dir: Option<&Path>) -> Result<(), String> {
        match peek(dir) {
            None => Err("Esa carpeta no tiene una secuencia Stop Motion (falta sequence.json)".into()),
            Some((id, name)) if id != self.id => Err(format!("Esa carpeta tiene otra secuencia («{name}»), no «{}»", self.name)),
            Some(_) => {
                self.abs = dir.to_string_lossy().to_string();
                self.rel = project_dir.and_then(|p| relative_path(p, dir));
                Ok(())
            }
        }
    }

    /// Recalcula la ruta relativa respecto a la carpeta del proyecto (al guardar el proyecto en otro sitio).
    pub fn refresh_paths(&mut self, project_dir: Option<&Path>) {
        if let Located::Found(dir) = self.locate(project_dir) {
            self.abs = dir.to_string_lossy().to_string();
            self.rel = project_dir.and_then(|p| relative_path(p, &dir));
        }
    }
}
