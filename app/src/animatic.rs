//! El animatic como composición (Issue #116, fase M7 de docs/editor-de-tiempo-montaje.md): cada plano del storyboard pasa a ser una
//! capa-clip con su barra de entrada y salida, y las transiciones son fundidos cruzados entre clips.
//!
//! Este módulo solo construye las capas; no depende de la interfaz.

use crate::model::{Interp, Layer, LayerKind, Panel};
use crate::tracks::Prop;

/// Nombre de la composición del animatic (y de su capa de precomposición).
pub const NAME: &str = "Animatic";

/// ¿La transición del plano es un fundido? Se reconoce por el texto que se escribió: «Fundido», «Disolvencia», «Fade»…
pub fn is_fade(trans: &str) -> bool {
    let t = trans.to_lowercase();
    ["fundido", "disolv", "fade", "dissolve"].iter().any(|w| t.contains(w))
}

/// Cuánto dura como máximo un fundido: medio segundo.
pub fn fade_frames(fps: u32) -> u32 {
    (fps / 2).max(2)
}

/// Cuadros que dura un plano.
pub fn panel_frames(p: &Panel, fps: u32) -> u32 {
    ((p.dur * fps as f32).round() as u32).max(1)
}

pub struct Built {
    /// De arriba abajo: el último plano arriba, para que un fundido cubra al anterior.
    pub layers: Vec<Layer>,
    /// Duración total en cuadros.
    pub length: u32,
}

