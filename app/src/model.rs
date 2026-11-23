//! Datos del proyecto: escena, capas, fotogramas clave y trazos vectoriales.

use crate::tracks::{Prop, Tracks};
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
    /// Presión del lápiz en cada punto, de 0 a 1 (vacía si no se capturó). Es parte de la geometría: sirve para
    /// recalcular el grosor si cambia el estilo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pressure: Vec<f32>,
    /// Estilo del trazo vectorial (extremos, uniones, perfil de grosor, respuesta a la presión). None es el trazo
    /// de siempre: grosor constante con extremos y uniones redondos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<crate::vector::VStyle>,
    /// Aspecto con el que se dibujó, guardado la primera vez que se edita el trazo (para restaurarlo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<Box<crate::vector::Look>>,
    /// Curva Bézier editable (nodos con tiradores). Si existe, points y pressure se calculan a partir de ella.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<Vec<crate::vector::bezier::Node>>,
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
        Stroke { kind, points, color, width, closed, fill, pressure: Vec::new(), style: None, original: None, nodes: None }
    }

    /// Recalcula la polilínea (y la presión) a partir de los nodos, si el trazo es una curva.
    pub fn regen(&mut self) {
        if let Some(nodes) = &self.nodes {
            let (pts, pr) = crate::vector::bezier::flatten(nodes, self.closed);
            self.points = pts;
            self.pressure = pr;
        }
    }

    /// Aplica una transformación a los puntos y también a los nodos y tiradores de la curva.
    fn map_geometry(&mut self, f: impl Fn([f32; 2]) -> [f32; 2]) {
        for p in &mut self.points {
            *p = f(*p);
        }
        if let Some(ns) = self.nodes.as_mut() {
            for n in ns {
                n.map(&f);
            }
        }
    }

    /// Aspecto actual (grosor, color y estilo).
    pub fn look(&self) -> crate::vector::Look {
        crate::vector::Look { width: self.width, color: self.color, style: self.style.clone() }
    }

    /// 	rue si se dibuja con el trazo rápido de siempre (grosor constante, extremos y uniones redondos).
    pub fn is_plain(&self) -> bool {
        crate::vector::is_plain(self.style.as_ref(), &self.pressure)
    }

    /// Polígonos convexos del trazo de grosor variable, en las coordenadas del dibujo (vectorial: no se rasteriza).
    pub fn pieces(&self) -> Vec<Vec<[f32; 2]>> {
        let st = self.style.clone().unwrap_or_default().clamped();
        let w = crate::vector::widths(&self.points, &self.pressure, self.width, &st, self.closed);
        crate::vector::pieces(&self.points, &w, self.closed, st.cap, st.join)
    }

    /// Deja la presión y el estilo en un estado válido (largo igual al de los puntos, valores en rango).
    pub fn fix_attrs(&mut self) {
        if self.pressure.len() != self.points.len() {
            self.pressure.clear();
        }
        for p in &mut self.pressure {
            *p = if p.is_finite() { p.clamp(0.0, 1.0) } else { 1.0 };
        }
        if let Some(s) = self.style.take() {
            self.style = Some(s.clamped());
        }
        // Una curva dañada (nodos que no son números) vuelve a ser una polilínea; si no, se recalcula desde sus nodos.
        if let Some(ns) = self.nodes.take() {
            if ns.len() >= 2 && ns.iter().all(|n| n.is_finite()) {
                self.nodes = Some(ns);
                self.regen();
            }
        }
        if let Some(o) = self.original.as_mut() {
            o.width = if o.width.is_finite() { o.width.clamp(0.1, 500.0) } else { self.width };
            o.style = o.style.take().map(|s| s.clamped());
        }
    }

    /// Simplifica la línea sin perder la presión de los puntos que se quedan.
    pub fn simplify_in_place(&mut self, eps: f32) {
        let keep = if self.pressure.len() == self.points.len() {
            simplify_idx_attr(&self.points, Some((&self.pressure, self.width * 0.5)), eps)
        } else {
            simplify_idx(&self.points, eps)
        };
        if self.pressure.len() == self.points.len() {
            self.pressure = keep.iter().map(|&i| self.pressure[i]).collect();
        }
        self.points = keep.iter().map(|&i| self.points[i]).collect();
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.map_geometry(|p| [p[0] + dx, p[1] + dy]);
    }

    /// Caja que envuelve el trazo (sin grosor): [x0, y0, x1, y1].
    pub fn bounds(&self) -> [f32; 4] {
        self.points.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |b, p| {
            [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
        })
    }

    /// Escala respecto a un punto fijo (el grosor sigue la escala media).
    pub fn scale_about(&mut self, c: [f32; 2], sx: f32, sy: f32) {
        self.map_geometry(|p| [c[0] + (p[0] - c[0]) * sx, c[1] + (p[1] - c[1]) * sy]);
        let k = (sx.abs() + sy.abs()) * 0.5;
        self.width = (self.width * k).clamp(0.1, 500.0);
        if let Some(o) = self.original.as_mut() {
            o.width = (o.width * k).clamp(0.1, 500.0);
        }
    }

    /// Gira `deg` grados alrededor de `c`.
    pub fn rotate_about(&mut self, c: [f32; 2], deg: f32) {
        let (s, co) = deg.to_radians().sin_cos();
        self.map_geometry(|p| {
            let (x, y) = (p[0] - c[0], p[1] - c[1]);
            [c[0] + x * co - y * s, c[1] + x * s + y * co]
        });
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
        // Presión de cada punto. Si el trazo tenía perfil de grosor (afilado) o respuesta a la presión, se «hornea» en
        // la presión de los trozos que quedan, para que se vean igual que antes de borrar y no se afilen de nuevo.
        let (src_p, out_style): (Option<Vec<f32>>, Option<crate::vector::VStyle>) = match &self.style {
            Some(st) if !self.is_plain() => {
                let f = crate::vector::widths(&self.points, &self.pressure, 1.0, st, self.closed);
                (Some(f), Some(crate::vector::VStyle { pressure: true, min_width: 0.0, curve: 1.0, profile: crate::vector::Profile::Uniform, ..st.clone() }))
            }
            _ if self.pressure.len() == self.points.len() => (Some(self.pressure.clone()), self.style.clone()),
            _ => (None, self.style.clone()),
        };
        let has_p = src_p.is_some();
        let pr = |i: usize| src_p.as_ref().map_or(1.0, |v| v[i % v.len()]);
        // Densificar la línea para poder cortar en cualquier punto.
        let mut src = self.points.clone();
        let mut idx: Vec<usize> = (0..self.points.len()).collect();
        if self.closed {
            src.push(self.points[0]);
            idx.push(0);
        }
        let step = (r * 0.4).max(0.5);
        let mut dense: Vec<([f32; 2], f32)> = Vec::with_capacity(src.len() * 2);
        for (k, w) in src.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let (pa, pb) = (pr(idx[k]), pr(idx[k + 1]));
            let n = (dist(a, b) / step).ceil().max(1.0) as usize;
            for i in 0..n {
                let t = i as f32 / n as f32;
                dense.push(([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], pa + (pb - pa) * t));
            }
        }
        dense.push((*src.last().unwrap(), pr(*idx.last().unwrap())));
        let limit = r + self.width * 0.5;
        let mut out: Vec<Vec<([f32; 2], f32)>> = Vec::new();
        let mut run: Vec<([f32; 2], f32)> = Vec::new();
        for q in dense {
            if dist(q.0, p) <= limit {
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
                .map(|run| {
                    let pts: Vec<[f32; 2]> = run.iter().map(|q| q.0).collect();
                    let prs: Vec<f32> = run.iter().map(|q| q.1).collect();
                    let keep = if has_p { simplify_idx_attr(&pts, Some((&prs, self.width * 0.5)), 0.35) } else { simplify_idx(&pts, 0.35) };
                    Stroke {
                        kind: ShapeKind::Free,
                        points: keep.iter().map(|&i| pts[i]).collect(),
                        color: self.color,
                        width: self.width,
                        closed: false,
                        fill: None,
                        pressure: if has_p { keep.iter().map(|&i| prs[i]).collect() } else { Vec::new() },
                        style: out_style.clone(),
                        original: None, nodes: None,
                    }
                })
                .collect(),
        )
    }
}

/// Simplificación sencilla (Ramer–Douglas–Peucker).
pub fn simplify(pts: &[[f32; 2]], eps: f32) -> Vec<[f32; 2]> {
    simplify_idx(pts, eps).into_iter().map(|i| pts[i]).collect()
}

/// Igual que simplify, pero devuelve los índices de los puntos que se quedan (para llevar datos aparte, como la presión).
pub fn simplify_idx(pts: &[[f32; 2]], eps: f32) -> Vec<usize> {
    simplify_idx_attr(pts, None, eps)
}

