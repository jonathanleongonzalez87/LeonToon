//! Pruebas del motor de trazos vectoriales.

use super::*;

fn inside(poly: &[[f32; 2]], p: [f32; 2]) -> bool {
    // Rayo hacia la derecha (los polígonos son convexos, así que basta con esto).
    let mut c = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
        j = i;
    }
    c
}

fn covered(pieces: &[Vec<[f32; 2]>], p: [f32; 2]) -> bool {
    pieces.iter().any(|q| q.len() >= 3 && inside(q, p))
}

/// Área aproximada de la unión, contando puntos de una cuadrícula.
fn area(pieces: &[Vec<[f32; 2]>], x: (f32, f32), y: (f32, f32)) -> f32 {
    let step = 0.5;
    let mut n = 0;
    let mut py = y.0;
    while py < y.1 {
        let mut px = x.0;
        while px < x.1 {
            if covered(pieces, [px + step / 2.0, py + step / 2.0]) {
                n += 1;
            }
            px += step;
        }
        py += step;
    }
    n as f32 * step * step
}

fn line(w: f32, cap: Cap, join: Join) -> Vec<Vec<[f32; 2]>> {
    pieces(&[[0.0, 0.0], [100.0, 0.0]], &[w, w], false, cap, join)
}

#[test]
fn widths_follow_profile_and_pressure() {
    let pts: Vec<[f32; 2]> = (0..=10).map(|i| [i as f32 * 10.0, 0.0]).collect();
    let uniform = widths(&pts, &[], 8.0, &VStyle::default(), false);
    assert!(uniform.iter().all(|w| (*w - 8.0).abs() < 1e-5));
    // Afilado al final: llega a cero y el inicio es pleno.
    let end = widths(&pts, &[], 8.0, &VStyle { profile: Profile::TaperEnd, taper: 0.3, ..Default::default() }, false);
    assert!((end[0] - 8.0).abs() < 1e-5 && end[10].abs() < 1e-5 && end[9] < 8.0 && end[5] > 7.9);
    // Afilado al inicio, al revés.
    let start = widths(&pts, &[], 8.0, &VStyle { profile: Profile::TaperStart, taper: 0.3, ..Default::default() }, false);
    assert!(start[0].abs() < 1e-5 && (start[10] - 8.0).abs() < 1e-5);
    // En ambos extremos: punta en los dos y pleno al centro.
    let both = widths(&pts, &[], 8.0, &VStyle { profile: Profile::TaperBoth, taper: 0.25, ..Default::default() }, false);
    assert!(both[0].abs() < 1e-5 && both[10].abs() < 1e-5 && (both[5] - 8.0).abs() < 1e-5);
    // El afilado es progresivo, sin saltos.
    assert!(both.windows(2).take(5).all(|w| w[1] >= w[0]));
    // Un trazo cerrado también se afila: el perfil recorre toda la vuelta desde el primer punto.
    let closed = widths(&pts, &[], 8.0, &VStyle { profile: Profile::TaperBoth, taper: 0.25, ..Default::default() }, true);
    assert!(closed[0].abs() < 1e-5 && closed.iter().any(|w| (*w - 8.0).abs() < 1e-5));
    // Presión: con el mínimo en 20 %, presión 0 deja 1,6 y presión 1 deja 8.
    let pr: Vec<f32> = (0..=10).map(|i| i as f32 / 10.0).collect();
    let w = widths(&pts, &pr, 8.0, &VStyle { pressure: true, min_width: 0.2, ..Default::default() }, false);
    assert!((w[0] - 1.6).abs() < 1e-4 && (w[10] - 8.0).abs() < 1e-4 && w.windows(2).all(|p| p[1] >= p[0]));
    // La curva cambia la respuesta a presión media.
    let soft = widths(&pts, &pr, 8.0, &VStyle { pressure: true, min_width: 0.0, curve: 0.5, ..Default::default() }, false)[5];
    let hard = widths(&pts, &pr, 8.0, &VStyle { pressure: true, min_width: 0.0, curve: 2.0, ..Default::default() }, false)[5];
    assert!(soft > 4.0 && hard < 4.0, "{soft} {hard}");
    // Sin presión activada, o con datos que no encajan, se ignoran.
    assert!(widths(&pts, &pr, 8.0, &VStyle { pressure: false, ..Default::default() }, false).iter().all(|w| (*w - 8.0).abs() < 1e-5));
    assert!(widths(&pts, &pr[..4], 8.0, &VStyle::default(), false).iter().all(|w| (*w - 8.0).abs() < 1e-5));
    // Valores imposibles del archivo se corrigen.
    let wild = VStyle { taper: f32::NAN, min_width: 9.0, curve: -3.0, ..Default::default() }.clamped();
    assert!(wild.taper == 0.25 && wild.min_width == 1.0 && wild.curve == 0.2);
}

#[test]
fn caps_set_where_the_stroke_ends() {
    // Línea de 100 de largo y 10 de grosor: el redondo y el cuadrado salen 5 por cada lado; el plano, no.
    for (cap, reaches) in [(Cap::Round, true), (Cap::Square, true), (Cap::Flat, false)] {
        let p = line(10.0, cap, Join::Round);
        assert!(covered(&p, [50.0, 4.0]) && !covered(&p, [50.0, 6.0]), "{cap:?}: el grosor");
        assert_eq!(covered(&p, [-3.0, 0.0]) && covered(&p, [103.0, 0.0]), reaches, "{cap:?}: sale más allá de los extremos");
        assert!(!covered(&p, [-6.0, 0.0]) && !covered(&p, [106.0, 0.0]), "{cap:?}: no se pasa de r");
    }
    // Las esquinas del extremo: el cuadrado las cubre y el redondo no.
    assert!(covered(&line(10.0, Cap::Square, Join::Round), [-4.0, 4.0]));
    assert!(!covered(&line(10.0, Cap::Round, Join::Round), [-4.0, 4.0]));
    // Áreas: plano = 1000, cuadrado = 1100, redondo = 1000 + π·25.
    let a = |c| area(&line(10.0, c, Join::Round), (-10.0, 110.0), (-10.0, 10.0));
    assert!((a(Cap::Flat) - 1000.0).abs() < 15.0, "{}", a(Cap::Flat));
    assert!((a(Cap::Square) - 1100.0).abs() < 15.0, "{}", a(Cap::Square));
    assert!((a(Cap::Round) - (1000.0 + std::f32::consts::PI * 25.0)).abs() < 15.0, "{}", a(Cap::Round));
}

