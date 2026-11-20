//! Motor de pinceles de píxeles.
//!
//! Separado de la interfaz y de la representación de los píxeles: recibe una lista de puntos
//! (con la presión del lápiz), coloca "marcas" (dabs) a lo largo del trazo según el espaciado y las
//! acumula en un búfer del trazo. Al terminar, el búfer se compone sobre la capa con la opacidad del
//! trazo. Como en Photoshop, el **flujo** se acumula marca a marca y la **opacidad** es un tope.
//!
//! Fase 1: forma de la punta (dureza, espaciado, flujo, redondez, ángulo).
//! Fase 2: dinámica de forma (variación y control de tamaño, ángulo y redondez) y transferencia
//! (variación y control de opacidad y de flujo) con presión del lápiz.
//! `BrushParams` se guarda con `serde(default)` para poder añadir parámetros sin romper presets antiguos.

use crate::raster::Raster;
use serde::{Deserialize, Serialize};

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

/// Parámetros de la punta y de sus dinámicas. Tamaño, opacidad y suavizado son los de la herramienta.
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

    // ---- Dinámica de forma ----
    /// Variación aleatoria del tamaño de cada marca, 0 a 1 (1 = hasta desaparecer).
    pub size_jitter: f32,
    /// Qué controla el tamaño (presión o inclinación).
    pub size_control: Control,
    /// Tamaño mínimo (fracción del tamaño) cuando el control está al mínimo.
    pub min_size: f32,
    /// Variación aleatoria del ángulo, 0 a 1 (1 = cualquier ángulo).
    pub angle_jitter: f32,
    /// Variación aleatoria de la redondez, 0 a 1.
    pub roundness_jitter: f32,
    /// Redondez mínima al variar.
    pub min_roundness: f32,

    // ---- Transferencia ----
    /// Variación aleatoria de la opacidad de cada marca, 0 a 1.
    pub opacity_jitter: f32,
    /// Qué controla la opacidad de cada marca.
    pub opacity_control: Control,
    /// Variación aleatoria del flujo de cada marca, 0 a 1.
    pub flow_jitter: f32,
    /// Qué controla el flujo de cada marca.
    pub flow_control: Control,
}

impl Default for BrushParams {
    fn default() -> Self {
        // Valores que dan una línea redonda y dura, igual que el trazo vectorial, sin dinámicas.
        BrushParams {
            hardness: 1.0,
            spacing: 10.0,
            flow: 1.0,
            roundness: 1.0,
            angle: 0.0,
            size_jitter: 0.0,
            size_control: Control::Off,
            min_size: 0.0,
            angle_jitter: 0.0,
            roundness_jitter: 0.0,
            min_roundness: 0.25,
            opacity_jitter: 0.0,
            opacity_control: Control::Off,
            flow_jitter: 0.0,
            flow_control: Control::Off,
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
            flow_jitter: u(self.flow_jitter),
            ..self
        }
    }

    /// `true` si alguna propiedad depende del lápiz o del azar.
    pub fn has_dynamics(&self) -> bool {
        self.size_jitter > 0.0
            || self.size_control != Control::Off
            || self.angle_jitter > 0.0
            || self.roundness_jitter > 0.0
            || self.opacity_jitter > 0.0
            || self.opacity_control != Control::Off
            || self.flow_jitter > 0.0
            || self.flow_control != Control::Off
    }
}

/// Todo lo que hace falta para pintar un trazo.
#[derive(Clone, Copy, Debug)]
pub struct BrushSpec {
    /// Diámetro mayor de la punta, en píxeles.
    pub size: f32,
    /// Opacidad máxima del trazo completo, 0 a 1.
    pub opacity: f32,
    pub color: [u8; 3],
    pub params: BrushParams,
    /// Semilla de las variaciones aleatorias (la misma semilla repite el mismo trazo).
    pub seed: u64,
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

/// Propiedades de una marca ya resueltas (con variaciones y control del lápiz).
#[derive(Clone, Copy, Debug)]
struct Dab {
    size: f32,
    angle: f32,
    roundness: f32,
    flow: f32,
    /// Tope de opacidad de esta marca, 0 a 1.
    cap: f32,
}

/// Trazo en curso: búfer del trazo + posición de la última marca.
pub struct LiveStroke {
    pub spec: BrushSpec,
    /// Pintura acumulada del trazo (RGBA premultiplicado, sin la opacidad del trazo).
    pub buf: Raster,
    last: Option<([f32; 2], Pen)>,
    to_next: f32,
    bounds: Option<[u32; 4]>,
    dirty: Option<[u32; 4]>,
    rng: Rng,
    /// Marcas colocadas hasta ahora.
    pub dabs: u32,
}

fn union(a: Option<[u32; 4]>, b: [u32; 4]) -> Option<[u32; 4]> {
    Some(match a {
        None => b,
        Some(a) => [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])],
    })
}

