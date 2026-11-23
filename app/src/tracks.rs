//! Pistas de animación de una capa (Issue #98, fase M1 de docs/editor-de-tiempo-montaje.md).
//!
//! Antes una capa tenía una sola pista con todo junto: cada clave guardaba posición, giro, escala y opacidad a la vez.
//! Ahora hay **una pista por propiedad** (punto de anclaje, posición, escala, rotación y opacidad), como en el prototipo:
//! cada propiedad tiene sus propias claves, con su valor y su interpolación, y se puede animar una sin tocar las demás.
//!
//! Este módulo no depende de la interfaz. `eval` devuelve la transformación completa (`XKey`) en un cuadro, que es lo que
//! usan el visor y la exportación.

use crate::model::{ease, Interp, XKey};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Propiedad animable de una capa.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Prop {
    Anchor,
    Position,
    Scale,
    Rotation,
    Opacity,
}

impl Prop {
    pub const ALL: [Prop; 5] = [Prop::Anchor, Prop::Position, Prop::Scale, Prop::Rotation, Prop::Opacity];

    pub fn name(self) -> &'static str {
        match self {
            Prop::Anchor => "Punto de anclaje",
            Prop::Position => "Posición",
            Prop::Scale => "Escala",
            Prop::Rotation => "Rotación",
            Prop::Opacity => "Opacidad",
        }
    }

    /// Valor por defecto (el de una capa sin animar). Las propiedades escalares usan solo el primer número.
    pub fn default_value(self) -> [f32; 2] {
        match self {
            Prop::Anchor | Prop::Position | Prop::Rotation => [0.0, 0.0],
            Prop::Scale | Prop::Opacity => [1.0, 0.0],
        }
    }

    /// El valor de esta propiedad dentro de una transformación completa.
    pub fn of(self, k: &XKey) -> [f32; 2] {
        match self {
            Prop::Anchor => [k.ax, k.ay],
            Prop::Position => [k.x, k.y],
            Prop::Scale => [k.s, 0.0],
            Prop::Rotation => [k.r, 0.0],
            Prop::Opacity => [k.o, 0.0],
        }
    }

    fn put(self, k: &mut XKey, v: [f32; 2]) {
        match self {
            Prop::Anchor => (k.ax, k.ay) = (v[0], v[1]),
            Prop::Position => (k.x, k.y) = (v[0], v[1]),
            Prop::Scale => k.s = v[0],
            Prop::Rotation => k.r = v[0],
            Prop::Opacity => k.o = v[0],
        }
    }
}

/// Una clave de una pista: el valor y cómo se pasa a la siguiente clave.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub struct PKey {
    pub v: [f32; 2],
    #[serde(default)]
    pub i: Interp,
}

/// Las cinco pistas de una capa. Una pista vacía significa «sin animar»: la propiedad vale su valor por defecto.
#[derive(Clone, PartialEq, Default, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Tracks {
    anchor: BTreeMap<u32, PKey>,
    position: BTreeMap<u32, PKey>,
    scale: BTreeMap<u32, PKey>,
    rotation: BTreeMap<u32, PKey>,
    opacity: BTreeMap<u32, PKey>,
}

impl Tracks {
    fn map(&self, p: Prop) -> &BTreeMap<u32, PKey> {
        match p {
            Prop::Anchor => &self.anchor,
            Prop::Position => &self.position,
            Prop::Scale => &self.scale,
            Prop::Rotation => &self.rotation,
            Prop::Opacity => &self.opacity,
        }
    }

    fn map_mut(&mut self, p: Prop) -> &mut BTreeMap<u32, PKey> {
        match p {
            Prop::Anchor => &mut self.anchor,
            Prop::Position => &mut self.position,
            Prop::Scale => &mut self.scale,
            Prop::Rotation => &mut self.rotation,
            Prop::Opacity => &mut self.opacity,
        }
    }

    pub fn is_empty(&self) -> bool {
        Prop::ALL.iter().all(|p| self.map(*p).is_empty())
    }

    /// ¿Esta propiedad está animada (tiene alguna clave)?
    pub fn has(&self, p: Prop) -> bool {
        !self.map(p).is_empty()
    }

    /// Cuadros que tienen alguna clave, de cualquier propiedad.
    pub fn frames(&self) -> BTreeSet<u32> {
        Prop::ALL.iter().flat_map(|p| self.map(*p).keys().copied()).collect()
    }

    /// Cuántos cuadros distintos tienen clave.
    pub fn count(&self) -> usize {
        self.frames().len()
    }

    pub fn has_key_at(&self, f: u32) -> bool {
        Prop::ALL.iter().any(|p| self.map(*p).contains_key(&f))
    }

    pub fn has_prop_key_at(&self, p: Prop, f: u32) -> bool {
        self.map(p).contains_key(&f)
    }

