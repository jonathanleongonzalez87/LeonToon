//! Curvas Bézier editables (Issue #31, Fase 4 / Issue #2): nodos con tiradores.
//!
//! Un trazo con nodos guarda la **curva** como fuente de verdad (`Stroke::nodes`) y su polilínea (`points`) se
//! recalcula a partir de ella, así que el dibujo, la selección, la exportación y la animación siguen funcionando
//! con la polilínea y el trazo se puede seguir editando punto por punto sin perder la curva.
//! Los tiradores se guardan en posiciones absolutas: mover, girar o escalar el trazo es transformar los tres puntos.

use serde::{Deserialize, Serialize};

fn one() -> f32 {
    1.0
}

/// Un nodo de la curva: el punto y sus dos tiradores (entrada y salida).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Node {
    pub p: [f32; 2],
    pub hin: [f32; 2],
    pub hout: [f32; 2],
    /// Presión en el nodo (1 si el trazo no tiene presión): se interpola a lo largo de la curva.
    #[serde(default = "one")]
    pub pr: f32,
    /// Suave: los tiradores son colineales y al mover uno el otro lo sigue. Esquina: independientes.
    #[serde(default)]
    pub smooth: bool,
}

impl Node {
    /// Nodo de esquina sin tiradores.
    pub fn corner(p: [f32; 2]) -> Node {
        Node { p, hin: p, hout: p, pr: 1.0, smooth: false }
    }

    pub fn has_in(&self) -> bool {
        dist(self.p, self.hin) > 1e-3
    }

    pub fn has_out(&self) -> bool {
        dist(self.p, self.hout) > 1e-3
    }

    pub fn is_finite(&self) -> bool {
        [self.p, self.hin, self.hout].iter().all(|q| q[0].is_finite() && q[1].is_finite()) && self.pr.is_finite()
    }

