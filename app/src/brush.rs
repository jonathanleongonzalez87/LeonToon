//! Motor de pinceles de píxeles.
//!
//! Separado de la interfaz y de la representación de los píxeles: recibe una lista de puntos
//! (con la presión del lápiz), coloca "marcas" a lo largo del trazo según el espaciado y las
//! acumula en un búfer del trazo. Al terminar, el búfer se compone sobre la capa con la opacidad del
//! trazo. Como en Photoshop, el **flujo** se acumula marca a marca y la **opacidad** es un tope.
//!
//! - Fase 1: forma de la punta (dureza, espaciado, flujo, redondez, ángulo).
//! - Fase 2: dinámica de forma y transferencia (variación y control por presión o inclinación).
//! - Fase 3: dispersión, textura, doble pincel, dinámica de color, bordes húmedos y puntas de imagen.
//!
//! `BrushParams` se guarda con `serde(default)` para poder añadir parámetros sin romper presets antiguos.
//! Las imágenes (punta y textura) no van dentro de los parámetros: se pasan aparte en `BrushSpec`.

use crate::raster::Raster;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// Qué controla una propiedad dinámica.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Control {
    /// Nada: la propiedad no cambia con el lápiz.
    #[default]
    Off,
    /// La presión del lápiz.
    Pressure,
    /// La inclinación del lápiz (solo con dispositivos que la den; si no, no tiene efecto).
    Tilt,
}

impl Control {
    pub const ALL: [Control; 3] = [Control::Off, Control::Pressure, Control::Tilt];
    pub fn label(self) -> &'static str {
        match self {
            Control::Off => "Ninguno",
            Control::Pressure => "Presión",
            Control::Tilt => "Inclinación",
        }
    }
}

/// Patrón de textura del pincel.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum TexKind {
    #[default]
    None,
    /// Ruido suave (nubes).
    Noise,
    /// Grano fino de papel.
    Paper,
    /// Tejido de lienzo.
    Canvas,
    /// Puntos de trama.
    Dots,
    /// Rayas horizontales.
    Lines,
    /// Imagen cargada por la persona (PNG o WebP), en mosaico.
    Image,
}

impl TexKind {
    pub const ALL: [TexKind; 7] = [TexKind::None, TexKind::Noise, TexKind::Paper, TexKind::Canvas, TexKind::Dots, TexKind::Lines, TexKind::Image];
    pub fn label(self) -> &'static str {
        match self {
            TexKind::None => "Ninguna",
            TexKind::Noise => "Ruido",
            TexKind::Paper => "Papel",
            TexKind::Canvas => "Lienzo",
            TexKind::Dots => "Puntos",
            TexKind::Lines => "Rayas",
            TexKind::Image => "Imagen…",
        }
    }
}

/// Cómo se combina la textura con la pintura.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum TexMode {
    /// La pintura se atenúa donde la textura es clara.
    #[default]
    Multiply,
    /// La textura quita pintura (deja huecos).
    Subtract,
    /// Queda lo más tenue entre la pintura y la textura.
    Darken,
}

impl TexMode {
    pub const ALL: [TexMode; 3] = [TexMode::Multiply, TexMode::Subtract, TexMode::Darken];
    pub fn label(self) -> &'static str {
        match self {
            TexMode::Multiply => "Multiplicar",
            TexMode::Subtract => "Restar",
            TexMode::Darken => "Oscurecer",
        }
    }
}

