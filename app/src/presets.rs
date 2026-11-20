//! Pinceles guardados: configuración del pincel, presets con categorías, imágenes de punta y de textura,
//! y la biblioteca del usuario.
//!
//! Todo se guarda con `serde(default)` para poder añadir campos sin romper archivos antiguos.
//! El proyecto (`.ltoon`) lleva sus propios presets e imágenes, así que es autosuficiente; la biblioteca
//! del usuario (un archivo en la carpeta de datos del programa) solo sirve para repartir los presets
//! entre proyectos.

use crate::brush::{BrushParams, Control, Gray, TexKind, TexMode};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Categoría de los pinceles guardados por la persona cuando no elige otra.
pub const USER_CATEGORY: &str = "Mis pinceles";
/// Largo máximo de nombres y categorías.
const MAX_NAME: usize = 40;
/// Lado máximo de las imágenes guardadas.
const MAX_IMAGE_SIDE: u32 = 2048;

/// Todo lo que define un pincel: forma, dinámicas, efectos, tamaño, opacidad, suavizado e imágenes.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BrushSetup {
    pub params: BrushParams,
    pub size: f32,
    pub opacity: f32,
    pub smoothing: f32,
    /// Identificador de la imagen de la punta (en las imágenes del proyecto), si la hay.
    pub tip: Option<String>,
    /// Identificador de la imagen de la textura, si la hay.
    pub texture: Option<String>,
}

impl Default for BrushSetup {
    fn default() -> Self {
        BrushSetup { params: BrushParams::default(), size: 8.0, opacity: 1.0, smoothing: 0.35, tip: None, texture: None }
    }
}

impl BrushSetup {
    /// Valores dentro de los rangos que acepta la interfaz.
    pub fn clamped(mut self) -> Self {
        self.params = self.params.clamped();
        self.size = if self.size.is_finite() { self.size.clamp(1.0, 300.0) } else { 8.0 };
        self.opacity = if self.opacity.is_finite() { self.opacity.clamp(0.05, 1.0) } else { 1.0 };
        self.smoothing = if self.smoothing.is_finite() { self.smoothing.clamp(0.0, 0.85) } else { 0.35 };
        self
    }
}

/// Un pincel guardado, con nombre y categoría.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct BrushPreset {
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub setup: BrushSetup,
    /// Los pinceles incluidos con el programa no se guardan en los archivos ni se pueden modificar.
    #[serde(skip)]
    pub builtin: bool,
}

/// Identificador de un preset: categoría y nombre.
pub type PresetKey = (String, String);

impl BrushPreset {
    pub fn key(&self) -> PresetKey {
        (self.category.clone(), self.name.clone())
    }
}

/// Pincel que estaba en uso al guardar el proyecto.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BrushState {
    pub setup: BrushSetup,
    /// Preset del que viene (categoría y nombre), si no se ha movido de él.
    pub preset: Option<PresetKey>,
}

/// Imagen de punta o de textura guardada en el proyecto: máscara en grises dentro de un PNG en base64.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct BrushImage {
    pub id: String,
    pub name: String,
    pub w: u32,
    pub h: u32,
    /// PNG RGBA donde R = G = B = la máscara y A = 255 (así se recupera exacta).
    pub png: String,
}

/// Identificador de una imagen: huella de su contenido (la misma imagen no se guarda dos veces).
pub fn gray_id(g: &Gray) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for b in g.w.to_le_bytes().into_iter().chain(g.h.to_le_bytes()).chain(g.data.iter().copied()) {
        feed(b);
    }
    format!("{h:016x}")
}

impl BrushImage {
    pub fn from_gray(name: &str, g: &Gray) -> Result<Self, String> {
        let size = tiny_skia::IntSize::from_wh(g.w, g.h).ok_or("Imagen vacía")?;
        let mut rgba = Vec::with_capacity(g.data.len() * 4);
        for v in &g.data {
            rgba.extend_from_slice(&[*v, *v, *v, 255]);
        }
        let pm = tiny_skia::Pixmap::from_vec(rgba, size).ok_or("Datos de imagen incorrectos")?;
        let png = pm.encode_png().map_err(|e| format!("No se pudo guardar la imagen: {e}"))?;
        Ok(BrushImage { id: gray_id(g), name: name.chars().take(MAX_NAME).collect(), w: g.w, h: g.h, png: crate::raster::base64_encode(&png) })
    }