    /// Aplica una transformación a los tres puntos del nodo.
    pub fn map(&mut self, f: impl Fn([f32; 2]) -> [f32; 2]) {
        self.p = f(self.p);
        self.hin = f(self.hin);
        self.hout = f(self.hout);
    }
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn lerp(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// Cantidad de segmentos de la curva: cerrada o no.
pub fn segments(n: usize, closed: bool) -> usize {
    if n < 2 {
        0
    } else if closed && n >= 3 {
        n
    } else {
        n - 1
    }
}

/// Los cuatro puntos de control del segmento `i` (del nodo `i` al siguiente).
fn seg(nodes: &[Node], i: usize) -> [[f32; 2]; 4] {
    let (a, b) = (&nodes[i], &nodes[(i + 1) % nodes.len()]);
    [a.p, a.hout, b.hin, b.p]
}

fn eval(c: &[[f32; 2]; 4], t: f32) -> [f32; 2] {
    let (a, b, d) = (lerp(c[0], c[1], t), lerp(c[1], c[2], t), lerp(c[2], c[3], t));
    lerp(lerp(a, b, t), lerp(b, d, t), t)
}

fn is_line(c: &[[f32; 2]; 4]) -> bool {
    dist(c[0], c[1]) < 1e-3 && dist(c[2], c[3]) < 1e-3
}

/// Puntos por unidad de longitud al aplanar (la densidad fija mantiene la curva suave a cualquier zoom razonable).
const STEP: f32 = 3.0;

/// Aplana la curva en una polilínea y la presión en cada punto (vacía si todos los nodos tienen presión 1).
pub fn flatten(nodes: &[Node], closed: bool) -> (Vec<[f32; 2]>, Vec<f32>) {
    let mut pts: Vec<[f32; 2]> = Vec::new();
    let mut pr: Vec<f32> = Vec::new();
    if nodes.is_empty() {
        return (pts, pr);
    }
    let segs = segments(nodes.len(), closed);
    for i in 0..segs {
        let c = seg(nodes, i);
        let (p0, p1) = (nodes[i].pr, nodes[(i + 1) % nodes.len()].pr);
        let n = if is_line(&c) {
            1
        } else {
            let len = dist(c[0], c[1]) + dist(c[1], c[2]) + dist(c[2], c[3]);
            ((len / STEP).ceil() as usize).clamp(6, 96)
        };
        for k in 0..n {
            let t = k as f32 / n as f32;
            pts.push(eval(&c, t));
            pr.push(p0 + (p1 - p0) * t);
        }
    }
    if !(closed && nodes.len() >= 3) {
        pts.push(nodes[nodes.len() - 1].p);
        pr.push(nodes[nodes.len() - 1].pr);
    }
    if pr.iter().all(|v| (v - 1.0).abs() < 1e-4) {
        pr.clear();
    }
    (pts, pr)
}

/// Punto del trazo más cercano a `q`: (segmento, t, distancia).
pub fn nearest(nodes: &[Node], closed: bool, q: [f32; 2]) -> Option<(usize, f32, f32)> {
    let mut best: Option<(usize, f32, f32)> = None;
    for i in 0..segments(nodes.len(), closed) {
        let c = seg(nodes, i);
        const N: usize = 48;
        let mut prev = c[0];
        for k in 1..=N {
            let t1 = k as f32 / N as f32;
            let cur = eval(&c, t1);
            // Proyección de q sobre el trozo prev–cur.
            let (dx, dy) = (cur[0] - prev[0], cur[1] - prev[1]);
            let l2 = dx * dx + dy * dy;
            let u = if l2 > 1e-12 { (((q[0] - prev[0]) * dx + (q[1] - prev[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let at = [prev[0] + dx * u, prev[1] + dy * u];
            let d = dist(q, at);
            if best.map_or(true, |b| d < b.2) {
                let t0 = (k - 1) as f32 / N as f32;
                best = Some((i, t0 + (t1 - t0) * u, d));
            }
            prev = cur;
        }
    }
    best
}

/// Parte la curva en el segmento `seg` y el parámetro `t` sin cambiar su forma. Devuelve el índice del nodo nuevo.
pub fn insert(nodes: &mut Vec<Node>, closed: bool, seg_i: usize, t: f32) -> usize {
    let n = nodes.len();
    let segs = segments(n, closed);
    if seg_i >= segs {
        return n.saturating_sub(1);
    }
    let t = t.clamp(0.02, 0.98);
    let j = (seg_i + 1) % n;
    let c = seg(nodes, seg_i);
    // De Casteljau.
    let (a, b, d) = (lerp(c[0], c[1], t), lerp(c[1], c[2], t), lerp(c[2], c[3], t));
    let (e, f) = (lerp(a, b, t), lerp(b, d, t));
    let m = lerp(e, f, t);
    let pr = nodes[seg_i].pr + (nodes[j].pr - nodes[seg_i].pr) * t;
    let curved = !is_line(&c);
    nodes[seg_i].hout = a;
    nodes[j].hin = d;
    let new = Node { p: m, hin: e, hout: f, pr, smooth: curved };
    nodes.insert(seg_i + 1, new);
    seg_i + 1
}

/// Quita el nodo `i` (una curva abierta necesita 2 nodos; una cerrada, 3).
pub fn delete(nodes: &mut Vec<Node>, closed: bool, i: usize) -> bool {
    let min = if closed { 3 } else { 2 };
    if nodes.len() <= min || i >= nodes.len() {
        return false;
    }
    nodes.remove(i);
    true
}

/// Tiradores automáticos del nodo `i` según sus vecinos (curva suave que pasa por los nodos).
pub fn auto_handles(nodes: &mut [Node], closed: bool, i: usize) {
    let n = nodes.len();
    if n < 2 || i >= n {
        return;
    }
    let wrap = closed && n >= 3;
    let prev = if i > 0 { Some(i - 1) } else if wrap { Some(n - 1) } else { None };
    let next = if i + 1 < n { Some(i + 1) } else if wrap { Some(0) } else { None };
    let p = nodes[i].p;
    match (prev, next) {
        (Some(a), Some(b)) => {
            let (pa, pb) = (nodes[a].p, nodes[b].p);
            let t = [pb[0] - pa[0], pb[1] - pa[1]];
            let l = (t[0] * t[0] + t[1] * t[1]).sqrt();
            if l < 1e-6 {
                return;
            }
            let u = [t[0] / l, t[1] / l];
            let (lin, lout) = (dist(p, pa) / 3.0, dist(p, pb) / 3.0);
            nodes[i].hin = [p[0] - u[0] * lin, p[1] - u[1] * lin];
            nodes[i].hout = [p[0] + u[0] * lout, p[1] + u[1] * lout];
        }
        (None, Some(b)) => {
            nodes[i].hout = lerp(p, nodes[b].p, 1.0 / 3.0);
            nodes[i].hin = p;
        }
        (Some(a), None) => {
            nodes[i].hin = lerp(p, nodes[a].p, 1.0 / 3.0);
            nodes[i].hout = p;
        }
        (None, None) => {}
    }
    nodes[i].smooth = true;
}

/// Convierte el nodo en suave (tiradores automáticos) o en esquina (sin tiradores).
pub fn set_smooth(nodes: &mut [Node], closed: bool, i: usize, smooth: bool) {
    if i >= nodes.len() {
        return;
    }
    if smooth {
        auto_handles(nodes, closed, i);
    } else {
        nodes[i].hin = nodes[i].p;
        nodes[i].hout = nodes[i].p;
        nodes[i].smooth = false;
    }
}

/// Mueve el nodo y sus dos tiradores juntos.
pub fn move_anchor(nodes: &mut [Node], i: usize, to: [f32; 2]) {
    let Some(n) = nodes.get_mut(i) else { return };
    let (dx, dy) = (to[0] - n.p[0], to[1] - n.p[1]);
    n.p = to;
    n.hin = [n.hin[0] + dx, n.hin[1] + dy];
    n.hout = [n.hout[0] + dx, n.hout[1] + dy];
}

/// Mueve un tirador (`out`: el de salida). En un nodo suave el otro tirador gira con él y conserva su largo.
pub fn move_handle(nodes: &mut [Node], i: usize, out: bool, to: [f32; 2]) {
    let Some(n) = nodes.get_mut(i) else { return };
    let (mine, other) = if out { (&mut n.hout, &mut n.hin) } else { (&mut n.hin, &mut n.hout) };
    *mine = to;
    if n.smooth {
        let d = [n.p[0] - to[0], n.p[1] - to[1]];
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if l > 1e-6 {
            let lo = dist(n.p, *other);
            let lo = if lo > 1e-3 { lo } else { l };
            *other = [n.p[0] + d[0] / l * lo, n.p[1] + d[1] / l * lo];
        }
    }
}

/// Ajusta una curva a una polilínea ya dibujada: se queda con los puntos que definen la forma y los une con
/// tramos suaves; los giros marcados quedan como esquinas. `pressure` (si tiene el largo de los puntos) se conserva.
pub fn fit(points: &[[f32; 2]], pressure: &[f32], closed: bool, eps: f32, width: f32) -> Vec<Node> {
    if points.len() < 2 {
        return points.iter().map(|p| Node::corner(*p)).collect();
    }
    let has_p = pressure.len() == points.len();
    let keep = crate::model::simplify_idx_attr(points, if has_p { Some((pressure, width * 0.5)) } else { None }, eps);
    let mut nodes: Vec<Node> = keep.iter().map(|&k| Node { pr: if has_p { pressure[k] } else { 1.0 }, ..Node::corner(points[k]) }).collect();
    let n = nodes.len();
    let wrap = closed && n >= 3;
    for i in 0..n {
        let interior = i > 0 && i + 1 < n;
        if !interior && !wrap {
            continue;
        }
        let a = nodes[(i + n - 1) % n].p;
        let b = nodes[(i + 1) % n].p;
        let p = nodes[i].p;
        let (v1, v2) = ([p[0] - a[0], p[1] - a[1]], [b[0] - p[0], b[1] - p[1]]);
        let (l1, l2) = ((v1[0] * v1[0] + v1[1] * v1[1]).sqrt(), (v2[0] * v2[0] + v2[1] * v2[1]).sqrt());
        if l1 < 1e-6 || l2 < 1e-6 {
            continue;
        }
        let cos = (v1[0] * v2[0] + v1[1] * v2[1]) / (l1 * l2);
        if cos < 0.3 {
            continue; // giro marcado: esquina
        }
        auto_handles(&mut nodes, closed, i);
    }
    nodes
}