/// Parámetros de la punta, sus dinámicas y sus efectos. Tamaño, opacidad y suavizado son los de la herramienta.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BrushParams {
    /// Dureza del borde, 0 (difuso) a 1 (duro).
    pub hardness: f32,
    /// Espaciado entre marcas, en % del tamaño.
    pub spacing: f32,
    /// Flujo de cada marca, 0 a 1: cuánta pintura se acumula al pasar varias veces.
    pub flow: f32,
    /// Redondez de la punta, 0,05 (plana) a 1 (circular).
    pub roundness: f32,
    /// Ángulo de la punta en grados.
    pub angle: f32,
    /// Usar como punta la imagen cargada (la dureza no se aplica).
    pub use_tip_image: bool,

    // ---- Dinámica de forma ----
    pub size_jitter: f32,
    pub size_control: Control,
    pub min_size: f32,
    pub angle_jitter: f32,
    pub roundness_jitter: f32,
    pub min_roundness: f32,

    // ---- Transferencia ----
    pub opacity_jitter: f32,
    pub opacity_control: Control,
    /// Opacidad mínima de la marca cuando el control está al mínimo, 0 a 1.
    pub opacity_min: f32,
    pub flow_jitter: f32,
    pub flow_control: Control,
    /// Flujo mínimo de la marca cuando el control está al mínimo, 0 a 1.
    pub flow_min: f32,

    // ---- Dispersión ----
    /// Distancia de dispersión, como múltiplo del tamaño (0 = sin dispersión, 10 = 1000 %).
    pub scatter: f32,
    /// Si es `true`, se dispersa en los dos ejes; si no, solo perpendicular al trazo.
    pub scatter_both: bool,
    /// Marcas por posición.
    pub scatter_count: u8,
    /// Variación aleatoria del número de marcas, 0 a 1.
    pub count_jitter: f32,

    // ---- Textura ----
    pub texture: TexKind,
    /// Escala de la textura (1 = tamaño normal).
    pub texture_scale: f32,
    /// Profundidad: cuánto afecta la textura, 0 a 1.
    pub texture_depth: f32,
    pub texture_mode: TexMode,
    /// `true`: la textura se recoloca en cada marca; `false`: es continua en todo el trazo.
    pub texture_per_tip: bool,

    // ---- Doble pincel ----
    /// Una segunda punta recorta la primera (efecto de cerdas o grano).
    pub dual: bool,
    /// Tamaño de las marcas del segundo pincel, como fracción de la marca principal.
    pub dual_size: f32,
    /// Marcas del segundo pincel dentro de cada marca principal.
    pub dual_count: u8,
    /// Qué tanto se reparten dentro de la marca principal (1 = toda).
    pub dual_spread: f32,

    // ---- Dinámica de color ----
    pub hue_jitter: f32,
    pub sat_jitter: f32,
    pub bri_jitter: f32,
    /// Variación entre el color principal y el secundario.
    pub fg_bg_jitter: f32,

    // ---- Bordes húmedos ----
    /// Efecto acuarela: bordes más marcados y centro más ligero, 0 a 1.
    pub wet_edges: f32,
}

impl Default for BrushParams {
    fn default() -> Self {
        // Valores que dan una línea redonda y dura, igual que el trazo vectorial, sin dinámicas ni efectos.
        BrushParams {
            hardness: 1.0,
            spacing: 10.0,
            flow: 1.0,
            roundness: 1.0,
            angle: 0.0,
            use_tip_image: false,
            size_jitter: 0.0,
            size_control: Control::Off,
            min_size: 0.0,
            angle_jitter: 0.0,
            roundness_jitter: 0.0,
            min_roundness: 0.25,
            opacity_jitter: 0.0,
            opacity_control: Control::Off,
            opacity_min: 0.0,
            flow_jitter: 0.0,
            flow_control: Control::Off,
            flow_min: 0.0,
            scatter: 0.0,
            scatter_both: false,
            scatter_count: 1,
            count_jitter: 0.0,
            texture: TexKind::None,
            texture_scale: 1.0,
            texture_depth: 1.0,
            texture_mode: TexMode::Multiply,
            texture_per_tip: false,
            dual: false,
            dual_size: 0.35,
            dual_count: 12,
            dual_spread: 1.0,
            hue_jitter: 0.0,
            sat_jitter: 0.0,
            bri_jitter: 0.0,
            fg_bg_jitter: 0.0,
            wet_edges: 0.0,
        }
    }
}

impl BrushParams {
    /// Valores dentro de los rangos válidos (por si vienen de un archivo o de un preset).
    pub fn clamped(self) -> Self {
        let u = |v: f32| v.clamp(0.0, 1.0);
        BrushParams {
            hardness: u(self.hardness),
            spacing: self.spacing.clamp(1.0, 200.0),
            flow: self.flow.clamp(0.01, 1.0),
            roundness: self.roundness.clamp(0.05, 1.0),
            angle: self.angle.rem_euclid(360.0),
            size_jitter: u(self.size_jitter),
            min_size: u(self.min_size),
            angle_jitter: u(self.angle_jitter),
            roundness_jitter: u(self.roundness_jitter),
            min_roundness: self.min_roundness.clamp(0.05, 1.0),
            opacity_jitter: u(self.opacity_jitter),
            opacity_min: u(self.opacity_min),
            flow_jitter: u(self.flow_jitter),
            flow_min: u(self.flow_min),
            scatter: self.scatter.clamp(0.0, 10.0),
            scatter_count: self.scatter_count.clamp(1, 16),
            count_jitter: u(self.count_jitter),
            texture_scale: self.texture_scale.clamp(0.1, 8.0),
            texture_depth: u(self.texture_depth),
            dual_size: self.dual_size.clamp(0.05, 1.5),
            dual_count: self.dual_count.clamp(1, 40),
            dual_spread: self.dual_spread.clamp(0.0, 1.5),
            hue_jitter: u(self.hue_jitter),
            sat_jitter: u(self.sat_jitter),
            bri_jitter: u(self.bri_jitter),
            fg_bg_jitter: u(self.fg_bg_jitter),
            wet_edges: u(self.wet_edges),
            ..self
        }
    }

