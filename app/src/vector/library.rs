//! Biblioteca de pinceles vectoriales (Issue #31, Fase 2).
//!
//! Es **independiente** de la biblioteca de pinceles de píxeles (`presets.rs`): archivo, formato y tipos propios.
//! Un preset es solo una receta (grosor, opacidad, suavizado y `VStyle`). Al dibujar, cada trazo copia el estilo,
//! así que editar o borrar un preset nunca cambia los trazos ya dibujados.
//!
//! - Los pinceles incluidos no se pisan: si se editan, la versión editada se guarda aparte (`edited`) y se puede restaurar.
//! - Los del usuario (`user`) se pueden crear, duplicar, renombrar, mover de categoría y eliminar.
//! - Favoritos por identificador `categoría/nombre`.
//! - Formato nativo `.ltvb` (JSON con `format`, `version` y la lista de presets) para exportar e importar.

use super::{Cap, Join, Profile, VStyle};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const USER_CATEGORY: &str = "Pinceles personalizados";
const FILE: &str = "pinceles_vectoriales.json";
pub const FORMAT: &str = "leontoon-vector-brushes";
pub const VERSION: u32 = 1;

/// Receta de un pincel vectorial.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct VPreset {
    pub name: String,
    pub category: String,
    /// Grosor pleno en px.
    pub size: f32,
    /// Opacidad del trazo (0 a 1).
    pub opacity: f32,
    /// Suavizado del trazo (0 a 0,85).
    pub smoothing: f32,
    pub style: VStyle,
    #[serde(skip)]
    pub builtin: bool,
}

impl Default for VPreset {
    fn default() -> Self {
        VPreset { name: "Pincel".into(), category: USER_CATEGORY.into(), size: 4.0, opacity: 1.0, smoothing: 0.3, style: VStyle::default(), builtin: false }
    }
}

impl VPreset {
    /// Identificador estable: `categoría/nombre`.
    pub fn id(&self) -> String {
        format!("{}/{}", self.category, self.name)
    }

    /// Valores dentro de rango y nombre/categoría no vacíos (por si vienen de un archivo).
    pub fn clamped(mut self) -> Self {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        self.size = f(self.size, 1.0, 1000.0, 4.0);
        self.opacity = f(self.opacity, 0.15, 1.0, 1.0);
        self.smoothing = f(self.smoothing, 0.0, 0.85, 0.3);
        self.style = self.style.clamped();
        self.name = self.name.trim().to_string();
        self.category = self.category.trim().to_string();
        if self.name.is_empty() {
            self.name = "Pincel".into();
        }
        if self.category.is_empty() {
            self.category = USER_CATEGORY.into();
        }
        self
    }

    /// ¿Hacen lo mismo? Se ignora el nombre del pincel de origen que lleva el estilo.
    pub fn same_recipe(&self, size: f32, opacity: f32, smoothing: f32, style: &VStyle) -> bool {
        let (mut a, mut b) = (self.style.clone().clamped(), style.clone().clamped());
        a.brush = None;
        b.brush = None;
        (self.size - size).abs() < 0.01 && (self.opacity - opacity).abs() < 0.005 && (self.smoothing - smoothing).abs() < 0.005 && a == b
    }

    /// Resumen de las propiedades principales, para mostrarlas al usuario.
    pub fn summary(&self) -> Vec<(&'static str, String)> {
        let s = &self.style;
        vec![
            ("Grosor", format!("{:.0} px", self.size)),
            ("Opacidad", format!("{:.0} %", self.opacity * 100.0)),
            ("Suavizado", format!("{:.0} %", self.smoothing * 100.0)),
            ("Presión", if s.pressure { format!("sí · mín. {:.0} % · curva {:.2}", s.min_width * 100.0, s.curve) } else { "no".into() }),
            ("Perfil", if s.profile == Profile::Uniform { s.profile.label().into() } else { format!("{} ({:.0} %)", s.profile.label(), s.taper * 100.0) }),
            ("Extremo", s.cap.label().into()),
            ("Unión", s.join.label().into()),
        ]
    }
}

