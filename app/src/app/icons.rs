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

/// Escala de la interfaz (un solo sitio): tamaño del texto de cuerpo y de botones, alto mínimo de los controles y
/// separaciones. Los tamaños fijos de las cabeceras salen de UiScale.
pub(super) struct UiScale;

impl UiScale {
    pub const BODY: f32 = 11.5;
    pub const SMALL: f32 = 10.0;
    pub const HEADING: f32 = 14.0;
    pub const CONTROL_H: f32 = 18.0;
    /// Alto de la cabecera de cada área (selector de editor, pestañas).
    pub const AREA_HEADER: f32 = 25.0;
    /// Alto de la cabecera de herramienta.
    pub const TOOL_HEAD: f32 = 30.0;
    /// Alto del selector de editor.
    pub const SELECTOR_H: f32 = 20.0;
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
        s.spacing.item_spacing = Vec2::new(5.0, 4.0);
        s.spacing.button_padding = Vec2::new(6.0, 2.0);
        s.spacing.interact_size.y = UiScale::CONTROL_H;
        s.spacing.slider_rail_height = 4.0;
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(UiScale::BODY));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(UiScale::BODY));
        s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(UiScale::SMALL));
        s.text_styles.insert(egui::TextStyle::Heading, FontId::proportional(UiScale::HEADING));
    });
}

/// Botón de la barra de herramientas: el icono lleva el color de su herramienta (de la paleta), el elegido se pone sobre
/// fondo naranja con el icono oscuro, y el que está pendiente se apaga.
pub(super) fn tool_button(
    ui: &mut egui::Ui,
    size: Vec2,
    selected: bool,
    pending: bool,
    tip: &str,
    tint: Color32,
    draw: impl FnOnce(&egui::Painter, Rect, Color32),
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 6.0, ACCENT);
    } else if resp.hovered() {
        p.rect_filled(rect, 6.0, BG4);
        p.rect_stroke(rect, 6.0, stk(1.0, with_alpha(tint, 0.55)), EStrokeKind::Inside);
    }
    let fg = if selected {
        ACCENT_INK
    } else if pending {
        with_alpha(tint, 0.35)
    } else if resp.hovered() {
        lighten(tint, 0.25)
    } else {
        tint
    };
    // El icono crece con el botón (barra de herramientas más ancha).
    draw(p, Rect::from_center_size(rect.center(), Vec2::splat((size.y * 0.58).max(18.0))), fg);
    resp.on_hover_text(tip)
}

