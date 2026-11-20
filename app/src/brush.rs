//! Motor de pinceles de píxeles.
//!
//! Separado de la interfaz y de la representación de los píxeles: recibe una lista de puntos,
//! coloca "marcas" (dabs) a lo largo del trazo según el espaciado y las acumula en un búfer del trazo.
//! Al terminar, el búfer se compone sobre la capa con la opacidad del trazo (como Photoshop:
//! el **flujo** se acumula marca a marca; la **opacidad** es el tope de todo el trazo).
//! `BrushParams` se guarda con `serde(default)` para poder añadir parámetros sin romper presets antiguos.

use crate::raster::Raster;
use serde::{Deserialize, Serialize};

/// Parámetros de la forma de la punta (Fase 1). Tamaño, opacidad y suavizado son los de la herramienta.
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
}

impl Default for BrushParams {
    fn default() -> Self {
        // Valores que dan una línea redonda y dura, igual que el trazo vectorial.
        BrushParams { hardness: 1.0, spacing: 10.0, flow: 1.0, roundness: 1.0, angle: 0.0 }
    }
}

impl BrushParams {
    /// Valores dentro de los rangos válidos (por si vienen de un archivo o de un preset).
    pub fn clamped(self) -> Self {
        BrushParams {
            hardness: self.hardness.clamp(0.0, 1.0),
            spacing: self.spacing.clamp(1.0, 200.0),
            flow: self.flow.clamp(0.01, 1.0),
            roundness: self.roundness.clamp(0.05, 1.0),
            angle: self.angle.rem_euclid(360.0),
        }
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
}

/// Trazo en curso: búfer del trazo + posición de la última marca.
pub struct LiveStroke {
    pub spec: BrushSpec,
    /// Pintura acumulada del trazo (RGBA premultiplicado, sin la opacidad del trazo).
    pub buf: Raster,
    last: Option<[f32; 2]>,
    to_next: f32,
    bounds: Option<[u32; 4]>,
    dirty: Option<[u32; 4]>,
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
        LiveStroke { spec, buf: Raster::new(w, h), last: None, to_next: 0.0, bounds: None, dirty: None, dabs: 0 }
    }

    /// Distancia entre marcas, en píxeles.
    pub fn step(&self) -> f32 {
        (self.spec.size * self.spec.params.spacing / 100.0).max(0.5)
    }

    /// Añade un punto al trazo: coloca las marcas que correspondan entre el punto anterior y este.
    pub fn push(&mut self, p: [f32; 2]) {
        let step = self.step();
        let Some(l) = self.last else {
            self.stamp(p);
            self.last = Some(p);
            self.to_next = step;
            return;
        };
        let d = ((p[0] - l[0]).powi(2) + (p[1] - l[1]).powi(2)).sqrt();
        if d <= 0.0 {
            return;
        }
        let mut s = self.to_next;
        let mut n = 0;
        while s <= d && n < 5000 {
            let t = s / d;
            self.stamp([l[0] + (p[0] - l[0]) * t, l[1] + (p[1] - l[1]) * t]);
            s += step;
            n += 1;
        }
        self.to_next = (s - d).max(0.0);
        self.last = Some(p);
    }

    /// Una marca de la punta centrada en `c`.
    fn stamp(&mut self, c: [f32; 2]) {
        self.dabs += 1;
        let r = (self.spec.size * 0.5).max(0.25);
        let p = self.spec.params;
        let (sa, ca) = p.angle.to_radians().sin_cos();
        let ext = r + 1.5;
        let (bw, bh) = (self.buf.w as f32, self.buf.h as f32);
        let x0 = (c[0] - ext).floor().clamp(0.0, bw) as u32;
        let y0 = (c[1] - ext).floor().clamp(0.0, bh) as u32;
        let x1 = (c[0] + ext).ceil().clamp(0.0, bw) as u32;
        let y1 = (c[1] + ext).ceil().clamp(0.0, bh) as u32;
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let minor = (r * p.roundness).max(0.25);
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
                let cover = if p.hardness >= 0.999 {
                    ((1.0 - d) / aa + 0.5).clamp(0.0, 1.0)
                } else {
                    let t = ((d - p.hardness) / (1.0 - p.hardness)).clamp(0.0, 1.0);
                    1.0 - t * t * (3.0 - 2.0 * t)
                };
                let a = cover * p.flow;
                if a <= 0.0 {
                    continue;
                }
                let i = ((y * w + x) * 4) as usize;
                let px = &mut self.buf.data[i..i + 4];
                let keep = 1.0 - a;
                for k in 0..3 {
                    px[k] = ((col[k] * a + px[k] as f32 / 255.0 * keep) * 255.0).round().min(255.0) as u8;
                }
                px[3] = ((a + px[3] as f32 / 255.0 * keep) * 255.0).round().min(255.0) as u8;
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

    /// Compone el trazo sobre la capa con la opacidad del trazo.
    pub fn finish_into(&self, dst: &mut Raster) {
        if let Some(b) = self.bounds {
            dst.composite_over(&self.buf, b, self.spec.opacity);
        }
    }
}

/// Imagen de muestra de un pincel (un trazo ondulado) para el panel de parámetros.
pub fn preview(spec: BrushSpec, w: u32, h: u32) -> Raster {
    let mut ls = LiveStroke::new(w, h, spec);
    let n = 80;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let x = 18.0 + t * (w as f32 - 36.0);
        let y = h as f32 * 0.5 + (t * std::f32::consts::TAU).sin() * h as f32 * 0.2;
        ls.push([x, y]);
    }
    let mut out = Raster::new(w, h);
    ls.finish_into(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(size: f32, params: BrushParams) -> BrushSpec {
        BrushSpec { size, opacity: 1.0, color: [200, 30, 30], params }
    }

    fn alpha_at(r: &Raster, x: u32, y: u32) -> u8 {
        r.data[((y * r.w + x) * 4 + 3) as usize]
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
    fn params_roundtrip_and_old_presets_use_defaults() {
        let p = BrushParams { hardness: 0.3, spacing: 40.0, flow: 0.5, roundness: 0.7, angle: 45.0 };
        let back: BrushParams = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
        let old: BrushParams = serde_json::from_str(r#"{"hardness":0.5}"#).unwrap();
        assert_eq!(old.hardness, 0.5);
        assert_eq!(old.flow, 1.0);
        assert_eq!(BrushParams { spacing: 9999.0, roundness: 0.0, ..p }.clamped().spacing, 200.0);
    }

    #[test]
    fn preview_paints_something_inside_the_image() {
        let r = preview(spec(14.0, BrushParams::default()), 200, 56);
        assert!(!r.is_blank());
        assert_eq!(alpha_at(&r, 0, 0), 0);
    }
}
