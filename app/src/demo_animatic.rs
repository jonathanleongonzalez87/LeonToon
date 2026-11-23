//! Proyecto de muestra del animatic: la historia de la pelota contada con **bocetos en mapa de bits**, al estilo de un storyboard
//! a lápiz y marcador: líneas dobles y temblorosas, rellenos de color suave que no encajan del todo con el contorno, y un marco
//! de viñeta. Dos personajes con colores básicos: el **Personaje A** (roja) y el **Personaje B** (azul).
//!
//! Cada plano es una composición con tres capas de mapa de bits («Personaje B», «Personaje A» y «Fondo»); el animatic es la
//! composición que los une, con su duración, fundidos y cámara. Todo se dibuja aquí, con un generador de números fijo, así que
//! el resultado es siempre el mismo.

use crate::model::{CamMove, Drawing, Layer, LayerKind, Panel, Project};
use crate::raster::Raster;
use std::sync::Arc;
use tiny_skia::{Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

const W: u32 = 1280;
const H: u32 = 720;
type P = [f32; 2];

const PENCIL: [u8; 3] = [46, 46, 50];
const PAPER: [u8; 3] = [250, 247, 242];
const RED: [u8; 3] = [232, 82, 66];
const BLUE: [u8; 3] = [64, 118, 224];

/// Generador de números sencillo y repetible (xorshift).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / 16_777_216.0
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

fn dist(a: P, b: P) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Puntos a lo largo de la polilínea, uno cada `step` píxeles.
fn resample(pts: &[P], step: f32) -> Vec<P> {
    let mut out = vec![pts[0]];
    for w in pts.windows(2) {
        let len = dist(w[0], w[1]);
        let n = (len / step).ceil().max(1.0) as usize;
        for k in 1..=n {
            let t = k as f32 / n as f32;
            out.push([w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t]);
        }
    }
    out
}

/// Tiembla la línea con un ruido suave (dos ondas de distinta frecuencia), como una mano que no es perfecta.
fn wobble(pts: &[P], amp: f32, rng: &mut Rng) -> Vec<P> {
    let dense = resample(pts, 9.0);
    let ph: [f32; 4] = [rng.range(0.0, 6.28), rng.range(0.0, 6.28), rng.range(0.0, 6.28), rng.range(0.0, 6.28)];
    let mut s = 0.0;
    let mut out = Vec::with_capacity(dense.len());
    for (i, p) in dense.iter().enumerate() {
        if i > 0 {
            s += dist(dense[i - 1], *p);
        }
        let dx = amp * ((s * 0.021 + ph[0]).sin() + 0.5 * (s * 0.073 + ph[1]).sin());
        let dy = amp * ((s * 0.019 + ph[2]).sin() + 0.5 * (s * 0.061 + ph[3]).sin());
        out.push([p[0] + dx, p[1] + dy]);
    }
    out
}

/// Camino suave que pasa cerca de los puntos (curvas con los puntos medios).
fn smooth_path(pts: &[P], close: bool) -> Option<tiny_skia::Path> {
    if pts.len() < 2 {
        return None;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0][0], pts[0][1]);
    for i in 1..pts.len() - 1 {
        let mid = [(pts[i][0] + pts[i + 1][0]) * 0.5, (pts[i][1] + pts[i + 1][1]) * 0.5];
        pb.quad_to(pts[i][0], pts[i][1], mid[0], mid[1]);
    }
    let last = pts[pts.len() - 1];
    pb.line_to(last[0], last[1]);
    if close {
        pb.close();
    }
    pb.finish()
}

fn paint(c: [u8; 3], a: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], a);
    p.anti_alias = true;
    p
}

