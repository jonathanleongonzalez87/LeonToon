//! Monitor «VU de audio»: dos medidores de aguja (izquierdo y derecho) como los VU clásicos, con luz cálida de fondo como la mesa de luz
//! y un piloto de recorte por canal: apagado con silencio, verde con sonido, amarillo cerca del tope (−3 a 0 dBFS) y rojo al pasarse
//! (el rojo se retiene unos segundos; un clic lo borra). Mide la mezcla final que suena, no las capas sueltas.

use super::*;
use crate::audio::Lamp;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, StrokeKind, Vec2};

/// Hasta dónde gira la aguja a cada lado de la vertical, en grados.
const SWEEP: f32 = 42.0;
/// Las marcas con número de la escala, en unidades VU.
const MAJORS: [f32; 7] = [-20.0, -10.0, -7.0, -5.0, -3.0, 0.0, 3.0];

/// El ángulo de la aguja (radianes desde la vertical, positivo hacia la derecha) para una lectura en unidades VU (−20 a +3).
pub(super) fn needle_angle(vu: f32) -> f32 {
    let f = ((vu + 20.0) / 23.0).clamp(0.0, 1.0);
    (-SWEEP + f * 2.0 * SWEEP).to_radians()
}

fn lamp_color(l: Lamp) -> Color32 {
    match l {
        Lamp::Off => Color32::from_rgb(0x3a, 0x2c, 0x22),
        Lamp::Green => Color32::from_rgb(0x4c, 0xd9, 0x64),
        Lamp::Yellow => Color32::from_rgb(0xff, 0xcc, 0x33),
        Lamp::Red => Color32::from_rgb(0xff, 0x3b, 0x30),
    }
}

fn dbfs(lin: f32) -> String {
    if lin <= 1e-5 {
        "−∞".into()
    } else {
        format!("{:+.1}", 20.0 * lin.log10()).replace('-', "−")
    }
}

/// Un punto del arco de la escala: `r` desde el pivote con el ángulo `a` desde la vertical.
fn at(pivot: Pos2, r: f32, a: f32) -> Pos2 {
    Pos2::new(pivot.x + a.sin() * r, pivot.y - a.cos() * r)
}

fn paint_face(p: &egui::Painter, f: Rect, m: &crate::audio::Meter, ch: usize) {
    let h = f.height();
    // El papel de la escala: crema con la luz cálida que entra por arriba.
    p.rect_filled(f, 4.0, Color32::from_rgb(0xea, 0xdb, 0xa8));
    let glow = Rect::from_min_max(f.min, Pos2::new(f.max.x, f.min.y + h * 0.35));
    p.rect_filled(glow, 4.0, Color32::from_rgba_unmultiplied(0xff, 0xa8, 0x30, 70));
    let shade = Rect::from_min_max(Pos2::new(f.min.x, f.max.y - h * 0.22), f.max);
    p.rect_filled(shade, 4.0, Color32::from_rgba_unmultiplied(0x60, 0x40, 0x10, 40));
    p.rect_stroke(f, 4.0, stk(1.5, Color32::from_rgb(0x2a, 0x2a, 0x2e)), StrokeKind::Outside);

    let clipped = p.with_clip_rect(f);
    let pivot = Pos2::new(f.center().x, f.max.y + h * 0.18);
    let r = h * 0.80;
    let ink = Color32::from_rgb(0x24, 0x20, 0x1c);
    let red = Color32::from_rgb(0xc8, 0x2a, 0x22);
    // Arco: negro hasta 0 VU y rojo grueso de 0 a +3.
    let arc = |from: f32, to: f32, rr: f32| -> Vec<Pos2> { (0..=24).map(|i| at(pivot, rr, needle_angle(from + (to - from) * i as f32 / 24.0))).collect() };
    clipped.add(Shape::line(arc(-20.0, 0.0, r), stk(1.6, ink)));
    clipped.add(Shape::line(arc(0.0, 3.0, r), stk(1.6, red)));
    clipped.add(Shape::line(arc(0.0, 3.0, r - h * 0.035), stk(h * 0.07, red)));
    // Marcas pequeñas cada unidad y marcas con número.
    for v in -20..=3 {
        let v = v as f32;
        if v > -10.0 && v < 0.0 || v >= 0.0 || MAJORS.contains(&v) {
            let a = needle_angle(v);
            let major = MAJORS.contains(&v);
            let len = if major { h * 0.07 } else { h * 0.035 };
            clipped.line_segment([at(pivot, r, a), at(pivot, r + len, a)], stk(if major { 1.4 } else { 1.0 }, if v >= 0.0 { red } else { ink }));
        }
    }
    for v in MAJORS {
        let a = needle_angle(v);
        let txt = if v > 0.0 { format!("+{}", v as i32) } else { format!("{}", v as i32).replace('-', "−") };
        clipped.text(at(pivot, r + h * 0.16, a), Align2::CENTER_CENTER, txt, FontId::proportional((h * 0.1).clamp(8.0, 15.0)), if v >= 0.0 { red } else { ink });
    }
    clipped.text(Pos2::new(f.center().x, f.min.y + h * 0.6), Align2::CENTER_CENTER, "VU", FontId::proportional((h * 0.13).clamp(9.0, 18.0)), ink);
    clipped.text(Pos2::new(f.min.x + h * 0.1, f.max.y - h * 0.1), Align2::LEFT_CENTER, if ch == 0 { "L" } else { "R" }, FontId::proportional((h * 0.12).clamp(9.0, 16.0)), ink);

    // La aguja, con una sombra suave.
    let a = needle_angle(m.needle_vu(ch));
    let tip = at(pivot, r * 1.02, a);
    clipped.line_segment([pivot + Vec2::new(2.0, 1.5), tip + Vec2::new(2.0, 1.5)], stk(1.6, Color32::from_rgba_unmultiplied(0, 0, 0, 50)));
    clipped.line_segment([pivot, tip], stk(1.6, Color32::from_rgb(0x14, 0x12, 0x10)));

    // El piloto de recorte, arriba a la derecha, con halo cuando está encendido.
    let lamp = m.lamp(ch);
    let c = Pos2::new(f.max.x - h * 0.11, f.min.y + h * 0.12);
    let lr = (h * 0.055).max(4.0);
    let col = lamp_color(lamp);
    if lamp != Lamp::Off {
        p.circle_filled(c, lr * 2.4, col.gamma_multiply(0.22));
        p.circle_filled(c, lr * 1.6, col.gamma_multiply(0.35));
    }
    p.circle_filled(c, lr, col);
    p.circle_stroke(c, lr, stk(1.0, Color32::from_rgb(0x20, 0x20, 0x20)));
    if lamp != Lamp::Off {
        p.circle_filled(c - Vec2::splat(lr * 0.3), lr * 0.3, Color32::from_rgba_unmultiplied(255, 255, 255, 140));
    }
}

