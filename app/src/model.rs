//! Datos del proyecto: escena, capas, fotogramas clave y trazos vectoriales.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type Rgba = [u8; 4];

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum ShapeKind {
    Free,
    Line,
    Rect,
    Ellipse,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Stroke {
    pub kind: ShapeKind,
    pub points: Vec<[f32; 2]>,
    pub color: Rgba,
    pub width: f32,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub fill: Option<Rgba>,
}

fn dist_seg(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let (apx, apy) = (p[0] - a[0], p[1] - a[1]);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 { ((apx * abx + apy * aby) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (cx, cy) = (a[0] + abx * t - p[0], a[1] + aby * t - p[1]);
    (cx * cx + cy * cy).sqrt()
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

pub fn point_in_polygon(p: [f32; 2], poly: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0];
            if p[0] < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Puntos de una figura (línea, rectángulo, elipse) entre dos esquinas.
pub fn shape_points(kind: ShapeKind, a: [f32; 2], b: [f32; 2]) -> (Vec<[f32; 2]>, bool) {
    match kind {
        ShapeKind::Free | ShapeKind::Line => (vec![a, b], false),
        ShapeKind::Rect => (vec![a, [b[0], a[1]], b, [a[0], b[1]]], true),
        ShapeKind::Ellipse => {
            let c = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
            let r = [(b[0] - a[0]).abs() * 0.5, (b[1] - a[1]).abs() * 0.5];
            let n = 72;
            let pts = (0..n)
                .map(|i| {
                    let t = i as f32 / n as f32 * std::f32::consts::TAU;
                    [c[0] + r[0] * t.cos(), c[1] + r[1] * t.sin()]
                })
                .collect();
            (pts, true)
        }
    }
}

impl Stroke {
    pub fn shape(kind: ShapeKind, a: [f32; 2], b: [f32; 2], color: Rgba, width: f32, fill: Option<Rgba>) -> Self {
        let (points, closed) = shape_points(kind, a, b);
        let fill = if closed { fill } else { None };
        Stroke { kind, points, color, width, closed, fill }
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        for p in &mut self.points {
            p[0] += dx;
            p[1] += dy;
        }
    }

    /// Caja que envuelve el trazo (sin grosor): [x0, y0, x1, y1].
    pub fn bounds(&self) -> [f32; 4] {
        self.points.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |b, p| {
            [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
        })
    }

    /// Escala respecto a un punto fijo (el grosor sigue la escala media).
    pub fn scale_about(&mut self, c: [f32; 2], sx: f32, sy: f32) {
        for p in &mut self.points {
            p[0] = c[0] + (p[0] - c[0]) * sx;
            p[1] = c[1] + (p[1] - c[1]) * sy;
        }
        self.width = (self.width * ((sx.abs() + sy.abs()) * 0.5)).clamp(0.1, 500.0);
    }

    /// Gira `deg` grados alrededor de `c`.
    pub fn rotate_about(&mut self, c: [f32; 2], deg: f32) {
        let (s, co) = deg.to_radians().sin_cos();
        for p in &mut self.points {
            let (x, y) = (p[0] - c[0], p[1] - c[1]);
            *p = [c[0] + x * co - y * s, c[1] + x * s + y * co];
        }
    }

    /// Distancia del punto al contorno del trazo (sin contar el grosor).
    pub fn outline_distance(&self, p: [f32; 2]) -> f32 {
        match self.points.len() {
            0 => f32::MAX,
            1 => dist(p, self.points[0]),
            n => {
                let mut d = f32::MAX;
                for i in 0..n - 1 {
                    d = d.min(dist_seg(p, self.points[i], self.points[i + 1]));
                }
                if self.closed {
                    d = d.min(dist_seg(p, self.points[n - 1], self.points[0]));
                }
                d
            }
        }
    }

    pub fn hits_outline(&self, p: [f32; 2], radius: f32) -> bool {
        self.outline_distance(p) <= radius + self.width * 0.5
    }

    pub fn hits_fill(&self, p: [f32; 2]) -> bool {
        self.fill.is_some() && self.closed && point_in_polygon(p, &self.points)
    }

    /// Borra la parte del trazo que toca el círculo (p, r). Devuelve los trozos que quedan.
    fn erase(&self, p: [f32; 2], r: f32) -> Option<Vec<Stroke>> {
        let touched = self.hits_outline(p, r) || self.hits_fill(p);
        if !touched {
            return None;
        }
        if self.fill.is_some() || self.points.len() <= 1 {
            return Some(Vec::new());
        }
        // Densificar la línea para poder cortar en cualquier punto.
        let mut src = self.points.clone();
        if self.closed {
            src.push(self.points[0]);
        }
        let step = (r * 0.4).max(0.5);
        let mut dense: Vec<[f32; 2]> = Vec::with_capacity(src.len() * 2);
        for w in src.windows(2) {
            let (a, b) = (w[0], w[1]);
            let n = (dist(a, b) / step).ceil().max(1.0) as usize;
            for i in 0..n {
                let t = i as f32 / n as f32;
                dense.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            }
        }
        dense.push(*src.last().unwrap());
        let limit = r + self.width * 0.5;
        let mut out = Vec::new();
        let mut run: Vec<[f32; 2]> = Vec::new();
        for q in dense {
            if dist(q, p) <= limit {
                if run.len() >= 2 {
                    out.push(std::mem::take(&mut run));
                } else {
                    run.clear();
                }
            } else {
                run.push(q);
            }
        }
        if run.len() >= 2 {
            out.push(run);
        }
        Some(
            out.into_iter()
                .map(|pts| Stroke {
                    kind: ShapeKind::Free,
                    points: simplify(&pts, 0.35),
                    color: self.color,
                    width: self.width,
                    closed: false,
                    fill: None,
                })
                .collect(),
        )
    }
}

/// Simplificación sencilla (Ramer–Douglas–Peucker).
pub fn simplify(pts: &[[f32; 2]], eps: f32) -> Vec<[f32; 2]> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((s, e)) = stack.pop() {
        let mut best = (0.0f32, 0usize);
        for i in s + 1..e {
            let d = dist_seg(pts[i], pts[s], pts[e]);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > eps {
            keep[best.1] = true;
            stack.push((s, best.1));
            stack.push((best.1, e));
        }
    }
    pts.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Drawing {
    pub strokes: Vec<Stroke>,
    /// Píxeles de las capas de mapa de bits (las capas vectoriales no lo usan).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster: Option<Arc<crate::raster::Raster>>,
}

impl Drawing {
    /// Sin trazos ni píxeles visibles.
    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty() && self.raster.as_ref().map_or(true, |r| r.is_blank())
    }

    pub fn erase(&mut self, p: [f32; 2], r: f32) -> bool {
        let mut changed = false;
        let mut out = Vec::with_capacity(self.strokes.len());
        for s in self.strokes.drain(..) {
            match s.erase(p, r) {
                Some(pieces) => {
                    changed = true;
                    out.extend(pieces);
                }
                None => out.push(s),
            }
        }
        self.strokes = out;
        changed
    }

    #[allow(dead_code)]
    pub fn translate(&mut self, dx: f32, dy: f32) {
        for s in &mut self.strokes {
            s.translate(dx, dy);
        }
    }

    /// Color visible en el punto (el trazo de más arriba primero).
    pub fn pick(&self, p: [f32; 2]) -> Option<Rgba> {
        for s in self.strokes.iter().rev() {
            if s.hits_outline(p, 1.0) {
                return Some(s.color);
            }
            if s.hits_fill(p) {
                return s.fill;
            }
        }
        self.raster.as_ref().and_then(|r| r.pick(p))
    }
}

/// Cómo se pasa de una clave de animación a la siguiente.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
pub enum Interp {
    #[default]
    Linear,
    Ease,
    Step,
}

/// Clave de animación de una capa: posición, giro (grados), escala y opacidad.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub struct XKey {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub s: f32,
    pub o: f32,
    #[serde(default)]
    pub i: Interp,
}

impl Default for XKey {
    fn default() -> Self {
        XKey { x: 0.0, y: 0.0, r: 0.0, s: 1.0, o: 1.0, i: Interp::Linear }
    }
}

impl XKey {
    pub fn is_identity(&self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.r == 0.0 && self.s == 1.0 && self.o == 1.0
    }
    /// Punto del dibujo → punto de la escena (giro y escala alrededor del centro de la cámara).
    pub fn apply(&self, w: f32, h: f32, p: [f32; 2]) -> [f32; 2] {
        let (x, y) = ((p[0] - w / 2.0) * self.s, (p[1] - h / 2.0) * self.s);
        let (sn, cs) = self.r.to_radians().sin_cos();
        [x * cs - y * sn + w / 2.0 + self.x, x * sn + y * cs + h / 2.0 + self.y]
    }
    /// Punto de la escena → punto del dibujo (lo contrario de `apply`).
    pub fn invert(&self, w: f32, h: f32, p: [f32; 2]) -> [f32; 2] {
        let (x, y) = (p[0] - w / 2.0 - self.x, p[1] - h / 2.0 - self.y);
        let (sn, cs) = (-self.r).to_radians().sin_cos();
        let s = if self.s.abs() < 1e-4 { 1e-4 } else { self.s };
        [(x * cs - y * sn) / s + w / 2.0, (x * sn + y * cs) / s + h / 2.0]
    }
}

/// Curva suave (cúbica de entrada y salida).
pub fn ease(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Plano del StoryBoard.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Panel {
    pub title: String,
    pub dur: f32,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub dialogue: String,
    #[serde(default)]
    pub camera: String,
    #[serde(default)]
    pub trans: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub drawing: Arc<Drawing>,
}

impl Panel {
    pub fn new(n: usize) -> Self {
        Panel { title: format!("Plano {n}"), dur: 2.0, ..Default::default() }
    }
}

fn default_plane() -> u8 {
    1
}

/// Distancia de cada plano pictórico (prototipo): BG al fondo, MG en el centro, FG delante.
pub fn plane_z(plane: u8) -> f32 {
    match plane {
        0 => 900.0,
        2 => -380.0,
        _ => 0.0,
    }
}

pub fn plane_name(plane: u8) -> &'static str {
    match plane {
        0 => "BG",
        2 => "FG",
        _ => "MG",
    }
}

/// Distancia de la cámara a la escena y su «zoom» (prototipo: 2400).
pub const CAM_DIST: f32 = 2400.0;

/// Tipo de capa (menú «Agregar capa», Mayús+A, del prototipo v0.3).
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
pub enum LayerKind {
    #[default]
    Vector,
    Bitmap,
    Solid,
    Null,
    Text,
    Light,
    Camera,
}

impl LayerKind {
    pub const ALL: [LayerKind; 7] = [
        LayerKind::Vector,
        LayerKind::Bitmap,
        LayerKind::Solid,
        LayerKind::Null,
        LayerKind::Text,
        LayerKind::Light,
        LayerKind::Camera,
    ];
    pub fn label(self) -> &'static str {
        match self {
            LayerKind::Vector => "Vectorial",
            LayerKind::Bitmap => "Mapa de bits",
            LayerKind::Solid => "Color sólido",
            LayerKind::Null => "Nulo",
            LayerKind::Text => "Texto",
            LayerKind::Light => "Luz",
            LayerKind::Camera => "Cámara",
        }
    }
    /// Letra de la etiqueta en la lista de capas.
    pub fn badge(self) -> &'static str {
        match self {
            LayerKind::Vector => "V",
            LayerKind::Bitmap => "B",
            LayerKind::Solid => "S",
            LayerKind::Null => "N",
            LayerKind::Text => "T",
            LayerKind::Light => "L",
            LayerKind::Camera => "C",
        }
    }
    /// Color de etiqueta del prototipo.
    pub fn tint(self) -> Rgba {
        match self {
            LayerKind::Vector => [0xfd, 0x67, 0x24, 255],
            LayerKind::Bitmap => [0x8f, 0xb3, 0xcf, 255],
            LayerKind::Solid => [0xc5, 0x8f, 0x5e, 255],
            LayerKind::Null | LayerKind::Camera => [0xb7, 0xa2, 0xe6, 255],
            LayerKind::Text => [0xe9, 0xa0, 0x7a, 255],
            LayerKind::Light => [0xf2, 0xd2, 0x7a, 255],
        }
    }
    /// Se crean y se guardan, pero aún no se dibujan ni actúan en la escena (necesitan 3D o texto).
    pub fn pending(self) -> bool {
        matches!(self, LayerKind::Text | LayerKind::Light | LayerKind::Null)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
    #[serde(default)]
    pub kind: LayerKind,
    /// Color de las capas de color sólido.
    #[serde(default)]
    pub solid: Rgba,
    /// Capa 3D (eje de profundidad Y del prototipo): plano 0 BG, 1 MG, 2 FG y distancia en px.
    #[serde(default)]
    pub is3d: bool,
    #[serde(default = "default_plane")]
    pub plane: u8,
    #[serde(default)]
    pub depth: f32,
    /// Fotogramas clave: cada dibujo se mantiene hasta la siguiente clave.
    /// Los dibujos se comparten (Arc) para que deshacer no copie todo el proyecto.
    pub keys: BTreeMap<u32, Arc<Drawing>>,
    /// Claves de animación (mover, girar, escalar, opacidad).
    #[serde(default)]
    pub xform: BTreeMap<u32, XKey>,
}

impl Layer {
    pub fn new(name: impl Into<String>) -> Self {
        Layer {
            name: name.into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            kind: LayerKind::Vector,
            solid: [246, 239, 226, 255],
            is3d: false,
            plane: 1,
            depth: 0.0,
            keys: BTreeMap::new(),
            xform: BTreeMap::new(),
        }
    }

    /// Distancia en profundidad: 0 para capas 2D.
    pub fn z(&self) -> f32 {
        if self.is3d || self.kind == LayerKind::Camera {
            self.depth
        } else {
            0.0
        }
    }

    /// Transformación de la capa en el fotograma `f`, interpolada entre claves.
    pub fn transform_at(&self, f: u32) -> XKey {
        let a = self.xform.range(..=f).next_back();
        let b = self.xform.range(f..).next();
        match (a, b) {
            (None, None) => XKey::default(),
            (None, Some((_, k))) | (Some((_, k)), None) => *k,
            (Some((ka, a)), Some((kb, b))) => {
                if ka == kb {
                    return *a;
                }
                let mut t = (f - ka) as f32 / (kb - ka) as f32;
                match a.i {
                    Interp::Step => t = 0.0,
                    Interp::Ease => t = ease(t),
                    Interp::Linear => {}
                }
                let l = |p: f32, q: f32| p + (q - p) * t;
                XKey { x: l(a.x, b.x), y: l(a.y, b.y), r: l(a.r, b.r), s: l(a.s, b.s), o: l(a.o, b.o), i: a.i }
            }
        }
    }

    fn shift_from(&mut self, f: u32, up: bool) {
        let moved: Vec<(u32, Arc<Drawing>)> = self.keys.split_off(&f).into_iter().collect();
        for (k, d) in moved {
            self.keys.insert(if up { k + 1 } else { k.saturating_sub(1) }, d);
        }
        let moved: Vec<(u32, XKey)> = self.xform.split_off(&f).into_iter().collect();
        for (k, x) in moved {
            self.xform.insert(if up { k + 1 } else { k.saturating_sub(1) }, x);
        }
    }

    pub fn key_at_or_before(&self, f: u32) -> Option<u32> {
        self.keys.range(..=f).next_back().map(|(k, _)| *k)
    }

    pub fn drawing_at(&self, f: u32) -> Option<&Drawing> {
        self.keys.range(..=f).next_back().map(|(_, d)| &**d)
    }

    pub fn drawing_at_mut(&mut self, f: u32) -> Option<&mut Drawing> {
        self.keys.range_mut(..=f).next_back().map(|(_, d)| Arc::make_mut(d))
    }

    /// Inserta un fotograma tras `f`: alarga la exposición del dibujo actual.
    pub fn insert_frame(&mut self, f: u32) {
        self.shift_from(f + 1, true);
    }

    /// Quita el fotograma `f`: acorta la exposición.
    pub fn remove_frame(&mut self, f: u32) {
        if self.keys.contains_key(&f) && self.keys.contains_key(&(f + 1)) {
            self.keys.remove(&f);
        }
        if self.xform.contains_key(&f) && self.xform.contains_key(&(f + 1)) {
            self.xform.remove(&f);
        }
        self.shift_from(f + 1, false);
    }

    /// Dibujos distintos de la capa en orden de aparición: los fotogramas que muestran
    /// el mismo dibujo (sustitución) comparten número. Devuelve (primer fotograma, dibujo).
    pub fn unique_drawings(&self) -> Vec<(u32, Arc<Drawing>)> {
        let mut out: Vec<(u32, Arc<Drawing>)> = Vec::new();
        for (k, d) in &self.keys {
            if !out.iter().any(|(_, e)| Arc::ptr_eq(e, d)) {
                out.push((*k, d.clone()));
            }
        }
        out
    }

    /// Número (desde 0) del dibujo que empieza en la clave `k`.
    pub fn drawing_number(&self, k: u32) -> Option<usize> {
        let d = self.keys.get(&k)?;
        self.unique_drawings().iter().position(|(_, e)| Arc::ptr_eq(e, d))
    }

    /// Dibujos (clave de inicio) dentro de [a, b], en orden.
    pub fn keys_in(&self, a: u32, b: u32) -> Vec<u32> {
        self.keys.range(a..=b).map(|(k, _)| *k).collect()
    }

    /// Duplica la duración de cada dibujo (×2).
    pub fn double_timing(&mut self) {
        self.keys = std::mem::take(&mut self.keys).into_iter().map(|(k, d)| (k * 2, d)).collect();
        self.xform = std::mem::take(&mut self.xform).into_iter().map(|(k, x)| (k * 2, x)).collect();
    }

    /// Invierte el orden de los dibujos entre a y b, conservando cuándo empieza cada uno.
    pub fn reverse_range(&mut self, a: u32, b: u32) {
        let ks = self.keys_in(a, b);
        let ds: Vec<Arc<Drawing>> = ks.iter().map(|k| self.keys[k].clone()).collect();
        for (k, d) in ks.iter().zip(ds.into_iter().rev()) {
            self.keys.insert(*k, d);
        }
    }

    /// Ida y vuelta: tras b repite los dibujos de a..b al revés (sin repetir los extremos).
    pub fn swing_range(&mut self, a: u32, b: u32, length: u32) -> u32 {
        let ks = self.keys_in(a, b);
        if ks.len() < 3 {
            return length;
        }
        let end = self.keys.range(b + 1..).next().map(|(k, _)| *k).unwrap_or(length).max(b + 1);
        let mut timings: Vec<(u32, Arc<Drawing>)> = Vec::new();
        for (i, k) in ks.iter().enumerate() {
            let next = ks.get(i + 1).copied().unwrap_or(end);
            timings.push((next - k, self.keys[k].clone()));
        }
        let back: Vec<(u32, Arc<Drawing>)> = timings[1..timings.len() - 1].iter().rev().cloned().collect();
        let total: u32 = back.iter().map(|t| t.0).sum();
        // Hacer sitio después del bloque
        let moved: Vec<(u32, Arc<Drawing>)> = self.keys.split_off(&end).into_iter().collect();
        for (k, d) in moved {
            self.keys.insert(k + total, d);
        }
        let mut f = end;
        for (dur, d) in back {
            self.keys.insert(f, d);
            f += dur;
        }
        debug_assert!(f == end + total);
        if end < length {
            length + total
        } else {
            end + total
        }
    }

    /// Vacía los fotogramas a..b: quita los dibujos que empiezan ahí y deja un hueco.
    pub fn clear_range(&mut self, a: u32, b: u32) {
        let after = self.drawing_at(b + 1).cloned();
        let resume = self.keys.contains_key(&(b + 1));
        for k in self.keys_in(a, b) {
            self.keys.remove(&k);
        }
        self.keys.insert(a, Arc::new(Drawing::default()));
        if !resume {
            if let Some(d) = after {
                self.keys.insert(b + 1, Arc::new(d));
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Project {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub length: u32,
    pub background: Rgba,
    /// La capa 0 es la de más arriba.
    pub layers: Vec<Layer>,
    pub palette: Vec<Rgba>,
    /// Planos del StoryBoard.
    #[serde(default)]
    pub storyboard: Vec<Panel>,
    /// Rango de reproducción (primer y último fotograma, desde 0).
    #[serde(default)]
    pub range: Option<(u32, u32)>,
    /// Paletas de la escena, con colores con nombre.
    #[serde(default)]
    pub palettes: Vec<NamedPalette>,
    /// Conjunto de salas con el que se creó el proyecto (trad, cutout, mg, story, animatic).
    #[serde(default)]
    pub room_set: String,
    /// Salas abiertas (pestañas), en orden: tipo de sala y nombre propio si se renombró.
    #[serde(default)]
    pub rooms: Vec<RoomTab>,
    /// Pinceles guardados del proyecto (los incluidos con el programa no se guardan aquí).
    #[serde(default)]
    pub brush_presets: Vec<crate::presets::BrushPreset>,
    /// Imágenes de punta y de textura que usan el pincel y los presets.
    #[serde(default)]
    pub brush_images: Vec<crate::presets::BrushImage>,
    /// Pincel que estaba en uso al guardar.
    #[serde(default)]
    pub brush_state: Option<crate::presets::BrushState>,
}

/// Una pestaña de sala guardada en el proyecto.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct RoomTab {
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Swatch {
    pub name: String,
    pub color: Rgba,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct NamedPalette {
    pub name: String,
    pub colors: Vec<Swatch>,
}

fn sw(name: &str, c: [u8; 3]) -> Swatch {
    Swatch { name: name.into(), color: [c[0], c[1], c[2], 255] }
}

/// Paletas de escena por defecto: Boceto, Limpieza y Color (como en la captura del original,
/// reference/picture-1.png: «Rough», «CleanUp», «Color»; los primeros colores de «Color» se midieron allí).
pub fn default_palettes() -> Vec<NamedPalette> {
    vec![
        NamedPalette {
            name: "Boceto".into(),
            colors: vec![sw("Azul boceto", [70, 120, 220]), sw("Rojo boceto", [220, 70, 70]), sw("Gris", [128, 128, 132])],
        },
        NamedPalette { name: "Limpieza".into(), colors: vec![sw("Negro", [0, 0, 0]), sw("Línea vectorizada", [20, 20, 24])] },
        NamedPalette {
            name: "Color".into(),
            colors: vec![
                sw("Negro", [0, 0, 0]),
                sw("Blanco", [255, 255, 255]),
                sw("Rojo", [179, 57, 57]),
                sw("Verde", [63, 131, 63]),
                sw("Azul", [63, 63, 149]),
                sw("Azul claro", [58, 70, 191]),
                sw("Violeta", [158, 72, 159]),
                sw("Amarillo", [253, 216, 53]),
                sw("Naranja", [251, 140, 0]),
                sw("Piel", [255, 204, 170]),
                sw("Marrón", [141, 85, 36]),
                sw("Cielo", [207, 232, 255]),
            ],
        },
    ]
}

pub fn default_palette() -> Vec<Rgba> {
    vec![
        [20, 20, 24, 255],
        [255, 255, 255, 255],
        [128, 128, 132, 255],
        [200, 200, 204, 255],
        [229, 57, 53, 255],
        [251, 140, 0, 255],
        [253, 216, 53, 255],
        [124, 179, 66, 255],
        [46, 125, 50, 255],
        [0, 172, 193, 255],
        [30, 136, 229, 255],
        [57, 73, 171, 255],
        [142, 36, 170, 255],
        [216, 27, 96, 255],
        [255, 204, 170, 255],
        [141, 85, 36, 255],
    ]
}

impl Project {
    pub fn blank(width: u32, height: u32, fps: u32, length: u32) -> Self {
        Project {
            width,
            height,
            fps,
            length,
            background: [255, 255, 255, 255],
            layers: vec![Layer::new("Capa 1")],
            palette: default_palette(),
            storyboard: Vec::new(),
            range: None,
            palettes: default_palettes(),
            room_set: String::new(),
            rooms: Vec::new(),
            brush_presets: Vec::new(),
            brush_images: Vec::new(),
            brush_state: None,
        }
    }

    /// Dibujo con todo lo visible en el fotograma `f` (para los planos del StoryBoard).
    pub fn flatten(&self, f: u32) -> Drawing {
        let (w, h) = (self.width as f32, self.height as f32);
        let mut out = Drawing::default();
        for l in self.layers.iter().rev() {
            if !l.visible {
                continue;
            }
            let t = l.transform_at(f);
            if let Some(d) = l.drawing_at(f) {
                for s in &d.strokes {
                    let mut s = s.clone();
                    if !t.is_identity() {
                        for p in &mut s.points {
                            *p = t.apply(w, h, *p);
                        }
                        s.width *= t.s.abs();
                    }
                    out.strokes.push(s);
                }
            }
        }
        out
    }

    /// Proyecto de ejemplo: una pelota que bota, para empezar con algo en pantalla.
    pub fn demo() -> Self {
        let mut p = Project::blank(1280, 720, 24, 24);
        let mut bg = Layer::new("Fondo");
        let mut d = Drawing::default();
        d.strokes.push(Stroke::shape(ShapeKind::Rect, [0.0, 0.0], [1280.0, 600.0], [207, 232, 255, 255], 0.5, Some([207, 232, 255, 255])));
        // Planos del ejemplo para la vista 3D: fondo y sol al fondo, la pelota en el plano medio.
        bg.is3d = true;
        bg.plane = 0;
        bg.depth = plane_z(0);
        let mut sun = Layer::new("Sol");
        sun.is3d = true;
        sun.plane = 0;
        sun.depth = 600.0;
        let mut sd = Drawing::default();
        sd.strokes.push(Stroke::shape(ShapeKind::Ellipse, [1040.0, 60.0], [1180.0, 200.0], [245, 170, 0, 255], 5.0, Some([255, 214, 64, 255])));
        sun.keys.insert(0, Arc::new(sd));
        sun.xform.insert(0, XKey { i: Interp::Ease, ..XKey::default() });
        sun.xform.insert(23, XKey { x: -90.0, y: 30.0, ..XKey::default() });
        d.strokes.push(Stroke::shape(ShapeKind::Ellipse, [-200.0, 470.0], [520.0, 760.0], [104, 159, 56, 255], 0.5, Some([139, 195, 74, 255])));
        d.strokes.push(Stroke::shape(ShapeKind::Ellipse, [700.0, 500.0], [1500.0, 760.0], [104, 159, 56, 255], 0.5, Some([124, 179, 66, 255])));
        d.strokes.push(Stroke::shape(ShapeKind::Rect, [0.0, 600.0], [1280.0, 720.0], [93, 64, 55, 255], 0.5, Some([161, 136, 127, 255])));
        d.strokes.push(Stroke::shape(ShapeKind::Line, [0.0, 600.0], [1280.0, 600.0], [62, 39, 35, 255], 6.0, None));
        bg.keys.insert(0, Arc::new(d));

        let mut ball = Layer::new("Pelota");
        ball.is3d = true;
        let floor = 597.0;
        for i in 0..12u32 {
            let t = i as f32 / 11.0;
            let x = 180.0 + 920.0 * t;
            let phase = (t * 2.0 * std::f32::consts::PI).cos().abs();
            let h = 360.0 * (1.0 - (1.0 - phase).powi(2)) * (1.0 - t * 0.35);
            let cy = floor - 50.0 - h;
            let squash = cy > floor - 75.0;
            let (rx, ry) = if squash { (62.0, 38.0) } else if h < 150.0 { (46.0, 56.0) } else { (50.0, 50.0) };
            let cy = if squash { floor - ry } else { cy };
            let mut d = Drawing::default();
            d.strokes.push(Stroke::shape(
                ShapeKind::Ellipse,
                [x - rx, cy - ry],
                [x + rx, cy + ry],
                [122, 31, 31, 255],
                5.0,
                Some([229, 57, 53, 255]),
            ));
            // Brillo
            d.strokes.push(Stroke {
                kind: ShapeKind::Free,
                points: vec![[x - rx * 0.55, cy - ry * 0.1], [x - rx * 0.45, cy - ry * 0.45], [x - rx * 0.15, cy - ry * 0.62]],
                color: [255, 235, 235, 255],
                width: 7.0,
                closed: false,
                fill: None,
            });
            ball.keys.insert(i * 2, Arc::new(d));
        }
        p.layers = vec![ball, sun, bg];
        let texts = [
            ("La pelota entra", "La pelota cae desde la izquierda.", "Plano general fijo"),
            ("Primer bote", "Se aplasta al tocar el suelo.", "Plano general"),
            ("Sale rodando", "Pierde altura en cada bote.", "Paneo suave a la derecha"),
        ];
        for (i, (t, a, c)) in texts.iter().enumerate() {
            let mut pn = Panel::new(i + 1);
            pn.title = t.to_string();
            pn.action = a.to_string();
            pn.camera = c.to_string();
            pn.dur = 1.0;
            pn.drawing = Arc::new(p.flatten([0, 8, 18][i]));
            p.storyboard.push(pn);
        }
        p
    }

    /// Repara valores imposibles tras abrir un archivo.
    pub fn sanitize(&mut self) {
        crate::presets::sanitize(&mut self.brush_presets, &mut self.brush_images);
        crate::presets::sanitize_state(&mut self.brush_state, &self.brush_images);
        self.width = self.width.clamp(16, 8192);
        self.height = self.height.clamp(16, 8192);
        self.fps = self.fps.clamp(1, 120);
        self.length = self.length.clamp(1, 100_000);
        if self.layers.is_empty() {
            self.layers.push(Layer::new("Capa 1"));
        }
        for l in &mut self.layers {
            l.opacity = l.opacity.clamp(0.0, 1.0);
            // Fotogramas imposibles (archivo dañado) se descartan.
            l.keys.retain(|k, _| *k < 1_000_000);
            if !l.opacity.is_finite() {
                l.opacity = 1.0;
            }
            for d in l.keys.values_mut() {
                let d = Arc::make_mut(d);
                d.strokes.retain(|s| !s.points.is_empty() && s.points.iter().all(|p| p[0].is_finite() && p[1].is_finite()));
                for s in &mut d.strokes {
                    if !s.width.is_finite() {
                        s.width = 1.0;
                    }
                    s.width = s.width.clamp(0.1, 500.0);
                }
            }
        }
        if self.palette.is_empty() {
            self.palette = default_palette();
        }
        // Proyectos anteriores: su paleta suelta pasa a ser la paleta «Mis colores».
        if self.palettes.is_empty() {
            self.palettes = default_palettes();
            let mine: Vec<Swatch> = self
                .palette
                .iter()
                .enumerate()
                .map(|(i, c)| Swatch { name: format!("Color {}", i + 1), color: *c })
                .collect();
            self.palettes.push(NamedPalette { name: "Mis colores".into(), colors: mine });
        }
        let cut = |s: &mut String| {
            if s.chars().count() > 120 {
                *s = s.chars().take(120).collect();
            }
        };
        for p in &mut self.palettes {
            cut(&mut p.name);
            for s in &mut p.colors {
                cut(&mut s.name);
            }
        }
        for l in &mut self.layers {
            l.xform.retain(|k, x| {
                *k < 1_000_000 && [x.x, x.y, x.r, x.s, x.o].iter().all(|v| v.is_finite())
            });
            for x in l.xform.values_mut() {
                x.o = x.o.clamp(0.0, 1.0);
                x.s = x.s.clamp(-50.0, 50.0);
            }
        }
        for pn in &mut self.storyboard {
            if !pn.dur.is_finite() {
                pn.dur = 2.0;
            }
            pn.dur = pn.dur.clamp(0.1, 3600.0);
        }
        if let Some((a, b)) = self.range {
            let b = b.min(self.length - 1);
            self.range = if a <= b { Some((a, b)) } else { None };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_and_frames() {
        let mut l = Layer::new("a");
        l.keys.insert(0, Arc::default());
        l.keys.insert(4, Arc::default());
        assert_eq!(l.key_at_or_before(3), Some(0));
        l.insert_frame(2);
        assert!(l.keys.contains_key(&5));
        l.remove_frame(2);
        assert!(l.keys.contains_key(&4));
    }

    #[test]
    fn undo_snapshots_share_drawings() {
        let p = Project::demo();
        let mut q = p.clone();
        let a = &p.layers[0].keys[&0];
        assert!(Arc::ptr_eq(a, &q.layers[0].keys[&0]));
        q.layers[0].drawing_at_mut(0).unwrap().strokes.clear();
        // Cambiar la copia no toca el original.
        assert!(!p.layers[0].keys[&0].strokes.is_empty());
    }

    #[test]
    fn save_format_roundtrip_and_repair() {
        let p = Project::demo();
        let json = serde_json::to_string(&p).unwrap();
        let mut back: Project = serde_json::from_str(&json).unwrap();
        back.sanitize();
        assert_eq!(back.layers.len(), p.layers.len());
        assert_eq!(back.layers[0].keys.len(), p.layers[0].keys.len());
        // Un archivo dañado se repara al abrirlo.
        let mut bad = p.clone();
        bad.fps = 0;
        bad.length = 0;
        bad.layers.clear();
        bad.sanitize();
        assert!(bad.fps >= 1 && bad.length >= 1 && !bad.layers.is_empty());
    }

    #[test]
    fn remove_frame_never_underflows() {
        let mut l = Layer::new("a");
        l.keys.insert(0, Arc::default());
        l.keys.insert(1, Arc::default());
        for _ in 0..5 {
            l.remove_frame(0);
        }
        assert!(l.keys.contains_key(&0));
    }

    #[test]
    fn transform_interpolates_and_inverts() {
        let mut l = Layer::new("a");
        l.xform.insert(0, XKey::default());
        l.xform.insert(10, XKey { x: 100.0, r: 90.0, s: 2.0, ..XKey::default() });
        let t = l.transform_at(5);
        assert!((t.x - 50.0).abs() < 1e-3 && (t.s - 1.5).abs() < 1e-3);
        let p = [300.0, 200.0];
        let q = t.invert(1280.0, 720.0, t.apply(1280.0, 720.0, p));
        assert!((q[0] - p[0]).abs() < 1e-2 && (q[1] - p[1]).abs() < 1e-2);
        l.xform.get_mut(&0).unwrap().i = Interp::Step;
        assert_eq!(l.transform_at(5).x, 0.0);
    }

    #[test]
    fn xsheet_operations() {
        let mut l = Layer::new("a");
        for k in [0u32, 2, 4] {
            l.keys.insert(k, Arc::new(Drawing { strokes: vec![Stroke::shape(ShapeKind::Line, [k as f32, 0.0], [1.0, 1.0], [0; 4], 1.0, None)], ..Default::default() }));
        }
        l.double_timing();
        assert_eq!(l.keys.keys().copied().collect::<Vec<_>>(), vec![0, 4, 8]);
        l.reverse_range(0, 8);
        assert_eq!(l.keys[&0].strokes[0].points[0][0], 4.0);
        let len = l.swing_range(0, 8, 12);
        assert_eq!(len, 16);
        assert_eq!(l.keys.len(), 4);
    }

    #[test]
    fn stroke_scale_and_rotate() {
        let mut s = Stroke::shape(ShapeKind::Line, [0.0, 0.0], [10.0, 0.0], [0; 4], 2.0, None);
        s.scale_about([0.0, 0.0], 2.0, 2.0);
        assert_eq!(s.points[1], [20.0, 0.0]);
        assert!((s.width - 4.0).abs() < 1e-4);
        s.rotate_about([0.0, 0.0], 90.0);
        assert!(s.points[1][0].abs() < 1e-3 && (s.points[1][1] - 20.0).abs() < 1e-3);
    }

    #[test]
    fn old_projects_get_named_palettes_and_shared_drawings_share_numbers() {
        let mut p = Project::demo();
        p.palettes.clear();
        p.sanitize();
        assert!(p.palettes.iter().any(|x| x.name == "Mis colores"));
        let mut l = Layer::new("a");
        let d1 = Arc::new(Drawing::default());
        let d2 = Arc::new(Drawing::default());
        l.keys.insert(0, d1.clone());
        l.keys.insert(2, d2);
        l.keys.insert(4, d1);
        assert_eq!(l.unique_drawings().len(), 2);
        assert_eq!(l.drawing_number(4), Some(0));
    }

    #[test]
    fn eraser_splits() {
        let mut d = Drawing::default();
        d.strokes.push(Stroke { kind: ShapeKind::Line, points: vec![[0.0, 0.0], [100.0, 0.0]], color: [0, 0, 0, 255], width: 2.0, closed: false, fill: None });
        assert!(d.erase([50.0, 0.0], 5.0));
        assert_eq!(d.strokes.len(), 2);
    }
}