#[test]
fn variable_width_ribbon_matches_the_widths() {
    // De 2 a 20 de grosor en 100 de largo: a mitad (grosor 11) cubre ±5,5 y no ±7.
    let p = pieces(&[[0.0, 0.0], [50.0, 0.0], [100.0, 0.0]], &[2.0, 11.0, 20.0], false, Cap::Flat, Join::Round);
    assert!(covered(&p, [50.0, 5.0]) && !covered(&p, [50.0, 7.0]));
    assert!(covered(&p, [95.0, 9.0]) && !covered(&p, [95.0, 11.0]));
    assert!(covered(&p, [5.0, 0.4]) && !covered(&p, [5.0, 2.0]));
    // Una punta cerrada (grosor 0 en un extremo) no rompe nada y termina en punta.
    let tip = pieces(&[[0.0, 0.0], [100.0, 0.0]], &[0.0, 12.0], false, Cap::Round, Join::Round);
    assert!(covered(&tip, [90.0, 4.0]) && !covered(&tip, [10.0, 3.0]));
    assert!(tip.iter().all(|q| q.iter().all(|v| v[0].is_finite() && v[1].is_finite())));
}

#[test]
fn joins_shape_the_outer_corner() {
    // Esquina de 90° en (100, 0): sube hasta (100, 100). Grosor 20, radio 10.
    let corner = |j| pieces(&[[0.0, 0.0], [100.0, 0.0], [100.0, 100.0]], &[20.0; 3], false, Cap::Flat, j);
    // El punto del vértice exterior del inglete es (110, -10).
    assert!(covered(&corner(Join::Miter), [109.0, -9.0]), "inglete");
    assert!(!covered(&corner(Join::Bevel), [109.0, -9.0]), "bisel");
    assert!(!covered(&corner(Join::Round), [109.0, -9.0]), "redonda");
    // El redondo cubre el arco (a 45°, a distancia 9 del vértice) y el bisel no llega tan lejos.
    let d = 9.0 / 2f32.sqrt();
    assert!(covered(&corner(Join::Round), [100.0 + d, -d]));
    assert!(covered(&corner(Join::Bevel), [100.0 + 4.0, -4.0]));
    assert!(!covered(&corner(Join::Bevel), [100.0 + d, -d]));
    // Un giro muy cerrado no dispara una espina de inglete (se corta en bisel): nada a más de 4 radios.
    let sharp = pieces(&[[0.0, 0.0], [100.0, 0.0], [0.0, 3.0]], &[20.0; 3], false, Cap::Flat, Join::Miter);
    // La espina saldría más allá del vértice (x = 100) hacia la derecha.
    let far = sharp.iter().flatten().map(|p| p[0] - 100.0).fold(0.0f32, f32::max);
    assert!(far <= 4.0 * 10.0 + 1.0, "espina de {far}");
    assert!(far < 1.0, "un giro casi de vuelta se corta en bisel, sin espina: {far}");
    // Un tramo recto no genera uniones.
    let straight = pieces(&[[0.0, 0.0], [50.0, 0.0], [100.0, 0.0]], &[10.0; 3], false, Cap::Flat, Join::Round);
    assert_eq!(straight.len(), 2);
}

#[test]
fn dots_closed_loops_and_degenerate_input() {
    // Un punto: círculo de radio 5 (redondo) o cuadrado de 10 (cuadrado).
    let dot = pieces(&[[20.0, 20.0]], &[10.0], false, Cap::Round, Join::Round);
    assert!((area(&dot, (10.0, 30.0), (10.0, 30.0)) - std::f32::consts::PI * 25.0).abs() < 4.0);
    let sq = pieces(&[[20.0, 20.0]], &[10.0], false, Cap::Square, Join::Round);
    assert!((area(&sq, (10.0, 30.0), (10.0, 30.0)) - 100.0).abs() < 2.0);
    // Cerrado: un cuadrado de 40 con trazo de 10 es un aro; el centro queda vacío y sin extremos sueltos.
    let ring = pieces(&[[0.0, 0.0], [40.0, 0.0], [40.0, 40.0], [0.0, 40.0]], &[10.0; 4], true, Cap::Round, Join::Miter);
    assert!(covered(&ring, [20.0, 2.0]) && covered(&ring, [38.0, 20.0]) && !covered(&ring, [20.0, 20.0]));
    assert!(covered(&ring, [-4.0, -4.0]), "esquina con inglete");
    // Entrada degenerada: vacía, repetida, con NaN o con grosor cero. Nunca falla.
    assert!(pieces(&[], &[], false, Cap::Round, Join::Round).is_empty());
    assert!(pieces(&[[1.0, 1.0], [1.0, 1.0], [1.0, 1.0]], &[4.0; 3], false, Cap::Round, Join::Round).len() == 1);
    let nan = pieces(&[[0.0, 0.0], [f32::NAN, 3.0], [10.0, 0.0]], &[4.0; 3], false, Cap::Round, Join::Round);
    assert!(nan.iter().all(|q| q.iter().all(|v| v[0].is_finite())));
    assert!(pieces(&[[0.0, 0.0], [10.0, 0.0]], &[0.0, 0.0], false, Cap::Round, Join::Round).is_empty());
    assert!(pieces(&[[0.0, 0.0], [10.0, 0.0]], &[], false, Cap::Round, Join::Round).is_empty());
}

#[test]
fn every_piece_is_convex_counterclockwise_and_finite() {
    let wavy: Vec<[f32; 2]> = (0..60).map(|i| [i as f32 * 4.0, (i as f32 * 0.4).sin() * 25.0]).collect();
    let w: Vec<f32> = (0..60).map(|i| 3.0 + (i as f32 * 0.2).sin().abs() * 14.0).collect();
    for join in Join::ALL {
        for cap in Cap::ALL {
            let ps = pieces(&wavy, &w, false, cap, join);
            assert!(!ps.is_empty());
            for q in &ps {
                assert!(q.len() >= 3);
                let a: f32 = (0..q.len()).map(|i| q[i][0] * q[(i + 1) % q.len()][1] - q[(i + 1) % q.len()][0] * q[i][1]).sum::<f32>() / 2.0;
                assert!(a >= -1e-3, "polígono en sentido horario ({a})");
                assert!(q.iter().all(|v| v[0].is_finite() && v[1].is_finite()));
                // Convexo: todos los giros del contorno son del mismo lado.
                let turns: Vec<f32> = (0..q.len()).map(|i| {
                    let (p0, p1, p2) = (q[i], q[(i + 1) % q.len()], q[(i + 2) % q.len()]);
                    (p1[0] - p0[0]) * (p2[1] - p1[1]) - (p1[1] - p0[1]) * (p2[0] - p1[0])
                }).collect();
                assert!(turns.iter().all(|t| *t >= -1e-2), "polígono no convexo");
            }
        }
    }
}