impl LeonApp {
    /// El monitor de audio: dos medidores de aguja con pilotos de recorte sobre la mezcla que suena.
    pub(super) fn audio_vu_editor(&mut self, ui: &mut egui::Ui) {
        self.audio_meter_tick(ui.ctx());
        let m = self.audio.meter;
        let avail = ui.available_rect_before_wrap();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail.width(), avail.height().max(80.0)), Sense::click());
        let p = ui.painter_at(rect);
        // La carcasa metálica y la luz cálida, como la mesa de luz.
        p.rect_filled(rect, 6.0, Color32::from_rgb(0x4a, 0x4c, 0x52));
        let inner = rect.shrink(8.0);
        p.rect_filled(inner, 4.0, Color32::from_rgb(0x2b, 0x2c, 0x30));
        let text_h = 16.0;
        let gap = 10.0;
        let area = inner.shrink(8.0);
        // Cada cara tiene proporción 3:2 y una línea de texto debajo. Lado a lado o apilados, según lo que dé caras más grandes
        // (un hueco estrecho y alto apila los dos medidores).
        let side_fw = ((area.width() - gap) / 2.0).min((area.height() - text_h) * 1.5).max(40.0);
        let stack_fw = area.width().min(((area.height() - gap) / 2.0 - text_h) * 1.5).max(40.0);
        let stacked = stack_fw > side_fw;
        let fw = if stacked { stack_fw } else { side_fw };
        let fh = fw / 1.5;
        let (cols, rows) = if stacked { (1.0, 2.0) } else { (2.0, 1.0) };
        let total_w = fw * cols + gap * (cols - 1.0);
        let total_h = (fh + text_h) * rows + gap * (rows - 1.0);
        let x0 = area.center().x - total_w / 2.0;
        let y0 = area.center().y - total_h / 2.0;
        for ch in 0..2 {
            let (cx, cy) = if stacked { (0.0, ch as f32) } else { (ch as f32, 0.0) };
            let f = Rect::from_min_size(Pos2::new(x0 + cx * (fw + gap), y0 + cy * (fh + text_h + gap)), Vec2::new(fw, fh));
            paint_face(&p, f, &m, ch);
            p.text(
                Pos2::new(f.center().x, f.max.y + 9.0),
                Align2::CENTER_CENTER,
                format!("pico {} dBFS", dbfs(m.peak[ch])),
                FontId::proportional(10.5),
                Color32::from_rgb(0xb8, 0xb0, 0xa0),
            );
        }
        if resp.on_hover_text("Mide la mezcla final que suena (0 VU = −12 dBFS). Clic: borrar el rojo retenido").clicked() {
            self.audio.meter.clear_clip();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_aguja_barre_de_izquierda_a_derecha_y_no_se_sale_de_la_escala() {
        assert!((needle_angle(-20.0).to_degrees() + SWEEP).abs() < 1e-3);
        assert!((needle_angle(3.0).to_degrees() - SWEEP).abs() < 1e-3);
        assert!(needle_angle(-10.0) < needle_angle(0.0));
        assert_eq!(needle_angle(-99.0), needle_angle(-20.0));
        assert_eq!(needle_angle(40.0), needle_angle(3.0));
    }

    #[test]
    fn el_monitor_se_dibuja_en_reposo_y_con_senal_sin_fallar() {
        let ctx = egui::Context::default();
        let mut m = crate::audio::Meter::default();
        for level in [0.0f32, 0.2, 0.9, 1.5] {
            m.peak = [level; 2];
            m.vu = [level * 0.5; 2];
            if level > 1.0 {
                m.clip = [2.0; 2];
            }
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let p = ui.painter().clone();
                    paint_face(&p, Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(300.0, 200.0)), &m, 0);
                });
            });
        }
    }
}
