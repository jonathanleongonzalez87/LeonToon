//! Edición de trazos vectoriales ya dibujados (Issue #31, Fase 3).
//!
//! Todo es **no destructivo**: solo cambia el estilo, el grosor o el color del trazo; la trayectoria y la presión
//! capturada no se tocan, y nada se rasteriza. La primera vez que se edita un trazo se guarda su aspecto original
//! (`Stroke::original`), de modo que se puede restaurar aunque se haya guardado y reabierto el proyecto.
//!
//! Con varios trazos, cada propiedad se aplica por separado: cambiar el perfil no toca el grosor ni el color de cada uno.

use super::library::VPreset;
use super::{Cap, Join, Look, Profile, VStyle};
use crate::model::Stroke;

/// Una propiedad que se puede cambiar en un trazo.
#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    Width(f32),
    /// Opacidad del trazo (canal alfa de su color).
    Alpha(u8),
    Rgb([u8; 3]),
    Cap(Cap),
    Join(Join),
    Profile(Profile),
    Taper(f32),
    Pressure(bool),
    MinWidth(f32),
    Curve(f32),
    /// Perfil de grosor personalizado (puntos posición–grosor). También pone el perfil en «Personalizado».
    Shape(Vec<[f32; 2]>),
}

/// Guarda el aspecto actual como original, si todavía no se había guardado.
pub fn remember(s: &mut Stroke) {
    if s.original.is_none() {
        s.original = Some(Box::new(s.look()));
    }
}

fn style_mut(s: &mut Stroke) -> &mut VStyle {
    s.style.get_or_insert_with(VStyle::default)
}

/// Cambia una propiedad del trazo (recordando su aspecto original).
pub fn set_prop(s: &mut Stroke, p: &Prop) {
    remember(s);
    match p {
        Prop::Width(w) => s.width = if w.is_finite() { w.clamp(0.1, 500.0) } else { s.width },
        Prop::Alpha(a) => s.color[3] = *a,
        Prop::Rgb(c) => s.color = [c[0], c[1], c[2], s.color[3]],
        Prop::Cap(c) => style_mut(s).cap = *c,
        Prop::Join(j) => style_mut(s).join = *j,
        Prop::Profile(pr) => style_mut(s).profile = *pr,
        Prop::Taper(t) => style_mut(s).taper = *t,
        Prop::Pressure(b) => style_mut(s).pressure = *b,
        Prop::MinWidth(m) => style_mut(s).min_width = *m,
        Prop::Curve(c) => style_mut(s).curve = *c,
        Prop::Shape(v) => {
            let st = style_mut(s);
            st.profile = Profile::Custom;
            st.shape = v.clone();
        }
    }
    if let Some(st) = s.style.take() {
        s.style = Some(st.clamped());
    }
}

/// Vuelve al aspecto original. Devuelve false si el trazo no se había editado.
pub fn restore(s: &mut Stroke) -> bool {
    let Some(o) = s.original.take() else { return false };
    s.width = o.width;
    s.color = o.color;
    s.style = o.style;
    true
}

/// Pega propiedades copiadas de otro trazo (solo los grupos marcados). El color conserva la opacidad propia
/// salvo que se pegue también la opacidad.
pub fn paste_look(s: &mut Stroke, look: &Look, width: bool, style: bool, color: bool) {
    remember(s);
    if width {
        s.width = look.width.clamp(0.1, 500.0);
    }
    if style {
        s.style = look.style.clone().map(|st| st.clamped());
    }
    if color {
        s.color = look.color;
    }
}

/// Asigna otro pincel de la biblioteca: sustituye el estilo (y el grosor si se pide) y conserva trayectoria,
/// presión, color y opacidad del trazo.
pub fn apply_brush(s: &mut Stroke, p: &VPreset, with_width: bool) {
    remember(s);
    let p = p.clone().clamped();
    s.style = Some(VStyle { brush: Some(p.id()), ..p.style });
    if with_width {
        s.width = p.size;
    }
}

/// Avisos de lo que el pincel no podrá hacer con la geometría de estos trazos.
pub fn brush_notes(list: &[&Stroke], p: &VPreset) -> Vec<String> {
    let mut notes = Vec::new();
    if p.style.pressure {
        let n = list.iter().filter(|s| !has_pressure(s)).count();
        if n > 0 {
            notes.push(format!(
                "{n} {} sin presión capturada (dibujado con ratón o con otro pincel): el grosor por presión no se nota en {}; sí se aplican perfil, extremos y uniones.",
                if n == 1 { "trazo está" } else { "trazos están" },
                if n == 1 { "él" } else { "ellos" }
            ));
        }
    }
    notes
}

/// ¿Tiene presión realmente variable (no una constante)?
pub fn has_pressure(s: &Stroke) -> bool {
    s.pressure.len() == s.points.len() && s.pressure.windows(2).any(|w| (w[0] - w[1]).abs() > 1e-3)
}

/// Valores que comparten todos los trazos elegidos. `None` significa «mixto».
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Common {
    pub n: usize,
    pub edited: usize,
    pub without_pressure: usize,
    pub width: Option<f32>,
    pub alpha: Option<u8>,
    pub rgb: Option<[u8; 3]>,
    pub cap: Option<Cap>,
    pub join: Option<Join>,
    pub profile: Option<Profile>,
    pub taper: Option<f32>,
    pub pressure: Option<bool>,
    pub min_width: Option<f32>,
    pub curve: Option<f32>,
    pub shape: Option<Vec<[f32; 2]>>,
    pub brush: Option<Option<String>>,
}

fn same<T: PartialEq + Copy>(it: impl Iterator<Item = T>) -> Option<T> {
    let mut it = it.peekable();
    let first = *it.peek()?;
    it.all(|v| v == first).then_some(first)
}

fn same_f(it: impl Iterator<Item = f32>) -> Option<f32> {
    let v: Vec<f32> = it.collect();
    let first = *v.first()?;
    v.iter().all(|x| (x - first).abs() < 1e-3).then_some(first)
}

/// Propiedades comunes de los trazos. Un trazo sin estilo cuenta con el estilo de siempre.
pub fn common(list: &[&Stroke]) -> Common {
    let st: Vec<VStyle> = list.iter().map(|s| s.style.clone().unwrap_or_default().clamped()).collect();
    let brush: Vec<Option<String>> = list.iter().map(|s| s.style.as_ref().and_then(|x| x.brush.clone())).collect();
    Common {
        n: list.len(),
        edited: list.iter().filter(|s| s.original.is_some()).count(),
        without_pressure: list.iter().filter(|s| !has_pressure(s)).count(),
        width: same_f(list.iter().map(|s| s.width)),
        alpha: same(list.iter().map(|s| s.color[3])),
        rgb: same(list.iter().map(|s| [s.color[0], s.color[1], s.color[2]])),
        cap: same(st.iter().map(|s| s.cap)),
        join: same(st.iter().map(|s| s.join)),
        profile: same(st.iter().map(|s| s.profile)),
        taper: same_f(st.iter().map(|s| s.taper)),
        pressure: same(st.iter().map(|s| s.pressure)),
        min_width: same_f(st.iter().map(|s| s.min_width)),
        curve: same_f(st.iter().map(|s| s.curve)),
        shape: if st.is_empty() || !st.iter().all(|s| s.shape == st[0].shape) { None } else { Some(st[0].shape.clone()) },
        brush: if brush.is_empty() || !brush.iter().all(|b| *b == brush[0]) { None } else { Some(brush[0].clone()) },
    }
}
