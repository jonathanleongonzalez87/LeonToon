//! Monitor «VU de audio»: dos medidores de aguja (izquierdo y derecho) como los VU clásicos, con luz cálida de fondo como la mesa de luz
//! y un piloto de recorte por canal: apagado con silencio, verde con sonido, amarillo cerca del tope (−3 a 0 dBFS) y rojo al pasarse
//! (el rojo se retiene unos segundos; un clic lo borra). Mide la mezcla final que suena, no las capas sueltas.

use super::*;
use crate::audio::Lamp;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, StrokeKind, Vec2};

/// Hasta dónde gira la aguja a cada lado de la vertical, en grados.
const SWEEP: f32 = 48.0;
/// El rango de la escala en dBFS: de −21 (reposo) a 0 (tope).
const DB_MIN: f32 = -21.0;
/// Las marcas con número de la escala, en dBFS.
const MAJORS: [f32; 7] = [-21.0, -18.0, -15.0, -12.0, -9.0, -6.0, -3.0];

/// El ángulo de la aguja (radianes desde la vertical, positivo hacia la derecha) para una lectura en dBFS.
pub(super) fn needle_angle(db: f32) -> f32 {
    let f = ((db - DB_MIN) / -DB_MIN).clamp(0.0, 1.0);
    (-SWEEP + f * 2.0 * SWEEP).to_radians()
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

/// Un medidor: cara oscura con la luz de un foco que cae desde arriba (el degradado cálido de la mesa de luz), escala fina y neutra
/// (sin brillo) y el piloto de recorte arriba a la derecha, que es lo único que se enciende.
fn paint_face(p: &egui::Painter, f: Rect, m: &crate::audio::Meter, ch: usize) {
    let h = f.height();
    p.rect_filled(f, 5.0, Color32::from_rgb(0x1a, 0x19, 0x18));
    let clipped = p.with_clip_rect(f);
    let pivot = Pos2::new(f.center().x, f.max.y - h * 0.1);
    let db = m.needle_db(ch);
    // El foco: la luz entra por el borde de arriba y se desvanece hacia abajo; un resplandor radial en el centro del borde la suaviza.
    let level = ((db - DB_MIN) / -DB_MIN).clamp(0.0, 1.0);
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

    let r = h * 0.66;
    let ink = Color32::from_rgb(0xd8, 0xd2, 0xc4);
    let arc = |from: f32, to: f32| -> Vec<Pos2> { (0..=32).map(|i| at(pivot, r, needle_angle(from + (to - from) * i as f32 / 32.0))).collect() };
    clipped.add(Shape::line(arc(DB_MIN, 0.0), stk(1.2, with_alpha(ink, 0.85))));
    // Una marca por dB (la escala es lineal en dB: cada marca separa lo mismo) y marcas largas con número cada 3 dB.
    for k in 0..=(-DB_MIN as i32) {
        let v = DB_MIN + k as f32;
        let major = k % 3 == 0;
        let a = needle_angle(v);
        let len = if major { h * 0.07 } else { h * 0.035 };
        clipped.line_segment([at(pivot, r, a), at(pivot, r + len, a)], stk(if major { 1.3 } else { 0.9 }, with_alpha(ink, if major { 0.95 } else { 0.7 })));
    }
    for v in MAJORS {
        let a = needle_angle(v);
        clipped.text(at(pivot, r + h * 0.16, a), Align2::CENTER_CENTER, format!("{}", v as i32).replace('-', "−"), FontId::proportional((h * 0.095).clamp(7.5, 11.0)), with_alpha(ink, 0.9));
    }
    clipped.text(Pos2::new(f.min.x + h * 0.09, f.min.y + h * 0.12), Align2::LEFT_CENTER, if ch == 0 { "L" } else { "R" }, FontId::proportional((h * 0.12).clamp(9.0, 13.0)), with_alpha(ink, 0.8));

    // La aguja: un trazo limpio, sin brillo.
    let tip = at(pivot, r * 1.04, needle_angle(db));
    clipped.line_segment([pivot, tip], stk(1.6, Color32::from_rgb(0xf0, 0xea, 0xdc)));
    clipped.circle_filled(pivot, h * 0.03, Color32::from_rgb(0xf0, 0xea, 0xdc));

    // El piloto de recorte, con halo cuando está encendido.
    let lamp = m.lamp(ch);
    let c = Pos2::new(f.max.x - h * 0.1, f.min.y + h * 0.12);
    let lr = (h * 0.045).max(3.5);
    let col = lamp_color(lamp);
    if lamp != Lamp::Off {
        p.circle_filled(c, lr * 2.4, col.gamma_multiply(0.18));
        p.circle_filled(c, lr * 1.6, col.gamma_multiply(0.30));
    }
    p.circle_filled(c, lr, col);
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
        let panel = Rect::from_center_size(rect.center(), Vec2::new(fw * 2.0 + gap + pad * 2.0, fh + pad * 2.0));
        p.rect_filled(panel, 7.0, Color32::from_rgb(0x2a, 0x29, 0x27));
        p.rect_stroke(panel, 7.0, stk(1.0, Color32::from_rgb(0x3e, 0x3c, 0x39)), StrokeKind::Inside);
        for ch in 0..2 {
            let f = Rect::from_min_size(Pos2::new(panel.min.x + pad + ch as f32 * (fw + gap), panel.min.y + pad), Vec2::new(fw, fh));
            paint_face(&p, f, &m, ch);
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
        format!("{:+.1} dB", 20.0 * lin.log10()).replace('-', "−")
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_aguja_barre_de_izquierda_a_derecha_y_no_se_sale_de_la_escala() {
        assert!((needle_angle(-21.0).to_degrees() + SWEEP).abs() < 1e-3);
        assert!((needle_angle(0.0).to_degrees() - SWEEP).abs() < 1e-3);
        assert!(needle_angle(-12.0) < needle_angle(-3.0));
        assert_eq!(needle_angle(-99.0), needle_angle(-21.0));
        assert_eq!(needle_angle(40.0), needle_angle(0.0));
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