    /// `true` si alguna propiedad depende del lápiz, del azar o del color.
    pub fn has_dynamics(&self) -> bool {
        self.size_jitter > 0.0
            || self.size_control != Control::Off
            || self.angle_jitter > 0.0
            || self.roundness_jitter > 0.0
            || self.opacity_jitter > 0.0
            || self.opacity_control != Control::Off
            || self.flow_jitter > 0.0
            || self.flow_control != Control::Off
            || self.hue_jitter > 0.0
            || self.sat_jitter > 0.0
            || self.bri_jitter > 0.0
            || self.fg_bg_jitter > 0.0
    }

    /// `true` si hay algún efecto de la Fase 3 activo.
    pub fn has_effects(&self) -> bool {
        self.scatter > 0.0 || self.texture != TexKind::None || self.dual || self.wet_edges > 0.0 || self.use_tip_image
    }
}

/// Imagen en escala de grises (0 a 255) para puntas y texturas: 255 = pintura plena.
#[derive(Clone, Debug)]
pub struct Gray {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

impl Gray {
    /// Convierte una imagen RGBA: si tiene transparencia usa el alfa; si es opaca, lo oscuro es pintura.
    pub fn from_rgba(w: u32, h: u32, rgba: &[u8]) -> Option<Gray> {
        if w == 0 || h == 0 || rgba.len() != (w as usize) * (h as usize) * 4 {
            return None;
        }
        let any_alpha = rgba.chunks_exact(4).any(|p| p[3] < 250);
        let data = rgba
            .chunks_exact(4)
            .map(|p| if any_alpha { p[3] } else { 255 - ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8 })
            .collect();
        Some(Gray { w, h, data })
    }

    fn at(&self, x: i32, y: i32, wrap: bool) -> f32 {
        let (w, h) = (self.w as i32, self.h as i32);
        let (x, y) = if wrap { (x.rem_euclid(w), y.rem_euclid(h)) } else { (x.clamp(0, w - 1), y.clamp(0, h - 1)) };
        self.data[(y * w + x) as usize] as f32 / 255.0
    }

    /// Valor (0 a 1) con interpolación bilinear en coordenadas de píxel.
    pub fn sample(&self, x: f32, y: f32, wrap: bool) -> f32 {
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (ix, iy) = (x0 as i32, y0 as i32);
        let a = self.at(ix, iy, wrap) * (1.0 - fx) + self.at(ix + 1, iy, wrap) * fx;
        let b = self.at(ix, iy + 1, wrap) * (1.0 - fx) + self.at(ix + 1, iy + 1, wrap) * fx;
        a * (1.0 - fy) + b * fy
    }
}

/// Lado máximo de una imagen de punta o de textura.
const MAX_IMAGE_SIDE: u32 = 2048;

/// Carga una imagen PNG o WebP como máscara de punta o de textura.
pub fn load_gray(path: &Path) -> Result<Gray, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
    let (w, h, rgba) = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let pm = tiny_skia::Pixmap::decode_png(&bytes).map_err(|e| format!("PNG no válido: {e}"))?;
        let mut v = Vec::with_capacity(pm.pixels().len() * 4);
        for px in pm.pixels() {
            let c = px.demultiply();
            v.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
        }
        (pm.width(), pm.height(), v)
    } else {
        let img = image::load_from_memory(&bytes).map_err(|e| format!("Imagen no válida (usa PNG o WebP): {e}"))?.to_rgba8();
        (img.width(), img.height(), img.into_raw())
    };
    if w > MAX_IMAGE_SIDE || h > MAX_IMAGE_SIDE {
        return Err(format!("La imagen es demasiado grande ({w}×{h}); el máximo es {MAX_IMAGE_SIDE} px por lado"));
    }
    Gray::from_rgba(w, h, &rgba).ok_or_else(|| "La imagen está vacía".to_string())
}

