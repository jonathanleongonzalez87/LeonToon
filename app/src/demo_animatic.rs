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

// Los sonidos del ejemplo (recortes de los archivos de la carpeta «referencia/Sonido», pasados a MP3 más ligero).
const BOING_A: &[u8] = include_bytes!("../assets/audio_demo/boing_a.mp3");
const BOING_B: &[u8] = include_bytes!("../assets/audio_demo/boing_b.mp3");
const AMBIENTE: &[u8] = include_bytes!("../assets/audio_demo/ambiente.mp3");
const MUSICA: &[u8] = include_bytes!("../assets/audio_demo/musica.mp3");

/// Un trozo de sonido colocado en el tiempo: de qué archivo, cuándo suena en el animatic, qué parte del archivo (desde `offset`, durante
/// `len` segundos) y con qué paneo, ganancia y eco.
struct Piece {
    name: &'static str,
    file: &'static str,
    at: f32,
    offset: f32,
    len: f32,
    pan: f32,
    gain_db: f32,
    echo: f32,
}

/// Cada «boing» está recortado al justo momento: el archivo trae varios botes seguidos, y de cada uno se toma solo su trozo (con una
/// brizna de silencio antes) y se coloca en el cuadro en que la pelota toca el suelo o habla. A suena a la izquierda y B a la derecha.
const PIECES: [Piece; 8] = [
    Piece { name: "Boing A 1", file: "boing_a.mp3", at: 3.5, offset: 0.22, len: 0.35, pan: -0.45, gain_db: 7.0, echo: 0.0 },
    Piece { name: "Boing A 2", file: "boing_a.mp3", at: 6.7, offset: 1.33, len: 0.38, pan: -0.45, gain_db: 7.0, echo: 0.0 },
    Piece { name: "Boing A 3", file: "boing_a.mp3", at: 8.7, offset: 2.58, len: 0.34, pan: -0.45, gain_db: 7.0, echo: 0.0 },
    Piece { name: "Boing A 4", file: "boing_a.mp3", at: 9.5, offset: 3.69, len: 0.30, pan: -0.45, gain_db: 7.0, echo: 0.0 },
    Piece { name: "Boing B 1", file: "boing_b.mp3", at: 5.0, offset: 0.06, len: 0.78, pan: 0.45, gain_db: 0.0, echo: 0.2 },
    Piece { name: "Boing B 2", file: "boing_b.mp3", at: 7.6, offset: 1.10, len: 0.85, pan: 0.45, gain_db: 0.0, echo: 0.2 },
    Piece { name: "Boing B 3", file: "boing_b.mp3", at: 8.86, offset: 0.06, len: 0.78, pan: 0.45, gain_db: 0.0, echo: 0.2 },
    Piece { name: "Boing B 4", file: "boing_b.mp3", at: 9.66, offset: 1.10, len: 0.84, pan: 0.45, gain_db: 0.0, echo: 0.2 },
];

/// Escribe en `dir` los sonidos del ejemplo (si no están ya) y devuelve las rutas.
fn extract_sounds(dir: &std::path::Path) -> Option<std::collections::HashMap<&'static str, std::path::PathBuf>> {
    std::fs::create_dir_all(dir).ok()?;
    let mut out = std::collections::HashMap::new();
    for (name, bytes) in [("boing_a.mp3", BOING_A), ("boing_b.mp3", BOING_B), ("ambiente.mp3", AMBIENTE), ("musica.mp3", MUSICA)] {
        let path = dir.join(name);
        if std::fs::metadata(&path).map_or(true, |m| m.len() != bytes.len() as u64) {
            std::fs::write(&path, bytes).ok()?;
        }
        out.insert(name, path);
    }
    Some(out)
}

/// Una capa de audio de las del ejemplo.
#[allow(clippy::too_many_arguments)]
fn sound_layer(name: &str, path: &std::path::Path, in_f: u32, out_f: u32, offset: f32, params: crate::model::AudioParams, label: u8) -> Layer {
    let mut l = Layer::new(name);
    l.kind = LayerKind::Audio;
    l.audio = Some(crate::model::AudioRef { path: path.to_string_lossy().to_string(), rel: None, name: name.into(), offset, params });
    l.in_f = in_f;
    l.out_f = Some(out_f);
    l.label = label;
    l
}

