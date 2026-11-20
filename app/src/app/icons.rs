//! Estilo de la interfaz e iconos dibujados a mano.

use super::*;

/// Familia de texto seminegrita (Inter 600) para títulos y pestañas activas.
pub(super) fn semi(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("semi".into()))
}
/// Familia de la marca (Space Grotesk 700), solo para «LeonToon» y títulos grandes.
pub(super) fn display(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("display".into()))
}

/// Fuentes del prototipo: Inter para la interfaz y Space Grotesk para la marca (licencia OFL).
/// Las de egui quedan detrás como respaldo para los símbolos que Inter no tiene.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("inter".into(), egui::FontData::from_static(include_bytes!("../../assets/fonts/Inter_400Regular.ttf")).into());
    fonts.font_data.insert("inter-semi".into(), egui::FontData::from_static(include_bytes!("../../assets/fonts/Inter_600SemiBold.ttf")).into());
    fonts.font_data.insert("grotesk".into(), egui::FontData::from_static(include_bytes!("../../assets/fonts/SpaceGrotesk_700Bold.ttf")).into());
    let fallback = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    let mut prop = vec!["inter".to_string()];
    prop.extend(fallback.iter().cloned());
    fonts.families.insert(egui::FontFamily::Proportional, prop);
    let with = |first: &str| {
        let mut f = vec![first.to_string()];
        f.extend(fallback.iter().cloned());
        f
    };
    fonts.families.insert(egui::FontFamily::Name("semi".into()), with("inter-semi"));
    fonts.families.insert(egui::FontFamily::Name("display".into()), with("grotesk"));
    ctx.set_fonts(fonts);
}

/// Chevrón hacia abajo (▾ no existe en la fuente).
pub(super) fn draw_chevron(p: &egui::Painter, c: Pos2, col: Color32) {
    p.add(Shape::line(vec![c + Vec2::new(-3.5, -1.5), c + Vec2::new(0.0, 2.0), c + Vec2::new(3.5, -1.5)], stk(1.4, col)));
}

/// Iconos de las píldoras del visor: 0 sol (mesa de luz), 1 cuadrícula (guías), 2 cebolla.
pub(super) fn draw_hud_icon(kind: u8, p: &egui::Painter, c: Pos2, col: Color32) {
    let st = stk(1.4, col);
    match kind {
        0 => {
            p.circle_stroke(c, 3.2, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + d * 5.4, c + d * 7.2], st);
            }
        }
        1 => {
            let r = Rect::from_center_size(c, Vec2::splat(13.0));
            p.rect_stroke(r, 2.0, st, EStrokeKind::Middle);
            p.line_segment([Pos2::new(c.x - 2.2, r.min.y), Pos2::new(c.x - 2.2, r.max.y)], st);
            p.line_segment([Pos2::new(c.x + 2.2, r.min.y), Pos2::new(c.x + 2.2, r.max.y)], st);
            p.line_segment([Pos2::new(r.min.x, c.y - 2.2), Pos2::new(r.max.x, c.y - 2.2)], st);
            p.line_segment([Pos2::new(r.min.x, c.y + 2.2), Pos2::new(r.max.x, c.y + 2.2)], st);
        }
        _ => {
            p.circle_stroke(c + Vec2::new(-2.6, 0.0), 5.2, st);
            p.circle_stroke(c + Vec2::new(2.6, 0.0), 5.2, st);
        }
    }
}

pub(super) fn setup_style(ctx: &egui::Context) {
    setup_fonts(ctx);
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG2;
    v.window_fill = BG3;
    v.window_stroke = stk(1.0, Color32::BLACK);
    v.extreme_bg_color = Color32::from_rgb(0x15, 0x15, 0x15);
    v.faint_bg_color = BG3;
    v.selection.bg_fill = SEL_BG;
    v.selection.stroke = stk(1.0, ACCENT);
    v.hyperlink_color = ACCENT2;
    v.slider_trailing_fill = true;
    v.widgets.inactive.weak_bg_fill = BG4;
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x55, 0x55, 0x55);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x4a, 0x4a, 0x4a);
    v.widgets.hovered.bg_fill = ACCENT2;
    v.widgets.active.weak_bg_fill = ACCENT;
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.noninteractive.fg_stroke = stk(1.0, TX);
    v.widgets.inactive.fg_stroke = stk(1.0, TX);
    v.widgets.hovered.fg_stroke = stk(1.5, Color32::WHITE);
    v.widgets.active.fg_stroke = stk(1.5, Color32::WHITE);
    v.widgets.noninteractive.bg_stroke = stk(1.0, Color32::from_black_alpha(110));
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = egui::CornerRadius::same(4);
    }
    // Siempre oscuro, aunque Windows esté en modo claro.
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = Vec2::new(6.0, 5.0);
        s.spacing.button_padding = Vec2::new(8.0, 3.0);
        s.spacing.interact_size.y = 20.0;
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(12.5));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(12.5));
    });
}

