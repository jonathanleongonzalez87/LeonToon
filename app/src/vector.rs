//! Motor de trazos vectoriales de grosor variable (capas vectoriales; Lápiz vectorial y Pluma).
//!
//! Edición no destructiva (Issue #31, sección 6): el trazo guarda por separado
//! - su **geometría**: puntos, presión capturada y si está cerrado (en `Stroke`);
//! - su **estilo**: extremos, uniones, perfil de grosor, respuesta a la presión (`VStyle`, en cada trazo,
//!   así que cambiar un pincel de la biblioteca no altera los trazos ya dibujados);
//! - y un **renderizador**: `pieces` convierte geometría y estilo en polígonos convexos que rellenan tanto la
//!   pantalla como la exportación. Nada se rasteriza: el trazo sigue siendo vectorial a cualquier ampliación.
//!
//! Este módulo es independiente del motor de pinceles de píxeles (`brush`): no comparten parámetros ni formato.

use serde::{Deserialize, Serialize};

pub mod edit;
pub mod library;

/// Aspecto de un trazo que se puede editar después de dibujarlo (para restaurarlo y para copiar propiedades).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Look {
    pub width: f32,
    pub color: crate::model::Rgba,
    pub style: Option<VStyle>,
}

/// Forma de los extremos del trazo.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Cap {
    #[default]
    Round,
    Square,
    Flat,
}

impl Cap {
    pub const ALL: [Cap; 3] = [Cap::Round, Cap::Square, Cap::Flat];
    pub fn label(self) -> &'static str {
        match self {
            Cap::Round => "Redondo",
            Cap::Square => "Cuadrado",
            Cap::Flat => "Plano",
        }
    }
}

/// Unión entre segmentos.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Join {
    #[default]
    Round,
    Bevel,
    Miter,
}

impl Join {
    pub const ALL: [Join; 3] = [Join::Round, Join::Bevel, Join::Miter];
    pub fn label(self) -> &'static str {
        match self {
            Join::Round => "Redonda",
            Join::Bevel => "Bisel",
            Join::Miter => "Inglete",
        }
    }
}

/// Cómo cambia el grosor a lo largo del trazo.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Profile {
    /// Grosor constante.
    #[default]
    Uniform,
    /// Empieza en punta y llega al grosor pleno.
    TaperStart,
    /// Termina en punta.
    TaperEnd,
    /// Punta en los dos extremos.
    TaperBoth,
}

impl Profile {
    pub const ALL: [Profile; 4] = [Profile::Uniform, Profile::TaperStart, Profile::TaperEnd, Profile::TaperBoth];
    pub fn label(self) -> &'static str {
        match self {
            Profile::Uniform => "Uniforme",
            Profile::TaperStart => "Afilado al inicio",
            Profile::TaperEnd => "Afilado al final",
            Profile::TaperBoth => "Afilado en ambos",
        }
    }
}

/// Estilo de un trazo vectorial. Cada trazo lleva el suyo (no una referencia a la biblioteca).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct VStyle {
    pub cap: Cap,
    pub join: Join,
    pub profile: Profile,
    /// Largo del afilado, como fracción del largo del trazo (0,02 a 0,5).
    pub taper: f32,
    /// El grosor sigue la presión del lápiz capturada al dibujar.
    pub pressure: bool,
    /// Grosor con presión 0, como fracción del grosor pleno (0 a 1).
    pub min_width: f32,
    /// Curva de respuesta de la presión: 1 lineal, menos de 1 más sensible, más de 1 más dura.
    pub curve: f32,
    /// Nombre del pincel vectorial del que viene (para la biblioteca de la Fase 2).
    pub brush: Option<String>,
    /// Versión del estilo, para poder reproducir trazos antiguos si el motor cambia.
    pub version: u32,
}

impl Default for VStyle {
    fn default() -> Self {
        VStyle { cap: Cap::Round, join: Join::Round, profile: Profile::Uniform, taper: 0.25, pressure: true, min_width: 0.2, curve: 1.0, brush: None, version: 1 }
    }
}

impl VStyle {
    /// Valores dentro de los rangos válidos (por si vienen de un archivo).
    pub fn clamped(mut self) -> Self {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        self.taper = f(self.taper, 0.02, 0.5, 0.25);
        self.min_width = f(self.min_width, 0.0, 1.0, 0.2);
        self.curve = f(self.curve, 0.2, 4.0, 1.0);
        self
    }
}