    pub fn to_gray(&self) -> Result<Gray, String> {
        let bytes = crate::raster::base64_decode(&self.png).ok_or("La imagen guardada está dañada")?;
        let pm = tiny_skia::Pixmap::decode_png(&bytes).map_err(|e| format!("La imagen guardada está dañada: {e}"))?;
        if pm.width() > MAX_IMAGE_SIDE || pm.height() > MAX_IMAGE_SIDE {
            return Err("La imagen guardada es demasiado grande".into());
        }
        Ok(Gray { w: pm.width(), h: pm.height(), data: pm.pixels().iter().map(|p| p.red()).collect() })
    }
}

// ---------------- Pinceles incluidos ----------------

fn builtin_preset(category: &str, name: &str, size: f32, opacity: f32, params: BrushParams) -> BrushPreset {
    BrushPreset { name: name.into(), category: category.into(), setup: BrushSetup { params, size, opacity, ..Default::default() }, builtin: true }
}

/// Los pinceles que trae el programa, en cuatro categorías.
pub fn builtin() -> Vec<BrushPreset> {
    let d = BrushParams::default();
    vec![
        builtin_preset("Básicos", "Redondo duro", 12.0, 1.0, d),
        builtin_preset("Básicos", "Redondo suave", 40.0, 1.0, BrushParams { hardness: 0.1, flow: 0.6, spacing: 8.0, ..d }),
        builtin_preset("Básicos", "Lápiz", 4.0, 1.0, BrushParams { size_control: Control::Pressure, min_size: 0.3, spacing: 5.0, ..d }),
        builtin_preset("Básicos", "Marcador", 30.0, 1.0, BrushParams { hardness: 0.9, flow: 0.35, spacing: 6.0, ..d }),
        builtin_preset("Entintado", "Tinta con presión", 14.0, 1.0, BrushParams { size_control: Control::Pressure, min_size: 0.1, spacing: 6.0, ..d }),
        builtin_preset("Entintado", "Plumilla", 18.0, 1.0, BrushParams { roundness: 0.3, angle: 45.0, spacing: 5.0, ..d }),
        builtin_preset("Pintura", "Acuarela", 54.0, 0.9, BrushParams { wet_edges: 1.0, hardness: 0.35, flow: 0.5, spacing: 8.0, ..d }),
        builtin_preset("Pintura", "Óleo con grano", 44.0, 1.0, BrushParams { texture: TexKind::Paper, texture_depth: 0.9, texture_mode: TexMode::Multiply, hardness: 0.7, spacing: 12.0, ..d }),
        builtin_preset("Pintura", "Pincel seco", 46.0, 1.0, BrushParams { dual: true, dual_size: 0.18, dual_count: 18, spacing: 14.0, ..d }),
        builtin_preset("Efectos", "Salpicado", 14.0, 1.0, BrushParams { scatter: 2.5, scatter_count: 4, count_jitter: 0.4, spacing: 35.0, size_jitter: 0.5, ..d }),
        builtin_preset("Efectos", "Confeti", 34.0, 1.0, BrushParams { hue_jitter: 0.6, bri_jitter: 0.3, spacing: 70.0, size_jitter: 0.3, ..d }),
        builtin_preset("Efectos", "Hojas al viento", 30.0, 1.0, BrushParams { angle_jitter: 1.0, roundness: 0.5, roundness_jitter: 0.7, min_roundness: 0.15, size_jitter: 0.6, flow_jitter: 0.5, spacing: 45.0, ..d }),
    ]
}

// ---------------- Operaciones sobre la lista del usuario ----------------

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NAME).collect()
}

/// `true` si ya hay un pincel (incluido o del usuario) con esa categoría y ese nombre.
fn taken(user: &[BrushPreset], category: &str, name: &str) -> bool {
    let same = |p: &BrushPreset| p.category.eq_ignore_ascii_case(category) && p.name.eq_ignore_ascii_case(name);
    user.iter().any(same) || builtin().iter().any(same)
}

/// Un nombre libre en la categoría: el pedido, o con « 2», « 3»… detrás.
pub fn unique_name(user: &[BrushPreset], category: &str, wanted: &str) -> String {
    let base = clean(wanted);
    let base = if base.is_empty() { "Pincel".to_string() } else { base };
    if !taken(user, category, &base) {
        return base;
    }
    let stem: String = base.chars().take(MAX_NAME.saturating_sub(5)).collect();
    (2..)
        .map(|n| format!("{stem} {n}"))
        .find(|n| !taken(user, category, n))
        .unwrap_or(base)
}

