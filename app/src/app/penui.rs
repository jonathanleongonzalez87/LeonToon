//! Pluma vectorial (curvas Bézier) y edición de nodos (Issue #2 e Issue #31, Fase 4).
//!
//! Pluma: clic = nodo de esquina; clic y arrastrar = nodo suave con tiradores; clic en el primer nodo = cerrar;
//! Intro = terminar; Esc = cancelar; Retroceso = quitar el último nodo. La curva se guarda con sus nodos
//! (`Stroke::nodes`) y sigue siendo editable. Edición de nodos: con la Selección, un trazo con nodos muestra
//! sus nodos y tiradores; arrastrarlos los mueve, Ctrl+clic sobre la curva añade un nodo.
//! Solo en capas vectoriales; no se rasteriza nada.

use super::shell::muted;
use super::*;
use crate::vector::bezier::{self, Node};

/// Curva de la Pluma que todavía no se terminó.
pub(super) struct PenPath {
    pub nodes: Vec<Node>,
    /// Capa y cuadro donde se empezó.
    pub owner: (usize, u32),
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

const NODE_PX: f32 = 8.0;
const SEL_COL: Color32 = Color32::from_rgb(0x3d, 0xb4, 0xff);

impl LeonApp {
    // ---------- Pluma ----------

    /// Trazo con el estilo actual del lápiz para la curva de la Pluma.
    fn pen_stroke(&self, nodes: Vec<Node>, closed: bool) -> Stroke {
        let mut color = rgba(self.color);
        color[3] = (color[3] as f32 * self.stroke_opacity).round() as u8;
        let mut s = Stroke {
            kind: ShapeKind::Free,
            points: Vec::new(),
            color,
            width: self.pencil_size,
            closed,
            fill: None,
            pressure: Vec::new(),
            style: Some(self.vstyle.clone().clamped()),
            original: None,
            nodes: Some(nodes),
        };
        s.regen();
        s
    }

    /// Un clic de la Pluma. Devuelve el gesto de arrastre del nodo nuevo (None si cerró o terminó la curva).
    pub(super) fn pen_begin(&mut self, p: [f32; 2]) -> Option<Gesture> {
        let tol = 9.0 / self.view_scale();
        let owner = (self.cl, self.frame);
        if self.pen_path.as_ref().map_or(false, |pp| pp.owner != owner) {
            self.pen_commit(false);
        }
        if let Some(pp) = &self.pen_path {
            if pp.nodes.len() >= 3 && dist(p, pp.nodes[0].p) <= tol {
                self.pen_commit(true);
                return None;
            }
        }
        let pp = self.pen_path.get_or_insert_with(|| PenPath { nodes: Vec::new(), owner });
        pp.nodes.push(Node::corner(p));
        Some(Gesture::PenNode { idx: pp.nodes.len() - 1 })
    }

    /// Arrastre tras el clic: saca los tiradores del nodo nuevo (el de entrada, simétrico).
    pub(super) fn pen_drag(&mut self, idx: usize, p: [f32; 2]) {
        let tol = 3.0 / self.view_scale();
        if let Some(n) = self.pen_path.as_mut().and_then(|pp| pp.nodes.get_mut(idx)) {
            if dist(p, n.p) > tol {
                n.hout = p;
                n.hin = [2.0 * n.p[0] - p[0], 2.0 * n.p[1] - p[1]];
                n.smooth = true;
            }
        }
    }

    /// Termina la curva y la guarda en el dibujo donde se empezó (con un paso de Deshacer).
    pub(super) fn pen_commit(&mut self, close: bool) {
        let Some(pp) = self.pen_path.take() else { return };
        let (cl, f) = pp.owner;
        if pp.nodes.len() < 2 {
            if !pp.nodes.is_empty() {
                self.say("Una curva necesita al menos dos nodos");
            }
            return;
        }
        let Some(layer) = self.project.layers.get(cl) else { return };
        if layer.locked || layer.kind == LayerKind::Bitmap {
            self.say("No se pudo guardar la curva: la capa está bloqueada o no es vectorial");
            return;
        }
        let closed = close && pp.nodes.len() >= 3;
        let s = self.pen_stroke(pp.nodes, closed);
        self.checkpoint();
        let layer = &mut self.project.layers[cl];
        if layer.drawing_at(f).is_none() {
            layer.keys.insert(f, Default::default());
        }
        if let Some(d) = layer.drawing_at_mut(f) {
            d.strokes.push(s);
        }
        self.say("Curva creada. Con la Selección (V) puedes mover sus nodos y tiradores");
    }

