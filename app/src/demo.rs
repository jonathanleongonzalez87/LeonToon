//! Proyecto de muestra: una pelota que rebota en bucle (animada a mano, a dos cuadros), un sol que va y viene (interpolación)
//! y un paisaje en capas que se desplaza con paralaje. Los trazos se generan aquí, con el estilo de la imagen de referencia:
//! contornos oscuros, colores planos y planos pictóricos claros (cielo, nubes, montañas, arbustos, carretera, pelota).
//!
//! Para que el bucle no tenga saltos, cada capa del paisaje repite su dibujo con un período `p` y se desplaza exactamente
//! `d` píxeles (un múltiplo de `p`) durante los `LEN` cuadros del proyecto: al volver al cuadro 1 todo queda igual.

use crate::model::{plane_z, Drawing, Interp, Layer, Panel, Project, Rgba, ShapeKind, Stroke};
use crate::tracks::Prop;
use std::sync::Arc;

const W: f32 = 1280.0;
const H: f32 = 720.0;
/// Duración del bucle, en cuadros (4 segundos a 24 cuadros por segundo).
pub const LEN: u32 = 96;
/// Altura de la carretera donde acaba el paisaje y empieza el borde.
const HORIZON: f32 = 532.0;
const INK: Rgba = [28, 26, 26, 255];
const CREAM: Rgba = [250, 235, 210, 255];

fn poly(points: Vec<[f32; 2]>, color: Rgba, width: f32, fill: Option<Rgba>) -> Stroke {
    Stroke { kind: ShapeKind::Free, points, color, width, closed: true, fill, pressure: Vec::new(), style: None, original: None, nodes: None }
}

fn open(points: Vec<[f32; 2]>, color: Rgba, width: f32) -> Stroke {
    Stroke { kind: ShapeKind::Free, points, color, width, closed: false, fill: None, pressure: Vec::new(), style: None, original: None, nodes: None }
}

fn line(a: [f32; 2], b: [f32; 2], color: Rgba, width: f32) -> Stroke {
    Stroke::shape(ShapeKind::Line, a, b, color, width, None)
}

fn flat(a: [f32; 2], b: [f32; 2], fill: Rgba) -> Stroke {
    Stroke::shape(ShapeKind::Rect, a, b, fill, 0.5, Some(fill))
}

/// Suaviza una polilínea abierta (corte de esquinas de Chaikin) conservando los extremos.
fn chaikin(pts: &[[f32; 2]], times: u32) -> Vec<[f32; 2]> {
    let mut p = pts.to_vec();
    for _ in 0..times {
        let mut q = vec![p[0]];
        for w in p.windows(2) {
            q.push([w[0][0] * 0.75 + w[1][0] * 0.25, w[0][1] * 0.75 + w[1][1] * 0.25]);
            q.push([w[0][0] * 0.25 + w[1][0] * 0.75, w[0][1] * 0.25 + w[1][1] * 0.75]);
        }
        q.push(*p.last().unwrap());
        p = q;
    }
    p
}

/// Una montaña con la cumbre redondeada y una cara en sombra.
fn mountain(cx: f32, top: f32, half: f32) -> Vec<Stroke> {
    let base = HORIZON + 12.0;
    let outline = chaikin(&[[cx - half, base], [cx - 30.0, top + 14.0], [cx, top], [cx + 30.0, top + 14.0], [cx + half, base]], 3);
    let slope = |x: f32| base - (base - top) * (1.0 - ((x - cx).abs() / half));
    let (xa, xb) = (cx + half * 0.06, cx + half * 0.62);
    let shade = vec![[xa, slope(xa) + 14.0], [xb, slope(xb) + 7.0], [cx + half * 0.30, base]];
    vec![poly(outline, INK, 4.5, Some([109, 143, 100, 255])), poly(shade, [84, 118, 84, 255], 0.5, Some([84, 118, 84, 255]))]
}

/// Silueta de arbustos: la envolvente de varios medios círculos, repetida cada 1280 px.
fn bushes(x0: f32, x1: f32) -> Stroke {
    const ARCS: [(f32, f32); 13] = [(50.0, 40.0), (120.0, 62.0), (205.0, 44.0), (330.0, 50.0), (410.0, 36.0), (520.0, 58.0), (610.0, 42.0), (780.0, 48.0), (850.0, 66.0), (940.0, 40.0), (1060.0, 52.0), (1150.0, 40.0), (1230.0, 56.0)];
    let base = HORIZON + 6.0;
    let height = |x: f32| -> f32 {
        let mut best = 0.0f32;
        for k in -2..8 {
            for (cx, r) in ARCS {
                let dx = x - (cx + k as f32 * 1280.0);
                if dx.abs() < r {
                    best = best.max(1.25 * (r * r - dx * dx).sqrt());
                }
            }
        }
        best
    };
    let mut pts = Vec::new();
    let mut x = x0;
    while x <= x1 {
        pts.push([x, base - height(x)]);
        x += 8.0;
    }
    pts.push([x1, base + 10.0]);
    pts.push([x0, base + 10.0]);
    poly(pts, INK, 4.5, Some([82, 115, 82, 255]))
}