/// Una capa-clip por plano (una precomposición viva si el plano tiene composición, o su dibujo aplanado si no). La barra de cada una va de su entrada a su salida; con fundido, la entrada se adelanta (hasta medio
/// segundo, y nunca más que la duración de los dos planos) y la opacidad sube de 0 a 1 en ese tramo. La duración total no cambia.
pub fn build(panels: &[Panel], fps: u32) -> Built {
    let fps = fps.max(1);
    let mut clips: Vec<Layer> = Vec::with_capacity(panels.len());
    let (mut start, mut prev_len) = (0u32, 0u32);
    for (i, p) in panels.iter().enumerate() {
        let len = panel_frames(p, fps);
        let overlap = if i > 0 && is_fade(&p.trans) { fade_frames(fps).min(len).min(prev_len) } else { 0 };
        let in_f = start - overlap;
        let mut l = Layer::new(if p.title.trim().is_empty() { format!("Plano {}", i + 1) } else { p.title.clone() });
        match p.comp {
            // Un plano con composición entra como precomposición viva: sus capas, y lo que se cambie en él, se ven en el animatic.
            Some(id) => {
                l.kind = LayerKind::Precomp;
                l.precomp = Some(id);
            }
            // Sin composición (archivos antiguos sin abrir), se usa el dibujo aplanado del plano.
            None => {
                l.kind = LayerKind::Vector;
                l.keys.insert(in_f, p.drawing.clone());
            }
        }
        l.in_f = in_f;
        l.out_f = Some(start + len - 1);
        l.label = 1 + (i % 7) as u8;
        if overlap > 0 {
            l.tracks.set(Prop::Opacity, in_f, [0.0, 0.0], Interp::Linear);
            l.tracks.set(Prop::Opacity, in_f + overlap, [1.0, 0.0], Interp::Linear);
        }
        // Movimiento de cámara del plano: zoom y desplazamiento de la imagen entre el primer y el último cuadro.
        if let Some(c) = p.cam {
            let (a, z) = (start, start + len - 1);
            let i = if c.ease { Interp::Ease } else { Interp::Linear };
            l.tracks.set(Prop::Scale, a, [c.s0.max(0.1), 0.0], i);
            l.tracks.set(Prop::Position, a, [c.x0, c.y0], i);
            if z > a {
                l.tracks.set(Prop::Scale, z, [c.s1.max(0.1), 0.0], i);
                l.tracks.set(Prop::Position, z, [c.x1, c.y1], i);
            }
        }
        clips.push(l);
        start += len;
        prev_len = len;
    }
    clips.reverse();
    Built { layers: clips, length: start.max(1) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(title: &str, dur: f32, trans: &str) -> Panel {
        Panel { title: title.into(), dur, trans: trans.into(), ..Default::default() }
    }

    #[test]
    fn el_fundido_se_reconoce_por_lo_que_se_escribio() {
        for t in ["Fundido", "fundido a negro", "Disolvencia", "Fade", "cross DISSOLVE"] {
            assert!(is_fade(t), "{t}");
        }
        for t in ["", "Corte", "Barrido"] {
            assert!(!is_fade(t), "{t}");
        }
    }

    #[test]
    fn cada_plano_es_un_clip_con_su_barra_y_el_ultimo_va_arriba() {
        let b = build(&[panel("uno", 1.0, ""), panel("dos", 0.5, "Corte"), panel("tres", 2.0, "")], 24);
        assert_eq!(b.length, 24 + 12 + 48);
        let names: Vec<&str> = b.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["tres", "dos", "uno"]);
        let bars: Vec<(u32, Option<u32>)> = b.layers.iter().map(|l| (l.in_f, l.out_f)).collect();
        assert_eq!(bars, vec![(36, Some(83)), (24, Some(35)), (0, Some(23))]);
        // El dibujo del plano empieza donde empieza su barra.
        assert!(b.layers.iter().all(|l| l.keys.keys().next() == Some(&l.in_f)));
        assert!(b.layers.iter().all(|l| l.tracks.is_empty()), "sin fundidos no hay claves de opacidad");
    }

    #[test]
    fn un_fundido_solapa_el_clip_con_el_anterior_sin_cambiar_la_duracion_total() {
        let b = build(&[panel("uno", 1.0, ""), panel("dos", 1.0, "Fundido")], 24);
        assert_eq!(b.length, 48, "la duración total es la de siempre");
        let dos = &b.layers[0];
        assert_eq!((dos.in_f, dos.out_f), (12, Some(47)), "entra medio segundo antes");
        assert_eq!(dos.tracks.eval(12).o, 0.0);
        assert_eq!(dos.tracks.eval(24).o, 1.0);
        assert!((dos.tracks.eval(18).o - 0.5).abs() < 1e-4);
        // El anterior sigue entero hasta su final: durante el fundido se ven los dos.
        assert_eq!(b.layers[1].out_f, Some(23));
    }

    #[test]
    fn el_fundido_nunca_dura_mas_que_los_planos_que_une_y_el_primero_no_tiene() {
        let b = build(&[panel("corto", 0.25, "Fundido"), panel("otro", 0.25, "Fundido")], 24);
        // El primer plano no tiene con qué solaparse; el segundo, como mucho su propia duración (6 cuadros).
        assert_eq!(b.layers[1].in_f, 0);
        assert!(b.layers[1].tracks.is_empty());
        assert_eq!(b.layers[0].in_f, 0, "se solapa 6 cuadros con un plano de 6");
        assert_eq!(b.length, 12);
        // Sin planos, una composición de un cuadro.
        let e = build(&[], 24);
        assert!(e.layers.is_empty() && e.length == 1);
    }

    #[test]
    fn a_panel_camera_move_becomes_scale_and_position_keys_over_the_panel() {
        let mut ps = vec![panel("A", 1.0, ""), panel("B", 2.0, "")];
        ps[1].cam = Some(crate::model::CamMove { s0: 1.0, x0: 0.0, y0: 0.0, s1: 1.5, x1: -60.0, y1: 20.0, ease: false });
        let b = build(&ps, 24);
        // De arriba abajo: el último plano arriba. B empieza en el cuadro 24 y acaba en el 71.
        let clip_b = &b.layers[0];
        assert_eq!(clip_b.name, "B");
        let start = clip_b.tracks.eval(24);
        assert_eq!((start.s, start.x, start.y), (1.0, 0.0, 0.0));
        let end = clip_b.tracks.eval(71);
        assert_eq!((end.s, end.x, end.y), (1.5, -60.0, 20.0));
        let mid = clip_b.tracks.eval(47);
        assert!(mid.s > 1.2 && mid.s < 1.3, "zoom lineal a mitad del plano: {}", mid.s);
        // Un plano sin movimiento no gana claves.
        assert!(b.layers[1].tracks.is_empty());
    }}