/// Todo lo que hace falta para pintar un trazo.
#[derive(Clone, Debug)]
pub struct BrushSpec {
    /// Diámetro mayor de la punta, en píxeles.
    pub size: f32,
    /// Opacidad máxima del trazo completo, 0 a 1.
    pub opacity: f32,
    pub color: [u8; 3],
    /// Color secundario (para la variación entre principal y secundario).
    pub color2: [u8; 3],
    pub params: BrushParams,
    /// Semilla de las variaciones aleatorias (la misma semilla repite el mismo trazo).
    pub seed: u64,
    /// Imagen de la punta, si `params.use_tip_image`.
    pub tip: Option<Arc<Gray>>,
    /// Imagen de la textura, si `params.texture == Image`.
    pub texture_img: Option<Arc<Gray>>,
}

impl BrushSpec {
    pub fn new(size: f32, opacity: f32, color: [u8; 3], params: BrushParams, seed: u64) -> Self {
        BrushSpec { size, opacity, color, color2: color, params, seed, tip: None, texture_img: None }
    }
    pub fn with_color2(mut self, c: [u8; 3]) -> Self {
        self.color2 = c;
        self
    }
    pub fn with_images(mut self, tip: Option<Arc<Gray>>, texture: Option<Arc<Gray>>) -> Self {
        self.tip = tip;
        self.texture_img = texture;
        self
    }
}

/// Entrada del lápiz en un punto. Sin tableta: presión 1 e inclinación 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pen {
    /// 0 (sin presión) a 1 (máxima).
    pub pressure: f32,
    /// 0 (tumbado) a 1 (vertical). Sin dato del dispositivo, 1.
    pub tilt: f32,
}

impl Default for Pen {
    fn default() -> Self {
        Pen { pressure: 1.0, tilt: 1.0 }
    }
}

/// Generador de números pseudoaleatorios (xorshift): rápido y repetible.
#[derive(Clone, Copy)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    /// Número entre 0 y 1.
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        ((x >> 40) as f32) / ((1u64 << 24) as f32)
    }
}

/// Qué tanto influye el control: 1 sin control o sin dato del dispositivo.
fn control_factor(c: Control, pen: Pen) -> f32 {
    match c {
        Control::Off => 1.0,
        Control::Pressure => pen.pressure.clamp(0.0, 1.0),
        Control::Tilt => pen.tilt.clamp(0.0, 1.0),
    }
}

fn rgb_to_hsv(c: [f32; 3]) -> [f32; 3] {
    let (r, g, b) = (c[0], c[1], c[2]);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= 1e-6 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    [h, if max <= 1e-6 { 0.0 } else { d / max }, max]
}

fn hsv_to_rgb(c: [f32; 3]) -> [f32; 3] {
    let (h, s, v) = (c[0].rem_euclid(1.0) * 6.0, c[1].clamp(0.0, 1.0), c[2].clamp(0.0, 1.0));
    let f = h - h.floor();
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match h.floor() as i32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// Ruido de valor: hash entero suave en 0..1.
fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x85EB_CA6B) ^ (y as u32).wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / ((1u32 << 24) as f32)
}

fn value_noise(x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (ix, iy) = (x0 as i32, y0 as i32);
    let a = hash2(ix, iy) * (1.0 - sx) + hash2(ix + 1, iy) * sx;
    let b = hash2(ix, iy + 1) * (1.0 - sx) + hash2(ix + 1, iy + 1) * sx;
    a * (1.0 - sy) + b * sy
}

/// Cantidad de pintura que deja la textura en (x, y) (en píxeles ya escalados por la escala), 0 a 1.
fn texture_value(kind: TexKind, img: Option<&Gray>, x: f32, y: f32) -> f32 {
    match kind {
        TexKind::None => 1.0,
        TexKind::Noise => (value_noise(x / 6.0, y / 6.0) * 0.65 + value_noise(x / 2.5 + 31.0, y / 2.5 + 17.0) * 0.35).clamp(0.0, 1.0),
        TexKind::Paper => hash2((x / 1.6).floor() as i32, (y / 1.6).floor() as i32),
        TexKind::Canvas => (0.5 + 0.3 * (x * 0.9).sin() + 0.25 * (y * 0.9).sin() + 0.15 * (x * 0.9).sin() * (y * 0.9).sin()).clamp(0.0, 1.0),
        TexKind::Dots => {
            let cell = 7.0;
            let (fx, fy) = (x.rem_euclid(cell) - cell * 0.5, y.rem_euclid(cell) - cell * 0.5);
            let d = (fx * fx + fy * fy).sqrt();
            (1.0 - ((d - cell * 0.28) / 1.2)).clamp(0.0, 1.0)
        }
        TexKind::Lines => {
            let v = (y * 0.9).sin();
            ((v + 0.3) * 2.5).clamp(0.0, 1.0)
        }
        TexKind::Image => img.map_or(1.0, |g| g.sample(x - 0.5, y - 0.5, true)),
    }
}

