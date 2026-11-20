//! Controles flotantes de la mesa de luz (prototipo v0.3): zoom y 2D/3D a la izquierda;
//! Mesa de luz, Guías y Papel cebolla a la derecha (cada uno abre su ventanita); y la barra
//! lateral N pegada al borde derecho.

use super::shell::GUIDE_NAMES;
use super::*;

const N_W: f32 = 236.0;

/// Papeles de la mesa de luz (prototipo v0.3): blanco, crema, arena y gris.
const PAPERS: [(&str, [u8; 3]); 4] = [("Blanco", [0xfb, 0xfa, 0xf7]), ("Crema", [0xf6, 0xef, 0xe2]), ("Arena", [0xec, 0xe2, 0xd2]), ("Gris", [0xd9, 0xd6, 0xd2])];

pub(super) fn pill_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(Color32::from_rgba_unmultiplied(26, 25, 24, 225))
        .stroke(stk(1.0, LINE_STRONG))
        .corner_radius(14.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
}

fn pop_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(BG2)
        .stroke(stk(1.0, LINE_STRONG))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::same(10))
        .shadow(egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(140) })
}

impl LeonApp {
    pub(super) fn viewport_hud(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let n_t = panel_t(ui.ctx(), "nside", self.show_n);
        let n_w = N_W * n_t;
        // ---- Izquierda: zoom y 2D/3D ----
        let band_l = Rect::from_min_size(rect.min + Vec2::new(8.0, 8.0), Vec2::new(420.0, 30.0));
        let mut cl = ui.new_child(egui::UiBuilder::new().max_rect(band_l).layout(egui::Layout::left_to_right(egui::Align::Center)));
        cl.spacing_mut().item_spacing.x = 6.0;
        pill_frame().show(&mut cl, |ui| {
            ui.spacing_mut().slider_width = 70.0;
            let (ir, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
            draw_tool_icon(Tool::Zoom, ui.painter(), ir, MU);
            let mut pct = (self.zoom * 100.0).clamp(10.0, 400.0);
            if ui.add(egui::Slider::new(&mut pct, 10.0..=400.0).show_value(false)).changed() {
                let f = pct / 100.0 / self.zoom;
                self.zoom_by(f);
            }
            ui.label(RichText::new(format!("{:.0} %", self.zoom * 100.0)).color(TX).size(12.0));
            if ui.add(egui::Button::new(RichText::new("Ajustar").size(12.0).color(MU)).frame(false)).clicked() {
                self.need_fit = true;
            }
        });
        pill_frame().show(&mut cl, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (lbl, is3d) in [("2D", false), ("3D", true)] {
                let on = self.view3d == is3d;
                let b = egui::Button::new(RichText::new(lbl).size(12.0).color(if on { ACCENT_INK } else { MU }))
                    .fill(if on { ACCENT } else { Color32::TRANSPARENT })
                    .corner_radius(10.0);
                if ui.add(b).clicked() && !on {
                    self.toggle_view3d();
                }
            }
        });

        // ---- Derecha: Mesa de luz, Guías, Papel cebolla ----
        let band_r = Rect::from_min_max(Pos2::new(rect.max.x - n_w - 470.0, rect.min.y + 8.0), Pos2::new(rect.max.x - n_w - 8.0, rect.min.y + 38.0));
        let mut cr = ui.new_child(egui::UiBuilder::new().max_rect(band_r).layout(egui::Layout::right_to_left(egui::Align::Center)));
        cr.spacing_mut().item_spacing.x = 6.0;
        let mut anchors = [Rect::NOTHING; 3];
        for (i, (lbl, id)) in [("Papel cebolla", 3u8), ("Guías", 2), ("Mesa de luz", 1)].into_iter().enumerate() {
            let on = self.hud_pop == id || (id == 3 && self.onion) || (id == 2 && self.guides.iter().any(|g| *g));
            let r = pill_frame().show(&mut cr, |ui| {
                let col = if on { TX } else { MU };
                let g = ui.painter().layout_no_wrap(lbl.to_string(), FontId::proportional(12.0), col);
                let (rr, resp) = ui.allocate_exact_size(Vec2::new(16.0 + 6.0 + g.size().x + 6.0 + 8.0, 20.0), Sense::click());
                draw_hud_icon(id - 1, ui.painter(), Pos2::new(rr.min.x + 8.0, rr.center().y), col);
                ui.painter().galley(Pos2::new(rr.min.x + 22.0, rr.center().y - g.size().y * 0.5), g.clone(), col);
                draw_chevron(ui.painter(), Pos2::new(rr.max.x - 4.0, rr.center().y), INK3);
                resp
            });
            if r.inner.clicked() {
                self.hud_pop = if self.hud_pop == id { 0 } else { id };
            }
            anchors[2 - i] = r.response.rect;
        }
        if self.hud_pop != 0 {
            let a = anchors[(self.hud_pop - 1) as usize];
            let pos = Pos2::new((a.max.x - 270.0).max(rect.min.x + 4.0), a.max.y + 6.0);
            let ctx = ui.ctx().clone();
            let inner = egui::Area::new(egui::Id::new("hud_pop")).order(egui::Order::Foreground).fixed_pos(pos).show(&ctx, |ui| {
                pop_frame().show(ui, |ui| {
                    ui.set_width(250.0);
                    match self.hud_pop {
                        1 => self.pop_light(ui),
                        2 => self.pop_guides(ui),
                        _ => self.pop_onion(ui),
                    }
                });
            });
            if ctx.input(|i| i.pointer.any_pressed()) {
                if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !inner.response.rect.contains(p) && !anchors.iter().any(|r| r.contains(p)) && !ctx.memory(|m| m.any_popup_open()) {
                        self.hud_pop = 0;
                    }
                }
            }
        }