    pub(super) fn pen_cancel(&mut self) {
        if self.pen_path.take().is_some() {
            self.say("Curva cancelada");
        }
    }

    /// Si la herramienta, la capa o el cuadro cambian, la curva en curso se guarda donde se empezó.
    pub(super) fn pen_housekeeping(&mut self) {
        if let Some(pp) = &self.pen_path {
            if self.tool != Tool::Pen || pp.owner != (self.cl, self.frame) {
                self.pen_commit(false);
            }
        }
    }

    /// Teclas de la Pluma con una curva en curso. Devuelve true si usó la tecla.
    pub(super) fn pen_keys(&mut self, ctx: &egui::Context) -> bool {
        if self.tool != Tool::Pen || self.pen_path.is_none() {
            return false;
        }
        let key = |k: Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
        if key(Key::Enter) {
            self.pen_commit(false);
            return true;
        }
        if key(Key::Escape) {
            self.pen_cancel();
            return true;
        }
        if key(Key::Backspace) || key(Key::Delete) {
            if let Some(pp) = self.pen_path.as_mut() {
                pp.nodes.pop();
                if pp.nodes.is_empty() {
                    self.pen_path = None;
                }
            }
            return true;
        }
        false
    }

    /// Vista previa de la curva en curso, con el mismo motor que el resultado final.
    pub(super) fn pen_preview(&self) -> Option<Stroke> {
        let pp = self.pen_path.as_ref().filter(|pp| pp.owner == (self.cl, self.frame))?;
        Some(self.pen_stroke(pp.nodes.clone(), false))
    }

    /// Nodos y tiradores de la curva en curso.
    pub(super) fn paint_pen_overlay(&self, p: &egui::Painter, origin: Pos2, z: f32) {
        let Some(pp) = self.pen_path.as_ref().filter(|pp| pp.owner == (self.cl, self.frame)) else { return };
        let map = self.layer_map(origin, z);
        paint_nodes(p, &pp.nodes, &map, None, false, true);
        if pp.nodes.len() >= 3 {
            // El primer nodo se agranda: un clic ahí cierra la curva.
            p.circle_stroke(map(pp.nodes[0].p), NODE_PX, stk(1.5, ACCENT2));
        }
    }

    /// Conversión de coordenadas del dibujo a pantalla para la capa actual.
    fn layer_map(&self, origin: Pos2, z: f32) -> impl Fn([f32; 2]) -> Pos2 {
        let (w, h) = (self.project.width as f32, self.project.height as f32);
        let t = self.layer().transform_at(self.frame);
        move |q: [f32; 2]| {
            let s = t.apply(w, h, q);
            Pos2::new(origin.x + s[0] * z, origin.y + s[1] * z)
        }
    }

    // ---------- Edición de nodos (Selección) ----------

    /// El trazo elegido, si es uno solo y es una curva con nodos.
    pub(super) fn node_stroke(&self) -> Option<&Stroke> {
        if self.tool != Tool::Select || self.obj_mode || self.picked.len() != 1 {
            return None;
        }
        self.layer().drawing_at(self.frame)?.strokes.get(self.picked[0]).filter(|s| s.nodes.is_some())
    }

    fn with_node_stroke<R>(&mut self, f: impl FnOnce(&mut Stroke) -> R) -> Option<R> {
        self.node_stroke()?;
        let (fr, i) = (self.frame, self.picked[0]);
        let s = self.project.layers[self.cl].drawing_at_mut(fr)?.strokes.get_mut(i)?;
        s.nodes.as_ref()?;
        Some(f(s))
    }