/// Ramer–Douglas–Peucker que también cuida un valor por punto (la presión o el grosor): un punto se conserva si se
/// desvía de la línea, o si su valor, multiplicado por scale (el grosor en px), se desvía de lo que daría la
/// interpolación entre los puntos que lo rodean. Así un trazo afilado o con presión no pierde su perfil.
pub fn simplify_idx_attr(pts: &[[f32; 2]], attr: Option<(&[f32], f32)>, eps: f32) -> Vec<usize> {
    if pts.len() < 3 {
        return (0..pts.len()).collect();
    }
    let attr = attr.filter(|(a, _)| a.len() == pts.len());
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((s, e)) = stack.pop() {
        let mut best = (0.0f32, 0usize);
        for i in s + 1..e {
            let mut d = dist_seg(pts[i], pts[s], pts[e]);
            if let Some((a, scale)) = attr {
                // Valor esperado en i si el valor cambiara de forma lineal a lo largo de la línea entre s y e.
                let (ls, le) = (dist(pts[s], pts[i]), dist(pts[i], pts[e]));
                let t = if ls + le > 1e-9 { ls / (ls + le) } else { 0.0 };
                let want = a[s] + (a[e] - a[s]) * t;
                d = d.max((a[i] - want).abs() * scale);
            }
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
    keep.iter().enumerate().filter(|(_, k)| **k).map(|(i, _)| i).collect()
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

/// Dónde se suelta una capa arrastrada: dentro de una colección, o antes o después de otra capa.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayerDrop {
    Into(usize),
    Before(usize),
    After(usize),
}

/// Modo de fusión de una capa con lo que hay debajo (los ocho del prototipo).
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
pub enum Blend {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Difference,
    Add,
}

impl Blend {
    pub const ALL: [Blend; 8] = [Blend::Normal, Blend::Multiply, Blend::Screen, Blend::Overlay, Blend::Darken, Blend::Lighten, Blend::Difference, Blend::Add];
    pub fn label(self) -> &'static str {
        match self {
            Blend::Normal => "Normal",
            Blend::Multiply => "Multiplicar",
            Blend::Screen => "Pantalla",
            Blend::Overlay => "Superponer",
            Blend::Darken => "Oscurecer",
            Blend::Lighten => "Aclarar",
            Blend::Difference => "Diferencia",
            Blend::Add => "Añadir",
        }
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
    /// Punto de anclaje: desplazamiento del centro de giro y escala respecto al centro de la cámara.
    #[serde(default)]
    pub ax: f32,
    #[serde(default)]
    pub ay: f32,
    /// Profundidad animada (eje Z): se suma a la profundidad de la capa 3D. Las capas 2D la ignoran.
    #[serde(default)]
    pub dz: f32,
}

impl Default for XKey {
    fn default() -> Self {
        XKey { x: 0.0, y: 0.0, r: 0.0, s: 1.0, o: 1.0, i: Interp::Linear, ax: 0.0, ay: 0.0, dz: 0.0 }
    }
}

impl XKey {
    pub fn is_identity(&self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.r == 0.0 && self.s == 1.0 && self.o == 1.0 && self.ax == 0.0 && self.ay == 0.0 && self.dz == 0.0
    }
    /// Punto del dibujo → punto de la escena (giro y escala alrededor del punto de anclaje, que por defecto es el centro de la cámara).
    pub fn apply(&self, w: f32, h: f32, p: [f32; 2]) -> [f32; 2] {
        let (cx, cy) = (w / 2.0 + self.ax, h / 2.0 + self.ay);
        let (x, y) = ((p[0] - cx) * self.s, (p[1] - cy) * self.s);
        let (sn, cs) = self.r.to_radians().sin_cos();
        [x * cs - y * sn + cx + self.x, x * sn + y * cs + cy + self.y]
    }
    /// La transformación de una capa hija dentro de su madre: primero la de la hija y después la de la madre. Es otra
    /// transformación del mismo tipo (con el anclaje en el centro de la cámara), así que todo lo que usa XKey sirve igual.
    pub fn compose(&self, parent: &XKey, w: f32, h: f32) -> XKey {
        let c = parent.apply(w, h, self.apply(w, h, [w / 2.0, h / 2.0]));
        XKey { x: c[0] - w / 2.0, y: c[1] - h / 2.0, r: self.r + parent.r, s: self.s * parent.s, o: self.o, i: self.i, ax: 0.0, ay: 0.0, dz: self.dz + parent.dz }
    }

    /// Punto de la escena → punto del dibujo (lo contrario de `apply`).
    pub fn invert(&self, w: f32, h: f32, p: [f32; 2]) -> [f32; 2] {
        let (cx, cy) = (w / 2.0 + self.ax, h / 2.0 + self.ay);
        let (x, y) = (p[0] - cx - self.x, p[1] - cy - self.y);
        let (sn, cs) = (-self.r).to_radians().sin_cos();
        let s = if self.s.abs() < 1e-4 { 1e-4 } else { self.s };
        [(x * cs - y * sn) / s + cx, (x * sn + y * cs) / s + cy]
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
    /// Secuencia de fotos (Stop Motion): sus fotos viven en una carpeta elegida por la persona.
    StopMotion,
    /// Colección (grupo o carpeta): organiza capas de una misma composición, sin tiempo propio (ver `Layer::group`).
    Collection,
    /// Precomposición: su contenido es otra composición (ver `Layer::precomp`), con sus propias capas y su duración.
    Precomp,
}

impl LayerKind {
    pub const ALL: [LayerKind; 10] = [
        LayerKind::Vector,
        LayerKind::Bitmap,
        LayerKind::Solid,
        LayerKind::Null,
        LayerKind::Text,
        LayerKind::Light,
        LayerKind::Camera,
        LayerKind::StopMotion,
        LayerKind::Collection,
        LayerKind::Precomp,
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
            LayerKind::StopMotion => "Stop Motion",
            LayerKind::Collection => "Colección",
            LayerKind::Precomp => "Precomposición",
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
            LayerKind::StopMotion => "M",
            LayerKind::Collection => "G",
            LayerKind::Precomp => "P",
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
            LayerKind::StopMotion => [0x7a, 0xc9, 0xa4, 255],
            LayerKind::Collection => [0xa8, 0xcf, 0x8f, 255],
            LayerKind::Precomp => [0xf0, 0xb0, 0x80, 255],
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
    /// Pistas de animación: una por propiedad (punto de anclaje, posición, escala, rotación y opacidad).
    #[serde(default, skip_serializing_if = "Tracks::is_empty")]
    pub tracks: Tracks,
    /// Solo para leer proyectos anteriores, que guardaban una sola pista con todo junto. Al abrir se convierte en 	racks.
    #[serde(default, rename = "xform", skip_serializing)]
    legacy_xform: BTreeMap<u32, XKey>,
    /// Referencia a la carpeta de la secuencia (solo en capas Stop Motion).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopmotion: Option<crate::stopmotion::SmRef>,
    /// Barra de la capa: primer cuadro en que se ve (entrada).
    #[serde(default)]
    pub in_f: u32,
    /// Último cuadro en que se ve (salida); sin valor, hasta el final del proyecto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_f: Option<u32>,
    /// Solo: si alguna capa está en solo, únicamente se ven las que lo están.
    #[serde(default)]
    pub solo: bool,
    /// Tímida: se puede ocultar de las listas con el botón maestro (no afecta al dibujo).
    #[serde(default)]
    pub shy: bool,
    /// Etiqueta de color (índice en LABELS; 0 = sin etiqueta).
    #[serde(default)]
    pub label: u8,
    /// Modo de fusión con lo que hay debajo.
    #[serde(default)]
    pub blend: Blend,
    /// Identificador de la capa (0 = aún sin asignar); lo usan los vínculos para no depender del orden de las capas.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub uid: u64,
    /// Capa madre (su uid): la hija hereda su posición, giro y escala.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u64>,
    /// Colección a la que pertenece (el uid de una capa de tipo Colección). Solo un nivel: una colección no está en otra.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u64>,
    /// Colección plegada en las listas (sus capas se siguen dibujando).
    #[serde(default, skip_serializing_if = "is_false")]
    pub collapsed: bool,
    /// Composición que contiene una capa de tipo Precomposición (su `id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precomp: Option<u64>,
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

/// Colores de etiqueta de las capas (el primero es «sin etiqueta»).
pub const LABELS: [Rgba; 8] = [[138, 138, 138, 255], [253, 103, 36, 255], [242, 210, 122, 255], [143, 179, 207, 255], [168, 207, 143, 255], [183, 162, 230, 255], [233, 160, 122, 255], [197, 143, 94, 255]];

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
            tracks: Tracks::default(),
            legacy_xform: BTreeMap::new(),
            stopmotion: None,
            in_f: 0,
            out_f: None,
            solo: false,
            shy: false,
            label: 0,
            blend: Blend::Normal,
            uid: 0,
            parent: None,
            group: None,
            collapsed: false,
            precomp: None,
        }
    }

    /// Profundidad en el cuadro f: la de la capa más lo que anima su pista de profundidad (sea o no 3D).
    pub fn depth_at(&self, f: u32) -> f32 {
        self.depth + self.tracks.value_at(Prop::Depth, f).0[0]
    }

    /// Profundidad con la que se ve la capa en el cuadro f: 0 para las 2D (fijas al tamaño de la cámara).
    pub fn z_at(&self, f: u32) -> f32 {
        if self.is3d || self.kind == LayerKind::Camera {
            self.depth_at(f)
        } else {
            0.0
        }
    }

    /// Pasa al siguiente espacio: 2D → BG → MG → FG → 2D. Una capa 2D queda fija al tamaño de la cámara; una 3D tiene eje Z.
    pub fn next_space(&mut self) {
        let idx = if self.is3d { 1 + self.plane as usize } else { 0 };
        match (idx + 1) % 4 {
            0 => self.is3d = false,
            n => {
                self.is3d = true;
                self.plane = (n - 1) as u8;
                self.depth = plane_z(self.plane);
            }
        }
    }

    /// Transformación de la capa en el fotograma `f`, interpolada entre claves.
    pub fn transform_at(&self, f: u32) -> XKey {
        self.tracks.eval(f)
    }

    /// ¿La barra de la capa cubre el cuadro `f`?
    pub fn active_at(&self, f: u32) -> bool {
        f >= self.in_f && self.out_f.map_or(true, |o| f <= o)
    }

    /// ¿Se dibuja la capa en el cuadro `f`? Visible, dentro de su barra y, si hay capas en solo, que esté en solo.
    #[allow(dead_code)]
    pub fn shows_at(&self, f: u32, any_solo: bool) -> bool {
        self.visible && self.active_at(f) && (!any_solo || self.solo)
    }

    /// Último cuadro de la barra dentro de un proyecto de `len` cuadros.
    pub fn out_frame(&self, len: u32) -> u32 {
        let last = len.saturating_sub(1);
        self.out_f.map_or(last, |o| o.min(last))
    }

    /// Primer cuadro que usa la capa (dibujos, claves, entrada o secuencia).
    fn first_used_frame(&self) -> u32 {
        let mut m = self.in_f;
        if let Some(k) = self.keys.keys().next() {
            m = m.min(*k);
        }
        if let Some(k) = self.tracks.first_frame() {
            m = m.min(k);
        }
        if let Some(r) = &self.stopmotion {
            m = m.min(r.start);
        }
        m
    }

    /// Mueve toda la capa en el tiempo `d` cuadros (la barra con sus dibujos y claves). No pasa del cuadro 0. Devuelve lo que se movió de verdad.
    pub fn shift_time(&mut self, d: i32) -> i32 {
        let d = d.max(-(self.first_used_frame() as i32));
        if d == 0 {
            return 0;
        }
        let mv = |k: u32| (k as i64 + d as i64).max(0) as u32;
        self.keys = std::mem::take(&mut self.keys).into_iter().map(|(k, x)| (mv(k), x)).collect();
        self.tracks.shift_all(d);
        self.in_f = mv(self.in_f);
        self.out_f = self.out_f.map(mv);
        if let Some(r) = self.stopmotion.as_mut() {
            r.start = mv(r.start);
        }
        d
    }

    /// Convierte las claves de proyectos anteriores (una sola pista) en pistas por propiedad.
    pub fn migrate_legacy(&mut self) {
        if !self.legacy_xform.is_empty() {
            if self.tracks.is_empty() {
                self.tracks = Tracks::from_legacy(&self.legacy_xform);
            }
            self.legacy_xform.clear();
        }
    }

    fn shift_from(&mut self, f: u32, up: bool) {
        let moved: Vec<(u32, Arc<Drawing>)> = self.keys.split_off(&f).into_iter().collect();
        for (k, d) in moved {
            self.keys.insert(if up { k + 1 } else { k.saturating_sub(1) }, d);
        }
        self.tracks.shift_from(f, up);
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
        self.tracks.remove_frame_keys(f);
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
        self.tracks.double();
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
    /// Carpeta donde está el archivo del proyecto (no se guarda): sirve para encontrar las carpetas de Stop Motion.
    #[serde(skip)]
    pub base_dir: Option<std::path::PathBuf>,
    /// Caché de las secuencias Stop Motion abiertas (no se guarda).
    #[serde(skip)]
    pub sm: crate::stopmotion::SmCache,
    /// Composiciones de las precomposiciones (la principal es `layers`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comps: Vec<Composition>,
    /// Precomposiciones abiertas para editarlas, de fuera adentro (no se guarda).
    #[serde(skip)]
    pub stack: Vec<Level>,
}

/// Una composición guardada: las capas de una precomposición y su duración (mismo tamaño y velocidad que el proyecto).
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Composition {
    pub id: u64,
    pub name: String,
    pub layers: Vec<Layer>,
    pub length: u32,
}

/// Un nivel de la pila de «entrar en la capa»: lo que había fuera de la composición que se está editando. Mientras se edita una
/// precomposición, sus capas ocupan `Project::layers` y las de fuera esperan aquí (no se guarda: se guarda siempre la principal).
#[derive(Clone, Debug)]
pub struct Level {
    pub comp_id: u64,
    pub layers: Vec<Layer>,
    pub length: u32,
    /// Capa y cuadro que había elegidos fuera, para volver a ellos al salir.
    pub cl: usize,
    pub frame: u32,
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
            base_dir: None,
            sm: Default::default(),
            comps: Vec::new(),
            stack: Vec::new(),
        }
    }

    /// Una copia con los mismos ajustes (tamaño, velocidad, fondo y caché de Stop Motion) pero sin capas ni composiciones.
    pub fn shell(&self) -> Project {
        Project {
            width: self.width,
            height: self.height,
            fps: self.fps,
            length: self.length,
            background: self.background,
            layers: Vec::new(),
            palette: Vec::new(),
            storyboard: Vec::new(),
            range: None,
            palettes: Vec::new(),
            room_set: String::new(),
            rooms: Vec::new(),
            brush_presets: Vec::new(),
            brush_images: Vec::new(),
            brush_state: None,
            base_dir: self.base_dir.clone(),
            sm: self.sm.clone(),
            comps: Vec::new(),
            stack: Vec::new(),
        }
    }


    /// Índice de la capa con ese uid.
    pub fn index_of_uid(&self, uid: u64) -> Option<usize> {
        (uid != 0).then(|| self.layers.iter().position(|l| l.uid == uid)).flatten()
    }

    /// Transformación con la que se dibuja la capa li en el cuadro f: la suya compuesta con la de sus madres (con un tope de
    /// profundidad por si un archivo roto trae un ciclo).
    pub fn effective_at(&self, li: usize, f: u32) -> XKey {
        self.effective_inner(li, f, 0)
    }

    fn effective_inner(&self, li: usize, f: u32, depth: u8) -> XKey {
        let (w, h) = (self.width as f32, self.height as f32);
        let mut t = self.layers[li].transform_at(f);
        let mut cur = li;
        for _ in 0..16 {
            let Some(pi) = self.layers[cur].parent.and_then(|u| self.index_of_uid(u)) else { break };
            if pi == li {
                break;
            }
            t = t.compose(&self.layers[pi].transform_at(f), w, h);
            cur = pi;
        }
        // Una capa de una colección se mueve con ella, y la colección con la suya, a cualquier profundidad.
        if let Some(ci) = self.collection_of(li) {
            if ci != li && depth < 32 {
                t = t.compose(&self.effective_inner(ci, f, depth + 1), w, h);
            }
        }
        t
    }

    /// ¿Vincular child a parent crearía un ciclo (o la capa a sí misma)?
    pub fn would_cycle(&self, child: usize, parent: usize) -> bool {
        let mut cur = parent;
        for _ in 0..=self.layers.len() {
            if cur == child {
                return true;
            }
            match self.layers[cur].parent.and_then(|u| self.index_of_uid(u)) {
                Some(p) => cur = p,
                None => return false,
            }
        }
        true
    }

    pub fn next_uid(&self) -> u64 {
        self.layers.iter().map(|l| l.uid).max().unwrap_or(0) + 1
    }

    /// Vincula la capa child a parent (o la suelta con None). Devuelve false si crearía un ciclo.
    pub fn set_parent(&mut self, child: usize, parent: Option<usize>) -> bool {
        let Some(p) = parent else {
            self.layers[child].parent = None;
            return true;
        };
        if p >= self.layers.len() || self.would_cycle(child, p) {
            return false;
        }
        if self.layers[p].uid == 0 {
            self.layers[p].uid = self.next_uid();
        }
        self.layers[child].parent = Some(self.layers[p].uid);
        true
    }

    /// Suelta a las hijas de la capa li (antes de borrarla).
    pub fn release_children(&mut self, li: usize) {
        let uid = self.layers[li].uid;
        if uid != 0 {
            for l in &mut self.layers {
                if l.parent == Some(uid) {
                    l.parent = None;
                }
            }
        }
    }

    /// Deja los vínculos válidos (por si el archivo está editado o dañado): uid únicos, madres que existen y sin ciclos.
    pub fn fix_links(&mut self) {
        let mut seen = std::collections::HashSet::new();
        for l in &mut self.layers {
            if l.uid != 0 && !seen.insert(l.uid) {
                l.uid = 0;
            }
        }
        for i in 0..self.layers.len() {
            let Some(u) = self.layers[i].parent else { continue };
            let ok = self.index_of_uid(u).map_or(false, |p| p != i && !self.would_cycle(i, p));
            if !ok {
                self.layers[i].parent = None;
            }
        }
    }

    /// La colección (índice) a la que pertenece directamente la capa li, si tiene una que existe.
    pub fn collection_of(&self, li: usize) -> Option<usize> {
        self.layers[li].group.and_then(|u| self.index_of_uid(u)).filter(|&ci| self.layers[ci].kind == LayerKind::Collection)
    }

    /// Las colecciones que contienen a la capa li, de la más cercana a la más lejana (sin límite de profundidad).
    pub fn ancestors(&self, li: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut cur = li;
        for _ in 0..self.layers.len() {
            match self.collection_of(cur) {
                Some(ci) if ci != li && !out.contains(&ci) => {
                    out.push(ci);
                    cur = ci;
                }
                _ => break,
            }
        }
        out
    }

    /// Cuántas colecciones envuelven a la capa li.
    pub fn depth_of(&self, li: usize) -> usize {
        self.ancestors(li).len()
    }

    /// ¿Alguna colección que envuelve a la capa li está plegada?
    pub fn is_hidden_by_fold(&self, li: usize) -> bool {
        self.ancestors(li).iter().any(|&a| self.layers[a].collapsed)
    }

    /// Las capas que están directamente en la colección ci, de arriba abajo.
    pub fn members_of(&self, ci: usize) -> Vec<usize> {
        let uid = self.layers[ci].uid;
        if uid == 0 || self.layers[ci].kind != LayerKind::Collection {
            return Vec::new();
        }
        (0..self.layers.len()).filter(|&i| self.layers[i].group == Some(uid)).collect()
    }

    /// Cuántas capas hay en el árbol de li (ella incluida): una colección con todo lo que contiene, a cualquier profundidad.
    pub fn subtree_len(&self, li: usize) -> usize {
        let mut n = 1;
        while li + n < self.layers.len() && self.ancestors(li + n).contains(&li) {
            n += 1;
        }
        n
    }

    /// ¿Se dibuja la capa li en el cuadro f? Las colecciones no dibujan nada por sí mismas; una capa dentro de colecciones
    /// necesita además que todas estén visibles y dentro de su barra, y vale el solo de cualquiera de ellas.
    pub fn shows(&self, li: usize, f: u32, any_solo: bool) -> bool {
        let l = &self.layers[li];
        if l.kind == LayerKind::Collection {
            return false;
        }
        let anc = self.ancestors(li);
        l.visible && l.active_at(f) && anc.iter().all(|&a| self.layers[a].visible && self.layers[a].active_at(f)) && (!any_solo || l.solo || anc.iter().any(|&a| self.layers[a].solo))
    }

    /// ¿Hay algo que el visor no sabe dibujar capa a capa en el cuadro f? Un modo de fusión que no es Normal, o una colección
    /// con opacidad o fusión propias (se componen aparte y luego se mezclan como una unidad).
    pub fn has_blend_at(&self, f: u32) -> bool {
        let any_solo = self.layers.iter().any(|l| l.solo);
        let shown: Vec<usize> = (0..self.layers.len()).filter(|&i| self.shows(i, f, any_solo)).collect();
        // Una precomposición se dibuja componiendo su contenido aparte, igual que las colecciones con fusión.
        if shown.iter().any(|&i| self.layers[i].blend != Blend::Normal || self.layers[i].kind == LayerKind::Precomp) {
            return true;
        }
        (0..self.layers.len()).any(|c| {
            let l = &self.layers[c];
            l.kind == LayerKind::Collection && l.visible && l.active_at(f) && (l.blend != Blend::Normal || l.opacity < 1.0 || l.tracks.has(Prop::Opacity)) && shown.iter().any(|&m| self.ancestors(m).contains(&c))
        })
    }

    /// Mete la capa li (o la colección li con todo lo que contiene) en una colección nueva, justo donde estaba y dentro de la
    /// misma colección que ella. Devuelve el índice de la colección nueva; lo agrupado queda debajo.
    pub fn group_layer(&mut self, li: usize) -> Option<usize> {
        if li >= self.layers.len() {
            return None;
        }
        let n = self.layers.iter().filter(|l| l.kind == LayerKind::Collection).count() + 1;
        let mut c = Layer::new(format!("Colección {n}"));
        c.kind = LayerKind::Collection;
        c.uid = self.next_uid();
        c.group = self.collection_of(li).map(|ci| self.layers[ci].uid);
        let uid = c.uid;
        self.layers.insert(li, c);
        self.layers[li + 1].group = Some(uid);
        Some(li)
    }

    /// Mete varias capas a la vez en una colección nueva, en el sitio de la de más arriba y dentro de su misma colección, con el
    /// orden que tenían. Si una de las elegidas es una colección y también lo está lo que contiene, solo se mueve la colección (con
    /// su contenido). Devuelve el índice de la colección nueva.
    pub fn group_layers(&mut self, idx: &[usize]) -> Option<usize> {
        let n = self.layers.len();
        let mut sel: Vec<usize> = idx.iter().copied().filter(|&i| i < n).collect();
        sel.sort_unstable();
        sel.dedup();
        match sel.len() {
            0 => return None,
            1 => return self.group_layer(sel[0]),
            _ => {}
        }
        let roots: Vec<usize> = sel.iter().copied().filter(|&i| !self.ancestors(i).iter().any(|a| sel.contains(a))).collect();
        let first = roots[0];
        let count = self.layers.iter().filter(|l| l.kind == LayerKind::Collection).count() + 1;
        let mut c = Layer::new(format!("Colección {count}"));
        c.kind = LayerKind::Collection;
        c.uid = self.next_uid();
        c.group = self.collection_of(first).map(|ci| self.layers[ci].uid);
        let uid = c.uid;
        // Se sacan los árboles de abajo arriba para que los índices de los de arriba no cambien.
        let mut trees: Vec<Vec<Layer>> = Vec::new();
        for &r in roots.iter().rev() {
            let len = self.subtree_len(r);
            trees.push(self.layers.drain(r..r + len).collect());
        }
        trees.reverse();
        self.layers.insert(first, c);
        let mut at = first + 1;
        for mut tree in trees {
            tree[0].group = Some(uid);
            for l in tree {
                self.layers.insert(at, l);
                at += 1;
            }
        }
        let map = self.normalize_groups();
        Some(map[first])
    }

    /// ¿Se puede soltar la capa li en ese sitio? No sobre sí misma ni dentro de su propio contenido, y «dentro» solo en
    /// colecciones.
    pub fn can_drop(&self, li: usize, d: LayerDrop) -> bool {
        let n = self.layers.len();
        let r = match d {
            LayerDrop::Into(r) | LayerDrop::Before(r) | LayerDrop::After(r) => r,
        };
        if li >= n || r >= n || r == li || self.ancestors(r).contains(&li) {
            return false;
        }
        !matches!(d, LayerDrop::Into(_)) || self.layers[r].kind == LayerKind::Collection
    }

    /// Mueve la capa li (con todo lo que contiene, si es una colección) a otro sitio de la lista: dentro de una colección
    /// (la primera de sus capas), o antes o después de otra capa, en la misma colección que esa. Devuelve el nuevo índice de li.
    pub fn move_layer_tree(&mut self, li: usize, d: LayerDrop) -> Option<usize> {
        if !self.can_drop(li, d) {
            return None;
        }
        let fresh = self.next_uid();
        let len = self.subtree_len(li);
        let mut moved: Vec<Layer> = self.layers.drain(li..li + len).collect();
        let adj = |i: usize| if i >= li + len { i - len } else { i };
        let (group, at) = match d {
            LayerDrop::Into(c) => {
                let c = adj(c);
                if self.layers[c].uid == 0 {
                    self.layers[c].uid = fresh;
                }
                (Some(self.layers[c].uid), c + 1)
            }
            LayerDrop::Before(r) => {
                let r = adj(r);
                (self.layers[r].group, r)
            }
            LayerDrop::After(r) => {
                let r = adj(r);
                (self.layers[r].group, r + self.subtree_len(r))
            }
        };
        moved[0].group = group;
        for (k, l) in moved.into_iter().enumerate() {
            self.layers.insert(at + k, l);
        }
        self.normalize_groups();
        Some(at)
    }

    /// Mueve la capa li a la colección ci. Devuelve el nuevo índice de la capa.
    pub fn move_to_collection(&mut self, li: usize, ci: usize) -> Option<usize> {
        self.move_layer_tree(li, LayerDrop::Into(ci))
    }

    /// Saca la capa li de su colección (queda justo debajo de ella, en la colección de fuera). Devuelve el nuevo índice.
    pub fn leave_collection(&mut self, li: usize) -> Option<usize> {
        let ci = self.collection_of(li)?;
        self.move_layer_tree(li, LayerDrop::After(ci))
    }

    /// Deshace la colección ci: lo que contiene pasa a la colección de fuera (o queda suelto). Devuelve el índice donde queda
    /// lo primero que contenía.
    pub fn ungroup(&mut self, ci: usize) -> Option<usize> {
        if ci >= self.layers.len() || self.layers[ci].kind != LayerKind::Collection {
            return None;
        }
        self.release_members(ci);
        self.release_children(ci);
        self.layers.remove(ci);
        Some(ci.min(self.layers.len().saturating_sub(1)))
    }

    /// Pasa lo que contiene la colección ci a la colección de fuera (antes de borrarla).
    pub fn release_members(&mut self, ci: usize) {
        let uid = self.layers[ci].uid;
        let up = self.layers[ci].group;
        if uid != 0 {
            for l in &mut self.layers {
                if l.group == Some(uid) {
                    l.group = up;
                }
            }
        }
    }

    /// Deja las colecciones en orden: lo que contiene cada una, justo debajo de ella (a cualquier profundidad), sin ciclos y sin
    /// referencias a colecciones que no existen. Devuelve dónde queda cada capa (índice antiguo → nuevo).
    pub fn normalize_groups(&mut self) -> Vec<usize> {
        let n = self.layers.len();
        let mut by_uid: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
        for (i, l) in self.layers.iter().enumerate() {
            if l.kind == LayerKind::Collection && l.uid != 0 {
                by_uid.entry(l.uid).or_insert(i);
            }
        }
        for l in &mut self.layers {
            if l.group.map_or(false, |g| !by_uid.contains_key(&g)) {
                l.group = None;
            }
        }
        // Sin ciclos: una capa que se contiene a sí misma por el camino se suelta.
        for i in 0..n {
            let mut cur = i;
            for _ in 0..=n {
                match self.layers[cur].group.map(|g| by_uid[&g]) {
                    Some(p) if p == i => {
                        self.layers[i].group = None;
                        break;
                    }
                    Some(p) => cur = p,
                    None => break,
                }
            }
        }
        let mut children: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
        for j in 0..n {
            if let Some(p) = self.layers[j].group.map(|g| by_uid[&g]) {
                children.entry(p).or_default().push(j);
            }
        }
        fn visit(i: usize, children: &std::collections::HashMap<usize, Vec<usize>>, seen: &mut Vec<bool>, order: &mut Vec<usize>) {
            if std::mem::replace(&mut seen[i], true) {
                return;
            }
            order.push(i);
            for &c in children.get(&i).map(|v| v.as_slice()).unwrap_or(&[]) {
                visit(c, children, seen, order);
            }
        }
        let mut seen = vec![false; n];
        let mut order: Vec<usize> = Vec::with_capacity(n);
        for i in 0..n {
            if self.layers[i].group.is_none() {
                visit(i, &children, &mut seen, &mut order);
            }
        }
        for i in 0..n {
            if !seen[i] {
                self.layers[i].group = None;
                visit(i, &children, &mut seen, &mut order);
            }
        }
        if order.iter().enumerate().all(|(k, &i)| k == i) {
            return (0..n).collect();
        }
        let mut map = vec![0usize; n];
        for (new, &old) in order.iter().enumerate() {
            map[old] = new;
        }
        let old_layers = std::mem::take(&mut self.layers);
        self.layers = order.iter().map(|&i| old_layers[i].clone()).collect();
        map
    }
    // ---------- Precomposiciones ----------

    /// La composición guardada con ese id (no está si es una que se está editando ahora mismo).
    pub fn comp(&self, id: u64) -> Option<&Composition> {
        self.comps.iter().find(|c| c.id == id)
    }

    fn next_comp_id(&self) -> u64 {
        self.comps.iter().map(|c| c.id).chain(self.stack.iter().map(|l| l.comp_id)).max().unwrap_or(0) + 1
    }

    /// ¿Se está editando una precomposición (y no la composición principal)?
    pub fn in_comp(&self) -> bool {
        !self.stack.is_empty()
    }

    /// El camino hasta lo que se está editando: «Escena» y cada precomposición abierta.
    pub fn comp_path(&self) -> Vec<String> {
        let mut v = vec!["Escena".to_string()];
        for lv in &self.stack {
            v.push(self.comp(lv.comp_id).map_or_else(|| self.open_comp_name(lv.comp_id), |c| c.name.clone()));
        }
        v
    }

    /// El nombre de una composición abierta: su capa en la composición de fuera.
    fn open_comp_name(&self, id: u64) -> String {
        for (k, lv) in self.stack.iter().enumerate() {
            if let Some(l) = lv.layers.iter().find(|l| l.precomp == Some(id)) {
                let _ = k;
                return l.name.clone();
            }
        }
        for l in &self.layers {
            if l.precomp == Some(id) {
                return l.name.clone();
            }
        }
        "Precomposición".into()
    }

    /// Entra a editar la precomposición de la capa li: sus capas pasan a ser ``layers`` y las de fuera esperan en la pila.
    /// ``cl`` y ``frame`` son lo que había elegido fuera, para volver a ello. Devuelve false si la capa no es una
    /// precomposición que se pueda abrir.
    pub fn enter_comp(&mut self, li: usize, cl: usize, frame: u32) -> bool {
        let Some(id) = self.layers.get(li).filter(|l| l.kind == LayerKind::Precomp).and_then(|l| l.precomp) else { return false };
        let Some(ci) = self.comps.iter().position(|c| c.id == id) else { return false };
        if self.stack.iter().any(|l| l.comp_id == id) {
            return false;
        }
        let inner = std::mem::take(&mut self.comps[ci].layers);
        let len = self.comps[ci].length.max(1);
        let outer = std::mem::replace(&mut self.layers, inner);
        self.stack.push(Level { comp_id: id, layers: outer, length: self.length, cl, frame });
        self.length = len;
        if self.layers.is_empty() {
            self.layers.push(Layer::new("Capa 1"));
        }
        true
    }

    /// Sale de la precomposición que se edita: lo hecho dentro se guarda en su composición y vuelven las capas de fuera.
    /// Devuelve la capa y el cuadro que había elegidos fuera.
    pub fn exit_comp(&mut self) -> Option<(usize, u32)> {
        let lv = self.stack.pop()?;
        let inner = std::mem::replace(&mut self.layers, lv.layers);
        if let Some(c) = self.comps.iter_mut().find(|c| c.id == lv.comp_id) {
            c.layers = inner;
            c.length = self.length;
        }
        self.length = lv.length;
        Some((lv.cl, lv.frame))
    }

    /// El proyecto tal como se guarda y se exporta: la composición principal, aunque se esté editando una precomposición.
    pub fn root_clone(&self) -> Project {
        let mut p = self.clone();
        while p.exit_comp().is_some() {}
        p
    }

    /// Pone el animatic en el proyecto: la composición «Animatic» con las capas-clip dadas (se actualiza si ya existía, en vez de duplicarla) y su capa de precomposición arriba del todo. Devuelve el índice de esa capa, o `None` si la composición está abierta ahora mismo.
    pub fn set_animatic(&mut self, built: crate::animatic::Built) -> Option<usize> {
        let existing = self.layers.iter().position(|l| l.kind == LayerKind::Precomp && l.precomp.and_then(|id| self.comp(id)).map_or(false, |c| c.name == crate::animatic::NAME));
        if let Some(i) = existing {
            let id = self.layers[i].precomp?;
            if self.stack.iter().any(|lv| lv.comp_id == id) {
                return None;
            }
            let c = self.comps.iter_mut().find(|c| c.id == id)?;
            c.layers = built.layers;
            c.length = built.length;
            return Some(i);
        }
        if self.stack.iter().any(|lv| self.comp(lv.comp_id).map_or(false, |c| c.name == crate::animatic::NAME)) {
            return None;
        }
        let id = self.next_comp_id();
        self.comps.push(Composition { id, name: crate::animatic::NAME.into(), layers: built.layers, length: built.length });
        let mut pl = Layer::new(crate::animatic::NAME);
        pl.kind = LayerKind::Precomp;
        pl.precomp = Some(id);
        pl.uid = self.next_uid();
        self.layers.insert(0, pl);
        self.normalize_groups();
        Some(0)
    }

    /// Convierte las capas elegidas (o una colección con su contenido) en una precomposición nueva, en el sitio de la de más
    /// arriba y con el mismo aspecto que antes. Devuelve el índice de la capa de precomposición, o ``None`` si hay una cámara.
    pub fn precompose(&mut self, idx: &[usize]) -> Option<usize> {
        let n = self.layers.len();
        let mut sel: Vec<usize> = idx.iter().copied().filter(|&i| i < n).collect();
        sel.sort_unstable();
        sel.dedup();
        if sel.is_empty() || sel.iter().any(|&i| self.layers[i].kind == LayerKind::Camera) {
            return None;
        }
        let roots: Vec<usize> = sel.iter().copied().filter(|&i| !self.ancestors(i).iter().any(|a| sel.contains(a))).collect();
        let first = roots[0];
        let id = self.next_comp_id();
        let name = format!("Precomp {}", self.comps.len() + self.stack.len() + 1);
        let group = self.collection_of(first).map(|ci| self.layers[ci].uid);
        let mut trees: Vec<Vec<Layer>> = Vec::new();
        for &r in roots.iter().rev() {
            let len = self.subtree_len(r);
            trees.push(self.layers.drain(r..r + len).collect());
        }
        trees.reverse();
        for t in &mut trees {
            t[0].group = None; // ya no está en la colección de fuera
        }
        let mut inner: Vec<Layer> = trees.into_iter().flatten().collect();
        // Los vínculos que cruzan el límite de la composición se sueltan en los dos sentidos.
        let moved: std::collections::HashSet<u64> = inner.iter().filter(|l| l.uid != 0).map(|l| l.uid).collect();
        for l in &mut inner {
            if l.parent.map_or(false, |p| !moved.contains(&p)) {
                l.parent = None;
            }
        }
        for l in &mut self.layers {
            if l.parent.map_or(false, |p| moved.contains(&p)) {
                l.parent = None;
            }
        }
        self.comps.push(Composition { id, name: name.clone(), layers: inner, length: self.length });
        let mut pl = Layer::new(name);
        pl.kind = LayerKind::Precomp;
        pl.precomp = Some(id);
        pl.group = group;
        pl.uid = self.next_uid();
        self.layers.insert(first, pl);
        let map = self.normalize_groups();
        Some(map[first])
    }

    /// Deshace la precomposición de la capa li: sus capas vuelven a la composición de fuera, en su sitio y en su tiempo (se
    /// pierden los ajustes propios de la capa de precomposición, como su opacidad o su fusión). Devuelve el índice de la primera.
    pub fn unprecompose(&mut self, li: usize) -> Option<usize> {
        let l = self.layers.get(li).filter(|l| l.kind == LayerKind::Precomp)?;
        let id = l.precomp?;
        let (group, shift) = (l.group, l.in_f as i32);
        let comp = self.comps.iter().find(|c| c.id == id)?.clone();
        if self.stack.iter().any(|lv| lv.comp_id == id) {
            return None;
        }
        // La composición se borra si ninguna otra capa la usa (una precomposición duplicada la comparte).
        let shared = self.layers.iter().enumerate().any(|(i, o)| i != li && o.precomp == Some(id)) || self.comps.iter().any(|c| c.layers.iter().any(|o| o.precomp == Some(id)));
        if !shared {
            self.comps.retain(|c| c.id != id);
        }
        // Los identificadores de las capas de dentro no pueden chocar con los de fuera.
        let mut base = self.next_uid();
        let mut remap: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
        let mut inner = comp.layers;
        for l in &mut inner {
            if l.uid != 0 {
                remap.insert(l.uid, base);
                l.uid = base;
                base += 1;
            }
        }
        for l in &mut inner {
            l.group = l.group.and_then(|g| remap.get(&g).copied());
            l.parent = l.parent.and_then(|p| remap.get(&p).copied());
            l.shift_time(shift);
        }
        for l in &mut inner {
            if l.group.is_none() {
                l.group = group;
            }
        }
        self.layers.remove(li);
        for (k, l) in inner.into_iter().enumerate() {
            self.layers.insert(li + k, l);
        }
        let map = self.normalize_groups();
        Some(map[li])
    }

    /// Dibujo con todo lo visible en el fotograma `f` (para los planos del StoryBoard).
    pub fn flatten(&self, f: u32) -> Drawing {
        let (w, h) = (self.width as f32, self.height as f32);
        let mut out = Drawing::default();
        let any_solo = self.layers.iter().any(|l| l.solo);
        for (li, l) in self.layers.iter().enumerate().rev() {
            if !self.shows(li, f, any_solo) {
                continue;
            }
            let t = self.effective_at(li, f);
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
        sun.tracks.set(Prop::Position, 0, [0.0, 0.0], Interp::Ease);
        sun.tracks.set(Prop::Position, 23, [-90.0, 30.0], Interp::Linear);
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
                pressure: Vec::new(),
                style: None,
                original: None, nodes: None,
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
                s.fix_attrs();
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
            l.migrate_legacy();
            l.tracks.sanitize();
        }
        self.fix_links();
        self.normalize_groups();
        // Las composiciones de las precomposiciones se reparan igual que la principal.
        for ci in 0..self.comps.len() {
            self.comps[ci].length = self.comps[ci].length.clamp(1, 100_000);
            for l in &mut self.comps[ci].layers {
                l.opacity = if l.opacity.is_finite() { l.opacity.clamp(0.0, 1.0) } else { 1.0 };
                l.keys.retain(|k, _| *k < 1_000_000);
                l.migrate_legacy();
                l.tracks.sanitize();
            }
            std::mem::swap(&mut self.layers, &mut self.comps[ci].layers);
            self.fix_links();
            self.normalize_groups();
            std::mem::swap(&mut self.layers, &mut self.comps[ci].layers);
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
        l.tracks.set_all(0, &XKey::default());
        l.tracks.set_all(10, &XKey { x: 100.0, r: 90.0, s: 2.0, ..XKey::default() });
        let t = l.transform_at(5);
        assert!((t.x - 50.0).abs() < 1e-3 && (t.s - 1.5).abs() < 1e-3);
        let p = [300.0, 200.0];
        let q = t.invert(1280.0, 720.0, t.apply(1280.0, 720.0, p));
        assert!((q[0] - p[0]).abs() < 1e-2 && (q[1] - p[1]).abs() < 1e-2);
        l.tracks.set(Prop::Position, 0, [0.0, 0.0], Interp::Step);
        assert_eq!(l.transform_at(5).x, 0.0);
    }

    #[test]
    fn el_punto_de_anclaje_mueve_el_centro_de_giro_y_se_puede_invertir() {
        let mut k = XKey { r: 90.0, ..XKey::default() };
        let (w, h) = (1280.0, 720.0);
        // Sin anclaje, el centro de la cámara no se mueve al girar.
        let c = k.apply(w, h, [w / 2.0, h / 2.0]);
        assert!((c[0] - w / 2.0).abs() < 1e-3 && (c[1] - h / 2.0).abs() < 1e-3);
        // Con el anclaje desplazado, el punto de anclaje es el que se queda quieto.
        k.ax = 100.0;
        k.ay = -50.0;
        let a = k.apply(w, h, [w / 2.0 + 100.0, h / 2.0 - 50.0]);
        assert!((a[0] - (w / 2.0 + 100.0)).abs() < 1e-3 && (a[1] - (h / 2.0 - 50.0)).abs() < 1e-3);
        let p = [300.0, 200.0];
        let q = k.invert(w, h, k.apply(w, h, p));
        assert!((q[0] - p[0]).abs() < 1e-2 && (q[1] - p[1]).abs() < 1e-2);
    }

    #[test]
    fn una_capa_guardada_con_la_pista_antigua_se_abre_con_pistas_por_propiedad() {
        let json = r#"{"name":"a","visible":true,"locked":false,"opacity":1.0,"keys":{},
            "xform":{"0":{"x":0.0,"y":0.0,"r":0.0,"s":1.0,"o":1.0},"8":{"x":30.0,"y":0.0,"r":12.0,"s":1.1,"o":0.5,"i":"Ease"}}}"#;
        let mut l: Layer = serde_json::from_str(json).unwrap();
        l.migrate_legacy();
        assert_eq!(l.tracks.count(), 2);
        let t = l.transform_at(8);
        assert_eq!((t.x, t.r, t.o), (30.0, 12.0, 0.5));
        // Al guardar, ya no se escribe la pista antigua y se lee igual.
        let out = serde_json::to_string(&l).unwrap();
        assert!(!out.contains("\"xform\""), "la pista antigua no se vuelve a guardar");
        let back: Layer = serde_json::from_str(&out).unwrap();
        assert_eq!(back.tracks, l.tracks);
    }

    #[test]
    fn la_barra_de_la_capa_decide_cuando_se_ve_y_solo_oculta_las_demas() {
        let mut l = Layer::new("a");
        assert!(l.shows_at(0, false) && l.shows_at(999, false), "por defecto dura todo el proyecto");
        l.in_f = 4;
        l.out_f = Some(9);
        assert!(!l.shows_at(3, false) && l.shows_at(4, false) && l.shows_at(9, false) && !l.shows_at(10, false));
        assert_eq!(l.out_frame(24), 9);
        l.out_f = None;
        assert_eq!(l.out_frame(24), 23);
        l.visible = false;
        assert!(!l.shows_at(5, false), "oculta sigue siendo oculta");
        l.visible = true;
        // Con una capa en solo, las que no lo están no se ven.
        assert!(!l.shows_at(5, true));
        l.solo = true;
        assert!(l.shows_at(5, true));
    }

    #[test]
    fn mover_la_capa_en_el_tiempo_arrastra_dibujos_claves_y_barra() {
        let mut l = Layer::new("a");
        l.keys.insert(2, Arc::default());
        l.keys.insert(6, Arc::default());
        l.tracks.set(Prop::Position, 4, [10.0, 0.0], Interp::Linear);
        l.in_f = 2;
        l.out_f = Some(8);
        assert_eq!(l.shift_time(3), 3);
        assert_eq!(l.keys.keys().copied().collect::<Vec<_>>(), vec![5, 9]);
        assert_eq!(l.tracks.frames().into_iter().collect::<Vec<_>>(), vec![7]);
        assert_eq!((l.in_f, l.out_f), (5, Some(11)));
        // Hacia atrás no pasa del cuadro 0: lo primero que usa la capa es el cuadro 5.
        assert_eq!(l.shift_time(-20), -5);
        assert_eq!(l.keys.keys().copied().collect::<Vec<_>>(), vec![0, 4]);
        assert_eq!(l.in_f, 0);
    }

    #[test]
    fn una_capa_guardada_antes_de_la_barra_se_abre_dura_todo_el_proyecto() {
        let l: Layer = serde_json::from_str(r#"{"name":"a","visible":true,"locked":false,"opacity":1.0,"keys":{}}"#).unwrap();
        assert_eq!((l.in_f, l.out_f, l.solo, l.shy, l.label), (0, None, false, false, 0));
        assert!(l.shows_at(500, false));
    }

    #[test]
    fn la_transformacion_de_la_hija_dentro_de_la_madre_es_aplicar_una_y_luego_la_otra() {
        let (w, h) = (1280.0, 720.0);
        let child = XKey { x: 30.0, r: 20.0, s: 1.5, ax: 10.0, ay: -5.0, ..XKey::default() };
        let parent = XKey { x: -50.0, y: 40.0, r: -35.0, s: 0.8, ..XKey::default() };
        let c = child.compose(&parent, w, h);
        for p in [[0.0, 0.0], [300.0, 200.0], [1000.0, 600.0]] {
            let a = parent.apply(w, h, child.apply(w, h, p));
            let b = c.apply(w, h, p);
            assert!((a[0] - b[0]).abs() < 1e-2 && (a[1] - b[1]).abs() < 1e-2, "{p:?}: {a:?} frente a {b:?}");
        }
    }

    #[test]
    fn el_vinculo_hereda_el_movimiento_de_la_madre_sin_ciclos() {
        let mut p = Project::blank(1280, 720, 24, 10);
        p.layers = vec![Layer::new("hija"), Layer::new("madre"), Layer::new("abuela")];
        p.layers[1].tracks.set(Prop::Position, 0, [100.0, 0.0], Interp::Linear);
        p.layers[0].tracks.set(Prop::Position, 0, [5.0, 0.0], Interp::Linear);
        assert!(p.set_parent(0, Some(1)));
        assert_eq!(p.effective_at(0, 0).x, 105.0, "la hija suma su movimiento al de la madre");
        // La madre sin vínculo conserva su propio movimiento; la hija se sigue animando por su cuenta.
        assert_eq!(p.effective_at(1, 0).x, 100.0);
        // Cadena de tres y ciclos.
        assert!(p.set_parent(1, Some(2)));
        p.layers[2].tracks.set(Prop::Position, 0, [1000.0, 0.0], Interp::Linear);
        assert_eq!(p.effective_at(0, 0).x, 1105.0);
        assert!(!p.set_parent(2, Some(0)), "la abuela no puede colgar de su nieta");
        assert!(!p.set_parent(1, Some(1)), "ni una capa de sí misma");
        // Al quitar la madre, la hija queda libre.
        p.release_children(1);
        assert_eq!(p.effective_at(0, 0).x, 5.0);
    }

    #[test]
    fn un_archivo_con_vinculos_rotos_se_repara() {
        let mut p = Project::blank(1280, 720, 24, 10);
        p.layers = vec![Layer::new("a"), Layer::new("b")];
        p.layers[0].uid = 7;
        p.layers[1].uid = 7; // repetido
        p.layers[0].parent = Some(99); // madre que no existe
        p.layers[1].parent = Some(7); // apunta a sí misma o a la otra con el mismo uid
        p.fix_links();
        assert_eq!(p.layers[1].uid, 0, "el uid repetido se descarta");
        assert!(p.layers[0].parent.is_none() && p.layers[1].parent.map_or(true, |u| p.index_of_uid(u).is_some()));
    }

    fn layers_named(names: &[&str]) -> Project {
        let mut p = Project::blank(1280, 720, 24, 10);
        p.layers = names.iter().map(|n| Layer::new(*n)).collect();
        p
    }

    #[test]
    fn agrupar_crea_la_coleccion_donde_estaba_la_capa_y_desagrupar_la_deshace() {
        let mut p = layers_named(&["a", "b", "c"]);
        let ci = p.group_layer(1).unwrap();
        assert_eq!((ci, p.layers[ci].kind, p.layers[ci + 1].name.as_str()), (1, LayerKind::Collection, "b"));
        assert_eq!(p.layers.len(), 4);
        assert_eq!(p.collection_of(2), Some(1));
        assert!(p.collection_of(0).is_none() && p.collection_of(3).is_none());
        let first = p.ungroup(1).unwrap();
        assert_eq!(first, 1);
        assert_eq!(p.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
        assert!(p.layers.iter().all(|l| l.group.is_none()));
    }

    #[test]
    fn mover_a_otra_coleccion_y_sacar_mantienen_las_capas_juntas() {
        let mut p = layers_named(&["a", "b", "c", "d"]);
        let c1 = p.group_layer(0).unwrap(); // [Col1, a, b, c, d]
        p.layers[c1].name = "Col1".into();
        let c2 = p.group_layer(4).unwrap(); // [Col1, a, b, c, Col2, d]
        assert_eq!(p.layers[c2].kind, LayerKind::Collection);
        // «b» pasa a la segunda colección: queda justo debajo de ella.
        let b = p.layers.iter().position(|l| l.name == "b").unwrap();
        let nb = p.move_to_collection(b, c2).unwrap();
        let names: Vec<&str> = p.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names[nb], "b");
        let c2 = p.layers.iter().position(|l| l.name == "Colección 2").unwrap();
        assert!(p.members_of(c2).contains(&nb) && nb > c2);
        assert!(p.members_of(c2).iter().all(|&m| m > c2), "las capas siguen debajo de su colección");
        // Sacarla la deja suelta justo después de su colección.
        let nb = p.leave_collection(nb).unwrap();
        assert!(p.layers[nb].group.is_none());
        // Una colección puede entrar en otra, pero no dentro de su propio contenido.
        let c1 = p.layers.iter().position(|l| l.name == "Col1").unwrap();
        p.move_to_collection(c1, c2).expect("una colección puede entrar en otra");
        let c2 = p.layers.iter().position(|l| l.name == "Colección 2").unwrap();
        let c1 = p.layers.iter().position(|l| l.name == "Col1").unwrap();
        assert_eq!(p.collection_of(c1), Some(c2));
        assert!(p.move_to_collection(c2, c1).is_none(), "no se mete una colección dentro de su propio contenido");
    }

    #[test]
    fn una_capa_de_una_coleccion_se_mueve_con_ella_y_hereda_su_visibilidad() {
        let mut p = layers_named(&["a", "b"]);
        let ci = p.group_layer(0).unwrap();
        p.layers[ci].tracks.set(Prop::Position, 0, [50.0, 0.0], Interp::Linear);
        p.layers[ci + 1].tracks.set(Prop::Position, 0, [5.0, 0.0], Interp::Linear);
        assert_eq!(p.effective_at(ci + 1, 0).x, 55.0, "suma el movimiento de la colección");
        let solo = |p: &Project, li: usize| p.shows(li, 0, false);
        assert!(solo(&p, ci + 1) && !solo(&p, ci), "la colección no dibuja nada por sí misma");
        p.layers[ci].visible = false;
        assert!(!solo(&p, ci + 1), "ocultar la colección oculta sus capas");
        p.layers[ci].visible = true;
        p.layers[ci].in_f = 5;
        assert!(!solo(&p, ci + 1), "fuera de la barra de la colección no se ve");
        // El solo de la colección vale para sus capas.
        p.layers[ci].in_f = 0;
        p.layers[ci].solo = true;
        let any = p.layers.iter().any(|l| l.solo);
        assert!(p.shows(ci + 1, 0, any) && !p.shows(ci + 2, 0, any));
    }

    #[test]
    fn la_fusion_o_la_opacidad_de_una_coleccion_piden_componer_aparte() {
        let mut p = layers_named(&["a", "b"]);
        let ci = p.group_layer(0).unwrap();
        assert!(!p.has_blend_at(0));
        p.layers[ci].opacity = 0.5;
        assert!(p.has_blend_at(0));
        p.layers[ci].opacity = 1.0;
        p.layers[ci].blend = Blend::Multiply;
        assert!(p.has_blend_at(0));
        // Una colección sin capas visibles no hace falta componerla.
        p.layers[ci + 1].visible = false;
        assert!(!p.has_blend_at(0));
    }

    #[test]
    fn borrar_una_coleccion_suelta_sus_capas() {
        let mut p = layers_named(&["a", "b"]);
        let ci = p.group_layer(0).unwrap();
        p.release_members(ci);
        p.layers.remove(ci);
        assert!(p.layers.iter().all(|l| l.group.is_none()));
    }

    #[test]
    fn las_colecciones_se_anidan_sin_limite_y_lo_heredan_todo_por_todos_los_niveles() {
        let mut p = layers_named(&["a", "b", "c"]);
        let c1 = p.group_layer(1).unwrap(); // [a, Col1, b, c]
        // Agrupar una capa que ya está dentro crea una colección dentro de la colección.
        let c2 = p.group_layer(c1 + 1).unwrap(); // [a, Col1, Col2, b, c]
        assert_eq!((p.collection_of(c2), p.collection_of(c2 + 1)), (Some(c1), Some(c2)));
        // Y se puede agrupar una colección entera, con lo que contiene.
        let c3 = p.group_layer(c1).unwrap(); // [a, Col3, Col1, Col2, b, c]
        let (c1, c2, b) = (c3 + 1, c3 + 2, c3 + 3);
        assert_eq!(p.layers[b].name, "b");
        assert_eq!(p.ancestors(b), vec![c2, c1, c3], "de la más cercana a la más lejana");
        assert_eq!(p.depth_of(b), 3);
        assert_eq!(p.subtree_len(c3), 4);
        // Movimiento, visibilidad, solo y plegado se heredan por todos los niveles.
        p.layers[c3].tracks.set(Prop::Position, 0, [100.0, 0.0], Interp::Linear);
        p.layers[c1].tracks.set(Prop::Position, 0, [10.0, 0.0], Interp::Linear);
        p.layers[b].tracks.set(Prop::Position, 0, [1.0, 0.0], Interp::Linear);
        assert_eq!(p.effective_at(b, 0).x, 111.0);
        p.layers[c3].visible = false;
        assert!(!p.shows(b, 0, false));
        p.layers[c3].visible = true;
        p.layers[c3].collapsed = true;
        assert!(p.is_hidden_by_fold(b) && !p.is_hidden_by_fold(c3));
        p.layers[c2].solo = true;
        assert!(p.shows(b, 0, true) && !p.shows(0, 0, true));
        // La opacidad de la colección más externa pide componer aparte.
        p.layers[c3].opacity = 0.5;
        assert!(p.has_blend_at(0));
        // Deshacer la colección del medio sube sus capas un nivel, no las suelta.
        let first = p.ungroup(c1).unwrap();
        let _ = first;
        let b = p.layers.iter().position(|l| l.name == "b").unwrap();
        assert_eq!(p.depth_of(b), 2);
    }

    #[test]
    fn soltar_una_capa_la_mete_en_una_coleccion_o_la_coloca_entre_filas() {
        let mut p = layers_named(&["a", "b", "c", "d"]);
        let ci = p.group_layer(1).unwrap(); // [a, Col, b, c, d]
        let names = |p: &Project| p.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>();
        // «d» dentro de la colección: queda la primera, justo debajo de ella.
        let d = p.layers.iter().position(|l| l.name == "d").unwrap();
        let n = p.move_layer_tree(d, LayerDrop::Into(ci)).unwrap();
        assert_eq!(n, ci + 1);
        assert_eq!(names(&p), vec!["a", "Colección 1", "d", "b", "c"]);
        assert_eq!(p.collection_of(n), Some(ci));
        // «a» después de «b» (que está dentro de la colección): queda dentro de esa misma colección.
        let b = p.layers.iter().position(|l| l.name == "b").unwrap();
        let a = p.move_layer_tree(0, LayerDrop::After(b)).unwrap();
        assert_eq!(p.layers[a].name, "a");
        assert_eq!(p.collection_of(a), p.layers.iter().position(|l| l.kind == LayerKind::Collection), "después de «b» entra en su colección");
        // Antes de la colección: fuera de ella.
        let a = p.move_layer_tree(a, LayerDrop::Before(0)).unwrap();
        assert!(p.collection_of(a).is_none() && a == 0);
        // Una colección se lleva su contenido; no se suelta sobre sí misma ni dentro de lo suyo.
        let col = p.layers.iter().position(|l| l.kind == LayerKind::Collection).unwrap();
        let inner = col + 1;
        assert!(!p.can_drop(col, LayerDrop::Into(col)) && !p.can_drop(col, LayerDrop::After(inner)));
        let end = p.layers.len() - 1;
        let before = names(&p);
        let n = p.move_layer_tree(col, LayerDrop::After(end)).unwrap();
        assert_eq!(p.layers[n].kind, LayerKind::Collection);
        assert_eq!(p.subtree_len(n), 3, "la colección sigue con sus dos capas");
        assert!(p.members_of(n).iter().all(|&m| m > n));
        assert_eq!(before.len(), p.layers.len());
        // Una capa normal no admite «dentro».
        assert!(!p.can_drop(0, LayerDrop::Into(1)) || p.layers[1].kind == LayerKind::Collection);
    }

    #[test]
    fn un_ciclo_de_colecciones_en_un_archivo_roto_se_repara() {
        let mut p = layers_named(&["A", "B"]);
        for (l, uid) in p.layers.iter_mut().zip([1u64, 2]) {
            l.kind = LayerKind::Collection;
            l.uid = uid;
        }
        p.layers[0].group = Some(2);
        p.layers[1].group = Some(1);
        let map = p.normalize_groups();
        assert_eq!(map.len(), 2);
        assert!(p.layers.iter().filter(|l| l.group.is_none()).count() >= 1, "el ciclo se rompe");
        for i in 0..p.layers.len() {
            assert!(p.ancestors(i).len() <= 1);
        }
    }

    #[test]
    fn el_espacio_de_la_capa_cicla_entre_2d_y_los_tres_planos_y_vuelve() {
        let mut l = Layer::new("a");
        assert!(!l.is3d);
        let mut seen = Vec::new();
        for _ in 0..5 {
            l.next_space();
            seen.push((l.is3d, l.plane));
        }
        assert_eq!(seen, vec![(true, 0), (true, 1), (true, 2), (false, 2), (true, 0)]);
        l.is3d = true;
        l.plane = 2;
        l.next_space();
        assert!(!l.is3d, "tras FG vuelve a 2D");
    }

    #[test]
    fn la_profundidad_solo_cuenta_en_capas_3d_y_se_anima() {
        let mut l = Layer::new("a");
        l.depth = 100.0;
        l.tracks.set(Prop::Depth, 0, [0.0, 0.0], Interp::Linear);
        l.tracks.set(Prop::Depth, 10, [200.0, 0.0], Interp::Linear);
        // Una capa 2D se queda fija al tamaño de la cámara aunque tenga pista de profundidad.
        assert_eq!(l.z_at(5), 0.0);
        assert_eq!(l.depth_at(5), 200.0, "base 100 + 100 animados");
        l.is3d = true;
        assert_eq!(l.z_at(0), 100.0);
        assert_eq!(l.z_at(10), 300.0);
        assert!((l.z_at(5) - 200.0).abs() < 1e-3);
        // La cámara siempre es 3D.
        let mut c = Layer::new("cam");
        c.kind = LayerKind::Camera;
        c.depth = -2400.0;
        c.tracks.set(Prop::Depth, 0, [0.0, 0.0], Interp::Linear);
        c.tracks.set(Prop::Depth, 24, [1200.0, 0.0], Interp::Linear);
        assert_eq!(c.z_at(24), -1200.0, "travelling: la cámara se acerca");
        // La profundidad de la madre y de la colección se suma a la de la hija.
        let mut p = Project::blank(1280, 720, 24, 10);
        p.layers = vec![Layer::new("hija"), Layer::new("madre")];
        p.layers[1].tracks.set(Prop::Depth, 0, [50.0, 0.0], Interp::Linear);
        p.layers[0].tracks.set(Prop::Depth, 0, [5.0, 0.0], Interp::Linear);
        assert!(p.set_parent(0, Some(1)));
        assert_eq!(p.effective_at(0, 0).dz, 55.0);
    }

    #[test]
    fn agrupar_varias_capas_las_mete_juntas_en_el_sitio_de_la_de_arriba() {
        let mut p = layers_named(&["a", "b", "c", "d", "e"]);
        let ci = p.group_layers(&[3, 1, 2]).unwrap(); // b, c, d (da igual el orden en que se pasen)
        let names: Vec<&str> = p.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["a", "Colección 1", "b", "c", "d", "e"]);
        assert_eq!(ci, 1);
        assert_eq!(p.members_of(ci), vec![2, 3, 4], "con el orden que tenían");
        assert!(p.collection_of(0).is_none() && p.collection_of(5).is_none());
        // Una sola capa se agrupa como siempre; una lista vacía o con índices que no existen no hace nada.
        assert!(p.group_layers(&[]).is_none() && p.group_layers(&[99]).is_none());
        assert!(p.group_layers(&[0]).is_some());
    }

    #[test]
    fn agrupar_capas_de_distintas_colecciones_y_una_coleccion_con_su_contenido() {
        let mut p = layers_named(&["a", "b", "c", "d"]);
        let c1 = p.group_layer(0).unwrap(); // [Col1, a, b, c, d]
        let c2 = p.group_layer(4).unwrap(); // [Col1, a, b, c, Col2, d]
        let _ = (c1, c2);
        // «a» (en Col1), la propia Col2 y «d» (dentro de Col2): se elige la colección y no hace falta mover «d» por separado.
        let a = p.layers.iter().position(|l| l.name == "a").unwrap();
        let col2 = p.layers.iter().position(|l| l.name == "Colección 2").unwrap();
        let d = p.layers.iter().position(|l| l.name == "d").unwrap();
        let g = p.group_layers(&[a, col2, d]).unwrap();
        assert_eq!(p.layers[g].kind, LayerKind::Collection);
        assert_eq!(p.subtree_len(g), 1 + 1 + 2, "la nueva colección, «a» y la colección 2 con «d»");
        let d = p.layers.iter().position(|l| l.name == "d").unwrap();
        assert_eq!(p.depth_of(d), 3, "«d» está en la colección 2, dentro de la nueva, dentro de la 1");
        // El sitio de la nueva colección es el de «a», dentro de Col1.
        assert_eq!(p.collection_of(g), p.layers.iter().position(|l| l.name == "Colección 1"));
    }

    fn names(p: &Project) -> Vec<String> {
        p.layers.iter().map(|l| l.name.clone()).collect()
    }

    #[test]
    fn precomponer_mete_las_capas_en_una_composicion_y_deja_una_capa_de_precomposicion() {
        let mut p = layers_named(&["a", "b", "c", "d"]);
        let pi = p.precompose(&[2, 1]).unwrap();
        assert_eq!(names(&p), vec!["a", "Precomp 1", "d"]);
        assert_eq!((pi, p.layers[pi].kind), (1, LayerKind::Precomp));
        let id = p.layers[pi].precomp.unwrap();
        let comp = p.comp(id).unwrap();
        assert_eq!(comp.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), vec!["b", "c"], "con el orden que tenían");
        assert_eq!(comp.length, p.length);
        // Una cámara no se precompone.
        p.layers[0].kind = LayerKind::Camera;
        assert!(p.precompose(&[0]).is_none());
        assert_eq!(p.layers.len(), 3, "no se tocó nada");
    }

    #[test]
    fn entrar_y_salir_de_la_precomposicion_conserva_lo_editado_dentro() {
        let mut p = layers_named(&["a", "b", "c"]);
        let pi = p.precompose(&[1, 2]).unwrap();
        assert!(!p.in_comp());
        assert!(p.enter_comp(pi, 0, 7));
        assert!(p.in_comp());
        assert_eq!(names(&p), vec!["b", "c"]);
        assert_eq!(p.comp_path(), vec!["Escena".to_string(), "Precomp 1".to_string()]);
        // Se edita dentro: otro nombre y otra duración.
        p.layers[0].name = "b2".into();
        p.length = 50;
        // La capa de fuera no se puede abrir otra vez estando dentro.
        assert!(!p.enter_comp(0, 0, 0));
        assert_eq!(p.exit_comp(), Some((0, 7)), "vuelve la capa y el cuadro que había fuera");
        assert!(!p.in_comp());
        assert_eq!(names(&p), vec!["a", "Precomp 1"]);
        let comp = p.comp(p.layers[pi].precomp.unwrap()).unwrap();
        assert_eq!(comp.layers[0].name, "b2");
        assert_eq!(comp.length, 50);
        assert_eq!(p.length, 10, "la duración de fuera no cambia");
        // Salir sin estar dentro no hace nada.
        assert!(p.exit_comp().is_none());
    }

    #[test]
    fn las_precomposiciones_se_anidan_y_la_raiz_es_siempre_la_principal() {
        let mut p = layers_named(&["a", "b", "c"]);
        let pi = p.precompose(&[1, 2]).unwrap(); // [a, Precomp 1]
        assert!(p.enter_comp(pi, 1, 0)); // dentro: [b, c]
        let inner = p.precompose(&[0]).unwrap(); // dentro: [Precomp 2, c]
        assert!(p.enter_comp(inner, 0, 0)); // dentro de la segunda: [b]
        assert_eq!(p.comp_path().len(), 3);
        assert_eq!(names(&p), vec!["b"]);
        let root = p.root_clone();
        assert!(!root.in_comp());
        assert_eq!(names(&root), vec!["a", "Precomp 1"]);
        assert_eq!(root.comps.len(), 2);
        // El original sigue dentro (la copia es otra cosa).
        assert!(p.in_comp() && p.stack.len() == 2);
        // Salir dos veces deja las cosas como estaban.
        p.exit_comp();
        p.exit_comp();
        assert_eq!(names(&p), vec!["a", "Precomp 1"]);
        assert_eq!(p.comp(p.layers[1].precomp.unwrap()).unwrap().layers.len(), 2);
    }

    #[test]
    fn deshacer_la_precomposicion_devuelve_las_capas_en_su_tiempo_y_sin_chocar() {
        let mut p = layers_named(&["a", "b", "c"]);
        p.layers[1].uid = 5;
        p.layers[2].uid = 6;
        p.layers[2].parent = Some(5);
        let pi = p.precompose(&[1, 2]).unwrap();
        assert!(p.layers.iter().all(|l| l.parent.is_none()), "el vínculo dentro de la composición se conserva dentro");
        p.layers[pi].in_f = 3;
        p.layers[0].uid = 5; // choca con el uid de una capa de dentro
        let first = p.unprecompose(pi).unwrap();
        assert_eq!(names(&p), vec!["a", "b", "c"]);
        assert_eq!(first, 1);
        assert!(p.comps.is_empty(), "la composición ya no la usa nadie");
        let mut uids: Vec<u64> = p.layers.iter().map(|l| l.uid).filter(|u| *u != 0).collect();
        let n = uids.len();
        uids.sort_unstable();
        uids.dedup();
        assert_eq!(uids.len(), n, "sin identificadores repetidos");
        assert_eq!(p.layers[1].in_f, 3, "recuperan su sitio en el tiempo: la precomposición empezaba en el cuadro 4");
        // Una precomposición duplicada comparte la composición: deshacer una no borra la otra.
        let mut q = layers_named(&["a", "b"]);
        let pj = q.precompose(&[0]).unwrap();
        let copy = q.layers[pj].clone();
        q.layers.insert(pj, copy);
        q.unprecompose(pj).unwrap();
        assert_eq!(q.comps.len(), 1);
    }

    #[test]
    fn los_vinculos_que_cruzan_el_limite_de_la_precomposicion_se_sueltan() {
        let mut p = layers_named(&["fuera", "dentro"]);
        p.layers[0].uid = 1;
        p.layers[1].uid = 2;
        p.layers[1].parent = Some(1); // «dentro» cuelga de «fuera»
        let pi = p.precompose(&[1]).unwrap();
        let comp = p.comp(p.layers[pi].precomp.unwrap()).unwrap();
        assert!(comp.layers[0].parent.is_none());
    }

    #[test]
    fn el_animatic_se_crea_una_vez_y_se_actualiza_en_vez_de_duplicarse() {
        let panels = |n: usize| (1..=n).map(|i| Panel { title: format!("P{i}"), dur: 1.0, ..Default::default() }).collect::<Vec<_>>();
        let mut p = layers_named(&["a"]);
        let i = p.set_animatic(crate::animatic::build(&panels(2), 24)).unwrap();
        assert_eq!((i, p.layers[0].kind, p.layers[0].name.as_str()), (0, LayerKind::Precomp, "Animatic"));
        assert_eq!(p.comps.len(), 1);
        assert_eq!(p.comp(p.layers[0].precomp.unwrap()).unwrap().layers.len(), 2);
        // Con más planos se actualiza la misma composición.
        let j = p.set_animatic(crate::animatic::build(&panels(3), 24)).unwrap();
        assert_eq!((j, p.comps.len(), p.layers.len()), (0, 1, 2));
        let c = p.comp(p.layers[0].precomp.unwrap()).unwrap();
        assert_eq!((c.layers.len(), c.length), (3, 72));
        // Estando dentro de la composición no se puede regenerar.
        assert!(p.enter_comp(0, 0, 0));
        assert!(p.set_animatic(crate::animatic::build(&panels(1), 24)).is_none());
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
        d.strokes.push(Stroke { kind: ShapeKind::Line, points: vec![[0.0, 0.0], [100.0, 0.0]], color: [0, 0, 0, 255], width: 2.0, closed: false, fill: None, pressure: Vec::new(), style: None, original: None, nodes: None });
        assert!(d.erase([50.0, 0.0], 5.0));
        assert_eq!(d.strokes.len(), 2);
    }
}