fn p(category: &str, name: &str, size: f32, opacity: f32, smoothing: f32, style: VStyle) -> VPreset {
    VPreset { name: name.into(), category: category.into(), size, opacity, smoothing, style, builtin: true }
}

fn st(cap: Cap, join: Join, profile: Profile, taper: f32, pressure: bool, min_width: f32, curve: f32) -> VStyle {
    VStyle { cap, join, profile, taper, pressure, min_width, curve, brush: None, version: 1 }
}

/// Pinceles incluidos, por categoría.
pub fn builtin() -> Vec<VPreset> {
    use Cap::{Flat, Round, Square};
    use Join::{Bevel, Miter, Round as JR};
    use Profile::{TaperBoth, TaperEnd, TaperStart, Uniform};
    let b = "Básicos";
    vec![
        p(b, "Redondo", 6.0, 1.0, 0.3, st(Round, JR, Uniform, 0.25, false, 0.2, 1.0)),
        p(b, "Cuadrado", 6.0, 1.0, 0.2, st(Square, Miter, Uniform, 0.25, false, 0.2, 1.0)),
        p(b, "Línea uniforme", 3.0, 1.0, 0.4, st(Flat, Bevel, Uniform, 0.25, false, 0.2, 1.0)),
        p(b, "Grosor variable", 8.0, 1.0, 0.3, st(Round, JR, Uniform, 0.25, true, 0.15, 1.0)),
        p(b, "Línea fina", 1.5, 1.0, 0.4, st(Round, JR, Uniform, 0.25, false, 0.2, 1.0)),
        p(b, "Línea gruesa", 18.0, 1.0, 0.3, st(Round, JR, Uniform, 0.25, false, 0.2, 1.0)),
        p("Boceto", "Lápiz de boceto", 3.0, 0.7, 0.1, st(Round, JR, Uniform, 0.25, true, 0.3, 0.8)),
        p("Boceto", "Boceto suave", 5.0, 0.45, 0.2, st(Round, JR, TaperBoth, 0.3, true, 0.1, 0.8)),
        p("Tinta", "Tinta con presión", 8.0, 1.0, 0.35, st(Round, JR, Uniform, 0.25, true, 0.05, 1.3)),
        p("Tinta", "Tinta afilada", 9.0, 1.0, 0.4, st(Round, JR, TaperBoth, 0.35, true, 0.05, 1.0)),
        p("Entintado", "Entintado limpio", 4.0, 1.0, 0.55, st(Round, JR, Uniform, 0.25, false, 0.2, 1.0)),
        p("Entintado", "Entintado con remate", 5.0, 1.0, 0.5, st(Round, JR, TaperEnd, 0.3, true, 0.25, 1.0)),
        p("Ilustración", "Ilustración", 7.0, 1.0, 0.35, st(Round, JR, TaperStart, 0.15, true, 0.2, 1.0)),
        p("Ilustración", "Silueta", 14.0, 1.0, 0.3, st(Round, JR, Uniform, 0.25, true, 0.5, 1.0)),
        p("Caligrafía", "Plumín", 8.0, 1.0, 0.4, st(Flat, Miter, Uniform, 0.25, true, 0.2, 1.6)),
        p("Caligrafía", "Pincel de caligrafía", 12.0, 1.0, 0.45, st(Round, JR, TaperBoth, 0.4, true, 0.05, 1.2)),
        p("Contornos", "Contorno fino", 2.0, 1.0, 0.5, st(Round, JR, Uniform, 0.25, false, 0.2, 1.0)),
        p("Contornos", "Contorno grueso", 6.0, 1.0, 0.5, st(Round, JR, TaperBoth, 0.12, false, 0.2, 1.0)),
        p("Pinceles artísticos", "Orgánico", 10.0, 0.9, 0.25, st(Round, JR, TaperBoth, 0.25, true, 0.0, 0.6)),
        p("Pinceles artísticos", "Trazo seco", 9.0, 0.75, 0.15, st(Square, Bevel, TaperEnd, 0.45, true, 0.1, 0.7)),
    ]
}