/// Una nube de contorno: la envolvente de tres bultos sobre una base plana.
fn cloud(x0: f32, base: f32, scale: f32) -> Stroke {
    const BUMPS: [(f32, f32, f32); 3] = [(40.0, 40.0, 58.0), (118.0, 75.0, 115.0), (212.0, 45.0, 64.0)];
    let width = 257.0;
    let mut pts = Vec::new();
    let mut x = 0.0f32;
    while x <= width {
        let mut best = 0.0f32;
        for (cx, rx, ry) in BUMPS {
            let t = (x - cx) / rx;
            if t.abs() < 1.0 {
                best = best.max(ry * (1.0 - t * t).sqrt());
            }
        }
        pts.push([x0 + x * scale, base - best * scale]);
        x += 5.0;
    }
    pts.push([x0 + width * scale, base]);
    pts.push([x0, base]);
    poly(pts, INK, 4.5, Some(CREAM))
}

fn sun() -> Drawing {
    let (cx, cy, r) = (863.0f32, 153.0f32, 60.0f32);
    let mut d = Drawing::default();
    for k in 0..8 {
        let a = (22.5 + 45.0 * k as f32).to_radians();
        let len = if k % 2 == 0 { 30.0 } else { 24.0 };
        let (r0, r1) = (r + 22.0, r + 22.0 + len);
        d.strokes.push(line([cx + r0 * a.cos(), cy + r0 * a.sin()], [cx + r1 * a.cos(), cy + r1 * a.sin()], INK, 5.5));
    }
    d.strokes.push(Stroke::shape(ShapeKind::Ellipse, [cx - r, cy - r], [cx + r, cy + r], INK, 4.5, Some([249, 215, 168, 255])));
    let arc: Vec<[f32; 2]> = (0..8)
        .map(|i| {
            let a = (200.0 + i as f32 * 7.0).to_radians();
            [cx + 40.0 * a.cos(), cy + 40.0 * a.sin()]
        })
        .collect();
    d.strokes.push(open(arc, [255, 242, 222, 255], 7.0));
    d
}

/// Los doce dibujos de un rebote (se muestran de dos en dos cuadros, 24 cuadros por rebote).
fn bounce() -> Vec<Arc<Drawing>> {
    let (cx, floor) = (520.0f32, 586.0f32);
    (0..12)
        .map(|i| {
            let t = i as f32 / 12.0;
            let lift = 300.0 * 4.0 * t * (1.0 - t);
            let (rx, ry) = match i {
                0 => (92.0, 54.0),
                1 | 11 => (64.0, 86.0),
                2 | 10 => (68.0, 80.0),
                6 => (74.0, 74.0),
                _ => (70.0, 76.0),
            };
            let cy = floor - lift - ry;
            let mut d = Drawing::default();
            // Sombra sobre la carretera: más pequeña cuanto más alto está la pelota.
            let sw = 64.0 * (1.0 - 0.45 * lift / 300.0);
            let sh = sw * 0.17;
            d.strokes.push(Stroke::shape(ShapeKind::Ellipse, [cx - sw, floor + 2.0 - sh], [cx + sw, floor + 2.0 + sh], [26, 24, 24, 255], 0.5, Some([26, 24, 24, 255])));
            // Líneas de impacto en el contacto.
            let rays: &[(f32, f32, f32)] = match i {
                0 => &[(160.0, 84.0, 128.0), (128.0, 80.0, 138.0), (52.0, 80.0, 138.0), (20.0, 84.0, 128.0)],
                1 | 11 => &[(138.0, 96.0, 122.0), (42.0, 96.0, 122.0)],
                _ => &[],
            };
            for (deg, r0, r1) in rays {
                let a = deg.to_radians();
                d.strokes.push(line([cx + r0 * a.cos(), floor - 4.0 - r0 * a.sin() * 0.9], [cx + r1 * a.cos(), floor - 4.0 - r1 * a.sin() * 0.9], [250, 238, 215, 255], 8.0));
            }
            d.strokes.push(Stroke::shape(ShapeKind::Ellipse, [cx - rx, cy - ry], [cx + rx, cy + ry], INK, 4.5, Some([214, 52, 41, 255])));
            d.strokes.push(open(vec![[cx - rx * 0.62, cy - ry * 0.05], [cx - rx * 0.52, cy - ry * 0.48], [cx - rx * 0.18, cy - ry * 0.68]], [255, 232, 226, 255], 8.0));
            Arc::new(d)
        })
        .collect()
}

/// Capa 3D en un plano: el paisaje queda detrás de la pelota para la vista en profundidad.
fn layer(name: &str, plane: u8, depth: f32, label: u8, d: Drawing) -> Layer {
    let mut l = Layer::new(name);
    l.is3d = true;
    l.plane = plane;
    l.depth = depth;
    l.label = label;
    l.keys.insert(0, Arc::new(d));
    l
}

