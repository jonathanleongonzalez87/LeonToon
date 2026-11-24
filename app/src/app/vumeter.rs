//! Monitor «VU de audio»: dos medidores de aguja (izquierdo y derecho) como los VU clásicos, con luz cálida de fondo como la mesa de luz
//! y un piloto de recorte por canal: apagado con silencio, verde con sonido, amarillo cerca del tope (−3 a 0 dBFS) y rojo al pasarse
//! (el rojo se retiene unos segundos; un clic lo borra). Mide la mezcla final que suena, no las capas sueltas.

use super::*;
use crate::audio::Lamp;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, StrokeKind, Vec2};

/// Hasta dónde gira la aguja a cada lado de la vertical, en grados.
const SWEEP: f32 = 55.0;
/// El rango de la escala en dBFS: de −21 (reposo) a +3 (más allá del tope).
const DB_MIN: f32 = -21.0;
const DB_MAX: f32 = 3.0;
/// Las marcas con número de la escala, en dBFS.
const MAJORS: [f32; 9] = [-21.0, -18.0, -15.0, -12.0, -9.0, -6.0, -3.0, 0.0, 3.0];

/// El ángulo de la aguja (radianes desde la vertical, positivo hacia la derecha) para una lectura en dBFS.
pub(super) fn needle_angle(db: f32) -> f32 {
    let f = ((db - DB_MIN) / (DB_MAX - DB_MIN)).clamp(0.0, 1.0);
    (-SWEEP + f * 2.0 * SWEEP).to_radians()
}

/// Color de una zona de la escala: neutro hasta −3, ámbar de −3 a 0 y rojo por encima de 0 dBFS.
fn zone(db: f32, ink: Color32) -> Color32 {
    if db > 0.001 {
        Color32::from_rgb(0xe8, 0x4a, 0x3c)
    } else if db >= -3.001 {
        Color32::from_rgb(0xff, 0xb4, 0x54)
    } else {
        ink
    }
}

/// El nivel en dBFS que corresponde a un porcentaje de la escala completa (100 % = 0 dBFS).
fn percent_db(p: f32) -> f32 {
    20.0 * (p / 100.0).log10()
}
fn lamp_color(l: Lamp) -> Color32 {
    match l {
        Lamp::Off => Color32::from_rgb(0x34, 0x2f, 0x2a),
        Lamp::Green => Color32::from_rgb(0x58, 0xd6, 0x6a),
        Lamp::Yellow => Color32::from_rgb(0xff, 0xcc, 0x33),
        Lamp::Red => Color32::from_rgb(0xff, 0x3b, 0x30),
    }
}

/// Un punto del arco de la escala: `r` desde el pivote con el ángulo `a` desde la vertical.
fn at(pivot: Pos2, r: f32, a: f32) -> Pos2 {
    Pos2::new(pivot.x + a.sin() * r, pivot.y - a.cos() * r)
}