/// Orden de las categorías incluidas.
const ORDER: [&str; 8] = ["Básicos", "Boceto", "Tinta", "Entintado", "Ilustración", "Caligrafía", "Contornos", "Pinceles artísticos"];

/// Lo que se guarda en el equipo.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct VLibrary {
    pub version: u32,
    /// Pinceles del usuario.
    pub user: Vec<VPreset>,
    /// Versiones editadas de pinceles incluidos (el original no se pierde).
    pub edited: Vec<VPreset>,
    /// Identificadores de los favoritos.
    pub favorites: Vec<String>,
}

impl VLibrary {
    /// Todos los pinceles: los incluidos (con su edición, si la hay) y los del usuario.
    pub fn all(&self) -> Vec<VPreset> {
        let mut out: Vec<VPreset> = builtin().into_iter().map(|b| self.edited.iter().find(|e| e.id() == b.id()).map(|e| VPreset { builtin: true, ..e.clone() }).unwrap_or(b)).collect();
        out.extend(self.user.iter().cloned());
        out
    }

    pub fn find(&self, id: &str) -> Option<VPreset> {
        self.all().into_iter().find(|p| p.id() == id)
    }

    /// El incluido tal como vino de fábrica.
    pub fn original(id: &str) -> Option<VPreset> {
        builtin().into_iter().find(|p| p.id() == id)
    }

    pub fn is_edited(&self, id: &str) -> bool {
        self.edited.iter().any(|e| e.id() == id)
    }

    pub fn is_favorite(&self, id: &str) -> bool {
        self.favorites.iter().any(|f| f == id)
    }

    pub fn toggle_favorite(&mut self, id: &str) {
        if self.is_favorite(id) {
            self.favorites.retain(|f| f != id);
        } else if self.find(id).is_some() {
            self.favorites.push(id.to_string());
        }
    }

    /// Categorías: primero las incluidas, luego las del usuario por orden alfabético.
    pub fn categories(&self) -> Vec<String> {
        let mut out: Vec<String> = ORDER.iter().map(|s| s.to_string()).collect();
        let mut extra: Vec<String> = Vec::new();
        for p in &self.user {
            if !out.iter().chain(extra.iter()).any(|c| c.eq_ignore_ascii_case(&p.category)) {
                extra.push(p.category.clone());
            }
        }
        if !out.iter().chain(extra.iter()).any(|c| c == USER_CATEGORY) {
            extra.push(USER_CATEGORY.to_string());
        }
        extra.sort_by_key(|c| c.to_lowercase());
        out.extend(extra);
        out
    }

    /// Pinceles cuyo nombre o categoría contienen el texto (sin distinguir mayúsculas ni acentos comunes).
    pub fn search(&self, query: &str) -> Vec<VPreset> {
        let q = fold(query);
        self.all().into_iter().filter(|p| q.is_empty() || fold(&p.name).contains(&q) || fold(&p.category).contains(&q)).collect()
    }

    fn taken(&self, category: &str, name: &str) -> bool {
        self.all().iter().any(|p| p.category.eq_ignore_ascii_case(category) && p.name.eq_ignore_ascii_case(name))
    }

    /// Nombre libre en la categoría: «Nombre», «Nombre (2)», …
    pub fn unique_name(&self, category: &str, wanted: &str) -> String {
        let base = if wanted.trim().is_empty() { "Pincel" } else { wanted.trim() };
        if !self.taken(category, base) {
            return base.to_string();
        }
        (2..).map(|n| format!("{base} ({n})")).find(|c| !self.taken(category, c)).unwrap_or_default()
    }