/// Capa que se desplaza `d` píxeles a la izquierda durante todo el bucle.
fn scrolling(mut l: Layer, d: f32) -> Layer {
    l.tracks.set(Prop::Position, 0, [0.0, 0.0], Interp::Linear);
    l.tracks.set(Prop::Position, LEN, [-d, 0.0], Interp::Linear);
    l
}

pub fn build() -> Project {
    let mut p = Project::blank(1280, 720, 24, LEN);

    // Pelota: animación tradicional (un dibujo cada dos cuadros, cuatro rebotes por bucle).
    let drawings = bounce();
    let mut ball = Layer::new("Pelota");
    ball.is3d = true;
    ball.label = 1;
    for n in 0..(LEN / 2) {
        ball.keys.insert(n * 2, drawings[(n % 12) as usize].clone());
    }

    // Sol: animación por interpolación (va y viene durante el bucle).
    let mut sun_layer = layer("Sol", 0, 800.0, 3, sun());
    sun_layer.tracks.set(Prop::Position, 0, [0.0, 0.0], Interp::Ease);
    sun_layer.tracks.set(Prop::Position, LEN / 2, [-110.0, 30.0], Interp::Ease);
    sun_layer.tracks.set(Prop::Position, LEN, [0.0, 0.0], Interp::Ease);

    // Rayas de la carretera (la capa más rápida: 3840 px por bucle, seis períodos de 640).
    let mut dashes = Drawing::default();
    let d_dash = 3840.0;
    let mut x = 52.0;
    while x < W + d_dash {
        dashes.strokes.push(poly(vec![[x + 12.0, 634.0], [x + 224.0, 634.0], [x + 212.0, 652.0], [x, 652.0]], CREAM, 0.5, Some(CREAM)));
        x += 640.0;
    }
    let dashes = scrolling(layer("Rayas de la carretera", 1, 100.0, 4, dashes), d_dash);

    // Carretera fija: borde y asfalto.
    let mut road = Drawing::default();
    road.strokes.push(flat([0.0, HORIZON + 18.0], [W, H], [106, 99, 99, 255]));
    road.strokes.push(flat([0.0, HORIZON], [W, HORIZON + 18.0], [160, 148, 142, 255]));
    road.strokes.push(line([0.0, HORIZON], [W, HORIZON], INK, 4.0));
    road.strokes.push(line([0.0, HORIZON + 18.0], [W, HORIZON + 18.0], INK, 4.0));
    let road = layer("Carretera", 1, 120.0, 4, road);

    // Arbustos (2560 px por bucle).
    let d_bush = 2560.0;
    let mut bd = Drawing::default();
    bd.strokes.push(bushes(-420.0, W + d_bush + 420.0));
    let bushes_layer = scrolling(layer("Arbustos", 0, 250.0, 5, bd), d_bush);

    // Montañas (1920 px por bucle: el dibujo se repite cada 1920).
    let d_mount = 1920.0;
    let mut md = Drawing::default();
    for k in -1..4 {
        for (cx, top, half) in [(170.0, 292.0, 520.0), (830.0, 335.0, 430.0), (1480.0, 285.0, 560.0)] {
            let cx = cx + k as f32 * d_mount;
            if cx + half > -50.0 && cx - half < W + d_mount + 50.0 {
                md.strokes.extend(mountain(cx, top, half));
            }
        }
    }
    let mountains = scrolling(layer("Montañas", 0, 500.0, 5, md), d_mount);

    // Nubes (la capa más lenta: 1280 px por bucle).
    let d_cloud = 1280.0;
    let mut cd = Drawing::default();
    for k in -1..3 {
        let o = k as f32 * d_cloud;
        cd.strokes.push(cloud(119.0 + o, 211.0, 1.0));
        cd.strokes.push(cloud(1056.0 + o, 198.0, 0.75));
    }
    let clouds = scrolling(layer("Nubes", 0, 700.0, 6, cd), d_cloud);

    // Cielo.
    let mut sky = Drawing::default();
    sky.strokes.push(flat([-10.0, -10.0], [W + 10.0, H + 10.0], CREAM));
    let sky = layer("Fondo", 0, plane_z(0), 0, sky);

    p.layers = vec![ball, sun_layer, dashes, road, bushes_layer, mountains, clouds, sky];

    let texts = [
        ("La pelota rebota", "Rebota en el centro del plano mientras el paisaje avanza.", "Plano general fijo"),
        ("El sol va y viene", "El sol cruza el cielo y regresa (interpolación).", "Plano general"),
        ("Paralaje", "Nubes, montañas, arbustos y carretera avanzan a distinta velocidad.", "Plano general fijo"),
    ];
    for (i, (t, a, c)) in texts.iter().enumerate() {
        let mut pn = Panel::new(i + 1);
        pn.title = t.to_string();
        pn.action = a.to_string();
        pn.camera = c.to_string();
        pn.dur = 1.0;
        pn.drawing = Arc::new(p.flatten([0, 36, 72][i]));
        p.storyboard.push(pn);
    }
    p
}