    /// Cuadros con clave de una propiedad, en orden (la usará el modo Montaje, M4).
    #[allow(dead_code)]
    pub fn prop_frames(&self, p: Prop) -> Vec<u32> {
        self.map(p).keys().copied().collect()
    }

    /// Valor de una propiedad en el cuadro `f` y la interpolación de la clave anterior. Entre dos claves se interpola con la
    /// interpolación de la primera; antes de la primera y después de la última vale el de esa clave.
    pub fn value_at(&self, p: Prop, f: u32) -> ([f32; 2], Interp) {
        let m = self.map(p);
        let a = m.range(..=f).next_back();
        let b = m.range(f..).next();
        match (a, b) {
            (None, None) => (p.default_value(), Interp::Linear),
            (None, Some((_, k))) | (Some((_, k)), None) => (k.v, k.i),
            (Some((ka, a)), Some((kb, b))) => {
                if ka == kb {
                    return (a.v, a.i);
                }
                let mut t = (f - ka) as f32 / (kb - ka) as f32;
                match a.i {
                    Interp::Step => t = 0.0,
                    Interp::Ease => t = ease(t),
                    Interp::Linear => {}
                }
                ([a.v[0] + (b.v[0] - a.v[0]) * t, a.v[1] + (b.v[1] - a.v[1]) * t], a.i)
            }
        }
    }

    /// La transformación completa de la capa en el cuadro `f`. Su `i` es la interpolación de la posición.
    pub fn eval(&self, f: u32) -> XKey {
        let mut k = XKey::default();
        for p in Prop::ALL {
            let (v, i) = self.value_at(p, f);
            p.put(&mut k, v);
            if p == Prop::Position {
                k.i = i;
            }
        }
        k
    }

    /// Pone (o cambia) una clave de una propiedad.
    pub fn set(&mut self, p: Prop, f: u32, v: [f32; 2], i: Interp) {
        self.map_mut(p).insert(f, PKey { v, i });
    }

    /// Clave en las cinco propiedades con los valores de `k` (como «añadir clave a todo»).
    pub fn set_all(&mut self, f: u32, k: &XKey) {
        for p in Prop::ALL {
            self.set(p, f, p.of(k), k.i);
        }
    }

    /// Clave solo en las propiedades que han cambiado de `old` a `new`: así mover una capa solo anima la posición. Si lo
    /// único que cambia es la interpolación, se aplica a las claves de este cuadro (o se crean si no hay ninguna).
    pub fn set_changed(&mut self, f: u32, new: &XKey, old: &XKey) {
        let mut any = false;
        for p in Prop::ALL {
            if p.of(new) != p.of(old) {
                let i = self.map(p).get(&f).map_or(new.i, |k| k.i);
                self.set(p, f, p.of(new), i);
                any = true;
            }
        }
        if new.i != old.i {
            if self.has_key_at(f) {
                for p in Prop::ALL {
                    if let Some(k) = self.map_mut(p).get_mut(&f) {
                        k.i = new.i;
                    }
                }
            } else if !any {
                self.set_all(f, new);
            }
        }
    }

    /// Quita las claves de todas las propiedades en el cuadro `f`. Devuelve true si había alguna.
    pub fn remove_at(&mut self, f: u32) -> bool {
        let mut any = false;
        for p in Prop::ALL {
            any |= self.map_mut(p).remove(&f).is_some();
        }
        any
    }

    /// Quita la clave de una propiedad en el cuadro `f` (la usará el modo Montaje, M4).
    #[allow(dead_code)]
    pub fn remove_prop_at(&mut self, p: Prop, f: u32) -> bool {
        self.map_mut(p).remove(&f).is_some()
    }

    /// Deja una propiedad sin animar (apaga su cronómetro).
    pub fn clear_prop(&mut self, p: Prop) {
        self.map_mut(p).clear();
    }

    /// Cuadro con clave justo antes de `f` (de cualquier propiedad).
    pub fn prev_frame(&self, f: u32) -> Option<u32> {
        self.frames().range(..f).next_back().copied()
    }

    /// Cuadro con clave justo después de `f` (de cualquier propiedad).
    pub fn next_frame(&self, f: u32) -> Option<u32> {
        self.frames().range(f + 1..).next().copied()
    }

    /// Desplaza una posición las claves desde `f` (al insertar o quitar un fotograma).
    pub fn shift_from(&mut self, f: u32, up: bool) {
        for p in Prop::ALL {
            let m = self.map_mut(p);
            let moved: Vec<(u32, PKey)> = m.split_off(&f).into_iter().collect();
            for (k, x) in moved {
                m.insert(if up { k + 1 } else { k.saturating_sub(1) }, x);
            }
        }
    }