#[test]
fn plain_strokes_keep_the_original_fast_path() {
    assert!(is_plain(None, &[]));
    assert!(is_plain(Some(&VStyle::default()), &[]));
    // Con presión que no varía (ratón: siempre 1), sigue siendo trazo plano.
    assert!(is_plain(Some(&VStyle::default()), &[1.0, 1.0, 1.0]));
    // Con presión que varía, o con perfil, extremos o uniones distintos, ya no.
    assert!(!is_plain(Some(&VStyle::default()), &[0.2, 0.9, 0.5]));
    assert!(is_plain(Some(&VStyle { pressure: false, ..Default::default() }), &[0.2, 0.9, 0.5]));
    assert!(!is_plain(Some(&VStyle { profile: Profile::TaperEnd, ..Default::default() }), &[]));
    assert!(!is_plain(Some(&VStyle { cap: Cap::Flat, ..Default::default() }), &[]));
    assert!(!is_plain(Some(&VStyle { join: Join::Miter, ..Default::default() }), &[]));
}

#[test]
fn style_roundtrips_and_old_files_load() {
    let s = VStyle { cap: Cap::Square, join: Join::Bevel, profile: Profile::TaperBoth, taper: 0.4, pressure: false, min_width: 0.5, curve: 1.5, shape: Vec::new(), brush: Some("Tinta".into()), version: 1 };
    let back: VStyle = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back, s);
    let old: VStyle = serde_json::from_str(r#"{"cap":"Flat"}"#).unwrap();
    assert_eq!(old.cap, Cap::Flat);
    assert_eq!(old.join, Join::Round);
    assert_eq!(old.profile, Profile::Uniform);
}

// ---------------- Trazos completos (modelo, borrado, guardado y exportación) ----------------

use crate::model::{Drawing, ShapeKind, Stroke};

fn pencil(points: Vec<[f32; 2]>, pressure: Vec<f32>, style: Option<VStyle>) -> Stroke {
    Stroke { kind: ShapeKind::Free, points, color: [10, 20, 30, 255], width: 10.0, closed: false, fill: None, pressure, style, original: None, nodes: None }
}

fn straight(n: usize) -> Vec<[f32; 2]> {
    (0..n).map(|i| [i as f32 * 10.0, 0.0]).collect()
}