impl LiveStroke {
    pub fn new(w: u32, h: u32, spec: BrushSpec) -> Self {
        let spec = BrushSpec { params: spec.params.clamped(), opacity: spec.opacity.clamp(0.0, 1.0), size: spec.size.max(0.5), ..spec };
        LiveStroke { spec, buf: Raster::new(w, h), last: None, to_next: 0.0, bounds: None, dirty: None, rng: Rng::new(spec.seed), dabs: 0 }
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
    fn resolve(&mut self, pen: Pen) -> Dab {
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
        let flow = (p.flow * (1.0 - p.flow_jitter * self.rng.next()) * control_factor(p.flow_control, pen)).clamp(0.0, 1.0);
        let cap = ((1.0 - p.opacity_jitter * self.rng.next()) * control_factor(p.opacity_control, pen)).clamp(0.0, 1.0);
        Dab { size: size.max(0.5), angle, roundness, flow, cap }
    }

    /// Una marca de la punta centrada en `c`. Devuelve la distancia hasta la próxima marca.
    fn stamp(&mut self, c: [f32; 2], pen: Pen) -> f32 {
        self.dabs += 1;
        let dab = self.resolve(pen);
        let step = (dab.size * self.spec.params.spacing / 100.0).max(0.5);
        let r = (dab.size * 0.5).max(0.25);
        let hardness = self.spec.params.hardness;
        let (sa, ca) = dab.angle.to_radians().sin_cos();
        let ext = r + 1.5;
        let (bw, bh) = (self.buf.w as f32, self.buf.h as f32);
        let x0 = (c[0] - ext).floor().clamp(0.0, bw) as u32;
        let y0 = (c[1] - ext).floor().clamp(0.0, bh) as u32;
        let x1 = (c[0] + ext).ceil().clamp(0.0, bw) as u32;
        let y1 = (c[1] + ext).ceil().clamp(0.0, bh) as u32;
        if x1 <= x0 || y1 <= y0 || dab.flow <= 0.0 || dab.cap <= 0.0 {
            return step;
        }
        let minor = (r * dab.roundness).max(0.25);
        let aa = 1.0 / minor;
        let col = [self.spec.color[0] as f32 / 255.0, self.spec.color[1] as f32 / 255.0, self.spec.color[2] as f32 / 255.0];
        let w = self.buf.w;
        for y in y0..y1 {
            for x in x0..x1 {
                let (dx, dy) = (x as f32 + 0.5 - c[0], y as f32 + 0.5 - c[1]);
                let u = dx * ca + dy * sa;
                let v = -dx * sa + dy * ca;
                let d = ((u / r).powi(2) + (v / minor).powi(2)).sqrt();
                if d >= 1.0 + aa {
                    continue;
                }
                let cover = if hardness >= 0.999 {
                    ((1.0 - d) / aa + 0.5).clamp(0.0, 1.0)
                } else {
                    let t = ((d - hardness) / (1.0 - hardness)).clamp(0.0, 1.0);
                    1.0 - t * t * (3.0 - 2.0 * t)
                };
                let a = cover * dab.flow;
                if a <= 0.0 {
                    continue;
                }
                let i = ((y * w + x) * 4) as usize;
                let px = &mut self.buf.data[i..i + 4];
                let keep = 1.0 - a;
                let da = px[3] as f32 / 255.0;
                let over_a = a + da * keep;
                // La marca no sube la pintura por encima de su tope de opacidad (si ya había más, no la quita).
                let new_a = over_a.min(da.max(dab.cap));
                let scale = if over_a > 0.0 { new_a / over_a } else { 0.0 };
                for k in 0..3 {
                    px[k] = (((col[k] * a + px[k] as f32 / 255.0 * keep) * scale) * 255.0).round().min(255.0) as u8;
                }
                px[3] = (new_a * 255.0).round().min(255.0) as u8;
            }
        }
        let b = [x0, y0, x1, y1];
        self.bounds = union(self.bounds, b);
        self.dirty = union(self.dirty, b);
        step
    }

    /// Zona modificada desde la última vez (para subir solo eso a la textura de vista previa).
    pub fn take_dirty(&mut self) -> Option<[u32; 4]> {
        self.dirty.take()
    }

    /// Marca toda la zona pintada como pendiente de subir.
    pub fn mark_all_dirty(&mut self) {
        self.dirty = self.bounds;
    }

    /// Compone el trazo sobre la capa con la opacidad del trazo.
    pub fn finish_into(&self, dst: &mut Raster) {
        if let Some(b) = self.bounds {
            dst.composite_over(&self.buf, b, self.spec.opacity);
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
mod tests {
    use super::*;

    fn spec(size: f32, params: BrushParams) -> BrushSpec {
        BrushSpec { size, opacity: 1.0, color: [200, 30, 30], params, seed: 1 }
    }

    fn alpha_at(r: &Raster, x: u32, y: u32) -> u8 {
        r.data[((y * r.w + x) * 4 + 3) as usize]
    }

    /// Filas con algo de pintura, de arriba abajo: (primera, última).
    fn rows(r: &Raster) -> (u32, u32) {
        let hit = |y: u32| (0..r.w).any(|x| alpha_at(r, x, y) > 8);
        let ys: Vec<u32> = (0..r.h).filter(|y| hit(*y)).collect();
        (*ys.first().unwrap_or(&0), *ys.last().unwrap_or(&0))
    }

    fn line(ls: &mut LiveStroke, pressure: f32) {
        for i in 0..=40 {
            ls.push_with([20.0 + i as f32 * 3.0, 50.0], Pen { pressure, tilt: 1.0 });
        }
    }

    #[test]
    fn spacing_sets_how_many_dabs_a_line_gets() {
        let count = |spacing: f32, pieces: usize| {
            let mut ls = LiveStroke::new(200, 40, spec(10.0, BrushParams { spacing, ..Default::default() }));
            // La misma línea de 100 px, entera o partida en trozos: el resultado no cambia.
            for i in 0..=pieces {
                ls.push([10.0 + 100.0 * i as f32 / pieces as f32, 20.0]);
            }
            ls.dabs
        };
        assert_eq!(count(25.0, 1), 41);
        assert_eq!(count(25.0, 7), 41);
        assert_eq!(count(100.0, 1), 11);
        assert_eq!(count(100.0, 13), 11);
        assert!(count(10.0, 1) > count(50.0, 1));
    }

    #[test]
    fn hardness_changes_the_edge_but_not_the_center() {
        let paint = |hardness: f32| {
            let mut ls = LiveStroke::new(80, 80, spec(40.0, BrushParams { hardness, ..Default::default() }));
            ls.push([40.0, 40.0]);
            ls.buf
        };
        let (hard, soft) = (paint(1.0), paint(0.0));
        assert_eq!(alpha_at(&hard, 40, 40), 255);
        assert!(alpha_at(&soft, 40, 40) >= 250);
        // A 3/4 del radio (15 px del centro) el duro sigue lleno y el difuso ya se desvaneció.
        assert_eq!(alpha_at(&hard, 55, 40), 255);
        assert!(alpha_at(&soft, 55, 40) < 90, "difuso: {}", alpha_at(&soft, 55, 40));
        // Fuera del radio no hay nada en ninguno.
        assert_eq!(alpha_at(&hard, 66, 40), 0);
        assert_eq!(alpha_at(&soft, 66, 40), 0);
    }

    #[test]
    fn flow_accumulates_and_opacity_caps_the_whole_stroke() {
        let wide = |flow: f32, opacity: f32| {
            let mut s = spec(30.0, BrushParams { flow, spacing: 5.0, ..Default::default() });
            s.opacity = opacity;
            let mut ls = LiveStroke::new(100, 60, s);
            // Ida y vuelta sobre la misma línea: con flujo bajo la pintura se acumula.
            for _ in 0..3 {
                ls.push([20.0, 30.0]);
                ls.push([80.0, 30.0]);
            }
            let mut out = Raster::new(100, 60);
            ls.finish_into(&mut out);
            alpha_at(&out, 50, 30)
        };
        let low_flow_many_passes = wide(0.2, 1.0);
        let single_pass_low_flow = {
            let mut ls = LiveStroke::new(100, 60, spec(30.0, BrushParams { flow: 0.2, spacing: 100.0, ..Default::default() }));
            ls.push([50.0, 30.0]);
            alpha_at(&ls.buf, 50, 30)
        };
        assert!(low_flow_many_passes > single_pass_low_flow + 80, "{low_flow_many_passes} vs {single_pass_low_flow}");
        // La opacidad es el tope, por muchas marcas que se acumulen.
        let capped = wide(1.0, 0.5);
        assert!((capped as i32 - 128).abs() <= 2, "alpha {capped}");
        // Flujo y opacidad son independientes: mismo flujo, distinta opacidad.
        assert!(wide(0.2, 0.4) < wide(0.2, 1.0));
    }

    #[test]
    fn roundness_and_angle_shape_the_tip() {
        let tip = |roundness: f32, angle: f32| {
            let mut ls = LiveStroke::new(100, 100, spec(60.0, BrushParams { roundness, angle, ..Default::default() }));
            ls.push([50.0, 50.0]);
            ls.buf
        };
        let flat = tip(0.25, 0.0);
        // Punta plana horizontal: ancha en X y estrecha en Y.
        assert!(alpha_at(&flat, 75, 50) > 200);
        assert_eq!(alpha_at(&flat, 50, 65), 0);
        // Girada 90°: ahora es estrecha en X y alta en Y.
        let turned = tip(0.25, 90.0);
        assert_eq!(alpha_at(&turned, 65, 50), 0);
        assert!(alpha_at(&turned, 50, 75) > 200);
    }

    #[test]
    fn pressure_controls_size_between_min_and_full() {
        let params = BrushParams { size_control: Control::Pressure, min_size: 0.2, ..Default::default() };
        let tall = |pressure: f32| {
            let mut ls = LiveStroke::new(200, 100, spec(40.0, params));
            line(&mut ls, pressure);
            let (a, b) = rows(&ls.buf);
            b - a + 1
        };
        let (full, half, none) = (tall(1.0), tall(0.5), tall(0.0));
        assert!((38..=42).contains(&full), "presión máxima: {full}");
        assert!(half < full && half > none, "{none} < {half} < {full}");
        // Con presión 0 queda el tamaño mínimo (20 % de 40 = 8 px).
        assert!((6..=11).contains(&none), "presión cero: {none}");
        // Sin control de tamaño, la presión no cambia nada.
        let plain = |pressure: f32| {
            let mut ls = LiveStroke::new(200, 100, spec(40.0, BrushParams::default()));
            line(&mut ls, pressure);
            rows(&ls.buf)
        };
        assert_eq!(plain(0.1), plain(1.0));
    }

    #[test]
    fn pressure_controls_flow_and_opacity_independently() {
        let single = |params: BrushParams, pressure: f32| {
            let mut ls = LiveStroke::new(80, 80, spec(30.0, BrushParams { spacing: 100.0, ..params }));
            ls.push_with([40.0, 40.0], Pen { pressure, tilt: 1.0 });
            alpha_at(&ls.buf, 40, 40)
        };
        let flow = BrushParams { flow_control: Control::Pressure, ..Default::default() };
        assert!((single(flow, 0.3) as i32 - 77).abs() <= 2, "flujo con presión 0,3: {}", single(flow, 0.3));
        assert_eq!(single(flow, 1.0), 255);
        let opacity = BrushParams { opacity_control: Control::Pressure, ..Default::default() };
        assert!((single(opacity, 0.5) as i32 - 128).abs() <= 2);
        // La opacidad por presión es un tope: muchas pasadas no la superan, el flujo por presión sí se acumula.
        let many = |params: BrushParams, pressure: f32| {
            let mut ls = LiveStroke::new(100, 60, spec(30.0, BrushParams { spacing: 5.0, ..params }));
            for _ in 0..4 {
                ls.push_with([20.0, 30.0], Pen { pressure, tilt: 1.0 });
                ls.push_with([80.0, 30.0], Pen { pressure, tilt: 1.0 });
            }
            alpha_at(&ls.buf, 50, 30)
        };
        assert!((many(opacity, 0.5) as i32 - 128).abs() <= 2, "tope: {}", many(opacity, 0.5));
        assert!(many(flow, 0.3) > 200, "acumula: {}", many(flow, 0.3));
    }

    #[test]
    fn tilt_control_has_no_effect_without_a_device() {
        let params = BrushParams { size_control: Control::Tilt, min_size: 0.1, ..Default::default() };
        let mut a = LiveStroke::new(200, 100, spec(40.0, params));
        let mut b = LiveStroke::new(200, 100, spec(40.0, BrushParams::default()));
        line(&mut a, 0.2);
        line(&mut b, 0.2);
        assert_eq!(a.buf.data, b.buf.data);
        // Con inclinación real (lápiz casi tumbado) el tamaño baja.
        let mut c = LiveStroke::new(200, 100, spec(40.0, params));
        for i in 0..=40 {
            c.push_with([20.0 + i as f32 * 3.0, 50.0], Pen { pressure: 1.0, tilt: 0.1 });
        }
        let (t0, t1) = rows(&c.buf);
        assert!(t1 - t0 < 20);
    }

    #[test]
    fn jitter_varies_marks_but_repeats_with_the_same_seed() {
        let wild = BrushParams { size_jitter: 1.0, angle_jitter: 1.0, roundness: 0.3, roundness_jitter: 0.8, flow_jitter: 0.8, spacing: 25.0, ..Default::default() };
        let run = |seed: u64| {
            let mut ls = LiveStroke::new(200, 100, BrushSpec { seed, ..spec(30.0, wild) });
            line(&mut ls, 1.0);
            ls.buf.data
        };
        assert_eq!(run(5), run(5));
        assert_ne!(run(5), run(6));
        // Sin variaciones, el trazo no depende de la semilla.
        let calm = |seed: u64| {
            let mut ls = LiveStroke::new(200, 100, BrushSpec { seed, ..spec(30.0, BrushParams::default()) });
            line(&mut ls, 1.0);
            ls.buf.data
        };
        assert_eq!(calm(1), calm(99));
    }

    #[test]
    fn size_angle_and_roundness_jitter_change_the_footprint() {
        let extent = |params: BrushParams| {
            let mut ls = LiveStroke::new(260, 140, BrushSpec { seed: 3, ..spec(40.0, params) });
            for i in 0..=60 {
                ls.push([20.0 + i as f32 * 3.5, 70.0]);
            }
            let (a, b) = rows(&ls.buf);
            b - a + 1
        };
        let flat = BrushParams { roundness: 0.2, spacing: 20.0, ..Default::default() };
        let flat_h = extent(flat);
        // Una punta plana horizontal es baja; con variación de ángulo, al girar, se vuelve más alta.
        assert!(extent(BrushParams { angle_jitter: 1.0, ..flat }) > flat_h + 10);
        // La variación de tamaño nunca supera el tamaño base y hace marcas más pequeñas.
        let round = BrushParams { spacing: 20.0, ..Default::default() };
        assert!(extent(BrushParams { size_jitter: 1.0, ..round }) <= extent(round));
        // La variación de redondez, hacia una punta plana, deja menos pintura que la punta redonda.
        let ink = |params: BrushParams| {
            let mut ls = LiveStroke::new(260, 140, BrushSpec { seed: 3, ..spec(40.0, params) });
            for i in 0..=60 {
                ls.push([20.0 + i as f32 * 3.5, 70.0]);
            }
            ls.buf.data.chunks_exact(4).map(|p| p[3] as u64).sum::<u64>()
        };
        let rj = BrushParams { roundness_jitter: 1.0, min_roundness: 0.1, ..round };
        assert!(ink(rj) < ink(round), "{} vs {}", ink(rj), ink(round));
    }

    #[test]
    fn params_roundtrip_and_old_presets_use_defaults() {
        let p = BrushParams { hardness: 0.3, spacing: 40.0, flow: 0.5, roundness: 0.7, angle: 45.0, size_control: Control::Pressure, flow_jitter: 0.4, ..Default::default() };
        let back: BrushParams = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
        // Un preset de la Fase 1 no trae las dinámicas: quedan apagadas.
        let old: BrushParams = serde_json::from_str(r#"{"hardness":0.5,"spacing":20.0,"flow":1.0,"roundness":1.0,"angle":0.0}"#).unwrap();
        assert_eq!(old.hardness, 0.5);
        assert!(!old.has_dynamics());
        assert_eq!(BrushParams { spacing: 9999.0, roundness: 0.0, size_jitter: 5.0, ..p }.clamped().spacing, 200.0);
        assert_eq!(BrushParams { size_jitter: 5.0, ..p }.clamped().size_jitter, 1.0);
        assert!(BrushParams { flow_control: Control::Pressure, ..Default::default() }.has_dynamics());
    }

    #[test]
    fn preview_paints_something_inside_the_image_and_shows_pressure() {
        let r = preview(spec(14.0, BrushParams::default()), 200, 56);
        assert!(!r.is_blank());
        assert_eq!(alpha_at(&r, 0, 0), 0);
        // Con control de tamaño por presión, la muestra es más fina en los extremos que en el centro.
        let p = preview(spec(20.0, BrushParams { size_control: Control::Pressure, ..Default::default() }), 240, 60);
        let col_height = |x: u32| (0..p.h).filter(|y| alpha_at(&p, x, *y) > 8).count();
        let ends = col_height(24).max(col_height(1).max(col_height(238)));
        assert!(col_height(120) > ends, "centro {} vs extremos {}", col_height(120), ends);
    }
}