/// Longitud acumulada de la línea, de 0 a 1 en cada punto.
fn arc_fractions(points: &[[f32; 2]]) -> Vec<f32> {
    let mut acc = vec![0.0f32; points.len()];
    for i in 1..points.len() {
        acc[i] = acc[i - 1] + dist(points[i - 1], points[i]);
    }
    let total = acc.last().copied().unwrap_or(0.0);
    if total <= 1e-6 {
        return vec![0.0; points.len()];
    }
    acc.iter().map(|a| a / total).collect()
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Grosor en cada punto: el grosor base por el perfil por la presión.
pub fn widths(points: &[[f32; 2]], pressure: &[f32], base: f32, style: &VStyle, closed: bool) -> Vec<f32> {
    let st = style.clone().clamped();
    let t = arc_fractions(points);
    let use_pressure = st.pressure && pressure.len() == points.len();
    (0..points.len())
        .map(|i| {
            let prof = if closed {
                1.0
            } else {
                match st.profile {
                    Profile::Uniform => 1.0,
                    Profile::TaperStart => smooth(t[i] / st.taper),
                    Profile::TaperEnd => smooth((1.0 - t[i]) / st.taper),
                    Profile::TaperBoth => smooth(t[i] / st.taper).min(smooth((1.0 - t[i]) / st.taper)),
                }
            };
            let pf = if use_pressure {
                let p = pressure[i].clamp(0.0, 1.0).powf(st.curve);
                st.min_width + (1.0 - st.min_width) * p
            } else {
                1.0
            };
            (base * prof * pf).max(0.0)
        })
        .collect()
}

/// `true` si el trazo se dibuja igual que antes de existir este motor: grosor constante con extremos y uniones
/// redondos. Esos trazos usan el dibujo rápido de siempre (y los proyectos anteriores se ven idénticos).
pub fn is_plain(style: Option<&VStyle>, pressure: &[f32]) -> bool {
    let Some(s) = style else { return true };
    let varies = s.pressure && pressure.windows(2).any(|w| (w[0] - w[1]).abs() > 1e-3) && s.min_width < 0.999;
    s.profile == Profile::Uniform && s.cap == Cap::Round && s.join == Join::Round && !varies
}

type Poly = Vec<[f32; 2]>;

fn add(a: [f32; 2], b: [f32; 2], k: f32) -> [f32; 2] {
    [a[0] + b[0] * k, a[1] + b[1] * k]
}

/// Orienta el polígono en sentido antihorario (para que el relleno de varios polígonos sea una unión).
fn ccw(mut p: Poly) -> Poly {
    let area: f32 = (0..p.len()).map(|i| {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        a[0] * b[1] - b[0] * a[1]
    }).sum();
    if area < 0.0 {
        p.reverse();
    }
    p
}

/// Abanico de arco alrededor de `c`, de la dirección `from` a `to` (en radianes, por el camino corto o el dado).
fn arc_fan(c: [f32; 2], r: f32, a0: f32, sweep: f32) -> Poly {
    let steps = ((sweep.abs() / (std::f32::consts::PI / 10.0)).ceil() as usize).max(1);
    let mut p = vec![c];
    for i in 0..=steps {
        let a = a0 + sweep * i as f32 / steps as f32;
        p.push([c[0] + r * a.cos(), c[1] + r * a.sin()]);
    }
    p
}

/// Polígonos convexos que, juntos, forman el trazo de grosor variable. La unión de todos es la forma del trazo.
pub fn pieces(points: &[[f32; 2]], widths: &[f32], closed: bool, cap: Cap, join: Join) -> Vec<Poly> {
    let mut pts: Vec<[f32; 2]> = Vec::with_capacity(points.len());
    let mut rad: Vec<f32> = Vec::with_capacity(points.len());
    for (i, p) in points.iter().enumerate() {
        if !(p[0].is_finite() && p[1].is_finite()) {
            continue;
        }
        if pts.last().map_or(false, |q| dist(*q, *p) < 1e-4) {
            // Punto repetido: se queda el más grueso.
            let r = widths.get(i).copied().unwrap_or(0.0) * 0.5;
            if let Some(last) = rad.last_mut() {
                *last = last.max(r);
            }
            continue;
        }
        pts.push(*p);
        rad.push(widths.get(i).copied().unwrap_or(0.0).max(0.0) * 0.5);
    }
    let n = pts.len();
    let mut out: Vec<Poly> = Vec::new();
    if n == 0 {
        return out;
    }
    if closed && n > 2 && dist(pts[0], pts[n - 1]) < 1e-4 {
        pts.pop();
        rad.pop();
    }
    let n = pts.len();
    if n == 1 {
        let r = rad[0];
        if r <= 1e-4 {
            return out;
        }
        let c = pts[0];
        match cap {
            Cap::Round => out.push(ccw(arc_fan(c, r, 0.0, std::f32::consts::TAU)[1..].to_vec())),
            _ => out.push(ccw(vec![[c[0] - r, c[1] - r], [c[0] + r, c[1] - r], [c[0] + r, c[1] + r], [c[0] - r, c[1] + r]])),
        }
        return out;
    }
    // Segmentos
    let seg_count = if closed && n > 2 { n } else { n - 1 };
    let mut dirs: Vec<[f32; 2]> = Vec::with_capacity(seg_count);
    for k in 0..seg_count {
        let (a, b) = (pts[k], pts[(k + 1) % n]);
        let d = dist(a, b).max(1e-9);
        dirs.push([(b[0] - a[0]) / d, (b[1] - a[1]) / d]);
    }
    let normal = |d: [f32; 2]| [-d[1], d[0]];
    for k in 0..seg_count {
        let (a, b) = (pts[k], pts[(k + 1) % n]);
        let (r0, r1) = (rad[k], rad[(k + 1) % n]);
        let nn = normal(dirs[k]);
        if r0 <= 1e-4 && r1 <= 1e-4 {
            continue;
        }
        out.push(ccw(vec![add(a, nn, r0), add(b, nn, r1), add(b, nn, -r1), add(a, nn, -r0)]));
    }
    // Uniones en los vértices interiores (y en todos si está cerrado)
    let first_join = if closed && n > 2 { 0 } else { 1 };
    let last_join = if closed && n > 2 { n } else { n - 1 };
    for i in first_join..last_join {
        let (k0, k1) = (if i == 0 { seg_count - 1 } else { i - 1 }, i % seg_count);
        let (d0, d1) = (dirs[k0], dirs[k1]);
        let cross = d0[0] * d1[1] - d0[1] * d1[0];
        let dot = d0[0] * d1[0] + d0[1] * d1[1];
        let r = rad[i];
        if r <= 1e-3 || (cross.abs() < 1e-4 && dot > 0.0) {
            continue;
        }
        let s = if cross > 0.0 { -1.0 } else { 1.0 };
        let (n0, n1) = (normal(d0), normal(d1));
        let c = pts[i];
        let (a0, a1) = (add(c, n0, s * r), add(c, n1, s * r));
        match join {
            Join::Bevel => out.push(ccw(vec![c, a0, a1])),
            Join::Round => {
                let (t0, t1) = ((s * n0[1]).atan2(s * n0[0]), (s * n1[1]).atan2(s * n1[0]));
                let mut sweep = t1 - t0;
                while sweep > std::f32::consts::PI {
                    sweep -= std::f32::consts::TAU;
                }
                while sweep < -std::f32::consts::PI {
                    sweep += std::f32::consts::TAU;
                }
                out.push(ccw(arc_fan(c, r, t0, sweep)));
            }
            Join::Miter => {
                let q = 1.0 + (n0[0] * n1[0] + n0[1] * n1[1]);
                // Inglete limitado a 4 veces el radio (2/q ≤ 16); más allá se corta en bisel.
                if q >= 0.125 {
                    let m = [c[0] + s * r * (n0[0] + n1[0]) / q, c[1] + s * r * (n0[1] + n1[1]) / q];
                    out.push(ccw(vec![c, a0, m, a1]));
                } else {
                    out.push(ccw(vec![c, a0, a1]));
                }
            }
        }
    }
    // Extremos
    if !(closed && n > 2) {
        let ends = [(0usize, [-dirs[0][0], -dirs[0][1]], dirs[0]), (n - 1, dirs[seg_count - 1], dirs[seg_count - 1])];
        for (idx, out_dir, d) in ends {
            let (c, r) = (pts[idx], rad[idx]);
            if r <= 1e-3 {
                continue;
            }
            let nn = normal(d);
            match cap {
                Cap::Flat => {}
                Cap::Square => {
                    let o = [out_dir[0] * r, out_dir[1] * r];
                    out.push(ccw(vec![add(c, nn, r), [c[0] + nn[0] * r + o[0], c[1] + nn[1] * r + o[1]], [c[0] - nn[0] * r + o[0], c[1] - nn[1] * r + o[1]], add(c, nn, -r)]));
                }
                Cap::Round => {
                    // Semicírculo del lado +normal al -normal pasando por la dirección hacia afuera.
                    let base = nn[1].atan2(nn[0]);
                    let cross = nn[0] * out_dir[1] - nn[1] * out_dir[0];
                    let sweep = if cross > 0.0 { std::f32::consts::PI } else { -std::f32::consts::PI };
                    out.push(ccw(arc_fan(c, r, base, sweep)));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