/// Un medidor: cara oscura con la luz cálida de un foco que cae desde arriba, escala en dBFS (de −21 a +3, con las zonas de −3 a 0 en
/// ámbar y de 0 a +3 en rojo), una escala en porcentaje debajo y el piloto de recorte, que se enciende según la intensidad.
fn paint_face(p: &egui::Painter, f: Rect, m: &crate::audio::Meter, ch: usize) {
    let h = f.height();
    p.rect_filled(f, 5.0, Color32::from_rgb(0x1a, 0x19, 0x18));
    let clipped = p.with_clip_rect(f);
    let pivot = Pos2::new(f.center().x, f.max.y - h * 0.08);
    let db = m.needle_db(ch);
    let level = ((db - DB_MIN) / (DB_MAX - DB_MIN)).clamp(0.0, 1.0);
    let warm = Color32::from_rgb(0xff, 0xa8, 0x40);
    let strength = 0.45 + 0.35 * level;
    let band = Rect::from_min_max(f.min, Pos2::new(f.max.x, f.min.y + h * 0.6));
    let mut mesh = egui::Mesh::default();
    let (top, none) = (with_alpha(warm, 0.38 * strength), with_alpha(warm, 0.0));
    mesh.colored_vertex(band.left_top(), top);
    mesh.colored_vertex(band.right_top(), top);
    mesh.colored_vertex(band.right_bottom(), none);
    mesh.colored_vertex(band.left_bottom(), none);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    clipped.add(Shape::mesh(mesh));
    super::canvas::glow(&clipped, Pos2::new(f.center().x, f.min.y), h * 0.05, h * 0.9, with_alpha(warm, 0.22 * strength));
    p.rect_stroke(f, 5.0, stk(1.0, Color32::from_rgb(0x3a, 0x38, 0x35)), StrokeKind::Inside);

    let r = h * 0.68;
    let ink = Color32::from_rgb(0xd8, 0xd2, 0xc4);
    let arc = |from: f32, to: f32, rr: f32| -> Vec<Pos2> { (0..=24).map(|i| at(pivot, rr, needle_angle(from + (to - from) * i as f32 / 24.0))).collect() };
    // La escala en dBFS: tres tramos de color (neutro, ámbar, rojo).
    clipped.add(Shape::line(arc(DB_MIN, -3.0, r), stk(1.2, with_alpha(ink, 0.85))));
    clipped.add(Shape::line(arc(-3.0, 0.0, r), stk(2.0, zone(-1.0, ink))));
    clipped.add(Shape::line(arc(0.0, DB_MAX, r), stk(3.0, zone(1.0, ink))));
    // Una marca por dB (la escala es lineal en dB) y marcas largas con número cada 3 dB.
    for k in 0..=((DB_MAX - DB_MIN) as i32) {
        let v = DB_MIN + k as f32;
        let major = k % 3 == 0;
        let a = needle_angle(v);
        let len = if major { h * 0.07 } else { h * 0.035 };
        clipped.line_segment([at(pivot, r, a), at(pivot, r + len, a)], stk(if major { 1.3 } else { 0.9 }, zone(v, ink).gamma_multiply(if major { 0.95 } else { 0.75 })));
    }
    for v in MAJORS {
        let a = needle_angle(v);
        let txt = if v > 0.0 { format!("+{}", v as i32) } else { format!("{}", v as i32).replace('-', "−") };
        clipped.text(at(pivot, r + h * 0.15, a), Align2::CENTER_CENTER, txt, FontId::proportional((h * 0.085).clamp(7.0, 10.5)), zone(v, ink).gamma_multiply(0.95));
    }
    // La escala en porcentaje (100 % = 0 dBFS), más adentro: 10, 20 … 100 y 120.
    let r2 = r * 0.8;
    clipped.add(Shape::line(arc(percent_db(10.0), 0.0, r2), stk(0.9, with_alpha(ink, 0.6))));
    clipped.add(Shape::line(arc(0.0, percent_db(141.0), r2), stk(2.0, zone(1.0, ink))));
    for pc in [10.0f32, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0, 120.0, 140.0] {
        let d = percent_db(pc);
        let a = needle_angle(d);
        let big = pc == 10.0 || pc == 50.0 || pc == 100.0;
        let len = if big { h * 0.055 } else { h * 0.03 };
        clipped.line_segment([at(pivot, r2, a), at(pivot, r2 - len, a)], stk(if big { 1.2 } else { 0.8 }, zone(d, ink).gamma_multiply(0.8)));
        if big {
            let txt = if pc == 100.0 { "100%".to_string() } else { format!("{}", pc as i32) };
            clipped.text(at(pivot, r2 - h * 0.12, a), Align2::CENTER_CENTER, txt, FontId::proportional((h * 0.075).clamp(6.5, 9.0)), zone(d, ink).gamma_multiply(0.8));
        }
    }
    clipped.text(Pos2::new(f.center().x, f.min.y + h * 0.5), Align2::CENTER_CENTER, "dB", FontId::proportional((h * 0.085).clamp(7.0, 10.0)), with_alpha(ink, 0.6));
    clipped.text(Pos2::new(f.min.x + h * 0.09, f.min.y + h * 0.12), Align2::LEFT_CENTER, if ch == 0 { "L" } else { "R" }, FontId::proportional((h * 0.12).clamp(9.0, 13.0)), with_alpha(ink, 0.8));

    // La aguja: un trazo limpio, sin brillo.
    let tip = at(pivot, r * 1.04, needle_angle(db));
    clipped.line_segment([pivot, tip], stk(1.6, Color32::from_rgb(0xf0, 0xea, 0xdc)));
    clipped.circle_filled(pivot, h * 0.03, Color32::from_rgb(0xf0, 0xea, 0xdc));

    // El piloto de recorte: se enciende según la intensidad del pico (tenue con poco sonido, pleno cerca del tope) y su halo crece.
    let lamp = m.lamp(ch);
    let c = Pos2::new(f.max.x - h * 0.1, f.min.y + h * 0.12);
    let lr = (h * 0.045).max(3.5);
    let col = lamp_color(lamp);
    if lamp != Lamp::Off {
        let k = if lamp == Lamp::Green { 0.35 + 0.65 * m.lamp_intensity(ch) } else { 1.0 };
        p.circle_filled(c, lr * (1.8 + 1.2 * k), col.gamma_multiply(0.14 * k));
        p.circle_filled(c, lr * (1.3 + 0.5 * k), col.gamma_multiply(0.30 * k));
        p.circle_filled(c, lr, col.gamma_multiply(0.45 + 0.55 * k));
    } else {
        p.circle_filled(c, lr, col);
    }
    p.circle_stroke(c, lr, stk(1.0, Color32::from_black_alpha(160)));
}
impl LeonApp {
    /// El monitor de audio: dos medidores de aguja (izquierdo y derecho) lado a lado, en un panel pequeño y centrado.
    pub(super) fn audio_vu_editor(&mut self, ui: &mut egui::Ui) {
        self.audio_meter_tick(ui.ctx());
        let m = self.audio.meter;
        let avail = ui.available_rect_before_wrap();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail.width(), avail.height().max(60.0)), Sense::click());
        let p = ui.painter_at(rect);
        let pad = 8.0;
        let gap = 6.0;
        // Cada cara tiene proporción 3:2 y no pasa de cierto tamaño: es un monitor, no un panel de control.
        let fw = ((rect.width() - pad * 2.0 - gap) / 2.0).min((rect.height() - pad * 2.0) * 1.5).clamp(40.0, 150.0);
        let fh = fw / 1.5;
        let text_h = 15.0;
        let panel = Rect::from_center_size(rect.center(), Vec2::new(fw * 2.0 + gap + pad * 2.0, fh + text_h + pad * 2.0));
        p.rect_filled(panel, 7.0, Color32::from_rgb(0x2a, 0x29, 0x27));
        p.rect_stroke(panel, 7.0, stk(1.0, Color32::from_rgb(0x3e, 0x3c, 0x39)), StrokeKind::Inside);
        for ch in 0..2 {
            let f = Rect::from_min_size(Pos2::new(panel.min.x + pad + ch as f32 * (fw + gap), panel.min.y + pad), Vec2::new(fw, fh));
            paint_face(&p, f, &m, ch);
            // El detalle de la medición, debajo de cada medidor: pico y nivel eficaz (dBFS y porcentaje).
            let detail = format!("pico {} · {:.0} %", dbfs(m.peak[ch]), m.percent(ch));
            let col = match m.lamp(ch) {
                Lamp::Red => Color32::from_rgb(0xe8, 0x4a, 0x3c),
                _ => Color32::from_rgb(0xb8, 0xb0, 0xa0),
            };
            p.text(Pos2::new(f.center().x, f.max.y + text_h * 0.55), Align2::CENTER_CENTER, detail, FontId::proportional(10.5), col);
        }
        if resp.on_hover_text(format!("Mide la mezcla final que suena (dBFS). Pico: L {} · R {}. Clic: borrar el rojo retenido", dbfs(m.peak[0]), dbfs(m.peak[1]))).clicked() {
            self.audio.meter.clear_clip();
        }
    }
}

fn dbfs(lin: f32) -> String {
    if lin <= 1e-5 {
        "−∞".into()
    } else {
        format!("{:+.1} dBFS", 20.0 * lin.log10()).replace('-', "−")
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_aguja_barre_de_izquierda_a_derecha_y_no_se_sale_de_la_escala() {
        assert!((needle_angle(-21.0).to_degrees() + SWEEP).abs() < 1e-3);
        assert!((needle_angle(3.0).to_degrees() - SWEEP).abs() < 1e-3);
        assert!(needle_angle(-12.0) < needle_angle(-3.0));
        assert_eq!(needle_angle(-99.0), needle_angle(-21.0));
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
                m.hold = [2.0; 2];
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