/// Botón de la columna de herramientas (30×28, naranja cuando está elegida).
/// Botón de la barra de herramientas (34×32, naranja con icono oscuro cuando está elegida;
/// apagado si la herramienta está pendiente).
pub(super) fn tool_button(
    ui: &mut egui::Ui,
    selected: bool,
    pending: bool,
    tip: &str,
    draw: impl FnOnce(&egui::Painter, Rect, Color32),
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(34.0, 32.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 6.0, ACCENT);
    } else if resp.hovered() {
        p.rect_filled(rect, 6.0, BG4);
    }
    let fg = if selected {
        ACCENT_INK
    } else if pending {
        with_alpha(MU, 0.45)
    } else if resp.hovered() {
        TX
    } else {
        MU
    };
    draw(p, Rect::from_center_size(rect.center(), Vec2::splat(16.0)), fg);
    resp.on_hover_text(tip)
}

pub(super) fn checker(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, 3.0, Color32::from_gray(200));
    let s = (r.width().min(r.height()) / 4.0).max(2.0);
    let (nx, ny) = ((r.width() / s).ceil() as i32, (r.height() / s).ceil() as i32);
    for i in 0..nx {
        for j in 0..ny {
            if (i + j) % 2 == 0 {
                let c = Rect::from_min_size(r.min + Vec2::new(i as f32 * s, j as f32 * s), Vec2::splat(s)).intersect(r);
                p.rect_filled(c, 0.0, Color32::from_gray(150));
            }
        }
    }
}

pub(super) fn icon_button(
    ui: &mut egui::Ui,
    size: f32,
    selected: bool,
    tip: &str,
    draw: impl FnOnce(&egui::Painter, Rect, Color32),
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let p = ui.painter();
    let bg = if selected {
        ACCENT
    } else if resp.hovered() {
        Color32::from_rgb(0x4a, 0x4a, 0x4a)
    } else {
        BG4
    };
    p.rect_filled(rect, 4.0, bg);
    p.rect_stroke(rect, 4.0, stk(1.0, Color32::from_black_alpha(100)), EStrokeKind::Inside);
    let fg = if selected { Color32::WHITE } else { Color32::from_gray(218) };
    draw(p, rect.shrink(size * 0.24), fg);
    resp.on_hover_text(tip)
}

pub(super) fn tri(p: &egui::Painter, a: Pos2, b: Pos2, c: Pos2, col: Color32) {
    p.add(Shape::convex_polygon(vec![a, b, c], col, EStroke::NONE));
}

