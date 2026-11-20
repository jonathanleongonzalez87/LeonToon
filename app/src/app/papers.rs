//! Papeles de estudio en la Mesa de luz (Issue #52): mosaico con tooltip, escala configurable y dibujo del grano.
//! El grano es una ayuda de la vista: no se exporta, no sale en las miniaturas y se oculta con «Ver transparencia».

use super::shell::muted;
use super::*;
use crate::paper::{self, Grain, GrainLayer, Paper, GRAIN_LAYERS, PAPERS, TILE};

/// Cuadro texturizado que cubre la hoja `cam`, con la textura repetida, girada y desplazada según la capa.
fn grain_quad(painter: &egui::Painter, tex: egui::TextureId, cam: Rect, tile_px: f32, l: &GrainLayer, alpha: f32) {
    let size = tile_px * l.size;
    let (s, c) = l.angle.to_radians().sin_cos();
    let uv = |p: Pos2| {
        let (x, y) = ((p.x - cam.min.x) / size, (p.y - cam.min.y) / size);
        Pos2::new(c * x - s * y + l.offset[0], s * x + c * y + l.offset[1])
    };
    let col = Color32::from_white_alpha((alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
    let mut mesh = egui::Mesh::with_texture(tex);
    for p in [cam.left_top(), cam.right_top(), cam.right_bottom(), cam.left_bottom()] {
        mesh.vertices.push(egui::epaint::Vertex { pos: p, uv: uv(p), color: col });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    painter.add(Shape::mesh(mesh));
}

impl LeonApp {
    /// El papel de estudio activo (el que tiene el color de la hoja), si lo hay.
    pub(super) fn active_paper(&self) -> Option<&'static Paper> {
        paper::for_background(self.project.background)
    }

    /// Genera y sube la textura de ese grano (una vez por semilla; 1 MB).
    fn ensure_grain(&mut self, ctx: &egui::Context, g: Grain) {
        if self.paper_tex.contains_key(&g) {
            return;
        }
        let img = egui::ColorImage::from_rgba_premultiplied([TILE, TILE], &paper::grain_image(g, self.paper_seed));
        let opts = egui::TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..egui::TextureOptions::LINEAR };
        self.paper_tex.insert(g, ctx.load_texture(format!("grano_{g:?}_{}", self.paper_seed), img, opts));
    }

    /// Prepara la textura del papel activo antes de dibujar el visor.
    pub(super) fn ensure_active_paper_texture(&mut self, ctx: &egui::Context) {
        if let Some(p) = self.active_paper() {
            self.ensure_grain(ctx, p.grain);
        }
    }

    /// Dibuja el grano del papel sobre la hoja `cam`: tres capas de la misma textura, con tamaños que no son múltiplos,
    /// giradas y desplazadas, para que no se note la repetición. La fuerza la dicta la luz de la mesa; el tamaño, la escala elegida.
    pub(super) fn paint_paper_grain(&self, painter: &egui::Painter, cam: Rect, z: f32) {
        if self.show_alpha {
            return;
        }
        let Some(p) = self.active_paper() else { return };
        let Some(tex) = self.paper_tex.get(&p.grain) else { return };
        let texel = paper::TEXEL * z * (self.paper_scale / 100.0);
        let fade = paper::grain_fade(texel);
        if fade <= 0.0 {
            return;
        }
        let tile_px = (TILE as f32 * texel).min(8192.0);
        let base = p.grain_alpha(self.light_amt / 100.0) * fade * 1.25 / paper::layers_norm();
        for l in &GRAIN_LAYERS {
            grain_quad(painter, tex.id(), cam, tile_px, l, base * l.weight);
        }
    }

    /// Sección «Papeles de estudio» del panel de la Mesa de luz.
    pub(super) fn paper_swatches_ui(&mut self, ui: &mut egui::Ui) {
        // Las texturas de las muestras se generan de una en una por fotograma (cada una tarda unas decenas de ms),
        // salvo en las capturas de pantalla, que las quieren todas de golpe.
        let all_now = self.shot.is_some();
        for g in Grain::ALL {
            if !self.paper_tex.contains_key(&g) {
                self.ensure_grain(ui.ctx(), g);
                if !all_now {
                    ui.ctx().request_repaint();
                    break;
                }
            }
        }
        ui.label(RichText::new("Papeles de estudio").strong().color(TX));
        let active = self.active_paper().map(|p| p.id);
        let light = self.light_amt / 100.0;
        let mut pick: Option<&'static Paper> = None;
        for row in PAPERS.chunks(5) {
            ui.horizontal(|ui| {
                for p in row {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(42.0, 32.0), Sense::click());
                    ui.painter().rect_filled(r, 4.0, Color32::from_rgb(p.rgb[0], p.rgb[1], p.rgb[2]));
                    // Muestra sutil de su fibra (más visible que sobre la hoja, para distinguir los papeles).
                    if let Some(tex) = self.paper_tex.get(&p.grain) {
                        let tile = 256.0;
                        let uv = Rect::from_min_size(Pos2::new(0.31, 0.47), Vec2::new(r.width() / tile, r.height() / tile));
                        let k = (p.grain_alpha(light) * 1.4).clamp(0.12, 0.6);
                        ui.painter().with_clip_rect(r).image(tex.id(), r, uv, Color32::from_white_alpha((k * 255.0).round() as u8));
                    }
                    let on = active == Some(p.id);
                    ui.painter().rect_stroke(r, 4.0, stk(if on { 2.0 } else { 1.0 }, if on { ACCENT } else { LINE_STRONG }), EStrokeKind::Outside);
                    if resp.on_hover_text(p.tooltip()).clicked() {
                        pick = Some(p);
                    }
                }
            });
        }
        if let Some(p) = pick {
            self.project.background = [p.rgb[0], p.rgb[1], p.rgb[2], 255];
            self.say(format!("Papel de estudio: {}", p.name));
        }
        match self.active_paper() {
            Some(p) => muted(ui, &format!("{} · {}", p.name, p.era)),
            None => muted(ui, "Sin papel de estudio. Elige uno para darle fibra a la hoja."),
        }
        egui::Grid::new("paper_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label(RichText::new("Escala del grano").color(MU));
            ui.add(egui::Slider::new(&mut self.paper_scale, paper::SCALE_MIN..=paper::SCALE_MAX).logarithmic(true).max_decimals(0).suffix(" %"));
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if ui.small_button("Variar").on_hover_text("Genera otra variante del grano: mismo papel, otro dibujo de fibra").clicked() {
                self.paper_seed = self.paper_seed.wrapping_add(1);
                self.paper_tex.clear();
            }
            if ui.small_button("Escala 100 %").clicked() {
                self.paper_scale = paper::SCALE_DEFAULT;
            }
        });
        muted(ui, "El color de la hoja sigue la regla de siempre. El grano solo es una ayuda de la vista: más luz, más fibra. No se exporta.");
    }
}