    /// Primer cuadro con clave de cualquier propiedad.
    pub fn first_frame(&self) -> Option<u32> {
        self.frames().iter().next().copied()
    }

    /// Mueve todas las claves `d` cuadros (positivo: más tarde). El llamador se asegura de que ninguna quede antes del cuadro 0.
    pub fn shift_all(&mut self, d: i32) {
        for p in Prop::ALL {
            let m = self.map_mut(p);
            *m = std::mem::take(m).into_iter().map(|(k, x)| ((k as i64 + d as i64).max(0) as u32, x)).collect();
        }
    }

    /// Al quitar el fotograma `f`: una clave que quedaría pegada a la siguiente se descarta.
    pub fn remove_frame_keys(&mut self, f: u32) {
        for p in Prop::ALL {
            let m = self.map_mut(p);
            if m.contains_key(&f) && m.contains_key(&(f + 1)) {
                m.remove(&f);
            }
        }
    }

    /// Duplica la duración (×2): cada clave pasa al cuadro doble.
    pub fn double(&mut self) {
        for p in Prop::ALL {
            let m = self.map_mut(p);
            *m = std::mem::take(m).into_iter().map(|(k, x)| (k * 2, x)).collect();
        }
    }

    /// Deja las pistas válidas (por si vienen de un archivo editado o dañado): números finitos, cuadros razonables, opacidad
    /// entre 0 y 1 y escala acotada.
    pub fn sanitize(&mut self) {
        for p in Prop::ALL {
            let m = self.map_mut(p);
            m.retain(|k, x| *k < 1_000_000 && x.v.iter().all(|v| v.is_finite()));
            for x in m.values_mut() {
                match p {
                    Prop::Opacity => x.v[0] = x.v[0].clamp(0.0, 1.0),
                    Prop::Scale => x.v[0] = x.v[0].clamp(-50.0, 50.0),
                    _ => {}
                }
            }
        }
    }