    /// Guarda una receta como pincel nuevo del usuario.
    pub fn save_new(&mut self, mut preset: VPreset) -> String {
        preset = preset.clamped();
        preset.builtin = false;
        preset.name = self.unique_name(&preset.category, &preset.name);
        preset.style.brush = Some(preset.id());
        let id = preset.id();
        self.user.push(preset);
        id
    }

    /// Cambia la receta de un pincel. En uno incluido se guarda como edición (se puede restaurar).
    pub fn update(&mut self, id: &str, size: f32, opacity: f32, smoothing: f32, style: &VStyle) -> Result<(), String> {
        let cur = self.find(id).ok_or("Ese pincel ya no existe")?;
        let mut new = VPreset { size, opacity, smoothing, style: style.clone(), ..cur.clone() }.clamped();
        new.style.brush = Some(id.to_string());
        new.builtin = false;
        if cur.builtin {
            match self.edited.iter_mut().find(|e| e.id() == id) {
                Some(e) => *e = new,
                None => self.edited.push(new),
            }
        } else if let Some(u) = self.user.iter_mut().find(|u| u.id() == id) {
            *u = new;
        }
        Ok(())
    }

    /// Devuelve un pincel incluido a su configuración original.
    pub fn restore(&mut self, id: &str) -> Result<(), String> {
        if Self::original(id).is_none() {
            return Err("Solo los pinceles incluidos se pueden restaurar".into());
        }
        if !self.is_edited(id) {
            return Err("Ese pincel ya tiene su configuración original".into());
        }
        self.edited.retain(|e| e.id() != id);
        Ok(())
    }

    /// Devuelve todos los pinceles incluidos a su configuración original (no toca los del usuario).
    pub fn restore_all(&mut self) -> usize {
        let n = self.edited.len();
        self.edited.clear();
        n
    }

    pub fn duplicate(&mut self, id: &str) -> Result<String, String> {
        let src = self.find(id).ok_or("Ese pincel ya no existe")?;
        // La copia es del usuario; si el original es incluido, va a «Pinceles personalizados».
        let category = if src.builtin { USER_CATEGORY.to_string() } else { src.category.clone() };
        let name = format!("{} (copia)", src.name);
        Ok(self.save_new(VPreset { name, category, builtin: false, ..src }))
    }

    pub fn rename(&mut self, id: &str, new_name: &str) -> Result<String, String> {
        let i = self.user.iter().position(|p| p.id() == id).ok_or("Solo los pinceles propios se renombran")?;
        let name = new_name.trim();
        if name.is_empty() {
            return Err("Escribe un nombre".into());
        }
        let cat = self.user[i].category.clone();
        if !name.eq_ignore_ascii_case(&self.user[i].name) && self.taken(&cat, name) {
            return Err(format!("Ya existe «{name}» en «{cat}»"));
        }
        self.reid(i, name, &cat, id)
    }

    pub fn set_category(&mut self, id: &str, category: &str) -> Result<String, String> {
        let i = self.user.iter().position(|p| p.id() == id).ok_or("Solo los pinceles propios cambian de categoría")?;
        let cat = category.trim();
        if cat.is_empty() {
            return Err("Escribe una categoría".into());
        }
        let name = self.user[i].name.clone();
        if !cat.eq_ignore_ascii_case(&self.user[i].category) && self.taken(cat, &name) {
            return Err(format!("Ya existe «{name}» en «{cat}»"));
        }
        self.reid(i, &name, cat, id)
    }

