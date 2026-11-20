//! Papeles de estudio en la Mesa de luz (Issue #52): mosaico con tooltip y dibujo del grano sobre la hoja.
//! El grano es una ayuda de la vista: no se exporta, no sale en las miniaturas y se oculta con «Ver transparencia».

use super::shell::muted;
use super::*;
use crate::paper::{self, Grain, Paper, PAPERS, TILE};

impl LeonApp {
    /// El papel de estudio activo (el que tiene el color de la hoja), si lo hay.
    pub(super) fn active_paper(&self) -> Option<&'static Paper> {
        paper::for_background(self.project.background)
    }

    /// Sube a la tarjeta gráfica la textura de ese grano (una vez; 256 KB).
    fn ensure_grain(&mut self, ctx: &egui::Context, g: Grain) {
        if self.paper_tex.contains_key(&g) {
            return;
        }
        let img = egui::ColorImage::from_rgba_premultiplied([TILE, TILE], &paper::grain_image(g));
        let opts = egui::TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..egui::TextureOptions::LINEAR };
        self.paper_tex.insert(g, ctx.load_texture(format!("grano_{g:?}"), img, opts));
    }

    /// Prepara la textura del papel activo antes de dibujar el visor.
    pub(super) fn ensure_active_paper_texture(&mut self, ctx: &egui::Context) {
        if let Some(p) = self.active_paper() {
            self.ensure_grain(ctx, p.grain);
        }
    }

    /// Dibuja el grano del papel sobre la hoja `cam`, con la fuerza que dicta la luz de la mesa. Un solo cuadro con textura repetida.
    pub(super) fn paint_paper_grain(&self, painter: &egui::Painter, cam: Rect, z: f32) {
        if self.show_alpha {
            return;
        }
        let Some(p) = self.active_paper() else { return };
        let Some(tex) = self.paper_tex.get(&p.grain) else { return };
        // La fibra tiene tamaño físico: crece con el zoom. Muy alejado se funde (evita el moaré).
        let tile = (TILE as f32 * z * 0.75).min(1024.0);
        if tile < 20.0 {
            return;
        }
        let fade = ((tile - 20.0) / 40.0).clamp(0.0, 1.0);
        let k = p.grain_alpha(self.light_amt / 100.0) * fade;
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(cam.width() / tile, cam.height() / tile));
        painter.image(tex.id(), cam, uv, Color32::from_white_alpha((k * 255.0).round() as u8));
    }

    /// Sección «Papeles de estudio» del panel de la Mesa de luz.
    pub(super) fn paper_swatches_ui(&mut self, ui: &mut egui::Ui) {
        for g in Grain::ALL {
            self.ensure_grain(ui.ctx(), g);
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
                        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(r.width() / 64.0, r.height() / 64.0));
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
        muted(ui, "El color de la hoja sigue la regla de siempre. El grano solo es una ayuda de la vista: más luz, más fibra. No se exporta.");
    }
}