/// Aclara un color mezclándolo con blanco.
pub(super) fn lighten(c: Color32, k: f32) -> Color32 {
    let m = |a: u8| (a as f32 + (255.0 - a as f32) * k).round() as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

/// Color de cada herramienta, tomado de la paleta de LeonToon: naranja para pintar, dorado para rellenar y la pluma,
/// azul para ver y escribir, verde para formas y el cuentagotas, lila para mover y la cámara, salmón para borrar.
pub(super) fn tool_tint(t: Tool) -> Color32 {
    const ORANGE: Color32 = Color32::from_rgb(0xfd, 0x67, 0x24);
    const GOLD: Color32 = Color32::from_rgb(0xf2, 0xd2, 0x7a);
    const SKY: Color32 = Color32::from_rgb(0x8f, 0xb3, 0xcf);
    const GREEN: Color32 = Color32::from_rgb(0xa8, 0xcf, 0x8f);
    const LILAC: Color32 = Color32::from_rgb(0xb7, 0xa2, 0xe6);
    const SALMON: Color32 = Color32::from_rgb(0xe9, 0xa0, 0x7a);
    match t {
        Tool::Select => Color32::from_rgb(0xee, 0xee, 0xee),
        Tool::RotateObj | Tool::RotateView | Tool::Anchor | Tool::Camera => LILAC,
        Tool::Brush | Tool::Vector => ORANGE,
        Tool::Pen | Tool::Fill => GOLD,
        Tool::Text | Tool::Hand | Tool::Zoom => SKY,
        Tool::Shape | Tool::Picker => GREEN,
        Tool::Eraser => SALMON,
    }
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
    let st = stk(1.8, c);
    // Los cuerpos llevan un relleno suave del mismo color para que el icono tenga presencia (solo en formas convexas).
    let soft = c.gamma_multiply(0.30);
    let poly = |pts: Vec<Pos2>| {
        p.add(Shape::convex_polygon(pts.clone(), soft, EStroke::NONE));
        p.add(Shape::closed_line(pts, st));
    };
    match t {
        Tool::Select => {
            // La flecha es cóncava: se rellena como dos triángulos.
            let (a, b, m, d) = (s(0.2, 0.1), s(0.78, 0.42), s(0.52, 0.52), s(0.42, 0.78));
            p.add(Shape::convex_polygon(vec![a, b, m], soft, EStroke::NONE));
            p.add(Shape::convex_polygon(vec![a, m, d], soft, EStroke::NONE));
            p.add(Shape::closed_line(vec![a, b, m, d], st));
        }
        Tool::RotateObj | Tool::RotateView => {
            let ctr = r.center();
            let pts: Vec<Pos2> = (0..=20).map(|i| { let a = 0.6 + i as f32 / 20.0 * 4.6; ctr + Vec2::new(a.cos(), a.sin()) * r.width() * 0.38 }).collect();
            let end = *pts.last().unwrap();
            p.add(Shape::line(pts, st));
            tri(p, end + Vec2::new(-1.0, -5.0), end + Vec2::new(5.0, 0.0), end + Vec2::new(-1.0, 4.0), c);
            if t == Tool::RotateObj {
                p.circle_filled(ctr, 2.2, c);
            }
        }
        Tool::Anchor => {
            p.circle_filled(r.center(), r.width() * 0.2, soft);
            p.circle_stroke(r.center(), r.width() * 0.2, st);
            p.line_segment([s(0.5, 0.05), s(0.5, 0.32)], st);
            p.line_segment([s(0.5, 0.68), s(0.5, 0.95)], st);
            p.line_segment([s(0.05, 0.5), s(0.32, 0.5)], st);
            p.line_segment([s(0.68, 0.5), s(0.95, 0.5)], st);
        }
        Tool::Camera => {
            let body = Rect::from_min_max(s(0.05, 0.28), s(0.65, 0.72));
            p.rect_filled(body, 2.0, soft);
            p.rect_stroke(body, 2.0, st, EStrokeKind::Middle);
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
            p.circle_filled(s(0.5, 0.52), 2.0, c);
        }
        Tool::Text => {
            p.line_segment([s(0.2, 0.15), s(0.8, 0.15)], stk(2.2, c));
            p.line_segment([s(0.5, 0.15), s(0.5, 0.88)], stk(2.2, c));
            p.line_segment([s(0.36, 0.88), s(0.64, 0.88)], stk(2.2, c));
        }
        Tool::Shape => {
            let sq = Rect::from_min_max(s(0.08, 0.42), s(0.5, 0.88));
            p.rect_filled(sq, 1.0, soft);
            p.rect_stroke(sq, 1.0, st, EStrokeKind::Middle);
            p.circle_filled(s(0.65, 0.35), r.width() * 0.22, soft);
            p.circle_stroke(s(0.65, 0.35), r.width() * 0.22, st);
        }
        Tool::Eraser => {
            poly(vec![s(0.2, 0.62), s(0.55, 0.25), s(0.85, 0.52), s(0.52, 0.85), s(0.35, 0.85)]);
            p.line_segment([s(0.3, 0.88), s(0.9, 0.88)], st);
        }
        Tool::Fill => {
            poly(vec![s(0.25, 0.45), s(0.5, 0.2), s(0.8, 0.5), s(0.55, 0.75)]);
            p.line_segment([s(0.1, 0.92), s(0.9, 0.92)], st);
            p.circle_filled(s(0.85, 0.72), r.width() * 0.09, c);
        }
        Tool::Picker => {
            p.line_segment([s(0.15, 0.85), s(0.62, 0.38)], stk(2.2, c));
            p.circle_filled(s(0.75, 0.25), r.width() * 0.13, soft);
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
            p.circle_filled(s(0.42, 0.42), r.width() * 0.28, soft);
            p.circle_stroke(s(0.42, 0.42), r.width() * 0.28, st);
            p.line_segment([s(0.63, 0.63), s(0.92, 0.92)], stk(2.4, c));
            p.line_segment([s(0.3, 0.42), s(0.54, 0.42)], st);
            p.line_segment([s(0.42, 0.3), s(0.42, 0.54)], st);
        }
    }
}

/// Color de cada grupo de editores (de la paleta): azul los visores, dorado el tiempo, verde los datos, lila la
/// producción y salmón el sistema.
pub(super) fn group_tint(g: EditorGroup) -> Color32 {
    match g {
        EditorGroup::Visores => Color32::from_rgb(0x8f, 0xb3, 0xcf),
        EditorGroup::Tiempo => Color32::from_rgb(0xf2, 0xd2, 0x7a),
        EditorGroup::Datos => Color32::from_rgb(0xa8, 0xcf, 0x8f),
        EditorGroup::Produccion => Color32::from_rgb(0xb7, 0xa2, 0xe6),
        EditorGroup::Sistema => Color32::from_rgb(0xe9, 0xa0, 0x7a),
    }
}

/// Icono de un editor, dibujado en el estilo de los de la barra de herramientas (trazo del color del grupo con un
/// relleno suave). `r` es un cuadrado.
pub(super) fn draw_editor_icon(e: Editor, p: &egui::Painter, r: Rect, c: Color32) {
    let s = |x: f32, y: f32| Pos2::new(r.min.x + x * r.width(), r.min.y + y * r.height());
    let st = stk(1.5, c);
    let soft = c.gamma_multiply(0.30);
    let boxr = |a: Pos2, b: Pos2| {
        let rc = Rect::from_min_max(a, b);
        p.rect_filled(rc, 1.5, soft);
        p.rect_stroke(rc, 1.5, st, EStrokeKind::Middle);
    };
    match e {
        Editor::Viewer => {
            boxr(s(0.08, 0.16), s(0.92, 0.7));
            p.line_segment([s(0.5, 0.7), s(0.5, 0.88)], st);
            p.line_segment([s(0.3, 0.88), s(0.7, 0.88)], st);
        }
        Editor::SmStage => {
            boxr(s(0.08, 0.16), s(0.92, 0.7));
            p.circle_filled(s(0.28, 0.43), r.width() * 0.09, Color32::from_rgb(0xe0, 0x40, 0x30));
            p.line_segment([s(0.5, 0.7), s(0.5, 0.88)], st);
            p.line_segment([s(0.3, 0.88), s(0.7, 0.88)], st);
        }
        Editor::Preview => {
            let pts: Vec<Pos2> = (0..=16).map(|i| { let x = i as f32 / 16.0; s(0.06 + x * 0.88, 0.5 - (std::f32::consts::PI * x).sin() * 0.3) }).collect();
            let low: Vec<Pos2> = (0..=16).map(|i| { let x = i as f32 / 16.0; s(0.06 + x * 0.88, 0.5 + (std::f32::consts::PI * x).sin() * 0.3) }).collect();
            p.add(Shape::line(pts, st));
            p.add(Shape::line(low, st));
            p.circle_filled(s(0.5, 0.5), r.width() * 0.13, c);
        }
        Editor::Timeline => {
            p.line_segment([s(0.06, 0.62), s(0.94, 0.62)], st);
            for i in 0..5 {
                let x = 0.12 + i as f32 * 0.19;
                p.line_segment([s(x, 0.5), s(x, 0.62)], st);
            }
            boxr(s(0.3, 0.22), s(0.46, 0.42));
            p.line_segment([s(0.62, 0.15), s(0.62, 0.85)], stk(1.8, Color32::from_rgb(0xfd, 0x67, 0x24)));
        }
        Editor::Montage => {
            for (y, x0, x1) in [(0.2f32, 0.1f32, 0.7f32), (0.46, 0.3, 0.9), (0.72, 0.18, 0.62)] {
                let bar = Rect::from_min_max(s(x0, y), s(x1, y + 0.16));
                p.rect_filled(bar, 2.0, soft);
                p.rect_stroke(bar, 2.0, st, EStrokeKind::Middle);
            }
            p.line_segment([s(0.52, 0.08), s(0.52, 0.94)], stk(1.8, Color32::from_rgb(0xfd, 0x67, 0x24)));
        }
        Editor::SmStrip => {
            for i in 0..3 {
                let x = 0.04 + i as f32 * 0.32;
                boxr(s(x, 0.3), s(x + 0.26, 0.7));
            }
        }
        Editor::SmXSheet => {
            boxr(s(0.14, 0.1), s(0.86, 0.9));
            p.line_segment([s(0.14, 0.37), s(0.86, 0.37)], st);
            p.line_segment([s(0.14, 0.63), s(0.86, 0.63)], st);
            p.line_segment([s(0.4, 0.1), s(0.4, 0.9)], st);
        }
        Editor::Curves => {
            let pts: Vec<Pos2> = (0..=18).map(|i| { let x = i as f32 / 18.0; s(0.1 + x * 0.8, 0.82 - (x * x * (3.0 - 2.0 * x)) * 0.64) }).collect();
            p.add(Shape::line(pts, stk(1.8, c)));
            p.circle_filled(s(0.1, 0.82), 2.4, c);
            p.circle_filled(s(0.9, 0.18), 2.4, c);
        }
        Editor::Layers => {
            for (i, y) in [0.62f32, 0.42, 0.22].iter().enumerate() {
                let pts = vec![s(0.5, *y), s(0.92, *y + 0.14), s(0.5, *y + 0.28), s(0.08, *y + 0.14)];
                p.add(Shape::convex_polygon(pts.clone(), if i == 2 { c.gamma_multiply(0.55) } else { soft }, EStroke::NONE));
                p.add(Shape::closed_line(pts, st));
            }
        }
        Editor::RoomProps => {
            for (i, (y, k)) in [(0.22f32, 0.3f32), (0.5, 0.68), (0.78, 0.45)].iter().enumerate() {
                let _ = i;
                p.line_segment([s(0.1, *y), s(0.9, *y)], st);
                p.circle_filled(s(*k, *y), r.width() * 0.1, c);
            }
        }
        Editor::Schematic => {
            boxr(s(0.06, 0.14), s(0.34, 0.38));
            boxr(s(0.06, 0.62), s(0.34, 0.86));
            boxr(s(0.66, 0.38), s(0.94, 0.62));
            p.line_segment([s(0.34, 0.26), s(0.66, 0.5)], st);
            p.line_segment([s(0.34, 0.74), s(0.66, 0.5)], st);
        }
        Editor::Explorer => {
            let pts = vec![s(0.06, 0.26), s(0.38, 0.26), s(0.48, 0.38), s(0.94, 0.38), s(0.94, 0.84), s(0.06, 0.84)];
            p.add(Shape::convex_polygon(pts.clone(), soft, EStroke::NONE));
            p.add(Shape::closed_line(pts, st));
        }
        Editor::Palettes => {
            p.circle_filled(s(0.5, 0.5), r.width() * 0.42, soft);
            p.circle_stroke(s(0.5, 0.5), r.width() * 0.42, st);
            for (x, y, col) in [(0.34, 0.38, Color32::from_rgb(0xfd, 0x67, 0x24)), (0.58, 0.32, Color32::from_rgb(0xf2, 0xd2, 0x7a)), (0.66, 0.56, Color32::from_rgb(0x8f, 0xb3, 0xcf)), (0.38, 0.64, Color32::from_rgb(0xa8, 0xcf, 0x8f))] {
                p.circle_filled(s(x, y), r.width() * 0.07, col);
            }
        }
        Editor::Substitution => {
            boxr(s(0.06, 0.2), s(0.46, 0.6));
            boxr(s(0.54, 0.4), s(0.94, 0.8));
            p.line_segment([s(0.3, 0.8), s(0.46, 0.8)], st);
            p.line_segment([s(0.54, 0.2), s(0.7, 0.2)], st);
        }
        Editor::Storyboard => {
            for (x, y) in [(0.06, 0.1), (0.54, 0.1), (0.06, 0.54), (0.54, 0.54)] {
                boxr(s(x, y), s(x + 0.4, y + 0.36));
            }
        }
        Editor::Camera => {
            boxr(s(0.05, 0.28), s(0.65, 0.72));
            let pts = vec![s(0.65, 0.45), s(0.95, 0.28), s(0.95, 0.72), s(0.65, 0.55)];
            p.add(Shape::convex_polygon(pts.clone(), soft, EStroke::NONE));
            p.add(Shape::closed_line(pts, st));
        }
    }
}