pub(super) fn draw_play(p: &egui::Painter, r: Rect, c: Color32) {
    tri(p, r.left_top() + Vec2::new(2.0, 0.0), r.left_bottom() + Vec2::new(2.0, 0.0), r.right_center(), c);
}
pub(super) fn draw_pause(p: &egui::Painter, r: Rect, c: Color32) {
    let w = r.width() * 0.32;
    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(w, r.height())), 1.0, c);
    p.rect_filled(Rect::from_min_size(Pos2::new(r.max.x - w, r.min.y), Vec2::new(w, r.height())), 1.0, c);
}
pub(super) fn draw_prev(p: &egui::Painter, r: Rect, c: Color32) {
    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(2.5, r.height())), 0.0, c);
    tri(p, r.right_top(), r.right_bottom(), r.left_center() + Vec2::new(3.0, 0.0), c);
}
pub(super) fn draw_next(p: &egui::Painter, r: Rect, c: Color32) {
    p.rect_filled(Rect::from_min_size(Pos2::new(r.max.x - 2.5, r.min.y), Vec2::new(2.5, r.height())), 0.0, c);
    tri(p, r.left_top(), r.left_bottom(), r.right_center() - Vec2::new(3.0, 0.0), c);
}
pub(super) fn draw_first(p: &egui::Painter, r: Rect, c: Color32) {
    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(2.5, r.height())), 0.0, c);
    let m = r.center().x;
    tri(p, Pos2::new(m, r.min.y), Pos2::new(m, r.max.y), r.left_center() + Vec2::new(3.0, 0.0), c);
    tri(p, r.right_top(), r.right_bottom(), Pos2::new(m, r.center().y), c);
}
pub(super) fn draw_last(p: &egui::Painter, r: Rect, c: Color32) {
    p.rect_filled(Rect::from_min_size(Pos2::new(r.max.x - 2.5, r.min.y), Vec2::new(2.5, r.height())), 0.0, c);
    let m = r.center().x;
    tri(p, r.left_top(), r.left_bottom(), Pos2::new(m, r.center().y), c);
    tri(p, Pos2::new(m, r.min.y), Pos2::new(m, r.max.y), r.right_center() - Vec2::new(3.0, 0.0), c);
}
pub(super) fn draw_loop(p: &egui::Painter, r: Rect, c: Color32) {
    let rad = r.width() * 0.45;
    let ctr = r.center();
    let pts: Vec<Pos2> = (0..=24)
        .map(|i| {
            let t = 0.35 + i as f32 / 24.0 * 5.2;
            ctr + Vec2::new(t.cos(), t.sin()) * rad
        })
        .collect();
    let end = *pts.last().unwrap();
    p.add(Shape::line(pts, stk(1.8, c)));
    tri(p, end + Vec2::new(-4.0, -3.0), end + Vec2::new(4.0, -1.0), end + Vec2::new(0.0, 5.0), c);
}

pub(super) fn draw_eye(p: &egui::Painter, r: Rect, on: bool, hover: bool) {
    let c = if on {
        Color32::from_gray(225)
    } else if hover {
        Color32::from_gray(140)
    } else {
        Color32::from_gray(85)
    };
    let ctr = r.center();
    let (w, h) = (8.0, 4.5);
    let arc = |sign: f32| -> Vec<Pos2> {
        (0..=12)
            .map(|i| {
                let t = i as f32 / 12.0 * std::f32::consts::PI;
                ctr + Vec2::new(-t.cos() * w, sign * t.sin() * h)
            })
            .collect()
    };
    p.add(Shape::line(arc(-1.0), stk(1.4, c)));
    p.add(Shape::line(arc(1.0), stk(1.4, c)));
    if on {
        p.circle_filled(ctr, 2.4, c);
    } else {
        p.line_segment([ctr + Vec2::new(-7.0, 6.0), ctr + Vec2::new(7.0, -6.0)], stk(1.4, c));
    }
}

pub(super) fn draw_lock(p: &egui::Painter, r: Rect, on: bool, hover: bool) {
    let c = if on {
        ACCENT
    } else if hover {
        Color32::from_gray(140)
    } else {
        Color32::from_gray(80)
    };
    let ctr = r.center();
    let body = Rect::from_center_size(ctr + Vec2::new(0.0, 2.5), Vec2::new(11.0, 8.0));
    if on {
        p.rect_filled(body, 1.5, c);
    } else {
        p.rect_stroke(body, 1.5, stk(1.3, c), EStrokeKind::Inside);
    }
    let lift = if on { 0.0 } else { -2.5 };
    let arc: Vec<Pos2> = (0..=10)
        .map(|i| {
            let t = i as f32 / 10.0 * std::f32::consts::PI;
            Pos2::new(ctr.x - t.cos() * 3.5, body.min.y - t.sin() * 4.5 + lift)
        })
        .collect();
    p.add(Shape::line(arc, stk(1.4, c)));
}