/// Busca un pincel por su clave entre los incluidos y los del usuario.
pub fn find(user: &[BrushPreset], key: &PresetKey) -> Option<BrushPreset> {
    user.iter().chain(builtin().iter()).find(|p| p.category == key.0 && p.name == key.1).cloned()
}

fn user_index(user: &[BrushPreset], key: &PresetKey) -> Result<usize, String> {
    if let Some(i) = user.iter().position(|p| p.category == key.0 && p.name == key.1) {
        return Ok(i);
    }
    if builtin().iter().any(|p| p.category == key.0 && p.name == key.1) {
        return Err("Los pinceles incluidos no se pueden modificar: duplícalo o guarda uno nuevo".into());
    }
    Err("Ese pincel ya no existe".into())
}

fn clean_category(c: &str) -> String {
    let c = clean(c);
    if c.is_empty() {
        USER_CATEGORY.to_string()
    } else {
        c
    }
}

/// Guarda la configuración como pincel nuevo (con un nombre libre si el pedido ya existe).
pub fn save_new(user: &mut Vec<BrushPreset>, name: &str, category: &str, setup: BrushSetup) -> PresetKey {
    let category = clean_category(category);
    let name = unique_name(user, &category, name);
    user.push(BrushPreset { name: name.clone(), category: category.clone(), setup: setup.clamped(), builtin: false });
    (category, name)
}

/// Copia un pincel (incluido o del usuario). Los incluidos se copian a «Mis pinceles».
pub fn duplicate(user: &mut Vec<BrushPreset>, key: &PresetKey) -> Result<PresetKey, String> {
    let src = find(user, key).ok_or("Ese pincel ya no existe")?;
    let category = if src.builtin { USER_CATEGORY.to_string() } else { src.category.clone() };
    let name = unique_name(user, &category, &format!("{} (copia)", src.name));
    user.push(BrushPreset { name: name.clone(), category: category.clone(), setup: src.setup, builtin: false });
    Ok((category, name))
}

pub fn rename(user: &mut Vec<BrushPreset>, key: &PresetKey, new_name: &str) -> Result<PresetKey, String> {
    let i = user_index(user, key)?;
    let new_name = clean(new_name);
    if new_name.is_empty() {
        return Err("El nombre no puede estar vacío".into());
    }
    if new_name.eq_ignore_ascii_case(&key.1) {
        user[i].name = new_name.clone();
        return Ok((key.0.clone(), new_name));
    }
    if taken(user, &key.0, &new_name) {
        return Err(format!("Ya hay un pincel «{new_name}» en la categoría «{}»", key.0));
    }
    user[i].name = new_name.clone();
    Ok((key.0.clone(), new_name))
}

pub fn set_category(user: &mut Vec<BrushPreset>, key: &PresetKey, category: &str) -> Result<PresetKey, String> {
    let i = user_index(user, key)?;
    let category = clean_category(category);
    if category.eq_ignore_ascii_case(&key.0) {
        user[i].category = category.clone();
        return Ok((category, key.1.clone()));
    }
    if taken(user, &category, &key.1) {
        return Err(format!("Ya hay un pincel «{}» en la categoría «{category}»", key.1));
    }
    user[i].category = category.clone();
    Ok((category, key.1.clone()))
}

pub fn remove(user: &mut Vec<BrushPreset>, key: &PresetKey) -> Result<(), String> {
    let i = user_index(user, key)?;
    user.remove(i);
    Ok(())
}

/// Reemplaza la configuración de un pincel del usuario por la actual.
pub fn update(user: &mut [BrushPreset], key: &PresetKey, setup: BrushSetup) -> Result<(), String> {
    let i = user_index(user, key)?;
    user[i].setup = setup.clamped();
    Ok(())
}

/// Categorías en orden: primero las de los pinceles incluidos y luego las nuevas del usuario.
pub fn categories(user: &[BrushPreset]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in builtin().iter().chain(user.iter()) {
        if !out.iter().any(|c| c.eq_ignore_ascii_case(&p.category)) {
            out.push(p.category.clone());
        }
    }
    out
}

// ---------------- Reparación al abrir ----------------