    /// Convierte las claves de una sola pista (proyectos anteriores) en pistas por propiedad. Cada clave antigua guardaba
    /// posición, giro, escala y opacidad; el punto de anclaje no existía y queda sin animar.
    pub fn from_legacy(old: &BTreeMap<u32, XKey>) -> Tracks {
        let mut t = Tracks::default();
        for (f, k) in old {
            for p in [Prop::Position, Prop::Scale, Prop::Rotation, Prop::Opacity] {
                t.set(p, *f, p.of(k), k.i);
            }
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: f32, y: f32) -> XKey {
        XKey { x, y, ..XKey::default() }
    }

    #[test]
    fn sin_pistas_la_capa_vale_sus_valores_por_defecto() {
        let t = Tracks::default();
        assert!(t.is_empty());
        let k = t.eval(7);
        assert!(k.is_identity());
    }

    #[test]
    fn cada_propiedad_se_anima_sola() {
        let mut t = Tracks::default();
        // Solo la opacidad: dos claves; la posición no se toca.
        t.set(Prop::Opacity, 0, [1.0, 0.0], Interp::Linear);
        t.set(Prop::Opacity, 10, [0.0, 0.0], Interp::Linear);
        assert!(t.has(Prop::Opacity) && !t.has(Prop::Position) && !t.has(Prop::Scale));
        let k = t.eval(5);
        assert!((k.o - 0.5).abs() < 1e-5);
        assert_eq!((k.x, k.y, k.r, k.s), (0.0, 0.0, 0.0, 1.0));
        assert_eq!(t.count(), 2);
    }

    #[test]
    fn mover_una_capa_solo_pone_clave_en_la_posicion() {
        let mut t = Tracks::default();
        let old = XKey::default();
        t.set_changed(3, &pos(40.0, -10.0), &old);
        assert!(t.has(Prop::Position));
        assert!(!t.has(Prop::Rotation) && !t.has(Prop::Scale) && !t.has(Prop::Opacity) && !t.has(Prop::Anchor));
        // Girarla después añade la pista de rotación sin tocar la de posición.
        let moved = t.eval(3);
        t.set_changed(3, &XKey { r: 30.0, ..moved }, &moved);
        assert!(t.has(Prop::Rotation));
        assert_eq!(t.prop_frames(Prop::Position), vec![3]);
    }

    #[test]
    fn la_interpolacion_se_toma_de_la_clave_anterior() {
        let mut t = Tracks::default();
        t.set(Prop::Position, 0, [0.0, 0.0], Interp::Step);
        t.set(Prop::Position, 10, [100.0, 0.0], Interp::Linear);
        assert_eq!(t.eval(5).x, 0.0, "escalón: no se mueve hasta la siguiente clave");
        assert_eq!(t.eval(10).x, 100.0);
        t.set(Prop::Position, 0, [0.0, 0.0], Interp::Linear);
        assert!((t.eval(5).x - 50.0).abs() < 1e-4);
        // Antes de la primera y después de la última clave se mantiene su valor.
        assert_eq!(t.eval(99).x, 100.0);
    }

    #[test]
    fn cambiar_solo_la_interpolacion_no_inventa_claves_si_hay_una_en_el_cuadro() {
        let mut t = Tracks::default();
        t.set(Prop::Position, 2, [5.0, 5.0], Interp::Linear);
        let old = t.eval(2);
        t.set_changed(2, &XKey { i: Interp::Ease, ..old }, &old);
        assert_eq!(t.map(Prop::Position)[&2].i, Interp::Ease);
        assert!(!t.has(Prop::Rotation));
        // Sin clave en el cuadro, cambiar la interpolación crea claves en todo (como antes).
        let mut u = Tracks::default();
        let base = XKey::default();
        u.set_changed(4, &XKey { i: Interp::Step, ..base }, &base);
        assert_eq!(u.count(), 1);
        assert!(u.has(Prop::Opacity) && u.has(Prop::Position));
    }

    #[test]
    fn los_proyectos_antiguos_se_convierten_sin_perder_nada() {
        let mut old = BTreeMap::new();
        old.insert(0, XKey::default());
        old.insert(8, XKey { x: 30.0, r: 12.0, s: 1.1, o: 0.5, i: Interp::Ease, ..XKey::default() });
        let t = Tracks::from_legacy(&old);
        assert_eq!(t.count(), 2, "los mismos cuadros con clave");
        for f in 0..=10 {
            // La capa se ve igual que con la pista antigua: la conversión no cambia ningún valor.
            let a = Tracks::from_legacy(&old).eval(f);
            let b = legacy_eval(&old, f);
            assert!((a.x - b.x).abs() < 1e-4 && (a.r - b.r).abs() < 1e-4 && (a.s - b.s).abs() < 1e-4 && (a.o - b.o).abs() < 1e-4, "cuadro {f}");
        }
        assert!(!t.has(Prop::Anchor));
    }

    /// La evaluación de la pista única de antes, para comparar.
    fn legacy_eval(m: &BTreeMap<u32, XKey>, f: u32) -> XKey {
        let a = m.range(..=f).next_back();
        let b = m.range(f..).next();
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
                XKey { x: l(a.x, b.x), y: l(a.y, b.y), r: l(a.r, b.r), s: l(a.s, b.s), o: l(a.o, b.o), ..*a }
            }
        }
    }

    #[test]
    fn insertar_y_quitar_fotogramas_mueve_las_claves_de_todas_las_pistas() {
        let mut t = Tracks::default();
        t.set(Prop::Position, 2, [1.0, 0.0], Interp::Linear);
        t.set(Prop::Opacity, 4, [0.5, 0.0], Interp::Linear);
        t.shift_from(3, true);
        assert_eq!(t.prop_frames(Prop::Position), vec![2]);
        assert_eq!(t.prop_frames(Prop::Opacity), vec![5]);
        t.double();
        assert_eq!(t.prop_frames(Prop::Position), vec![4]);
        assert_eq!(t.prop_frames(Prop::Opacity), vec![10]);
        assert_eq!((t.prev_frame(10), t.next_frame(4)), (Some(4), Some(10)));
        assert!(t.remove_at(4) && !t.remove_at(4));
        t.clear_prop(Prop::Opacity);
        assert!(t.is_empty());
    }

    #[test]
    fn un_archivo_roto_deja_las_pistas_validas() {
        let mut t = Tracks::default();
        t.set(Prop::Opacity, 0, [7.0, 0.0], Interp::Linear);
        t.set(Prop::Scale, 0, [999.0, 0.0], Interp::Linear);
        t.set(Prop::Position, 0, [f32::NAN, 0.0], Interp::Linear);
        t.set(Prop::Position, 2_000_000, [1.0, 1.0], Interp::Linear);
        t.sanitize();
        assert_eq!(t.eval(0).o, 1.0);
        assert_eq!(t.eval(0).s, 50.0);
        assert!(!t.has(Prop::Position), "la clave con NaN y la de un cuadro absurdo se descartan");
    }

    #[test]
    fn se_guardan_y_se_leen_igual() {
        let mut t = Tracks::default();
        t.set(Prop::Anchor, 1, [4.0, -2.0], Interp::Ease);
        t.set(Prop::Rotation, 6, [90.0, 0.0], Interp::Step);
        let s = serde_json::to_string(&t).unwrap();
        let back: Tracks = serde_json::from_str(&s).unwrap();
        assert_eq!(back, t);
        // Un texto sin pistas da pistas vacías.
        let none: Tracks = serde_json::from_str("{}").unwrap();
        assert!(none.is_empty());
    }
}