    /// Un clic de la Selección sobre un nodo o tirador de la curva elegida (o sobre la curva con Ctrl para añadir un nodo).
    pub(super) fn node_begin(&mut self, p: [f32; 2], command: bool) -> Option<Gesture> {
        let s = self.node_stroke()?;
        let nodes = s.nodes.clone()?;
        let (closed, width) = (s.closed, s.width);
        let tol = NODE_PX / self.view_scale();
        let near = |q: [f32; 2]| dist(q, p) <= tol;
        let n = nodes.len();
        if let Some(sel) = self.sel_node.filter(|i| *i < n) {
            for j in [sel, (sel + n - 1) % n, (sel + 1) % n] {
                if (j == (sel + n - 1) % n && sel == 0 && !closed) || (j == (sel + 1) % n && sel + 1 == n && !closed) {
                    continue;
                }
                if nodes[j].has_out() && near(nodes[j].hout) {
                    return Some(Gesture::NodeDrag { idx: j, part: 2 });
                }
                if nodes[j].has_in() && near(nodes[j].hin) {
                    return Some(Gesture::NodeDrag { idx: j, part: 1 });
                }
            }
        }
        if let Some(i) = nodes.iter().position(|q| near(q.p)) {
            self.sel_node = Some(i);
            return Some(Gesture::NodeDrag { idx: i, part: 0 });
        }
        if command {
            if let Some((seg, t, d)) = bezier::nearest(&nodes, closed, p) {
                if d <= tol + width * 0.5 {
                    let new = self.with_node_stroke(|s| {
                        let ns = s.nodes.as_mut()?;
                        let i = bezier::insert(ns, closed, seg, t);
                        s.regen();
                        Some(i)
                    });
                    if let Some(Some(i)) = new {
                        self.sel_node = Some(i);
                        self.changed = true;
                        return Some(Gesture::NodeDrag { idx: i, part: 0 });
                    }
                }
            }
        }
        None
    }

    /// Arrastre de un nodo (`part` 0) o de uno de sus tiradores (1 entrada, 2 salida).
    pub(super) fn node_drag(&mut self, idx: usize, part: u8, p: [f32; 2]) {
        self.with_node_stroke(|s| {
            if let Some(ns) = s.nodes.as_mut() {
                match part {
                    0 => bezier::move_anchor(ns, idx, p),
                    1 => bezier::move_handle(ns, idx, false, p),
                    _ => bezier::move_handle(ns, idx, true, p),
                }
            }
            s.regen();
        });
        self.changed = true;
    }

    /// Nodos y tiradores de la curva elegida.
    pub(super) fn paint_node_overlay(&self, p: &egui::Painter, origin: Pos2, z: f32) {
        let Some(s) = self.node_stroke() else { return };
        let Some(nodes) = &s.nodes else { return };
        let map = self.layer_map(origin, z);
        paint_nodes(p, nodes, &map, self.sel_node.filter(|i| *i < nodes.len()), s.closed, false);
    }

    /// Suprime el nodo elegido (tecla Supr). Devuelve true si lo usó.
    pub(super) fn node_delete_key(&mut self) -> bool {
        let Some(i) = self.sel_node else { return false };
        self.node_stroke().is_some() && self.delete_node(i)
    }

    fn delete_node(&mut self, i: usize) -> bool {
        let closed = self.node_stroke().map_or(false, |s| s.closed);
        let can = self.node_stroke().and_then(|s| s.nodes.as_ref()).map_or(false, |n| n.len() > if closed { 3 } else { 2 });
        if !can {
            self.say("Una curva necesita al menos 2 nodos (3 si está cerrada). Para quitarla entera, bórrala");
            return true;
        }
        self.checkpoint();
        self.with_node_stroke(|s| {
            if let Some(ns) = s.nodes.as_mut() {
                bezier::delete(ns, closed, i);
            }
            s.regen();
        });
        self.sel_node = None;
        true
    }