/// Deja presets e imágenes en un estado válido tras abrir un archivo (rangos, repetidos, imágenes que faltan).
pub fn sanitize(user: &mut Vec<BrushPreset>, images: &mut Vec<BrushImage>) {
    let mut seen_img: Vec<String> = Vec::new();
    images.retain(|im| {
        let ok = !im.id.is_empty() && im.w > 0 && im.h > 0 && im.w <= MAX_IMAGE_SIDE && im.h <= MAX_IMAGE_SIDE && !seen_img.contains(&im.id);
        if ok {
            seen_img.push(im.id.clone());
        }
        ok
    });
    let have = |id: &Option<String>| id.as_ref().map_or(true, |i| seen_img.contains(i));
    let mut keys: Vec<(String, String)> = Vec::new();
    user.retain_mut(|p| {
        p.name = clean(&p.name);
        p.category = clean_category(&p.category);
        p.builtin = false;
        let k = (p.category.to_lowercase(), p.name.to_lowercase());
        if p.name.is_empty() || keys.contains(&k) || builtin().iter().any(|b| b.category.eq_ignore_ascii_case(&p.category) && b.name.eq_ignore_ascii_case(&p.name)) {
            return false;
        }
        keys.push(k);
        p.setup = p.setup.clone().clamped();
        if !have(&p.setup.tip) {
            p.setup.tip = None;
            p.setup.params.use_tip_image = false;
        }
        if !have(&p.setup.texture) {
            p.setup.texture = None;
            if p.setup.params.texture == TexKind::Image {
                p.setup.params.texture = TexKind::None;
            }
        }
        true
    });
}

/// Repara el pincel en uso guardado en el proyecto.
pub fn sanitize_state(state: &mut Option<BrushState>, images: &[BrushImage]) {
    if let Some(s) = state {
        s.setup = s.setup.clone().clamped();
        let have = |id: &Option<String>| id.as_ref().map_or(true, |i| images.iter().any(|im| &im.id == i));
        if !have(&s.setup.tip) {
            s.setup.tip = None;
            s.setup.params.use_tip_image = false;
        }
        if !have(&s.setup.texture) {
            s.setup.texture = None;
            if s.setup.params.texture == TexKind::Image {
                s.setup.params.texture = TexKind::None;
            }
        }
    }
}

// ---------------- Biblioteca del usuario ----------------

/// Archivo de la biblioteca del usuario: presets y las imágenes que usan.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub version: u32,
    pub presets: Vec<BrushPreset>,
    pub images: Vec<BrushImage>,
}

const LIBRARY_FILE: &str = "pinceles.json";

/// Lee la biblioteca de la carpeta `dir`. Si no existe o está dañada, devuelve una vacía (nunca falla).
pub fn load_library(dir: &Path) -> Library {
    let mut lib: Library = std::fs::read_to_string(dir.join(LIBRARY_FILE)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    sanitize(&mut lib.presets, &mut lib.images);
    lib
}

/// Guarda la biblioteca (primero a un archivo temporal, para no dejarla a medias).
pub fn save_library(dir: &Path, lib: &Library) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string(&Library { version: 1, ..lib.clone() }).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{LIBRARY_FILE}.tmp"));
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join(LIBRARY_FILE)).map_err(|e| e.to_string())
}

/// Arma la biblioteca con los presets del proyecto y solo las imágenes que usan.
pub fn library_from(user: &[BrushPreset], images: &[BrushImage]) -> Library {
    let used = |id: &Option<String>| id.clone();
    let mut ids: Vec<String> = Vec::new();
    for p in user {
        ids.extend(used(&p.setup.tip));
        ids.extend(used(&p.setup.texture));
    }
    Library { version: 1, presets: user.to_vec(), images: images.iter().filter(|im| ids.contains(&im.id)).cloned().collect() }
}

/// Añade al proyecto los presets de la biblioteca que no tiene (por categoría y nombre), con sus imágenes.
/// Devuelve cuántos añadió. Los del proyecto mandan.
pub fn merge_library(user: &mut Vec<BrushPreset>, images: &mut Vec<BrushImage>, lib: Library) -> usize {
    let mut added = 0;
    for p in lib.presets {
        if taken(user, &p.category, &p.name) {
            continue;
        }
        for id in [&p.setup.tip, &p.setup.texture].into_iter().flatten() {
            if !images.iter().any(|im| &im.id == id) {
                if let Some(im) = lib.images.iter().find(|im| &im.id == id) {
                    images.push(im.clone());
                }
            }
        }
        user.push(p);
        added += 1;
    }
    sanitize(user, images);
    added
}

#[cfg(test)]
mod tests;