    /// Cambia nombre y categoría de un pincel propio y mantiene su favorito.
    fn reid(&mut self, i: usize, name: &str, cat: &str, old: &str) -> Result<String, String> {
        self.user[i].name = name.to_string();
        self.user[i].category = cat.to_string();
        let new = self.user[i].id();
        self.user[i].style.brush = Some(new.clone());
        for f in self.favorites.iter_mut().filter(|f| f.as_str() == old) {
            *f = new.clone();
        }
        Ok(new)
    }

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let i = self.user.iter().position(|p| p.id() == id).ok_or("Los pinceles incluidos no se eliminan")?;
        self.user.remove(i);
        self.favorites.retain(|f| f != id);
        Ok(())
    }

    /// Deja la biblioteca válida (valores en rango, sin duplicados, favoritos que existen).
    pub fn sanitize(&mut self) {
        let mut seen: Vec<String> = Vec::new();
        let mut keep = Vec::new();
        for u in std::mem::take(&mut self.user) {
            let mut u = u.clamped();
            u.builtin = false;
            u.style.brush = Some(u.id());
            let key = u.id().to_lowercase();
            if !seen.contains(&key) && Self::original(&u.id()).is_none() {
                seen.push(key);
                keep.push(u);
            }
        }
        self.user = keep;
        let mut ed = Vec::new();
        for e in std::mem::take(&mut self.edited) {
            let mut e = e.clamped();
            e.builtin = false;
            e.style.brush = Some(e.id());
            if Self::original(&e.id()).is_some() && !ed.iter().any(|x: &VPreset| x.id() == e.id()) {
                ed.push(e);
            }
        }
        self.edited = ed;
        let all: Vec<String> = self.all().iter().map(|p| p.id()).collect();
        self.favorites.retain(|f| all.contains(f));
        self.favorites.dedup();
        self.version = VERSION;
    }
}

fn fold(s: &str) -> String {
    s.to_lowercase().chars().map(|c| match c {
        'á' => 'a',
        'é' => 'e',
        'í' => 'i',
        'ó' => 'o',
        'ú' | 'ü' => 'u',
        c => c,
    }).collect()
}

/// Lee la biblioteca de la carpeta `dir`; si no existe o está dañada, devuelve una vacía (nunca falla).
pub fn load(dir: &Path) -> VLibrary {
    let mut lib: VLibrary = std::fs::read_to_string(dir.join(FILE)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    lib.sanitize();
    lib
}

/// Guarda la biblioteca (primero en un archivo temporal, para no dejarla a medias).
pub fn save(dir: &Path, lib: &VLibrary) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&VLibrary { version: VERSION, ..lib.clone() }).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join(FILE)).map_err(|e| e.to_string())
}

/// Archivo de intercambio `.ltvb`.
#[derive(Serialize, Deserialize)]
struct Pack {
    format: String,
    version: u32,
    presets: Vec<VPreset>,
}

/// Escribe pinceles en el formato nativo.
pub fn export_pack(presets: &[VPreset]) -> Result<String, String> {
    if presets.is_empty() {
        return Err("No hay pinceles que exportar".into());
    }
    serde_json::to_string_pretty(&Pack { format: FORMAT.into(), version: VERSION, presets: presets.to_vec() }).map_err(|e| e.to_string())
}

/// Lee un archivo `.ltvb`. Rechaza archivos de otro tipo (por ejemplo la biblioteca de pinceles de píxeles).
pub fn import_pack(text: &str) -> Result<Vec<VPreset>, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|_| "El archivo no es un paquete de pinceles vectoriales".to_string())?;
    if v.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
        return Err("Ese archivo no es de pinceles vectoriales de LeonToon (los pinceles de píxeles y los .abr van en capas de mapa de bits)".into());
    }
    let version = v.get("version").and_then(|n| n.as_u64()).unwrap_or(0);
    if version == 0 || version > VERSION as u64 {
        return Err(format!("El paquete es de una versión más nueva ({version}); actualiza LeonToon para leerlo"));
    }
    let pack: Pack = serde_json::from_value(v).map_err(|e| format!("Paquete dañado: {e}"))?;
    Ok(pack.presets.into_iter().map(|p| VPreset { builtin: false, ..p.clamped() }).collect())
}