    /// Sección «Nodos» del panel N con la Selección.
    pub(super) fn node_panel_ui(&mut self, ui: &mut egui::Ui) {
        if self.tool != Tool::Select || self.obj_mode || self.layer().kind != LayerKind::Vector || self.picked.is_empty() {
            return;
        }
        if let Some(s) = self.node_stroke() {
            let (n, closed) = (s.nodes.as_ref().map_or(0, |n| n.len()), s.closed);
            let sel = self.sel_node.filter(|i| *i < n);
            let smooth = sel.and_then(|i| s.nodes.as_ref().map(|ns| ns[i].smooth)).unwrap_or(false);
            ui.add_space(4.0);
            ui.label(RichText::new("Nodos de la curva").font(semi(12.0)).color(TX));
            ui.label(RichText::new(match sel {
                Some(i) => format!("{n} nodos · elegido el {}", i + 1),
                None => format!("{n} nodos · ninguno elegido"),
            }).size(11.5).color(MU));
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(sel.is_some(), egui::Button::new(if smooth { "Pasar a esquina" } else { "Pasar a suave" }).small()).on_hover_text("Suave: tiradores alineados. Esquina: sin tiradores").clicked() {
                    if let Some(i) = sel {
                        self.checkpoint();
                        self.with_node_stroke(|s| {
                            if let Some(ns) = s.nodes.as_mut() {
                                bezier::set_smooth(ns, closed, i, !smooth);
                            }
                            s.regen();
                        });
                    }
                }
                if ui.add_enabled(sel.is_some(), egui::Button::new("Eliminar nodo").small()).clicked() {
                    if let Some(i) = sel {
                        self.delete_node(i);
                    }
                }
                if ui.add_enabled(n >= 3, egui::Button::new(if closed { "Abrir curva" } else { "Cerrar curva" }).small()).clicked() {
                    self.checkpoint();
                    self.with_node_stroke(|s| {
                        s.closed = !s.closed;
                        s.regen();
                    });
                }
            });
            muted(ui, "Arrastra un nodo o su tirador para cambiar la curva. Ctrl + clic sobre la curva añade un nodo. Supr quita el nodo elegido. El trazo sigue siendo vectorial.");
            return;
        }
        // Trazos libres: se pueden pasar a curva editable.
        let strokes = self.picked_strokes();
        let convertible = strokes.iter().filter(|s| s.nodes.is_none() && s.points.len() >= 2).count();
        if convertible > 0 {
            ui.add_space(4.0);
            ui.label(RichText::new("Nodos").font(semi(12.0)).color(TX));
            if ui.small_button(format!("Convertir en curva ({convertible})")).on_hover_text("Ajusta una curva Bézier a los puntos del trazo para poder editar sus nodos. Se puede deshacer").clicked() {
                self.convert_picked_to_curves();
            }
            muted(ui, "Los trazos libres se vuelven curvas con nodos y tiradores; la forma cambia muy poco y conservan su grosor, color y presión.");
        }
    }

    /// Pasa los trazos elegidos a curvas Bézier (ajustando una curva a sus puntos).
    fn convert_picked_to_curves(&mut self) {
        let n = self.edit_picked(true, |s| {
            if s.nodes.is_none() && s.points.len() >= 2 {
                let ns = bezier::fit(&s.points, &s.pressure, s.closed, 1.5, s.width);
                if ns.len() >= 2 {
                    s.nodes = Some(ns);
                    s.regen();
                }
            }
        });
        self.sel_node = None;
        self.say(format!("{n} trazos convertidos en curvas editables"));
    }
}

/// Dibuja nodos y tiradores. `sel`: nodo elegido (sus tiradores y los de sus vecinos se muestran); `all`: mostrar todos.
fn paint_nodes(p: &egui::Painter, nodes: &[Node], map: &impl Fn([f32; 2]) -> Pos2, sel: Option<usize>, closed: bool, all: bool) {
    let n = nodes.len();
    let show = |i: usize| -> bool {
        all || sel.map_or(false, |s| {
            i == s || (i + 1) % n == s && (closed || i + 1 < n) || (s + 1) % n == i && (closed || s + 1 < n)
        })
    };
    for (i, nd) in nodes.iter().enumerate() {
        if !show(i) {
            continue;
        }
        let a = map(nd.p);
        for (has, h) in [(nd.has_in(), nd.hin), (nd.has_out(), nd.hout)] {
            if has {
                let hp = map(h);
                p.line_segment([a, hp], stk(1.0, SEL_COL));
                p.circle_filled(hp, 3.2, Color32::WHITE);
                p.circle_stroke(hp, 3.2, stk(1.0, SEL_COL));
            }
        }
    }
    for (i, nd) in nodes.iter().enumerate() {
        let r = Rect::from_center_size(map(nd.p), Vec2::splat(NODE_PX));
        let on = sel == Some(i);
        if nd.smooth {
            p.circle_filled(r.center(), NODE_PX * 0.5, if on { SEL_COL } else { Color32::WHITE });
            p.circle_stroke(r.center(), NODE_PX * 0.5, stk(1.2, ACCENT));
        } else {
            p.rect_filled(r, 1.0, if on { SEL_COL } else { Color32::WHITE });
            p.rect_stroke(r, 1.0, stk(1.2, ACCENT), EStrokeKind::Inside);
        }
    }
}