/// Línea a lápiz: dos pasadas, una firme y otra más fina y más temblorosa, que dan el aspecto de trazo repetido.
fn pencil(pm: &mut Pixmap, pts: &[P], width: f32, closed: bool, rng: &mut Rng) {
    let mut input = pts.to_vec();
    if closed {
        input.push(pts[0]);
    }
    for (amp, wd, alpha) in [(1.1f32, width, 215u8), (2.0, width * 0.55, 130)] {
        let w = wobble(&input, amp, rng);
        if let Some(path) = smooth_path(&w, false) {
            let st = Stroke { width: wd.max(1.0), line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
            pm.stroke_path(&path, &paint(PENCIL, alpha), &st, Transform::identity(), None);
        }
    }
}

/// Relleno de marcador: color suave, desplazado un poco del contorno y con una segunda pasada más pálida.
fn marker(pm: &mut Pixmap, pts: &[P], color: [u8; 3], alpha: u8, rng: &mut Rng) {
    for (amp, off, a) in [(2.4f32, 5.0f32, alpha), (3.4, 9.0, alpha / 3)] {
        let o = [rng.range(-off, off), rng.range(-off, off) * 0.6];
        let mut shifted: Vec<P> = pts.iter().map(|p| [p[0] + o[0], p[1] + o[1]]).collect();
        shifted.push(shifted[0]);
        let w = wobble(&shifted, amp, rng);
        if let Some(path) = smooth_path(&w, true) {
            pm.fill_path(&path, &paint(color, a), FillRule::Winding, Transform::identity(), None);
        }
    }
}

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<P> {
    (0..40)
        .map(|i| {
            let t = i as f32 / 40.0 * std::f32::consts::TAU;
            [cx + rx * t.cos(), cy + ry * t.sin()]
        })
        .collect()
}

fn line(pm: &mut Pixmap, a: P, b: P, width: f32, rng: &mut Rng) {
    pencil(pm, &[a, b], width, false, rng);
}

/// Una letra grande, a lápiz, dentro del personaje: A o B.
fn letter(pm: &mut Pixmap, ch: char, cx: f32, cy: f32, size: f32, rng: &mut Rng) {
    let at = |x: f32, y: f32| -> P { [cx + x * size, cy + y * size] };
    let strokes: Vec<Vec<P>> = match ch {
        'A' => vec![vec![at(-0.5, 0.6), at(0.0, -0.6), at(0.5, 0.6)], vec![at(-0.28, 0.2), at(0.28, 0.2)]],
        _ => vec![
            vec![at(-0.35, -0.6), at(-0.35, 0.6)],
            vec![at(-0.35, -0.6), at(0.2, -0.6), at(0.36, -0.32), at(0.2, 0.0), at(-0.35, 0.0)],
            vec![at(-0.35, 0.0), at(0.3, 0.0), at(0.46, 0.3), at(0.3, 0.6), at(-0.35, 0.6)],
        ],
    };
    for s in strokes {
        pencil(pm, &s, 4.5, false, rng);
    }
}

// ---------- El fondo ----------

fn cloud_points(x0: f32, base: f32, scale: f32) -> Vec<P> {
    const BUMPS: [(f32, f32, f32); 3] = [(40.0, 40.0, 58.0), (118.0, 75.0, 115.0), (212.0, 45.0, 64.0)];
    let mut pts = Vec::new();
    let mut x = 0.0f32;
    while x <= 257.0 {
        let mut best = 0.0f32;
        for (cx, rx, ry) in BUMPS {
            let t = (x - cx) / rx;
            if t.abs() < 1.0 {
                best = best.max(ry * (1.0 - t * t).sqrt());
            }
        }
        pts.push([x0 + x * scale, base - best * scale]);
        x += 9.0;
    }
    pts.push([x0 + 257.0 * scale, base]);
    pts.push([x0, base]);
    pts
}

fn bush_points(x0: f32, x1: f32, base: f32) -> Vec<P> {
    const ARCS: [(f32, f32); 13] = [(50.0, 40.0), (120.0, 62.0), (205.0, 44.0), (330.0, 50.0), (410.0, 36.0), (520.0, 58.0), (610.0, 42.0), (780.0, 48.0), (850.0, 66.0), (940.0, 40.0), (1060.0, 52.0), (1150.0, 40.0), (1230.0, 56.0)];
    let mut pts = Vec::new();
    let mut x = x0;
    while x <= x1 {
        let mut best = 0.0f32;
        for (cx, r) in ARCS {
            let dx = x - cx;
            if dx.abs() < r {
                best = best.max(1.25 * (r * r - dx * dx).sqrt());
            }
        }
        pts.push([x, base - best]);
        x += 8.0;
    }
    pts.push([x1, base + 8.0]);
    pts.push([x0, base + 8.0]);
    pts
}

/// El paisaje de la historia: nubes, sol, dos montañas, arbustos y la carretera, con el marco de la viñeta.
fn background(rng: &mut Rng) -> Pixmap {
    let mut pm = Pixmap::new(W, H).expect("lienzo");
    pm.fill(Color::from_rgba8(PAPER[0], PAPER[1], PAPER[2], 255));
    // Nubes
    for (x0, base, sc) in [(119.0, 211.0, 1.0), (1056.0, 198.0, 0.75)] {
        let pts = cloud_points(x0, base, sc);
        marker(&mut pm, &pts, [186, 201, 220], 150, rng);
        pencil(&mut pm, &pts, 3.4, true, rng);
    }
    // Sol
    let (sx, sy, sr) = (863.0f32, 153.0f32, 60.0f32);
    for k in 0..8 {
        let a = (22.5 + 45.0 * k as f32).to_radians();
        let (r0, r1) = (sr + 24.0, sr + 24.0 + if k % 2 == 0 { 30.0 } else { 24.0 });
        line(&mut pm, [sx + r0 * a.cos(), sy + r0 * a.sin()], [sx + r1 * a.cos(), sy + r1 * a.sin()], 4.2, rng);
    }
    let sun = ellipse(sx, sy, sr, sr);
    marker(&mut pm, &sun, [250, 212, 186], 210, rng);
    pencil(&mut pm, &sun, 3.6, true, rng);
    let arc: Vec<P> = (0..8).map(|i| {
        let a = (200.0 + i as f32 * 7.0).to_radians();
        [sx + 40.0 * a.cos(), sy + 40.0 * a.sin()]
    }).collect();
    if let Some(path) = smooth_path(&arc, false) {
        let st = Stroke { width: 7.0, line_cap: LineCap::Round, ..Default::default() };
        pm.stroke_path(&path, &paint([255, 255, 255], 170), &st, Transform::identity(), None);
    }
    // Montañas
    let base = 545.0f32;
    for (cx, top, half) in [(176.0f32, 292.0f32, 520.0f32), (1099.0, 284.0, 560.0)] {
        let outline = vec![[cx - half, base], [cx - 40.0, top + 22.0], [cx, top], [cx + 40.0, top + 22.0], [cx + half, base]];
        marker(&mut pm, &outline, [170, 188, 210], 205, rng);
        let slope = |x: f32| base - (base - top) * (1.0 - ((x - cx).abs() / half));
        let (xa, xb) = (cx + half * 0.05, cx + half * 0.6);
        let shade = vec![[xa, slope(xa) + 16.0], [xb, slope(xb) + 8.0], [cx + half * 0.3, base]];
        marker(&mut pm, &shade, [142, 164, 192], 150, rng);
        pencil(&mut pm, &[[cx - half, base], [cx, top], [cx + half, base]], 3.6, false, rng);
    }
    // Arbustos
    let bushes = bush_points(-20.0, W as f32 + 20.0, 546.0);
    marker(&mut pm, &bushes, [158, 158, 160], 205, rng);
    pencil(&mut pm, &bushes[..bushes.len() - 2], 3.2, false, rng);
    // Carretera: borde claro y asfalto gris
    let asphalt = vec![[-10.0, 550.0], [W as f32 + 10.0, 550.0], [W as f32 + 10.0, H as f32 + 10.0], [-10.0, H as f32 + 10.0]];
    marker(&mut pm, &asphalt, [176, 174, 172], 215, rng);
    let kerb = vec![[-10.0, 532.0], [W as f32 + 10.0, 532.0], [W as f32 + 10.0, 550.0], [-10.0, 550.0]];
    marker(&mut pm, &kerb, [238, 235, 230], 220, rng);
    line(&mut pm, [-10.0, 532.0], [W as f32 + 10.0, 532.0], 3.4, rng);
    line(&mut pm, [-10.0, 551.0], [W as f32 + 10.0, 551.0], 3.4, rng);
    for (x, w) in [(52.0f32, 212.0f32), (536.0, 215.0), (1013.0, 217.0)] {
        let dash = vec![[x + 12.0, 634.0], [x + w, 634.0], [x + w - 12.0, 652.0], [x, 652.0]];
        marker(&mut pm, &dash, [243, 241, 236], 235, rng);
    }
    // Marco de la viñeta: líneas que se pasan un poco de las esquinas
    for (a, b) in [([6.0, 26.0], [W as f32 - 6.0, 24.0]), ([6.0, H as f32 - 20.0], [W as f32 - 6.0, H as f32 - 22.0]), ([26.0, 6.0], [24.0, H as f32 - 6.0]), ([W as f32 - 26.0, 6.0], [W as f32 - 24.0, H as f32 - 6.0])] {
        line(&mut pm, a, b, 3.6, rng);
    }
    pm
}

// ---------- Los personajes ----------

/// Una pelota de color con su letra, y lo que la acompaña: sombra en el suelo, líneas de movimiento e impacto.
struct Pose {
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    /// Líneas de movimiento hacia la izquierda (viene de allí).
    trail: f32,
    shadow: bool,
    impact: bool,
}

const FLOOR: f32 = 590.0;

fn pose(cx: f32, cy: f32, r: f32) -> Pose {
    Pose { cx, cy, rx: r, ry: r, trail: 0.0, shadow: false, impact: false }
}

fn character(ch: char, color: [u8; 3], p: &Pose, rng: &mut Rng) -> Pixmap {
    let mut pm = Pixmap::new(W, H).expect("lienzo");
    if p.shadow {
        let lift = (FLOOR - (p.cy + p.ry)).max(0.0);
        let sw = p.rx * (1.0 - (lift / 500.0).min(0.5));
        let sh = ellipse(p.cx, FLOOR + 4.0, sw, sw * 0.17);
        marker(&mut pm, &sh, [110, 110, 112], 150, rng);
        pencil(&mut pm, &sh, 2.4, true, rng);
    }
    if p.impact {
        for (deg, r0, r1) in [(160.0f32, 86.0f32, 132.0f32), (128.0, 82.0, 142.0), (52.0, 82.0, 142.0), (20.0, 86.0, 132.0)] {
            let a = deg.to_radians();
            line(&mut pm, [p.cx + r0 * a.cos(), FLOOR - 4.0 - r0 * a.sin() * 0.9], [p.cx + r1 * a.cos(), FLOOR - 4.0 - r1 * a.sin() * 0.9], 4.0, rng);
        }
    }
    if p.trail > 0.0 {
        for (k, dy) in [(-22.0f32, 0.0f32), (0.0, 30.0), (22.0, 62.0)] {
            let (x1, y1) = (p.cx - p.rx - 18.0 - p.trail * 0.4, p.cy + dy * 0.2 + k * 0.4);
            let (x0, y0) = (p.cx - p.rx - 18.0 - p.trail, p.cy + dy + 40.0);
            pencil(&mut pm, &[[x0, y0], [(x0 + x1) * 0.5 - 10.0, (y0 + y1) * 0.5 - 24.0], [x1, y1]], 3.0, false, rng);
        }
    }
    let body = ellipse(p.cx, p.cy, p.rx, p.ry);
    marker(&mut pm, &body, color, 225, rng);
    pencil(&mut pm, &body, 4.2, true, rng);
    let hl: Vec<P> = (0..8).map(|i| {
        let a = (200.0 + i as f32 * 8.0).to_radians();
        [p.cx + p.rx * 0.72 * a.cos(), p.cy + p.ry * 0.72 * a.sin()]
    }).collect();
    if let Some(path) = smooth_path(&hl, false) {
        let st = Stroke { width: 8.0, line_cap: LineCap::Round, ..Default::default() };
        pm.stroke_path(&path, &paint([255, 240, 235], 190), &st, Transform::identity(), None);
    }
    letter(&mut pm, ch, p.cx + p.rx * 0.08, p.cy + p.ry * 0.12, p.rx.min(p.ry) * 0.5, rng);
    pm
}

// ---------- El proyecto ----------

fn bitmap_layer(name: &str, label: u8, pm: Option<Pixmap>) -> Layer {
    let mut l = Layer::new(name);
    l.kind = LayerKind::Bitmap;
    l.label = label;
    l.keys.insert(0, Arc::new(drawing_of(pm)));
    l
}

fn drawing_of(pm: Option<Pixmap>) -> Drawing {
    Drawing { strokes: Vec::new(), raster: pm.map(|pm| Arc::new(Raster::from_pixmap(pm))) }
}

struct Shot {
    title: &'static str,
    dur: f32,
    action: &'static str,
    dialogue: &'static str,
    camera: &'static str,
    trans: &'static str,
    cam: Option<CamMove>,
    a: Option<Pose>,
    b: Option<Pose>,
}

fn shots() -> Vec<Shot> {
    let a_land = Pose { cx: 520.0, cy: FLOOR - 52.0, rx: 92.0, ry: 52.0, trail: 0.0, shadow: true, impact: true };
    vec![
        Shot {
            title: "Plano general",
            dur: 2.0,
            action: "Plano general del paisaje: amanece sobre la carretera. Todavía no hay nadie.",
            dialogue: "",
            camera: "Zoom lento hacia adelante",
            trans: "",
            cam: Some(CamMove { s0: 1.0, x0: 0.0, y0: 0.0, s1: 1.12, x1: 0.0, y1: 0.0, ease: true }),
            a: None,
            b: None,
        },
        Shot {
            title: "A entra",
            dur: 1.5,
            action: "El Personaje A (roja) entra rebotando por la izquierda.",
            dialogue: "",
            camera: "Plano general fijo",
            trans: "",
            cam: None,
            a: Some(Pose { trail: 150.0, ..pose(430.0, 330.0, 70.0) }),
            b: None,
        },
        Shot {
            title: "A rebota",
            dur: 1.5,
            action: "A toca el suelo: se aplasta y salen líneas de impacto.",
            dialogue: "A: ¡Boing!",
            camera: "Plano general fijo",
            trans: "",
            cam: None,
            a: Some(a_land),
            b: None,
        },
        Shot {
            title: "B asoma",
            dur: 1.5,
            action: "El Personaje B (azul) asoma por el borde derecho, tímido.",
            dialogue: "",
            camera: "Plano general fijo",
            trans: "Fundido",
            cam: None,
            a: Some(Pose { shadow: true, ..pose(330.0, FLOOR - 70.0, 70.0) }),
            b: Some(Pose { shadow: true, ..pose(1255.0, FLOOR - 62.0, 62.0) }),
        },
        Shot {
            title: "A conoce a B",
            dur: 2.0,
            action: "Los dos frente a frente. Primer plano: la cámara se acerca a ellos.",
            dialogue: "A: ¿Quién eres?\nB: Soy B. ¿Jugamos?",
            camera: "Zoom hacia los dos personajes",
            trans: "",
            cam: Some(CamMove { s0: 1.0, x0: 0.0, y0: 0.0, s1: 1.3, x1: 0.0, y1: -60.0, ease: true }),
            a: Some(Pose { shadow: true, ..pose(430.0, FLOOR - 80.0, 80.0) }),
            b: Some(Pose { shadow: true, ..pose(860.0, FLOOR - 62.0, 62.0) }),
        },
        Shot {
            title: "Rebotan juntos",
            dur: 2.0,
            action: "Rebotan juntos hacia la derecha mientras sube el sol.",
            dialogue: "A y B: ¡Boing!",
            camera: "Paneo suave a la derecha",
            trans: "Fundido a negro",
            cam: Some(CamMove { s0: 1.12, x0: 20.0, y0: 0.0, s1: 1.12, x1: -60.0, y1: 0.0, ease: true }),
            a: Some(Pose { trail: 120.0, shadow: true, ..pose(480.0, 300.0, 72.0) }),
            b: Some(Pose { trail: 90.0, shadow: true, ..pose(790.0, 360.0, 58.0) }),
        },
    ]
}

/// El proyecto de muestra del animatic: seis planos de la historia de la pelota, con la composición «Animatic» ya creada y abierta.
pub fn build() -> Project {
    let mut p = Project::blank(W, H, 24, 24);
    p.background = [255, 255, 255, 255];
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let fondo = Arc::new(drawing_of(Some(background(&mut rng))));
    for (i, s) in shots().into_iter().enumerate() {
        p.storyboard.push(Panel {
            title: s.title.into(),
            dur: s.dur,
            action: s.action.into(),
            dialogue: s.dialogue.into(),
            camera: s.camera.into(),
            trans: s.trans.into(),
            scene: "1".into(),
            cam: s.cam,
            ..Default::default()
        });
        let id = p.ensure_panel_comp(i).expect("composición del plano");
        let a = s.a.as_ref().map(|pose| character('A', RED, pose, &mut rng));
        let b = s.b.as_ref().map(|pose| character('B', BLUE, pose, &mut rng));
        let mut fondo_layer = Layer::new("Fondo");
        fondo_layer.kind = LayerKind::Bitmap;
        fondo_layer.keys.insert(0, fondo.clone());
        let layers = vec![bitmap_layer("Personaje B", 3, b), bitmap_layer("Personaje A", 1, a), fondo_layer];
        if let Some(c) = p.comps.iter_mut().find(|c| c.id == id) {
            c.layers = layers;
        }
        // Abrir y cerrar el plano rehace su miniatura con lo dibujado.
        if p.open_panel(i, 0, 0) {
            p.exit_comp();
        }
    }
    p.renumber_panels(10);
    let built = p.build_animatic();
    p.length = built.length;
    p.layers.clear();
    p.set_animatic(built);
    p.enter_comp(0, 0, 0);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_animatic_sample_has_six_panels_with_bitmap_layers_for_each_character() {
        let p = build();
        assert_eq!(p.storyboard.len(), 6);
        assert!(p.in_comp(), "se abre dentro de la composición del animatic");
        assert_eq!(p.comp_path(), vec!["Escena".to_string(), "Animatic".to_string()]);
        assert_eq!(p.layers.iter().filter(|l| l.kind == LayerKind::Precomp).count(), 6, "un clip por plano");
        assert_eq!(p.layers.iter().filter(|l| l.name == "Negro").count(), 1, "el último plano entra con fundido a negro");
        let root = p.root_clone();
        assert_eq!(root.layers.len(), 1);
        for pn in &root.storyboard {
            let c = root.comp(pn.comp.unwrap()).unwrap();
            let names: Vec<&str> = c.layers.iter().map(|l| l.name.as_str()).collect();
            assert_eq!(names, ["Personaje B", "Personaje A", "Fondo"]);
            assert!(c.layers.iter().all(|l| l.kind == LayerKind::Bitmap));
        }
        // La numeración es de 10 en 10 dentro de la escena 1.
        assert_eq!(root.storyboard[2].label(2), "1·30");
        // Plano 5: están los dos personajes; plano 1: ninguno.
        let has = |pi: usize, layer: usize| root.comp(root.storyboard[pi].comp.unwrap()).unwrap().layers[layer].keys[&0].raster.is_some();
        assert!(has(4, 0) && has(4, 1));
        assert!(!has(0, 0) && !has(0, 1));
        assert!(has(1, 1) && !has(1, 0), "en el plano 2 solo está A");
    }

    #[test]
    fn the_thumbnails_and_the_animatic_show_the_bitmap_sketches() {
        let p = build();
        let root = p.root_clone();
        // La miniatura de cada plano trae los píxeles (no solo trazos vectoriales).
        for pn in &root.storyboard {
            assert!(pn.drawing.raster.as_ref().map_or(false, |r| !r.is_blank()), "miniatura con píxeles");
        }
        // El animatic dibuja los planos: el centro del plano 5 (A y B) no es el papel en blanco, y el rojo de A aparece en el plano 3.
        let img = |f: u32| crate::render::render_frame(&root, f, 0.25).unwrap();
        let fps = 24u32;
        let start5 = ((2.0 + 1.5 + 1.5 + 1.5) * fps as f32) as u32 + 6;
        let im = img(start5);
        let (w, h) = (im.width(), im.height());
        let px = im.pixel((w as f32 * 0.30) as u32, (h as f32 * 0.71) as u32).unwrap();
        assert!(px.red() > px.blue() + 40, "A (roja) se ve en el plano 5: {:?}", (px.red(), px.green(), px.blue()));
        let px_b = im.pixel((w as f32 * 0.645) as u32, (h as f32 * 0.733) as u32).unwrap();
        assert!(px_b.blue() > px_b.red() + 40, "B (azul) se ve en el plano 5: {:?}", (px_b.red(), px_b.green(), px_b.blue()));
    }
}