/// El proyecto de muestra con una mezcla breve: música ambiental, ambiente de escena y los «boings» de A y B recortados al justo momento,
/// con ganancia animada con claves (entradas, salidas y bajada de la música mientras hablan), paneo, ecualizador y eco. Los sonidos
/// se escriben en `dir` y las capas de audio van dentro de la composición del animatic.
pub fn build_with_audio(dir: &std::path::Path) -> Project {
    use crate::model::{AudioParams, Interp};
    use crate::tracks::Prop;
    let mut p = build();
    let Some(files) = extract_sounds(dir) else { return p };
    let fps = p.fps as f32;
    let frames = p.comps.iter().find(|c| c.name == crate::animatic::NAME).map_or(p.length, |c| c.length).max(p.length).max(1);
    let last = frames - 1;
    let at = |t: f32| ((t * fps).round() as u32).min(last);
    let mut layers: Vec<Layer> = Vec::new();

    // Música: entra con un fundido, baja mientras hablan (planos 5) y sale con otro fundido.
    let mut music = sound_layer("Música ambiental", &files["musica.mp3"], 0, last, 0.0, AudioParams { gain_db: -14.0, eq_lo: 2.0, ..Default::default() }, 6);
    for (f, db) in [(0, -60.0), (at(2.0), -14.0), (at(6.5), -14.0), (at(6.8), -22.0), (at(8.2), -22.0), (at(8.5), -14.0), (at(9.0), -14.0), (last, -60.0)] {
        music.tracks.set(Prop::Gain, f, [db, 0.0], Interp::Linear);
    }
    layers.push(music);

    // Ambiente de la escena: suave, con agudos recortados y un poco de espacio.
    let mut amb = sound_layer("Ambiente de la escena", &files["ambiente.mp3"], 0, last, 0.0, AudioParams { gain_db: 10.0, eq_hi: -4.0, reverb: 0.08, ..Default::default() }, 4);
    for (f, db) in [(0, -40.0), (at(1.5), 10.0), (at(9.0), 10.0), (last, -40.0)] {
        amb.tracks.set(Prop::Gain, f, [db, 0.0], Interp::Linear);
    }
    layers.push(amb);

    // Los «boings», recortados: cada uno empieza en el cuadro del bote y toma solo su trozo del archivo.
    let first_boing = layers.len();
    for pc in &PIECES {
        let in_f = at(pc.at);
        let out_f = (in_f + (pc.len * fps).ceil() as u32).saturating_sub(1).clamp(in_f, last);
        let params = AudioParams { gain_db: pc.gain_db, pan: pc.pan, echo: pc.echo, ..Default::default() };
        layers.push(sound_layer(pc.name, &files[pc.file], in_f, out_f, pc.offset, params, if pc.name.contains(" A ") { 1 } else { 3 }));
    }
    let boings: Vec<usize> = (first_boing..layers.len()).collect();

    // Dentro de la composición del animatic (si está abierta, en las capas que se editan).
    let open = p.stack.last().map_or(false, |lv| p.comp(lv.comp_id).map_or(false, |c| c.name == crate::animatic::NAME));
    if open {
        let base = p.layers.len();
        p.layers.extend(layers);
        let idx: Vec<usize> = boings.iter().map(|i| base + i).collect();
        if let Some(ci) = p.group_layers(&idx) {
            p.layers[ci].name = "Boings de A y B".into();
            p.layers[ci].collapsed = true;
        }
    } else if let Some(ci) = p.comps.iter().position(|c| c.name == crate::animatic::NAME) {
        std::mem::swap(&mut p.layers, &mut p.comps[ci].layers);
        let base = p.layers.len();
        p.layers.extend(layers);
        let idx: Vec<usize> = boings.iter().map(|i| base + i).collect();
        if let Some(gi) = p.group_layers(&idx) {
            p.layers[gi].name = "Boings de A y B".into();
            p.layers[gi].collapsed = true;
        }
        std::mem::swap(&mut p.layers, &mut p.comps[ci].layers);
    }
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

    #[test]
    fn the_sample_with_sound_is_a_short_mix_with_trimmed_boings_music_and_ambience() {
        let dir = std::env::temp_dir().join(format!("leontoon_demo_audio_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = build_with_audio(&dir);
        let root = p.root_clone();
        let items = crate::audio::collect(&root);
        assert_eq!(items.len(), 10, "música, ambiente y ocho boings");
        // Los boings están recortados: empiezan en medio del archivo y duran menos de un segundo.
        let boing = items.iter().find(|i| i.path.ends_with("boing_a.mp3") && i.start > 6.0 && i.start < 7.0).unwrap();
        assert!(boing.offset > 1.0 && boing.len < 0.5, "{boing:?}");
        // La música y el ambiente duran todo el animatic y su ganancia está animada con claves.
        let music = items.iter().find(|i| i.path.ends_with("musica.mp3")).unwrap();
        assert!((music.len - 10.5).abs() < 0.1 && music.tracks.has(crate::tracks::Prop::Gain));
        assert!(music.value(crate::tracks::Prop::Gain, 0.0) < -50.0 && (music.value(crate::tracks::Prop::Gain, 4.0) + 14.0).abs() < 0.1, "entra con un fundido");
        assert!(music.value(crate::tracks::Prop::Gain, 7.5) < -20.0, "baja mientras hablan");
        // Los boings quedan agrupados en una colección plegada dentro del animatic.
        let anim = root.comps.iter().find(|c| c.name == crate::animatic::NAME).unwrap();
        assert!(anim.layers.iter().any(|l| l.kind == crate::model::LayerKind::Collection && l.collapsed && l.name.contains("Boings")));
        // La mezcla: en el momento del primer bote suena más que antes, A más a la izquierda y B más a la derecha.
        let mut cache = crate::audio::Cache::default();
        for it in &items {
            cache.load(&it.path).unwrap();
        }
        let rate = 22_050;
        let m = crate::audio::mix(&items, &cache, rate, 0.0, 10.5);
        let side = |a: f32, b: f32, ch: usize| (((a * rate as f32) as usize)..((b * rate as f32) as usize)).fold(0.0f32, |x, i| x.max(m[i * 2 + ch].abs()));
        assert!(side(3.52, 3.8, 0) > side(0.6, 1.2, 0) * 1.5, "el boing se oye por encima del ambiente");
        assert!(side(3.52, 3.8, 0) > side(3.52, 3.8, 1), "A suena más a la izquierda");
        assert!(side(5.02, 5.6, 1) > side(5.02, 5.6, 0), "B suena más a la derecha");
        let top = m.iter().fold(0.0f32, |x, v| x.max(v.abs()));
        assert!(top <= 1.0, "la mezcla de ejemplo no debe sobremodular: pico {top}");
        // Al cambiar los planos, el audio se conserva en la composición rehecha.
        let mut q = p.clone();
        q.storyboard[0].dur = 3.0;
        assert!(q.refresh_animatic());
        assert!(q.layers.iter().any(|l| l.kind == crate::model::LayerKind::Audio), "el audio sobrevive al rehacer el animatic");
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn the_sample_exports_an_mp4_with_its_boings_when_ffmpeg_is_installed() {
        let Some(ffmpeg) = crate::render::find_ffmpeg() else {
            eprintln!("ffmpeg no está instalado en este equipo: se omite la prueba del MP4 del ejemplo");
            return;
        };
        let dir = std::env::temp_dir().join(format!("leontoon_demo_mp4_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = build_with_audio(&dir);
        let root = p.root_clone();
        let items = crate::audio::collect(&root);
        let mut cache = crate::audio::Cache::default();
        cache.load(&items[0].path).unwrap();
        // Los primeros cuatro segundos: incluyen el primer bote de A (en el segundo 3,5).
        let m = crate::audio::mix(&items, &cache, 48_000, 0.0, 4.0);
        let wav = dir.join("mezcla.wav");
        crate::audio::write_wav(&wav, 48_000, &m).unwrap();
        let mp4 = dir.join("animatic.mp4");
        let n = crate::render::export_video(&root, &ffmpeg, &mp4, 0, 95, 0.25, Some(&wav), None).unwrap();
        assert_eq!(n, 96);
        let out = std::process::Command::new(&ffmpeg).arg("-i").arg(&mp4).output().unwrap();
        let info = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(info.contains("Video:") && info.contains("Audio:"), "{info}");
        assert!(info.contains("Duration: 00:00:04.0"), "dura cuatro segundos: {info}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}