pub(super) fn draw_tool_icon(t: Tool, p: &egui::Painter, r: Rect, c: Color32) {
    let s = |x: f32, y: f32| Pos2::new(r.min.x + x * r.width(), r.min.y + y * r.height());
    let st = stk(1.5, c);
    let poly = |pts: Vec<Pos2>| p.add(Shape::closed_line(pts, st));
    match t {
        Tool::Select => {
            poly(vec![s(0.2, 0.1), s(0.78, 0.42), s(0.52, 0.52), s(0.42, 0.78)]);
        }
        Tool::RotateObj | Tool::RotateView => {
            let ctr = r.center();
            let pts: Vec<Pos2> = (0..=20).map(|i| { let a = 0.6 + i as f32 / 20.0 * 4.6; ctr + Vec2::new(a.cos(), a.sin()) * r.width() * 0.38 }).collect();
            let end = *pts.last().unwrap();
            p.add(Shape::line(pts, st));
            tri(p, end + Vec2::new(-1.0, -5.0), end + Vec2::new(5.0, 0.0), end + Vec2::new(-1.0, 4.0), c);
            if t == Tool::RotateObj {
                p.circle_filled(ctr, 1.8, c);
            }
        }
        Tool::Anchor => {
            p.circle_stroke(r.center(), r.width() * 0.2, st);
            p.line_segment([s(0.5, 0.05), s(0.5, 0.32)], st);
            p.line_segment([s(0.5, 0.68), s(0.5, 0.95)], st);
            p.line_segment([s(0.05, 0.5), s(0.32, 0.5)], st);
            p.line_segment([s(0.68, 0.5), s(0.95, 0.5)], st);
        }
        Tool::Camera => {
            p.rect_stroke(Rect::from_min_max(s(0.05, 0.28), s(0.65, 0.72)), 2.0, st, EStrokeKind::Middle);
            poly(vec![s(0.65, 0.45), s(0.95, 0.28), s(0.95, 0.72), s(0.65, 0.55)]);
        }
        Tool::Brush => {
            poly(vec![s(0.75, 0.1), s(0.9, 0.25), s(0.48, 0.65), s(0.35, 0.52)]);
            p.add(Shape::line(vec![s(0.35, 0.55), s(0.2, 0.62), s(0.18, 0.8), s(0.08, 0.92), s(0.3, 0.9), s(0.45, 0.68)], st));
        }
        Tool::Vector => {
            poly(vec![s(0.62, 0.12), s(0.88, 0.38), s(0.35, 0.9), s(0.1, 0.9), s(0.1, 0.65)]);
            p.line_segment([s(0.52, 0.22), s(0.78, 0.48)], st);
        }
        Tool::Pen => {
            poly(vec![s(0.5, 0.08), s(0.28, 0.5), s(0.5, 0.92), s(0.72, 0.5)]);
            p.line_segment([s(0.5, 0.08), s(0.5, 0.45)], st);
            p.circle_stroke(s(0.5, 0.52), 1.6, st);
        }
        Tool::Text => {
            p.line_segment([s(0.2, 0.15), s(0.8, 0.15)], st);
            p.line_segment([s(0.5, 0.15), s(0.5, 0.88)], st);
            p.line_segment([s(0.36, 0.88), s(0.64, 0.88)], st);
        }
        Tool::Shape => {
            p.rect_stroke(Rect::from_min_max(s(0.08, 0.42), s(0.5, 0.88)), 1.0, st, EStrokeKind::Middle);
            p.circle_stroke(s(0.65, 0.35), r.width() * 0.22, st);
        }
        Tool::Eraser => {
            poly(vec![s(0.2, 0.62), s(0.55, 0.25), s(0.85, 0.52), s(0.52, 0.85), s(0.35, 0.85)]);
            p.line_segment([s(0.3, 0.88), s(0.9, 0.88)], st);
        }
        Tool::Fill => {
            poly(vec![s(0.25, 0.45), s(0.5, 0.2), s(0.8, 0.5), s(0.55, 0.75)]);
            p.line_segment([s(0.1, 0.92), s(0.9, 0.92)], st);
            p.circle_filled(s(0.85, 0.72), r.width() * 0.07, c);
        }
        Tool::Picker => {
            p.line_segment([s(0.15, 0.85), s(0.62, 0.38)], stk(1.8, c));
            p.circle_stroke(s(0.75, 0.25), r.width() * 0.13, st);
            p.line_segment([s(0.55, 0.28), s(0.72, 0.45)], st);
        }
        Tool::Hand => {
            for (i, h) in [0.25f32, 0.12, 0.16, 0.3].iter().enumerate() {
                let x = 0.3 + i as f32 * 0.14;
                p.line_segment([s(x, *h), s(x, 0.6)], st);
            }
            p.add(Shape::line(vec![s(0.26, 0.6), s(0.12, 0.5), s(0.22, 0.75), s(0.42, 0.92), s(0.65, 0.92), s(0.74, 0.7), s(0.72, 0.4)], st));
        }
        Tool::Zoom => {
            p.circle_stroke(s(0.42, 0.42), r.width() * 0.28, st);
            p.line_segment([s(0.63, 0.63), s(0.92, 0.92)], stk(2.0, c));
            p.line_segment([s(0.3, 0.42), s(0.54, 0.42)], st);
            p.line_segment([s(0.42, 0.3), s(0.42, 0.54)], st);
        }
    }
}