        // ---- Barra lateral N ----
        // Entra y sale deslizándose desde el borde derecho (translateX del prototipo).
        if n_t > 0.01 {
            let nr = Rect::from_min_max(Pos2::new(rect.max.x - n_w, rect.min.y), Pos2::new(rect.max.x - n_w + N_W, rect.max.y));
            let clip = rect.intersect(ui.clip_rect());
            let pn = ui.painter().with_clip_rect(clip);
            pn.rect_filled(nr, egui::CornerRadius { nw: 0, ne: 6, sw: 0, se: 6 }, Color32::from_rgba_unmultiplied(33, 31, 30, 248));
            pn.vline(nr.min.x, nr.y_range(), stk(1.0, LINE_STRONG));
            let mut cn = ui.new_child(egui::UiBuilder::new().max_rect(nr.shrink(8.0)).layout(egui::Layout::top_down(egui::Align::Min)));
            cn.set_clip_rect(clip);
            // Bloquea el dibujo bajo la barra
            let _ = cn.interact(nr.intersect(clip), cn.id().with("n_block"), Sense::click_and_drag());
            egui::ScrollArea::vertical().id_salt("nside").show(&mut cn, |ui| self.n_sidebar(ui));
        }
    }

    fn pop_light(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Mesa de luz").strong().color(TX));
        egui::Grid::new("light_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label(RichText::new("Luz").color(MU));
            ui.add(egui::Slider::new(&mut self.light_amt, 0.0..=100.0).max_decimals(0).suffix(" %"));
            ui.end_row();
            ui.label(RichText::new("Temperatura").color(MU));
            let tname = if self.light_temp < 35.0 { "Cálida" } else if self.light_temp < 65.0 { "Neutra" } else { "Fría" };
            ui.add(egui::Slider::new(&mut self.light_temp, 0.0..=100.0).show_value(false));
            ui.end_row();
            ui.label("");
            ui.label(RichText::new(tname).size(11.0).color(INK3));
            ui.end_row();
        });
        ui.label(RichText::new("Papel").size(11.5).color(MU));
        ui.horizontal(|ui| {
            for (n, c) in PAPERS {
                let col = Color32::from_rgb(c[0], c[1], c[2]);
                let (r, resp) = ui.allocate_exact_size(Vec2::new(40.0, 20.0), Sense::click());
                ui.painter().rect_filled(r, 4.0, col);
                let on = self.project.background == [c[0], c[1], c[2], 255];
                ui.painter().rect_stroke(r, 4.0, stk(if on { 2.0 } else { 1.0 }, if on { ACCENT } else { LINE_STRONG }), EStrokeKind::Outside);
                if resp.on_hover_text(format!("Papel {n} (color de la hoja y del borrador)")).clicked() {
                    self.project.background = [c[0], c[1], c[2], 255];
                }
            }
        });
        ui.add_space(4.0);
        self.paper_swatches_ui(ui);
        ui.add_space(4.0);
        egui::Grid::new("light_grid2").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label(RichText::new("Fondo").color(MU));
            ui.add(egui::Slider::new(&mut self.light_gray, 0.0..=100.0).step_by(5.0).max_decimals(0));
            ui.end_row();
        });
        ui.label(RichText::new("0 es negro, 100 es blanco").size(11.0).color(INK3));
        ui.separator();
        ui.checkbox(&mut self.show_alpha, "Ver transparencia (cuadrícula)");
        egui::Grid::new("light_grid3").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label(RichText::new("Cuadros").color(MU));
            ui.add(egui::Slider::new(&mut self.chk_size, 6.0..=60.0).max_decimals(0).suffix(" px"));
            ui.end_row();
            ui.label(RichText::new("Contraste").color(MU));
            ui.add(egui::Slider::new(&mut self.chk_con, 4.0..=60.0).max_decimals(0).suffix(" %"));
            ui.end_row();
            ui.label(RichText::new("Opacidad").color(MU));
            ui.add(egui::Slider::new(&mut self.view_op, 10.0..=100.0).step_by(5.0).max_decimals(0).suffix(" %"));
            ui.end_row();
        });
        ui.separator();
        ui.checkbox(&mut self.pegbar, "Barra de registro (pegbar)");
    }

    fn pop_guides(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Superposición de guías").size(11.5).color(MU));
        for (i, n) in GUIDE_NAMES.iter().enumerate() {
            ui.checkbox(&mut self.guides[i], *n);
        }
    }

    fn pop_onion(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.onion, RichText::new("Activar papel cebolla").strong());
        ui.label(RichText::new("Alcance").size(11.5).color(MU));
        egui::Grid::new("onion_hud").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label(RichText::new("Atrás").color(ONION_BEFORE));
            ui.add(egui::Slider::new(&mut self.onion_before, 0..=ONION_UI_MAX));
            ui.end_row();
            ui.label(RichText::new("Adelante").color(ONION_AFTER));
            ui.add(egui::Slider::new(&mut self.onion_after, 0..=ONION_UI_MAX));
            ui.end_row();
            ui.label("Opacidad");
            let mut o = self.onion_opacity * 100.0;
            if ui.add(egui::Slider::new(&mut o, 10.0..=80.0).max_decimals(0).suffix(" %")).changed() {
                self.onion_opacity = o / 100.0;
            }
            ui.end_row();
            ui.label("Caída");
            let names = ["Como el original", "Rápida", "Ninguna"];
            egui::ComboBox::from_id_salt("falloff_hud").selected_text(names[self.onion_falloff as usize]).show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut self.onion_falloff, i as u8, *n);
                }
            });
            ui.end_row();
        });
        ui.label(
            RichText::new("Colores: anterior en ámbar, siguiente en azul. También se ajusta arrastrando los extremos de color en la barra de cuadros.")
                .size(11.0)
                .color(INK3),
        );
    }
}