/// Efecto de bordes húmedos sobre un valor de alfa: el centro se aligera y los bordes se marcan.
pub fn wet_alpha(a: f32, w: f32) -> f32 {
    if w <= 0.0 {
        return a;
    }
    let t = ((a - 0.55) / 0.45).clamp(0.0, 1.0);
    let s = t * t * (3.0 - 2.0 * t);
    let bump = (a * (1.0 - a) * 2.0).max(0.0);
    (a * (1.0 - 0.5 * w * s) + 0.5 * w * bump).clamp(0.0, 1.0)
}

/// Propiedades de una marca ya resueltas (con variaciones y control del lápiz).
#[derive(Clone, Copy, Debug)]
struct Mark {
    size: f32,
    angle: f32,
    roundness: f32,
    flow: f32,
    /// Tope de opacidad de esta marca, 0 a 1.
    cap: f32,
    color: [f32; 3],
    /// Desplazamiento de la textura en esta marca (si va por marca).
    tex_off: [f32; 2],
}

/// Trazo en curso: búfer del trazo + posición de la última marca.
pub struct LiveStroke {
    pub spec: BrushSpec,
    /// Pintura acumulada del trazo (RGBA premultiplicado, sin la opacidad del trazo ni los bordes húmedos).
    pub buf: Raster,
    last: Option<([f32; 2], Pen)>,
    to_next: f32,
    dir: [f32; 2],
    bounds: Option<[u32; 4]>,
    dirty: Option<[u32; 4]>,
    rng: Rng,
    /// Posiciones del trazo colocadas hasta ahora.
    pub dabs: u32,
    /// Marcas pintadas (con dispersión puede haber varias por posición).
    pub marks: u32,
}

fn union(a: Option<[u32; 4]>, b: [u32; 4]) -> Option<[u32; 4]> {
    Some(match a {
        None => b,
        Some(a) => [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])],
    })
}

impl LiveStroke {
    pub fn new(w: u32, h: u32, spec: BrushSpec) -> Self {
        let mut spec = spec;
        spec.params = spec.params.clamped();
        spec.opacity = spec.opacity.clamp(0.0, 1.0);
        spec.size = spec.size.max(0.5);
        if !spec.params.use_tip_image {
            spec.tip = None;
        }
        let seed = spec.seed;
        LiveStroke { spec, buf: Raster::new(w, h), last: None, to_next: 0.0, dir: [1.0, 0.0], bounds: None, dirty: None, rng: Rng::new(seed), dabs: 0, marks: 0 }
    }