/// Añade pinceles importados a la biblioteca (con nombre libre si chocan). Devuelve cuántos entraron.
pub fn merge(lib: &mut VLibrary, presets: Vec<VPreset>) -> usize {
    let n = presets.len();
    for p in presets {
        lib.save_new(p);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> VLibrary {
        VLibrary::default()
    }

    #[test]
    fn builtin_library_is_valid_and_organized() {
        let all = builtin();
        let mut ids: Vec<String> = all.iter().map(|p| p.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "ids repetidos");
        for p in &all {
            assert!(p.builtin);
            assert_eq!(p.clone().clamped().style, p.style, "{} fuera de rango", p.name);
            assert!(ORDER.contains(&p.category.as_str()), "categoría sin orden: {}", p.category);
        }
        for c in ORDER {
            assert!(all.iter().any(|p| p.category == c), "categoría vacía: {c}");
        }
        assert!(all.len() >= 18);
    }

    #[test]
    fn create_duplicate_rename_move_remove() {
        let mut l = lib();
        let mut r = VPreset { name: "Mío".into(), ..Default::default() };
        r.style.profile = Profile::TaperBoth;
        let id = l.save_new(r.clone());
        assert_eq!(id, format!("{USER_CATEGORY}/Mío"));
        assert_eq!(l.find(&id).unwrap().style.brush.as_deref(), Some(id.as_str()));
        // Mismo nombre: se numera.
        assert!(l.save_new(r).ends_with("Mío (2)"));
        let copy = l.duplicate(&id).unwrap();
        assert!(copy.ends_with("Mío (copia)"));
        l.toggle_favorite(&copy);
        let new = l.rename(&copy, "Otro").unwrap();
        assert!(l.is_favorite(&new) && !l.is_favorite(&copy), "el favorito sigue al pincel");
        assert!(l.rename(&new, "Mío").is_err(), "no pisa otro pincel");
        let moved = l.set_category(&new, "Mis tintas").unwrap();
        assert!(l.categories().contains(&"Mis tintas".to_string()));
        l.remove(&moved).unwrap();
        assert!(l.find(&moved).is_none() && !l.is_favorite(&moved));
    }

    #[test]
    fn included_brushes_are_protected_and_restorable() {
        let mut l = lib();
        let id = "Tinta/Tinta afilada".to_string();
        let original = l.find(&id).unwrap();
        assert!(l.remove(&id).is_err());
        assert!(l.rename(&id, "x").is_err());
        assert!(l.restore(&id).is_err(), "sin editar no hay qué restaurar");
        let mut s = original.style.clone();
        s.cap = Cap::Flat;
        l.update(&id, 20.0, 0.5, 0.1, &s).unwrap();
        let edited = l.find(&id).unwrap();
        assert!(edited.builtin && edited.size == 20.0 && edited.style.cap == Cap::Flat);
        assert_eq!(VLibrary::original(&id).unwrap(), original, "el original sigue intacto");
        assert_eq!(l.all().len(), builtin().len(), "la edición no duplica el pincel");
        l.restore(&id).unwrap();
        assert_eq!(l.find(&id).unwrap(), original);
    }

    #[test]
    fn editing_a_preset_never_changes_drawn_strokes() {
        let mut l = lib();
        let id = l.save_new(VPreset { name: "Prueba".into(), ..Default::default() });
        let p = l.find(&id).unwrap();
        // Un trazo copia el estilo al dibujarse.
        let stroke_style = p.style.clone();
        let mut s = p.style.clone();
        s.join = Join::Miter;
        l.update(&id, 30.0, 1.0, 0.0, &s).unwrap();
        assert_eq!(stroke_style.join, Join::Round);
        assert_eq!(l.find(&id).unwrap().style.join, Join::Miter);
        l.remove(&id).unwrap();
        assert_eq!(stroke_style.brush.as_deref(), Some(id.as_str()), "el trazo conserva su identificador aunque se borre el pincel");
    }

    #[test]
    fn search_ignores_case_and_accents() {
        let l = lib();
        assert!(l.search("tinta").iter().any(|p| p.name == "Tinta afilada"));
        assert!(l.search("CALIGRAFIA").iter().any(|p| p.name == "Plumín"));
        assert!(l.search("ilustracion").len() >= 2);
        assert!(l.search("zzzz").is_empty());
        assert_eq!(l.search("").len(), builtin().len());
    }

    #[test]
    fn favorites_only_for_existing_brushes() {
        let mut l = lib();
        l.toggle_favorite("Nada/Nada");
        assert!(l.favorites.is_empty());
        l.toggle_favorite("Básicos/Redondo");
        assert!(l.is_favorite("Básicos/Redondo"));
        l.favorites.push("Roto/Roto".into());
        l.sanitize();
        assert_eq!(l.favorites, vec!["Básicos/Redondo".to_string()]);
    }

    #[test]
    fn pack_roundtrip_and_rejects_foreign_files() {
        let mut l = lib();
        let mut r = VPreset { name: "Viaje".into(), size: 12.0, opacity: 0.6, ..Default::default() };
        r.style.cap = Cap::Square;
        r.style.curve = 2.0;
        let id = l.save_new(r);
        let text = export_pack(&[l.find(&id).unwrap()]).unwrap();
        let back = import_pack(&text).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].style.cap, Cap::Square);
        assert!((back[0].style.curve - 2.0).abs() < 1e-6 && (back[0].size - 12.0).abs() < 1e-6);
        let mut other = lib();
        assert_eq!(merge(&mut other, back.clone()), 1);
        assert_eq!(merge(&mut other, back), 1);
        assert_eq!(other.user.len(), 2, "el choque de nombres se resuelve numerando");
        // Archivos que no son de pinceles vectoriales.
        assert!(import_pack("{\"version\":1,\"presets\":[],\"images\":[]}").is_err(), "biblioteca de píxeles");
        assert!(import_pack("no es json").is_err());
        assert!(import_pack(&text.replace("\"version\": 1", "\"version\": 99")).is_err(), "versión futura");
        assert!(export_pack(&[]).is_err());
    }

    #[test]
    fn damaged_values_are_repaired() {
        let json = format!("{{\"format\":\"{FORMAT}\",\"version\":1,\"presets\":[{{\"name\":\"  \",\"category\":\"\",\"size\":-5,\"opacity\":9,\"smoothing\":1e9,\"style\":{{\"curve\":0}}}}]}}");
        let p = &import_pack(&json).unwrap()[0];
        assert_eq!((p.name.as_str(), p.category.as_str()), ("Pincel", USER_CATEGORY));
        assert!(p.size >= 1.0 && p.opacity <= 1.0 && p.smoothing <= 0.85 && p.style.curve >= 0.2);
    }

    #[test]
    fn library_file_roundtrip_and_survives_garbage() {
        let dir = std::env::temp_dir().join(format!("ltvb_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut l = lib();
        let id = l.save_new(VPreset { name: "Guardado".into(), ..Default::default() });
        l.toggle_favorite(&id);
        l.update("Básicos/Redondo", 33.0, 1.0, 0.3, &VStyle::default()).unwrap();
        save(&dir, &l).unwrap();
        let back = load(&dir);
        assert_eq!(back.user.len(), 1);
        assert!(back.is_favorite(&id) && back.is_edited("Básicos/Redondo"));
        assert_eq!(back.find("Básicos/Redondo").unwrap().size, 33.0);
        std::fs::write(dir.join(FILE), "basura").unwrap();
        assert!(load(&dir).user.is_empty(), "un archivo dañado no rompe la app");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_recipe_ignores_origin_name() {
        let p = &builtin()[0];
        let mut s = p.style.clone();
        s.brush = Some("lo-que-sea".into());
        assert!(p.same_recipe(p.size, p.opacity, p.smoothing, &s));
        s.cap = Cap::Flat;
        assert!(!p.same_recipe(p.size, p.opacity, p.smoothing, &s));
    }
}