#[test]
fn old_strokes_load_unchanged_and_new_ones_keep_everything() {
    // Un trazo guardado antes de existir el estilo y la presión: sin esos campos, y se dibuja como siempre.
    let old: Stroke = serde_json::from_str(r#"{"kind":"Free","points":[[0.0,0.0],[5.0,5.0]],"color":[1,2,3,255],"width":4.0,"closed":false,"fill":null}"#).unwrap();
    assert!(old.pressure.is_empty() && old.style.is_none() && old.is_plain());
    // Un trazo plano no escribe campos nuevos (los archivos nuevos siguen abriendo en versiones anteriores).
    let json = serde_json::to_string(&old).unwrap();
    assert!(!json.contains("pressure") && !json.contains("style"), "{json}");
    // Uno con presión y estilo los conserva exactos al guardar y abrir.
    let st = VStyle { profile: Profile::TaperEnd, cap: Cap::Flat, join: Join::Bevel, taper: 0.3, brush: Some("Tinta".into()), ..Default::default() };
    let s = pencil(straight(5), vec![0.1, 0.4, 0.8, 1.0, 0.6], Some(st));
    let d = Drawing { strokes: vec![s.clone()], raster: None };
    let back: Drawing = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
    assert_eq!(back.strokes[0].pressure, s.pressure);
    assert_eq!(back.strokes[0].style, s.style);
    assert!(!back.strokes[0].is_plain());
    // Los mismos polígonos antes y después de guardar: el trazo se reconstruye igual.
    assert_eq!(back.strokes[0].pieces(), s.pieces());
}

#[test]
fn damaged_attributes_are_repaired_on_open() {
    let mut s = pencil(straight(4), vec![0.5, 2.0, f32::NAN, -1.0], Some(VStyle { taper: 99.0, curve: -2.0, ..Default::default() }));
    s.fix_attrs();
    assert_eq!(s.pressure, vec![0.5, 1.0, 1.0, 0.0]);
    let st = s.style.as_ref().unwrap();
    assert!(st.taper == 0.5 && st.curve == 0.2);
    // Presión que no encaja con los puntos: se descarta (el trazo sigue siendo válido).
    let mut m = pencil(straight(4), vec![0.5, 0.5], None);
    m.fix_attrs();
    assert!(m.pressure.is_empty());
    // Y todo el proyecto lo hace al abrirse.
    let mut p = crate::model::Project::blank(64, 64, 24, 3);
    p.layers[0].keys.insert(0, std::sync::Arc::new(Drawing { strokes: vec![pencil(straight(4), vec![0.5], None)], raster: None }));
    p.sanitize();
    assert!(p.layers[0].keys[&0].strokes[0].pressure.is_empty());
}

#[test]
fn simplifying_keeps_pressure_aligned_with_the_points() {
    // Una línea casi recta con una esquina en el punto 5: la esquina se queda con SU presión.
    let mut pts: Vec<[f32; 2]> = (0..=5).map(|i| [i as f32 * 10.0, 0.0]).collect();
    pts.extend((1..=5).map(|i| [50.0, i as f32 * 10.0]));
    let pr: Vec<f32> = (0..pts.len()).map(|i| i as f32 / 10.0).collect();
    let mut s = pencil(pts, pr, Some(VStyle::default()));
    s.simplify_in_place(1.0);
    assert_eq!(s.points.len(), 3, "{:?}", s.points);
    assert_eq!(s.pressure.len(), s.points.len());
    assert_eq!(s.points[1], [50.0, 0.0]);
    assert!((s.pressure[1] - 0.5).abs() < 1e-6, "la esquina conserva su presión: {:?}", s.pressure);
    // Sin presión, simplifica igual y no inventa datos.
    let mut plain = pencil(straight(10), vec![], None);
    plain.simplify_in_place(1.0);
    assert_eq!(plain.points.len(), 2);
    assert!(plain.pressure.is_empty());
}

#[test]
fn erasing_keeps_pressure_and_the_look_of_a_tapered_stroke() {
    // Línea larga afilada en ambos extremos, con presión constante: se borra un trozo del centro.
    let pts = straight(31);
    let st = VStyle { profile: Profile::TaperBoth, taper: 0.2, pressure: true, min_width: 0.0, ..Default::default() };
    let s = pencil(pts.clone(), vec![1.0; 31], Some(st));
    let before = s.pieces();
    let cut = {
        let mut d = Drawing { strokes: vec![s.clone()], raster: None };
        assert!(d.erase([150.0, 0.0], 12.0));
        d.strokes
    };
    assert_eq!(cut.len(), 2, "queda un trozo a cada lado");
    for piece in &cut {
        assert_eq!(piece.pressure.len(), piece.points.len(), "la presión sigue a los puntos");
        // El perfil se «horneó» en la presión: los trozos ya no se afilan de nuevo por su cuenta.
        let ps = piece.style.as_ref().unwrap();
        assert_eq!(ps.profile, Profile::Uniform);
        assert!(ps.pressure && ps.min_width == 0.0 && ps.curve == 1.0);
        assert!(piece.pressure.iter().all(|p| (0.0..=1.0).contains(p)));
    }
    // Lo que queda mantiene el grosor que tenía: a 100 px del inicio (zona plena) el trozo izquierdo mide 10 de ancho;
    // cerca del inicio sigue siendo punta, y el corte nuevo NO se afila.
    let left = &cut[0];
    let w_end = *left.pressure.last().unwrap();
    assert!(w_end > 0.9, "el corte queda con grosor pleno, no en punta: {w_end}");
    assert!(left.pressure[0] < 0.2, "el inicio sigue siendo punta: {}", left.pressure[0]);
    assert!(covered(&left.pieces(), [100.0, 4.0]) && !covered(&left.pieces(), [100.0, 6.5]));
    // El trozo borrado no aparece en los polígonos de antes en x=150±12, y el resto coincide con el original.
    assert!(covered(&before, [150.0, 4.0]));
    assert!(!covered(&left.pieces(), [150.0, 0.0]) && !covered(&cut[1].pieces(), [150.0, 0.0]));
    // Un trazo plano se borra como siempre: sin presión, sin estilo.
    let plain = pencil(pts, vec![], None);
    let mut d = Drawing { strokes: vec![plain], raster: None };
    assert!(d.erase([150.0, 0.0], 12.0));
    assert!(d.strokes.iter().all(|s| s.pressure.is_empty() && s.style.is_none()));
}

#[test]
fn export_draws_variable_width_strokes_and_plain_ones_stay_identical() {
    use crate::render::draw_drawing;
    use tiny_skia::{Pixmap, Transform};
    let render = |s: Stroke| {
        let mut pm = Pixmap::new(140, 40).unwrap();
        draw_drawing(&mut pm, &Drawing { strokes: vec![s], raster: None }, Transform::identity());
        pm
    };
    let alpha = |pm: &Pixmap, x: u32, y: u32| pm.pixel(x, y).unwrap().alpha();
    let line = |style: Option<VStyle>, pressure: Vec<f32>| {
        let mut s = pencil((0..=12).map(|i| [10.0 + i as f32 * 10.0, 20.0]).collect(), pressure, style);
        s.width = 12.0;
        s
    };
    // Afilado al final: grueso al inicio (cubre ±5 en x=20) y casi nada cerca del final (x=120).
    let tapered = render(line(Some(VStyle { profile: Profile::TaperEnd, taper: 0.5, ..Default::default() }), vec![]));
    assert!(alpha(&tapered, 20, 25) > 200, "inicio grueso");
    assert_eq!(alpha(&tapered, 120, 24), 0, "punta fina al final");
    assert!(alpha(&tapered, 110, 20) > 200 && alpha(&tapered, 110, 25) == 0, "cerca del final sigue siendo una línea fina");
    assert!(alpha(&tapered, 118, 19) as u32 + alpha(&tapered, 118, 20) as u32 > 100, "la punta llega casi hasta el final");
    // Con presión que sube: más fino a la izquierda que a la derecha.
    let pr: Vec<f32> = (0..=12).map(|i| i as f32 / 12.0).collect();
    let ramp = render(line(Some(VStyle { pressure: true, min_width: 0.0, ..Default::default() }), pr));
    assert_eq!(alpha(&ramp, 20, 24), 0);
    assert!(alpha(&ramp, 110, 24) > 200);
    // Extremo plano: no sobresale; el redondo sí.
    let flat = render(line(Some(VStyle { cap: Cap::Flat, ..Default::default() }), vec![]));
    let round = render(line(None, vec![]));
    assert_eq!(alpha(&flat, 7, 20), 0);
    assert!(alpha(&round, 7, 20) > 200);
    // Un trazo con estilo que no cambia nada se dibuja idéntico al de siempre (sin estilo).
    let a = render(line(None, vec![]));
    let b = render(line(Some(VStyle::default()), vec![1.0; 13]));
    assert_eq!(a.data(), b.data(), "el trazo plano no debe cambiar");
    // Translúcido: el color se mezcla una sola vez en los cruces (la unión no oscurece).
    let mut s = line(Some(VStyle { join: Join::Bevel, cap: Cap::Flat, ..Default::default() }), vec![]);
    s.color = [10, 20, 30, 128];
    s.points = vec![[20.0, 20.0], [100.0, 20.0], [100.0, 30.0], [20.0, 30.0]];
    let tr = render(s);
    let center = alpha(&tr, 60, 20) as i32;
    assert!((center - 128).abs() <= 2, "alfa en el centro de un tramo: {center}");
}

// ---------- Edición de trazos existentes (Fase 3) ----------

use super::edit::{self, Prop};
use super::library::{self, VLibrary};

fn drawn() -> Stroke {
    let pts: Vec<[f32; 2]> = (0..20).map(|i| [i as f32 * 10.0, (i as f32 * 0.5).sin() * 30.0]).collect();
    let pr: Vec<f32> = (0..20).map(|i| 0.2 + i as f32 * 0.04).collect();
    pencil(pts, pr, Some(VStyle::default()))
}

#[test]
fn editing_a_property_changes_only_that_property_and_never_the_geometry() {
    let mut s = drawn();
    let (pts, pr) = (s.points.clone(), s.pressure.clone());
    edit::set_prop(&mut s, &Prop::Profile(Profile::TaperBoth));
    assert_eq!(s.style.as_ref().unwrap().profile, Profile::TaperBoth);
    assert_eq!((s.width, s.color), (10.0, [10, 20, 30, 255]));
    edit::set_prop(&mut s, &Prop::Width(25.0));
    edit::set_prop(&mut s, &Prop::Alpha(100));
    edit::set_prop(&mut s, &Prop::Rgb([1, 2, 3]));
    assert_eq!((s.width, s.color), (25.0, [1, 2, 3, 100]), "el color nuevo conserva la opacidad");
    assert_eq!((s.points.clone(), s.pressure.clone()), (pts, pr), "trayectoria y presión intactas");
    assert_eq!(s.style.as_ref().unwrap().profile, Profile::TaperBoth);
}

#[test]
fn original_is_remembered_once_and_restores_exactly() {
    let mut s = drawn();
    let before = s.look();
    edit::set_prop(&mut s, &Prop::Cap(Cap::Flat));
    edit::set_prop(&mut s, &Prop::Width(50.0));
    assert_eq!(s.original.as_deref(), Some(&before), "se guarda el primero, no el último");
    assert!(edit::restore(&mut s));
    assert_eq!(s.look(), before);
    assert!(s.original.is_none() && !edit::restore(&mut s));
}

#[test]
fn legacy_stroke_can_be_edited_and_stays_vector() {
    let mut s = pencil(vec![[0.0, 0.0], [100.0, 0.0]], Vec::new(), None);
    assert!(s.is_plain());
    edit::set_prop(&mut s, &Prop::Profile(Profile::TaperEnd));
    assert!(!s.is_plain());
    assert!(!s.pieces().is_empty());
    edit::restore(&mut s);
    assert!(s.style.is_none() && s.is_plain(), "restaurar devuelve el trazo de siempre");
}

#[test]
fn changing_style_changes_the_outline_without_moving_the_path() {
    let mut s = drawn();
    let a = s.pieces();
    edit::set_prop(&mut s, &Prop::Profile(Profile::TaperBoth));
    edit::set_prop(&mut s, &Prop::Cap(Cap::Flat));
    assert_ne!(a, s.pieces());
}

#[test]
fn common_values_and_mixed_ones() {
    let mut a = drawn();
    let mut b = drawn();
    let c = edit::common(&[&a, &b]);
    assert_eq!((c.n, c.width, c.profile, c.cap), (2, Some(10.0), Some(Profile::Uniform), Some(Cap::Round)));
    edit::set_prop(&mut b, &Prop::Width(20.0));
    edit::set_prop(&mut b, &Prop::Cap(Cap::Square));
    edit::set_prop(&mut a, &Prop::Rgb([9, 9, 9]));
    let c = edit::common(&[&a, &b]);
    assert_eq!((c.width, c.cap, c.rgb), (None, None, None));
    assert_eq!((c.profile, c.alpha, c.edited), (Some(Profile::Uniform), Some(255), 2));
    // Un trazo sin estilo cuenta con el estilo de siempre.
    let plain = pencil(vec![[0.0, 0.0], [5.0, 5.0]], Vec::new(), None);
    assert_eq!(edit::common(&[&plain, &drawn()]).cap, Some(Cap::Round));
    assert_eq!(edit::common(&[]).n, 0);
}

#[test]
fn editing_many_strokes_only_touches_the_chosen_property() {
    let mut a = drawn();
    let mut b = drawn();
    edit::set_prop(&mut a, &Prop::Width(5.0));
    edit::set_prop(&mut b, &Prop::Width(40.0));
    for s in [&mut a, &mut b] {
        edit::set_prop(s, &Prop::Join(Join::Miter));
    }
    assert_eq!((a.width, b.width), (5.0, 40.0), "cada uno conserva su grosor");
    assert!(a.style.as_ref().unwrap().join == Join::Miter && b.style.as_ref().unwrap().join == Join::Miter);
}

#[test]
fn paste_look_copies_only_the_marked_groups() {
    let mut src = drawn();
    edit::set_prop(&mut src, &Prop::Profile(Profile::TaperStart));
    edit::set_prop(&mut src, &Prop::Width(33.0));
    edit::set_prop(&mut src, &Prop::Rgb([200, 0, 0]));
    let look = src.look();
    let mut dst = drawn();
    edit::paste_look(&mut dst, &look, false, true, false);
    assert_eq!((dst.width, dst.color), (10.0, [10, 20, 30, 255]));
    assert_eq!(dst.style.as_ref().unwrap().profile, Profile::TaperStart);
    edit::paste_look(&mut dst, &look, true, false, true);
    assert_eq!((dst.width, dst.color), (33.0, [200, 0, 0, 255]));
}

#[test]
fn assigning_a_brush_keeps_path_pressure_color_and_reports_incompatibilities() {
    let lib = VLibrary::default();
    let brush = lib.find("Tinta/Tinta afilada").unwrap();
    let mut s = drawn();
    edit::set_prop(&mut s, &Prop::Alpha(128));
    let (pts, pr, col) = (s.points.clone(), s.pressure.clone(), s.color);
    edit::apply_brush(&mut s, &brush, false);
    assert_eq!((s.points.clone(), s.pressure.clone(), s.color), (pts, pr, col));
    assert_eq!(s.width, 10.0, "sin «usar su grosor» se conserva el del trazo");
    let st = s.style.as_ref().unwrap();
    assert_eq!((st.profile, st.brush.as_deref()), (Profile::TaperBoth, Some("Tinta/Tinta afilada")));
    edit::apply_brush(&mut s, &brush, true);
    assert_eq!(s.width, brush.size);

    // Un trazo hecho con ratón (sin presión) y uno cerrado: avisos claros.
    let mouse = pencil(vec![[0.0, 0.0], [50.0, 0.0], [90.0, 20.0]], Vec::new(), None);
    let mut closed = drawn();
    closed.closed = true;
    let notes = edit::brush_notes(&[&mouse, &closed, &drawn()], &brush);
    assert_eq!(notes.len(), 1, "los trazos cerrados ya no dan aviso: {notes:?}");
    assert!(notes[0].contains("1 trazo está sin presión"));
    assert!(edit::brush_notes(&[&drawn()], &lib.find("Básicos/Redondo").unwrap()).is_empty());
}

#[test]
fn original_survives_save_and_old_projects_still_load() {
    let mut s = drawn();
    edit::set_prop(&mut s, &Prop::Width(77.0));
    let json = serde_json::to_string(&s).unwrap();
    let back: Stroke = serde_json::from_str(&json).unwrap();
    assert_eq!(back.original.as_deref().map(|o| o.width), Some(10.0));
    // Un trazo guardado antes de esta fase no trae `original`.
    let old = r#"{"kind":"Free","points":[[0,0],[5,5]],"color":[0,0,0,255],"width":3.0}"#;
    let o: Stroke = serde_json::from_str(old).unwrap();
    assert!(o.original.is_none());
    // Sin editar no se escribe nada extra.
    assert!(!serde_json::to_string(&drawn()).unwrap().contains("original"));
}

#[test]
fn scaling_a_stroke_scales_its_remembered_width() {
    let mut s = drawn();
    edit::set_prop(&mut s, &Prop::Width(20.0));
    s.scale_about([0.0, 0.0], 2.0, 2.0);
    assert_eq!(s.width, 40.0);
    assert_eq!(s.original.as_ref().unwrap().width, 20.0);
    edit::restore(&mut s);
    assert_eq!(s.width, 20.0, "restaurar respeta la escala aplicada");
}

#[test]
fn damaged_original_is_repaired() {
    let mut s = drawn();
    s.original = Some(Box::new(Look { width: f32::NAN, color: [0, 0, 0, 255], style: Some(VStyle { curve: 0.0, ..Default::default() }) }));
    s.fix_attrs();
    let o = s.original.unwrap();
    assert!(o.width.is_finite() && o.style.unwrap().curve >= 0.2);
    let _ = library::USER_CATEGORY;
}

// ---------- Curvas Bézier (Fase 4) ----------

use super::bezier::{self, Node};

fn arc() -> Vec<Node> {
    // Dos nodos suaves que forman un arco.
    vec![
        Node { p: [0.0, 0.0], hin: [0.0, 0.0], hout: [0.0, -60.0], pr: 1.0, smooth: true },
        Node { p: [100.0, 0.0], hin: [100.0, -60.0], hout: [100.0, 0.0], pr: 1.0, smooth: true },
    ]
}

fn curve(nodes: Vec<Node>, closed: bool) -> Stroke {
    let mut s = pencil(Vec::new(), Vec::new(), Some(VStyle::default()));
    s.closed = closed;
    s.nodes = Some(nodes);
    s.regen();
    s
}

#[test]
fn flatten_passes_through_nodes_and_follows_the_curve() {
    let n = arc();
    let (pts, pr) = bezier::flatten(&n, false);
    assert_eq!(pts.first(), Some(&[0.0, 0.0]));
    assert_eq!(pts.last(), Some(&[100.0, 0.0]));
    assert!(pts.len() > 10 && pr.is_empty());
    let top = pts.iter().map(|p| p[1]).fold(0.0f32, f32::min);
    assert!((top + 45.0).abs() < 1.0, "la cima de un arco cúbico así llega a -45: {top}");
    // Sin tiradores es una recta.
    let (line, _) = bezier::flatten(&[Node::corner([0.0, 0.0]), Node::corner([50.0, 20.0])], false);
    assert_eq!(line.len(), 2);
}

#[test]
fn closed_curve_does_not_repeat_the_first_point() {
    let tri = vec![Node::corner([0.0, 0.0]), Node::corner([50.0, 0.0]), Node::corner([25.0, 40.0])];
    let (pts, _) = bezier::flatten(&tri, true);
    assert_eq!(pts.len(), 3);
    assert_ne!(pts.first(), pts.last());
    assert_eq!(bezier::segments(3, true), 3);
    assert_eq!(bezier::segments(3, false), 2);
}

#[test]
fn inserting_a_node_keeps_the_shape() {
    let mut n = arc();
    let (before, _) = bezier::flatten(&n, false);
    let i = bezier::insert(&mut n, false, 0, 0.4);
    assert_eq!((i, n.len()), (1, 3));
    let (after, _) = bezier::flatten(&n, false);
    // Cada punto de la curva nueva está sobre la curva vieja.
    let old = arc();
    for p in &after {
        let (_, _, d) = bezier::nearest(&old, false, *p).unwrap();
        assert!(d < 0.6, "se separó {d}");
    }
    assert!(before.len() > 2 && after.len() > 2);
}

#[test]
fn nearest_finds_the_segment_and_parameter() {
    let n = vec![Node::corner([0.0, 0.0]), Node::corner([100.0, 0.0]), Node::corner([100.0, 100.0])];
    let (seg, t, d) = bezier::nearest(&n, false, [100.0, 50.0]).unwrap();
    assert_eq!(seg, 1);
    assert!((t - 0.5).abs() < 0.03 && d < 0.5);
}

#[test]
fn delete_keeps_the_minimum_nodes() {
    let mut n = vec![Node::corner([0.0, 0.0]), Node::corner([10.0, 0.0]), Node::corner([20.0, 5.0])];
    assert!(bezier::delete(&mut n, false, 1));
    assert!(!bezier::delete(&mut n, false, 0), "una curva abierta necesita 2");
    let mut c = vec![Node::corner([0.0, 0.0]), Node::corner([10.0, 0.0]), Node::corner([20.0, 5.0])];
    assert!(!bezier::delete(&mut c, true, 0), "una cerrada necesita 3");
}

#[test]
fn smooth_and_corner_conversion() {
    let mut n = vec![Node::corner([0.0, 0.0]), Node::corner([50.0, 30.0]), Node::corner([100.0, 0.0])];
    bezier::set_smooth(&mut n, false, 1, true);
    assert!(n[1].smooth && n[1].has_in() && n[1].has_out());
    // Los tiradores quedan alineados con el nodo (tangente paralela a vecino-siguiente).
    let (a, b) = ([n[1].p[0] - n[1].hin[0], n[1].p[1] - n[1].hin[1]], [n[1].hout[0] - n[1].p[0], n[1].hout[1] - n[1].p[1]]);
    assert!((a[0] * b[1] - a[1] * b[0]).abs() < 1e-3);
    bezier::set_smooth(&mut n, false, 1, false);
    assert!(!n[1].smooth && !n[1].has_in() && !n[1].has_out());
}

#[test]
fn moving_a_smooth_handle_turns_the_opposite_one() {
    let mut n = vec![Node { p: [50.0, 50.0], hin: [30.0, 50.0], hout: [80.0, 50.0], pr: 1.0, smooth: true }, Node::corner([200.0, 50.0])];
    bezier::move_handle(&mut n, 0, true, [50.0, 100.0]);
    assert_eq!(n[0].hout, [50.0, 100.0]);
    // El de entrada apunta al lado contrario y conserva su largo (20).
    assert!((n[0].hin[0] - 50.0).abs() < 1e-3 && (n[0].hin[1] - 30.0).abs() < 1e-3, "{:?}", n[0].hin);
    let mut c = vec![Node { smooth: false, ..n[0] }, n[1]];
    let hin = c[0].hin;
    bezier::move_handle(&mut c, 0, true, [0.0, 0.0]);
    assert_eq!(c[0].hin, hin, "en una esquina los tiradores son independientes");
}

#[test]
fn moving_an_anchor_carries_its_handles() {
    let mut n = arc();
    let (hi, ho) = (n[1].hin, n[1].hout);
    bezier::move_anchor(&mut n, 1, [110.0, 10.0]);
    assert_eq!(n[1].p, [110.0, 10.0]);
    assert_eq!((n[1].hin, n[1].hout), ([hi[0] + 10.0, hi[1] + 10.0], [ho[0] + 10.0, ho[1] + 10.0]));
}

#[test]
fn fit_keeps_the_shape_and_pressure() {
    let pts: Vec<[f32; 2]> = (0..=60).map(|i| { let t = i as f32 / 60.0 * std::f32::consts::PI; [t.cos() * 100.0, t.sin() * 100.0] }).collect();
    let pr: Vec<f32> = (0..=60).map(|i| 0.3 + i as f32 / 100.0).collect();
    let nodes = bezier::fit(&pts, &pr, false, 1.5, 10.0);
    assert!(nodes.len() >= 3 && nodes.len() < 30, "{}", nodes.len());
    assert!((nodes[0].pr - 0.3).abs() < 1e-5 && (nodes.last().unwrap().pr - 0.9).abs() < 1e-5);
    let (flat, fp) = bezier::flatten(&nodes, false);
    assert_eq!(fp.len(), flat.len(), "la presión sigue a la curva");
    // La curva ajustada no se aleja de la semicircunferencia original.
    for p in &flat {
        assert!(((p[0] * p[0] + p[1] * p[1]).sqrt() - 100.0).abs() < 6.0, "se aleja en {p:?}");
    }
    // Un giro marcado queda como esquina.
    let corner = bezier::fit(&[[0.0, 0.0], [50.0, 0.0], [50.0, 50.0], [100.0, 50.0]], &[], false, 0.5, 4.0);
    assert!(corner.iter().skip(1).take(corner.len() - 2).all(|n| !n.smooth || !n.has_in()), "{corner:?}");
}

#[test]
fn curve_strokes_move_scale_and_rotate_with_their_nodes() {
    let mut s = curve(arc(), false);
    s.translate(10.0, 5.0);
    assert_eq!(s.nodes.as_ref().unwrap()[0].p, [10.0, 5.0]);
    s.scale_about([0.0, 0.0], 2.0, 2.0);
    s.rotate_about([0.0, 0.0], 90.0);
    // Los puntos transformados siguen sobre la curva de los nodos transformados (cambia el muestreo, no la forma).
    let nodes = s.nodes.clone().unwrap();
    for p in &s.points {
        let (_, _, d) = bezier::nearest(&nodes, false, *p).unwrap();
        assert!(d < 0.6, "los puntos y los nodos se separaron: {d}");
    }
}

#[test]
fn curve_survives_save_and_old_strokes_still_load() {
    let s = curve(arc(), false);
    let back: Stroke = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.nodes, s.nodes);
    assert_eq!(back.points.len(), s.points.len());
    assert!(!serde_json::to_string(&drawn()).unwrap().contains("nodes"));
    let old: Stroke = serde_json::from_str(r#"{"kind":"Free","points":[[0,0],[5,5]],"color":[0,0,0,255],"width":3.0}"#).unwrap();
    assert!(old.nodes.is_none());
}

#[test]
fn damaged_curve_becomes_a_plain_polyline() {
    let mut s = curve(arc(), false);
    s.nodes.as_mut().unwrap()[1].p = [f32::NAN, 0.0];
    s.points = vec![[0.0, 0.0], [10.0, 0.0]];
    s.fix_attrs();
    assert!(s.nodes.is_none());
    assert_eq!(s.points.len(), 2);
    let mut ok = curve(arc(), false);
    ok.points.clear();
    ok.fix_attrs();
    assert!(ok.points.len() > 2, "una curva sana se recalcula al cargar");
}

#[test]
fn style_edits_apply_to_curves_without_touching_nodes() {
    let mut s = curve(arc(), false);
    let nodes = s.nodes.clone();
    edit::set_prop(&mut s, &Prop::Profile(Profile::TaperBoth));
    edit::set_prop(&mut s, &Prop::Width(30.0));
    assert_eq!(s.nodes, nodes);
    assert!(!s.is_plain() && !s.pieces().is_empty());
}

#[test]
fn erasing_a_curve_bakes_it_into_plain_pieces() {
    let mut d = crate::model::Drawing::default();
    d.strokes.push(curve(arc(), false));
    let mid = d.strokes[0].points[d.strokes[0].points.len() / 2];
    assert!(d.erase(mid, 6.0));
    assert!(d.strokes.len() >= 2 && d.strokes.iter().all(|s| s.nodes.is_none()));
}

// ---------- Perfil de grosor personalizado (Fase 5) ----------

#[test]
fn custom_profile_follows_its_points() {
    let shape = vec![[0.0, 0.0], [0.5, 1.0], [1.0, 0.0]];
    assert_eq!(shape_at(&shape, 0.0), 0.0);
    assert_eq!(shape_at(&shape, 0.5), 1.0);
    assert_eq!(shape_at(&shape, 1.0), 0.0);
    let (a, b) = (shape_at(&shape, 0.2), shape_at(&shape, 0.3));
    assert!(a > 0.0 && b > a && b < 1.0, "sube de forma suave: {a} {b}");
    assert_eq!(shape_at(&[], 0.4), 1.0, "sin puntos no cambia el grosor");
    assert_eq!(shape_at(&[[0.3, 0.7]], 0.9), 0.7);
}

#[test]
fn custom_profile_shapes_the_stroke_width() {
    let pts: Vec<[f32; 2]> = (0..=40).map(|i| [i as f32 * 5.0, 0.0]).collect();
    let style = VStyle { profile: Profile::Custom, shape: vec![[0.0, 1.0], [0.5, 0.2], [1.0, 1.0]], pressure: false, ..Default::default() };
    let w = widths(&pts, &[], 20.0, &style, false);
    assert!((w[0] - 20.0).abs() < 1e-3 && (w[40] - 20.0).abs() < 1e-3);
    assert!((w[20] - 4.0).abs() < 0.3, "al centro queda estrecho: {}", w[20]);
    assert!(!is_plain(Some(&style), &[]));
    // Un trazo cerrado usa el perfil recorriendo toda la vuelta (ver las pruebas de trazos cerrados).
    assert!(widths(&pts, &[], 20.0, &style, true).iter().any(|v| *v < 19.0));
}

#[test]
fn custom_profile_is_cleaned_and_saved() {
    let messy = VStyle { profile: Profile::Custom, shape: vec![[0.9, 0.5], [f32::NAN, 1.0], [0.1, 3.0], [0.1, 0.4], [-2.0, 0.2]], ..Default::default() }.clamped();
    assert!(messy.shape.windows(2).all(|w| w[0][0] < w[1][0]), "ordenado y sin repetidos: {:?}", messy.shape);
    assert!(messy.shape.iter().all(|p| (0.0..=1.0).contains(&p[0]) && (0.0..=1.0).contains(&p[1])));
    let empty = VStyle { profile: Profile::Custom, shape: vec![], ..Default::default() }.clamped();
    assert_eq!(empty.shape, default_shape(), "un perfil personalizado vacío se repara");
    let many: Vec<[f32; 2]> = (0..40).map(|i| [i as f32 / 40.0, 0.5]).collect();
    assert_eq!(clean_shape(&many).len(), MAX_SHAPE_POINTS);
    let s = VStyle { profile: Profile::Custom, shape: default_shape(), ..Default::default() };
    let back: VStyle = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back, s);
    // Los estilos guardados antes no traen `shape` y no lo escriben si no se usa.
    assert!(!serde_json::to_string(&VStyle::default()).unwrap().contains("shape"));
    let old: VStyle = serde_json::from_str(r#"{"cap":"Flat","profile":"TaperEnd"}"#).unwrap();
    assert!(old.shape.is_empty() && old.profile == Profile::TaperEnd);
}

#[test]
fn editing_the_custom_profile_of_existing_strokes() {
    let mut a = drawn();
    let mut b = drawn();
    let shape = vec![[0.0, 0.3], [0.5, 1.0], [1.0, 0.3]];
    for s in [&mut a, &mut b] {
        edit::set_prop(s, &Prop::Shape(shape.clone()));
    }
    let c = edit::common(&[&a, &b]);
    assert_eq!((c.profile, c.shape.as_ref()), (Some(Profile::Custom), Some(&shape)));
    edit::set_prop(&mut b, &Prop::Shape(vec![[0.0, 1.0], [1.0, 0.0]]));
    assert_eq!(edit::common(&[&a, &b]).shape, None, "formas distintas: mixto");
    assert!(edit::restore(&mut a) && a.style.as_ref().unwrap().profile == Profile::Uniform);
    // Con el perfil Personalizado el trazo cambia de contorno sin mover su trayectoria.
    let mut s = drawn();
    let (pts, before) = (s.points.clone(), s.pieces());
    edit::set_prop(&mut s, &Prop::Shape(shape));
    assert_eq!(s.points, pts);
    assert_ne!(s.pieces(), before);
}

#[test]
fn included_custom_brushes_exist() {
    let l = VLibrary::default();
    for id in ["Entintado/Entintado con carga", "Pinceles artísticos/Gota"] {
        let p = l.find(id).unwrap();
        assert_eq!(p.style.profile, Profile::Custom, "{id}");
        assert!(p.style.shape.len() >= 3);
        assert_eq!(p.clone().clamped().style, p.style);
    }
}

#[test]
fn many_strokes_build_quickly() {
    // Control de rendimiento: 3000 trazos de 120 puntos con perfil y presión deben generarse en un instante.
    let strokes: Vec<Stroke> = (0..3000)
        .map(|k| {
            let pts: Vec<[f32; 2]> = (0..120).map(|i| [i as f32 * 4.0, (i as f32 * 0.2 + k as f32).sin() * 40.0]).collect();
            let pr: Vec<f32> = (0..120).map(|i| 0.3 + 0.7 * (i as f32 / 120.0)).collect();
            pencil(pts, pr, Some(VStyle { profile: Profile::Custom, shape: default_shape(), ..Default::default() }))
        })
        .collect();
    let t0 = std::time::Instant::now();
    let n: usize = strokes.iter().map(|s| s.pieces().len()).sum();
    let ms = t0.elapsed().as_millis();
    assert!(n > 3000 * 100);
    assert!(ms < 3000, "{ms} ms para 3000 trazos");
    eprintln!("3000 trazos × 120 puntos: {ms} ms ({n} piezas)");
}

// ---------- Perfil en trazos cerrados (#41) ----------

fn square_loop() -> Vec<[f32; 2]> {
    // Cuadrado de 100 de lado con un punto cada 10 (40 puntos), empezando en la esquina superior izquierda.
    let mut v = Vec::new();
    for i in 0..10 { v.push([i as f32 * 10.0, 0.0]); }
    for i in 0..10 { v.push([100.0, i as f32 * 10.0]); }
    for i in 0..10 { v.push([100.0 - i as f32 * 10.0, 100.0]); }
    for i in 0..10 { v.push([0.0, 100.0 - i as f32 * 10.0]); }
    v
}

#[test]
fn closed_stroke_pinches_at_the_seam_and_is_full_elsewhere() {
    let pts = square_loop();
    let style = VStyle { profile: Profile::TaperBoth, taper: 0.2, pressure: false, ..Default::default() };
    let w = widths(&pts, &[], 20.0, &style, true);
    assert!(w[0] < 0.5, "pellizco en el punto de cierre: {}", w[0]);
    assert!((w[20] - 20.0).abs() < 1e-3, "grosor pleno en el lado opuesto: {}", w[20]);
    assert!(w[39] < 20.0, "el tramo final también se afila: {}", w[39]);
    // El mismo trazo abierto da el mismo perfil en los puntos centrales.
    let open = widths(&pts, &[], 20.0, &style, false);
    assert!((open[20] - 20.0).abs() < 1e-3);
}

#[test]
fn closed_stroke_length_includes_the_closing_segment() {
    // Dos puntos opuestos de un cuadrado: con el tramo de cierre, la posición del último punto no llega a 1.
    let pts = square_loop();
    let style = VStyle { profile: Profile::Custom, shape: vec![[0.0, 0.0], [1.0, 1.0]], pressure: false, ..Default::default() };
    let w = widths(&pts, &[], 100.0, &style, true);
    let last = w[39];
    assert!(last < 100.0 && last > 80.0, "el último punto está cerca, pero antes del final de la vuelta: {last}");
    // Un trazo cerrado con perfil uniforme sigue igual que antes.
    let plain = widths(&pts, &[], 20.0, &VStyle { pressure: false, ..Default::default() }, true);
    assert!(plain.iter().all(|v| (*v - 20.0).abs() < 1e-3));
}

#[test]
fn closed_stroke_with_profile_builds_a_continuous_ribbon() {
    let mut s = pencil(square_loop(), Vec::new(), Some(VStyle { profile: Profile::TaperBoth, taper: 0.25, pressure: false, ..Default::default() }));
    s.closed = true;
    assert!(!s.is_plain());
    let pieces = s.pieces();
    assert!(!pieces.is_empty());
    assert!(pieces.iter().all(|poly| poly.iter().all(|p| p[0].is_finite() && p[1].is_finite())));
    // Cambiar el perfil de un trazo cerrado ya dibujado cambia su contorno sin mover los puntos.
    let (before, pts) = (s.pieces(), s.points.clone());
    edit::set_prop(&mut s, &Prop::Shape(vec![[0.0, 1.0], [0.5, 0.2], [1.0, 1.0]]));
    assert_eq!(s.points, pts);
    assert_ne!(s.pieces(), before);
}

#[test]
fn closing_a_curve_keeps_its_brush_profile() {
    let style = VStyle { profile: Profile::Custom, shape: default_shape(), ..Default::default() };
    let mut s = pencil(Vec::new(), Vec::new(), Some(style.clone()));
    s.nodes = Some(vec![Node::corner([0.0, 0.0]), Node::corner([100.0, 0.0]), Node::corner([100.0, 100.0]), Node::corner([0.0, 100.0])]);
    s.closed = true;
    s.regen();
    assert_eq!(s.style.as_ref().unwrap().profile, Profile::Custom);
    let w = widths(&s.points, &s.pressure, 10.0, s.style.as_ref().unwrap(), true);
    assert!(w.iter().any(|v| *v < 5.0) && w.iter().any(|v| *v > 9.0), "el perfil se ve en la figura cerrada");
}