    /// Añade un punto sin datos del lápiz (presión 1).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn push(&mut self, p: [f32; 2]) {
        self.push_with(p, Pen::default());
    }

    /// Añade un punto del trazo con la presión e inclinación del lápiz: coloca las marcas que
    /// correspondan entre el punto anterior y este (la presión se reparte entre los dos).
    pub fn push_with(&mut self, p: [f32; 2], pen: Pen) {
        let Some((l, lpen)) = self.last else {
            let step = self.stamp(p, pen);
            self.last = Some((p, pen));
            self.to_next = step;
            return;
        };
        let d = ((p[0] - l[0]).powi(2) + (p[1] - l[1]).powi(2)).sqrt();
        if d <= 0.0 {
            self.last = Some((l, pen));
            return;
        }
        self.dir = [(p[0] - l[0]) / d, (p[1] - l[1]) / d];
        let mut s = self.to_next;
        let mut n = 0;
        while s <= d && n < 5000 {
            let t = s / d;
            let at = [l[0] + (p[0] - l[0]) * t, l[1] + (p[1] - l[1]) * t];
            let mid = Pen { pressure: lpen.pressure + (pen.pressure - lpen.pressure) * t, tilt: lpen.tilt + (pen.tilt - lpen.tilt) * t };
            s += self.stamp(at, mid);
            n += 1;
        }
        self.to_next = (s - d).max(0.0);
        self.last = Some((p, pen));
    }

    /// Resuelve las propiedades de la próxima marca (variaciones al azar y control del lápiz).
    fn resolve(&mut self, pen: Pen) -> Mark {
        let p = self.spec.params;
        let mut size = self.spec.size;
        size *= 1.0 - p.size_jitter * self.rng.next();
        if p.size_control != Control::Off {
            let f = control_factor(p.size_control, pen);
            size *= p.min_size + (1.0 - p.min_size) * f;
        }
        let angle = p.angle + p.angle_jitter * 180.0 * (self.rng.next() * 2.0 - 1.0);
        let min_r = p.min_roundness.min(p.roundness);
        let roundness = (p.roundness - (p.roundness - min_r) * p.roundness_jitter * self.rng.next()).clamp(0.05, 1.0);
        let with_min = |c: Control, min: f32| if c == Control::Off { 1.0 } else { min + (1.0 - min) * control_factor(c, pen) };
        let flow = (p.flow * (1.0 - p.flow_jitter * self.rng.next()) * with_min(p.flow_control, p.flow_min)).clamp(0.0, 1.0);
        let cap = ((1.0 - p.opacity_jitter * self.rng.next()) * with_min(p.opacity_control, p.opacity_min)).clamp(0.0, 1.0);

        // Color: variación entre principal y secundario, y luego de tono, saturación y brillo.
        let fg = [self.spec.color[0] as f32 / 255.0, self.spec.color[1] as f32 / 255.0, self.spec.color[2] as f32 / 255.0];
        let mut color = fg;
        if p.fg_bg_jitter > 0.0 {
            let bg = [self.spec.color2[0] as f32 / 255.0, self.spec.color2[1] as f32 / 255.0, self.spec.color2[2] as f32 / 255.0];
            let t = p.fg_bg_jitter * self.rng.next();
            color = [fg[0] + (bg[0] - fg[0]) * t, fg[1] + (bg[1] - fg[1]) * t, fg[2] + (bg[2] - fg[2]) * t];
        }
        if p.hue_jitter > 0.0 || p.sat_jitter > 0.0 || p.bri_jitter > 0.0 {
            let mut hsv = rgb_to_hsv(color);
            hsv[0] += p.hue_jitter * 0.5 * (self.rng.next() * 2.0 - 1.0);
            hsv[1] = (hsv[1] + p.sat_jitter * (self.rng.next() * 2.0 - 1.0)).clamp(0.0, 1.0);
            hsv[2] = (hsv[2] + p.bri_jitter * (self.rng.next() * 2.0 - 1.0)).clamp(0.0, 1.0);
            color = hsv_to_rgb(hsv);
        }
        let tex_off = if p.texture != TexKind::None && p.texture_per_tip { [self.rng.next() * 997.0, self.rng.next() * 997.0] } else { [0.0, 0.0] };
        Mark { size: size.max(0.5), angle, roundness, flow, cap, color, tex_off }
    }

    /// Una posición del trazo: una o varias marcas (dispersión). Devuelve la distancia hasta la próxima.
    fn stamp(&mut self, c: [f32; 2], pen: Pen) -> f32 {
        self.dabs += 1;
        let p = self.spec.params;
        let scatter = p.scatter > 0.0;
        let n = if scatter { ((p.scatter_count as f32) * (1.0 - p.count_jitter * self.rng.next())).round().max(1.0) as u32 } else { 1 };
        let perp = [-self.dir[1], self.dir[0]];
        let mut step = 0.5;
        for i in 0..n {
            let m = self.resolve(pen);
            if i == 0 {
                step = (m.size * p.spacing / 100.0).max(0.5);
            }
            let mut at = c;
            if scatter {
                let a = (self.rng.next() * 2.0 - 1.0) * p.scatter * m.size;
                at[0] += perp[0] * a;
                at[1] += perp[1] * a;
                if p.scatter_both {
                    let b = (self.rng.next() * 2.0 - 1.0) * p.scatter * m.size;
                    at[0] += self.dir[0] * b;
                    at[1] += self.dir[1] * b;
                }
            }
            self.stamp_mark(at, m);
        }
        step
    }

    /// Máscara del doble pincel para una marca: segundas marcas repartidas dentro de la principal.
    fn dual_mask(&mut self, c: [f32; 2], r: f32, size: f32, org: [u32; 2], dim: [usize; 2]) -> Vec<f32> {
        let p = self.spec.params;
        let mut mask = vec![0.0f32; dim[0] * dim[1]];
        let mr = (size * p.dual_size * 0.5).max(0.5);
        for _ in 0..p.dual_count {
            let ang = self.rng.next() * std::f32::consts::TAU;
            let rad = self.rng.next().sqrt() * r * p.dual_spread;
            let mc = [c[0] + ang.cos() * rad, c[1] + ang.sin() * rad];
            let (bx0, bx1) = (((mc[0] - mr - 1.0).floor() as i32).max(org[0] as i32), ((mc[0] + mr + 1.0).ceil() as i32).min((org[0] as usize + dim[0]) as i32));
            let (by0, by1) = (((mc[1] - mr - 1.0).floor() as i32).max(org[1] as i32), ((mc[1] + mr + 1.0).ceil() as i32).min((org[1] as usize + dim[1]) as i32));
            for y in by0..by1 {
                for x in bx0..bx1 {
                    let d = ((x as f32 + 0.5 - mc[0]).powi(2) + (y as f32 + 0.5 - mc[1]).powi(2)).sqrt();
                    let cov = (mr - d + 0.5).clamp(0.0, 1.0);
                    let i = (y as usize - org[1] as usize) * dim[0] + (x as usize - org[0] as usize);
                    if cov > mask[i] {
                        mask[i] = cov;
                    }
                }
            }
        }
        mask
    }

    /// Una marca de la punta centrada en `c`.
    fn stamp_mark(&mut self, c: [f32; 2], m: Mark) {
        self.marks += 1;
        let r = (m.size * 0.5).max(0.25);
        let p = self.spec.params;
        let (sa, ca) = m.angle.to_radians().sin_cos();
        let ext = r + 1.5;
        let (bw, bh) = (self.buf.w as f32, self.buf.h as f32);
        let x0 = (c[0] - ext).floor().clamp(0.0, bw) as u32;
        let y0 = (c[1] - ext).floor().clamp(0.0, bh) as u32;
        let x1 = (c[0] + ext).ceil().clamp(0.0, bw) as u32;
        let y1 = (c[1] + ext).ceil().clamp(0.0, bh) as u32;
        if x1 <= x0 || y1 <= y0 || m.flow <= 0.0 || m.cap <= 0.0 {
            return;
        }
        let minor = (r * m.roundness).max(0.25);
        let aa = 1.0 / minor;
        let dim = [(x1 - x0) as usize, (y1 - y0) as usize];
        let dual = if p.dual { Some(self.dual_mask(c, r, m.size, [x0, y0], dim)) } else { None };
        let tex_on = p.texture != TexKind::None;
        let w = self.buf.w;
        let tip = self.spec.tip.clone();
        let tip_aspect = match &tip {
            Some(t) if t.w >= t.h => (1.0, t.h as f32 / t.w as f32),
            Some(t) => (t.w as f32 / t.h as f32, 1.0),
            None => (1.0, 1.0),
        };
        let tex_img = self.spec.texture_img.clone();
        for y in y0..y1 {
            for x in x0..x1 {
                let (dx, dy) = (x as f32 + 0.5 - c[0], y as f32 + 0.5 - c[1]);
                let u = dx * ca + dy * sa;
                let v = -dx * sa + dy * ca;
                let mut cover = if let Some(t) = &tip {
                    // La imagen mantiene su proporción: el lado mayor mide el diámetro de la marca.
                    let (ax, ay) = tip_aspect;
                    let (tu, tv) = (u / (r * ax), v / (r * ay * m.roundness));
                    if tu.abs() >= 1.0 || tv.abs() >= 1.0 {
                        continue;
                    }
                    t.sample((tu * 0.5 + 0.5) * t.w as f32 - 0.5, (tv * 0.5 + 0.5) * t.h as f32 - 0.5, false)
                } else {
                    let d = ((u / r).powi(2) + (v / minor).powi(2)).sqrt();
                    if d >= 1.0 + aa {
                        continue;
                    }
                    if p.hardness >= 0.999 {
                        ((1.0 - d) / aa + 0.5).clamp(0.0, 1.0)
                    } else {
                        let t = ((d - p.hardness) / (1.0 - p.hardness)).clamp(0.0, 1.0);
                        1.0 - t * t * (3.0 - 2.0 * t)
                    }
                };
                if let Some(dm) = &dual {
                    cover *= dm[(y - y0) as usize * dim[0] + (x - x0) as usize];
                }
                let mut a = cover * m.flow;
                if tex_on && a > 0.0 {
                    let (tx, ty) = if p.texture_per_tip { (dx + m.tex_off[0], dy + m.tex_off[1]) } else { (x as f32 + 0.5, y as f32 + 0.5) };
                    let t = texture_value(p.texture, tex_img.as_deref(), tx / p.texture_scale, ty / p.texture_scale);
                    let depth = p.texture_depth;
                    a = match p.texture_mode {
                        TexMode::Multiply => a * (1.0 - depth + depth * t),
                        TexMode::Subtract => (a - (1.0 - t) * depth).max(0.0),
                        TexMode::Darken => a * (1.0 - depth) + depth * a.min(t),
                    };
                }
                if a <= 0.0 {
                    continue;
                }
                let i = ((y * w + x) * 4) as usize;
                let px = &mut self.buf.data[i..i + 4];
                let keep = 1.0 - a;
                let da = px[3] as f32 / 255.0;
                let over_a = a + da * keep;
                // La marca no sube la pintura por encima de su tope de opacidad (si ya había más, no la quita).
                let new_a = over_a.min(da.max(m.cap));
                let scale = if over_a > 0.0 { new_a / over_a } else { 0.0 };
                for k in 0..3 {
                    px[k] = (((m.color[k] * a + px[k] as f32 / 255.0 * keep) * scale) * 255.0).round().min(255.0) as u8;
                }
                px[3] = (new_a * 255.0).round().min(255.0) as u8;
            }
        }
        let b = [x0, y0, x1, y1];
        self.bounds = union(self.bounds, b);
        self.dirty = union(self.dirty, b);
    }

    /// Zona modificada desde la última vez (para subir solo eso a la textura de vista previa).
    pub fn take_dirty(&mut self) -> Option<[u32; 4]> {
        self.dirty.take()
    }

    /// Marca toda la zona pintada como pendiente de subir.
    pub fn mark_all_dirty(&mut self) {
        self.dirty = self.bounds;
    }

    /// Píxeles (RGBA premultiplicado) de la zona `b` tal como quedarán, con los bordes húmedos aplicados.
    pub fn region_rgba(&self, b: [u32; 4]) -> Vec<u8> {
        let (bw, bh) = ((b[2] - b[0]) as usize, (b[3] - b[1]) as usize);
        let wet = self.spec.params.wet_edges;
        let mut out = Vec::with_capacity(bw * bh * 4);
        for y in b[1]..b[3] {
            let i = ((y * self.buf.w + b[0]) * 4) as usize;
            let row = &self.buf.data[i..i + bw * 4];
            if wet <= 0.0 {
                out.extend_from_slice(row);
                continue;
            }
            for px in row.chunks_exact(4) {
                let a = px[3] as f32 / 255.0;
                if a <= 0.0 {
                    out.extend_from_slice(&[0, 0, 0, 0]);
                    continue;
                }
                let na = wet_alpha(a, wet);
                let k = na / a;
                out.extend_from_slice(&[(px[0] as f32 * k).round().min(255.0) as u8, (px[1] as f32 * k).round().min(255.0) as u8, (px[2] as f32 * k).round().min(255.0) as u8, (na * 255.0).round() as u8]);
            }
        }
        out
    }

    /// Compone el trazo sobre la capa con la opacidad del trazo (y los bordes húmedos, si hay).
    pub fn finish_into(&self, dst: &mut Raster) {
        if let Some(b) = self.bounds {
            dst.composite_over_wet(&self.buf, b, self.spec.opacity, self.spec.params.wet_edges);
        }
    }
}

/// Imagen de muestra de un pincel (un trazo ondulado) para el panel de parámetros.
/// Simula una presión que sube y baja para que se vea el efecto de los controles del lápiz.
pub fn preview(spec: BrushSpec, w: u32, h: u32) -> Raster {
    let mut ls = LiveStroke::new(w, h, BrushSpec { seed: 7, ..spec });
    let n = 90;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let x = 18.0 + t * (w as f32 - 36.0);
        let y = h as f32 * 0.5 + (t * std::f32::consts::TAU).sin() * h as f32 * 0.2;
        let pressure = (t * std::f32::consts::PI).sin().clamp(0.05, 1.0);
        ls.push_with([x, y], Pen { pressure, tilt: 1.0 });
    }
    let mut out = Raster::new(w, h);
    ls.finish_into(&mut out);
    out
}

#[cfg(test)]
mod tests;
