//! Pruebas del motor de pinceles.

use super::*;

fn spec(size: f32, params: BrushParams) -> BrushSpec {
    BrushSpec::new(size, 1.0, [200, 30, 30], params, 1)
}

fn alpha_at(r: &Raster, x: u32, y: u32) -> u8 {
    r.data[((y * r.w + x) * 4 + 3) as usize]
}

fn rgb_at(r: &Raster, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * r.w + x) * 4) as usize;
    [r.data[i], r.data[i + 1], r.data[i + 2]]
}

/// Filas con algo de pintura, de arriba abajo: (primera, última).
fn rows(r: &Raster) -> (u32, u32) {
    let hit = |y: u32| (0..r.w).any(|x| alpha_at(r, x, y) > 8);
    let ys: Vec<u32> = (0..r.h).filter(|y| hit(*y)).collect();
    (*ys.first().unwrap_or(&0), *ys.last().unwrap_or(&0))
}

/// Columnas con algo de pintura: (primera, última).
fn cols(r: &Raster) -> (u32, u32) {
    let hit = |x: u32| (0..r.h).any(|y| alpha_at(r, x, y) > 8);
    let xs: Vec<u32> = (0..r.w).filter(|x| hit(*x)).collect();
    (*xs.first().unwrap_or(&0), *xs.last().unwrap_or(&0))
}

fn ink(r: &Raster) -> u64 {
    r.data.chunks_exact(4).map(|p| p[3] as u64).sum()
}

fn line(ls: &mut LiveStroke, pressure: f32) {
    for i in 0..=40 {
        ls.push_with([20.0 + i as f32 * 3.0, 50.0], Pen { pressure, tilt: 1.0 });
    }
}

// ---------------- Fase 1 ----------------

#[test]
fn spacing_sets_how_many_dabs_a_line_gets() {
    let count = |spacing: f32, pieces: usize| {
        let mut ls = LiveStroke::new(200, 40, spec(10.0, BrushParams { spacing, ..Default::default() }));
        // La misma línea de 100 px, entera o partida en trozos: el resultado no cambia.
        for i in 0..=pieces {
            ls.push([10.0 + 100.0 * i as f32 / pieces as f32, 20.0]);
        }
        ls.dabs
    };
    assert_eq!(count(25.0, 1), 41);
    assert_eq!(count(25.0, 7), 41);
    assert_eq!(count(100.0, 1), 11);
    assert_eq!(count(100.0, 13), 11);
    assert!(count(10.0, 1) > count(50.0, 1));
}

#[test]
fn hardness_changes_the_edge_but_not_the_center() {
    let paint = |hardness: f32| {
        let mut ls = LiveStroke::new(80, 80, spec(40.0, BrushParams { hardness, ..Default::default() }));
        ls.push([40.0, 40.0]);
        ls.buf
    };
    let (hard, soft) = (paint(1.0), paint(0.0));
    assert_eq!(alpha_at(&hard, 40, 40), 255);
    assert!(alpha_at(&soft, 40, 40) >= 250);
    // A 3/4 del radio (15 px del centro) el duro sigue lleno y el difuso ya se desvaneció.
    assert_eq!(alpha_at(&hard, 55, 40), 255);
    assert!(alpha_at(&soft, 55, 40) < 90, "difuso: {}", alpha_at(&soft, 55, 40));
    // Fuera del radio no hay nada en ninguno.
    assert_eq!(alpha_at(&hard, 66, 40), 0);
    assert_eq!(alpha_at(&soft, 66, 40), 0);
}

#[test]
fn flow_accumulates_and_opacity_caps_the_whole_stroke() {
    let wide = |flow: f32, opacity: f32| {
        let mut s = spec(30.0, BrushParams { flow, spacing: 5.0, ..Default::default() });
        s.opacity = opacity;
        let mut ls = LiveStroke::new(100, 60, s);
        // Ida y vuelta sobre la misma línea: con flujo bajo la pintura se acumula.
        for _ in 0..3 {
            ls.push([20.0, 30.0]);
            ls.push([80.0, 30.0]);
        }
        let mut out = Raster::new(100, 60);
        ls.finish_into(&mut out);
        alpha_at(&out, 50, 30)
    };
    let low_flow_many_passes = wide(0.2, 1.0);
    let single_pass_low_flow = {
        let mut ls = LiveStroke::new(100, 60, spec(30.0, BrushParams { flow: 0.2, spacing: 100.0, ..Default::default() }));
        ls.push([50.0, 30.0]);
        alpha_at(&ls.buf, 50, 30)
    };
    assert!(low_flow_many_passes > single_pass_low_flow + 80, "{low_flow_many_passes} vs {single_pass_low_flow}");
    // La opacidad es el tope, por muchas marcas que se acumulen.
    let capped = wide(1.0, 0.5);
    assert!((capped as i32 - 128).abs() <= 2, "alpha {capped}");
    // Flujo y opacidad son independientes: mismo flujo, distinta opacidad.
    assert!(wide(0.2, 0.4) < wide(0.2, 1.0));
}

#[test]
fn roundness_and_angle_shape_the_tip() {
    let tip = |roundness: f32, angle: f32| {
        let mut ls = LiveStroke::new(100, 100, spec(60.0, BrushParams { roundness, angle, ..Default::default() }));
        ls.push([50.0, 50.0]);
        ls.buf
    };
    let flat = tip(0.25, 0.0);
    // Punta plana horizontal: ancha en X y estrecha en Y.
    assert!(alpha_at(&flat, 75, 50) > 200);
    assert_eq!(alpha_at(&flat, 50, 65), 0);
    // Girada 90°: ahora es estrecha en X y alta en Y.
    let turned = tip(0.25, 90.0);
    assert_eq!(alpha_at(&turned, 65, 50), 0);
    assert!(alpha_at(&turned, 50, 75) > 200);
}

// ---------------- Fase 2 ----------------

#[test]
fn pressure_controls_size_between_min_and_full() {
    let params = BrushParams { size_control: Control::Pressure, min_size: 0.2, ..Default::default() };
    let tall = |pressure: f32| {
        let mut ls = LiveStroke::new(200, 100, spec(40.0, params));
        line(&mut ls, pressure);
        let (a, b) = rows(&ls.buf);
        b - a + 1
    };
    let (full, half, none) = (tall(1.0), tall(0.5), tall(0.0));
    assert!((38..=42).contains(&full), "presión máxima: {full}");
    assert!(half < full && half > none, "{none} < {half} < {full}");
    // Con presión 0 queda el tamaño mínimo (20 % de 40 = 8 px).
    assert!((6..=11).contains(&none), "presión cero: {none}");
    // Sin control de tamaño, la presión no cambia nada.
    let plain = |pressure: f32| {
        let mut ls = LiveStroke::new(200, 100, spec(40.0, BrushParams::default()));
        line(&mut ls, pressure);
        rows(&ls.buf)
    };
    assert_eq!(plain(0.1), plain(1.0));
}

#[test]
fn pressure_controls_flow_and_opacity_independently() {
    let single = |params: BrushParams, pressure: f32| {
        let mut ls = LiveStroke::new(80, 80, spec(30.0, BrushParams { spacing: 100.0, ..params }));
        ls.push_with([40.0, 40.0], Pen { pressure, tilt: 1.0 });
        alpha_at(&ls.buf, 40, 40)
    };
    let flow = BrushParams { flow_control: Control::Pressure, ..Default::default() };
    assert!((single(flow, 0.3) as i32 - 77).abs() <= 2, "flujo con presión 0,3: {}", single(flow, 0.3));
    assert_eq!(single(flow, 1.0), 255);
    let opacity = BrushParams { opacity_control: Control::Pressure, ..Default::default() };
    assert!((single(opacity, 0.5) as i32 - 128).abs() <= 2);
    // La opacidad por presión es un tope: muchas pasadas no la superan, el flujo por presión sí se acumula.
    let many = |params: BrushParams, pressure: f32| {
        let mut ls = LiveStroke::new(100, 60, spec(30.0, BrushParams { spacing: 5.0, ..params }));
        for _ in 0..4 {
            ls.push_with([20.0, 30.0], Pen { pressure, tilt: 1.0 });
            ls.push_with([80.0, 30.0], Pen { pressure, tilt: 1.0 });
        }
        alpha_at(&ls.buf, 50, 30)
    };
    assert!((many(opacity, 0.5) as i32 - 128).abs() <= 2, "tope: {}", many(opacity, 0.5));
    assert!(many(flow, 0.3) > 200, "acumula: {}", many(flow, 0.3));
}

#[test]
fn tilt_control_has_no_effect_without_a_device() {
    let params = BrushParams { size_control: Control::Tilt, min_size: 0.1, ..Default::default() };
    let mut a = LiveStroke::new(200, 100, spec(40.0, params));
    let mut b = LiveStroke::new(200, 100, spec(40.0, BrushParams::default()));
    line(&mut a, 0.2);
    line(&mut b, 0.2);
    assert_eq!(a.buf.data, b.buf.data);
    // Con inclinación real (lápiz casi tumbado) el tamaño baja.
    let mut c = LiveStroke::new(200, 100, spec(40.0, params));
    for i in 0..=40 {
        c.push_with([20.0 + i as f32 * 3.0, 50.0], Pen { pressure: 1.0, tilt: 0.1 });
    }
    let (t0, t1) = rows(&c.buf);
    assert!(t1 - t0 < 20);
}

#[test]
fn jitter_varies_marks_but_repeats_with_the_same_seed() {
    let wild = BrushParams { size_jitter: 1.0, angle_jitter: 1.0, roundness: 0.3, roundness_jitter: 0.8, flow_jitter: 0.8, spacing: 25.0, ..Default::default() };
    let run = |seed: u64| {
        let mut ls = LiveStroke::new(200, 100, BrushSpec { seed, ..spec(30.0, wild) });
        line(&mut ls, 1.0);
        ls.buf.data
    };
    assert_eq!(run(5), run(5));
    assert_ne!(run(5), run(6));
    // Sin variaciones, el trazo no depende de la semilla.
    let calm = |seed: u64| {
        let mut ls = LiveStroke::new(200, 100, BrushSpec { seed, ..spec(30.0, BrushParams::default()) });
        line(&mut ls, 1.0);
        ls.buf.data
    };
    assert_eq!(calm(1), calm(99));
}

#[test]
fn size_angle_and_roundness_jitter_change_the_footprint() {
    let extent = |params: BrushParams| {
        let mut ls = LiveStroke::new(260, 140, BrushSpec { seed: 3, ..spec(40.0, params) });
        for i in 0..=60 {
            ls.push([20.0 + i as f32 * 3.5, 70.0]);
        }
        let (a, b) = rows(&ls.buf);
        b - a + 1
    };
    let flat = BrushParams { roundness: 0.2, spacing: 20.0, ..Default::default() };
    let flat_h = extent(flat);
    // Una punta plana horizontal es baja; con variación de ángulo, al girar, se vuelve más alta.
    assert!(extent(BrushParams { angle_jitter: 1.0, ..flat }) > flat_h + 10);
    // La variación de tamaño nunca supera el tamaño base y hace marcas más pequeñas.
    let round = BrushParams { spacing: 20.0, ..Default::default() };
    assert!(extent(BrushParams { size_jitter: 1.0, ..round }) <= extent(round));
    // La variación de redondez, hacia una punta plana, deja menos pintura que la punta redonda.
    let total = |params: BrushParams| {
        let mut ls = LiveStroke::new(260, 140, BrushSpec { seed: 3, ..spec(40.0, params) });
        for i in 0..=60 {
            ls.push([20.0 + i as f32 * 3.5, 70.0]);
        }
        ink(&ls.buf)
    };
    let rj = BrushParams { roundness_jitter: 1.0, min_roundness: 0.1, ..round };
    assert!(total(rj) < total(round), "{} vs {}", total(rj), total(round));
}

// ---------------- Fase 3 ----------------

#[test]
fn scatter_spreads_marks_and_count_adds_more() {
    let run = |params: BrushParams, seed: u64| {
        let mut ls = LiveStroke::new(220, 160, BrushSpec { seed, ..spec(16.0, params) });
        for i in 0..=40 {
            ls.push([20.0 + i as f32 * 4.0, 80.0]);
        }
        (rows(&ls.buf), ink(&ls.buf), ls.marks, ls.dabs)
    };
    let plain = run(BrushParams::default(), 1);
    let scat = run(BrushParams { scatter: 3.0, scatter_count: 3, ..Default::default() }, 1);
    // Sin dispersión el trazo mide lo que la punta; con dispersión se reparte mucho más alto.
    assert!(plain.0 .1 - plain.0 .0 <= 18, "sin dispersión: {:?}", plain.0);
    assert!(scat.0 .1 - scat.0 .0 > 60, "con dispersión: {:?}", scat.0);
    // La cantidad multiplica las marcas por posición.
    assert_eq!(scat.2, scat.3 * 3);
    assert_eq!(plain.2, plain.3);
    // El azar de la dispersión se repite con la misma semilla.
    assert_eq!(run(BrushParams { scatter: 3.0, scatter_count: 3, ..Default::default() }, 9).1, run(BrushParams { scatter: 3.0, scatter_count: 3, ..Default::default() }, 9).1);
}

#[test]
fn scatter_axes_and_count_jitter() {
    // Un solo punto: el trazo no tiene dirección y la dispersión simple es perpendicular (vertical).
    let one = |both: bool| {
        let mut ls = LiveStroke::new(260, 260, spec(14.0, BrushParams { scatter: 4.0, scatter_count: 16, scatter_both: both, ..Default::default() }));
        ls.push([130.0, 130.0]);
        (cols(&ls.buf), rows(&ls.buf))
    };
    let (vert_c, vert_r) = one(false);
    let (both_c, _) = one(true);
    assert!(vert_c.1 - vert_c.0 <= 16, "solo perpendicular: ancho {:?}", vert_c);
    assert!(vert_r.1 - vert_r.0 > 50, "perpendicular: alto {:?}", vert_r);
    assert!(both_c.1 - both_c.0 > 40, "dos ejes: ancho {:?}", both_c);
    // La variación de cantidad reduce marcas.
    let marks = |cj: f32| {
        let mut ls = LiveStroke::new(220, 100, spec(10.0, BrushParams { scatter: 1.0, scatter_count: 8, count_jitter: cj, ..Default::default() }));
        for i in 0..=30 {
            ls.push([20.0 + i as f32 * 6.0, 50.0]);
        }
        ls.marks as f32 / ls.dabs as f32
    };
    assert!((marks(0.0) - 8.0).abs() < 0.01);
    assert!(marks(1.0) < 6.0, "con variación: {}", marks(1.0));
}

#[test]
fn texture_depth_modes_and_behavior() {
    let paint = |params: BrushParams, center: [f32; 2]| {
        let mut ls = LiveStroke::new(120, 120, spec(60.0, BrushParams { spacing: 100.0, ..params }));
        ls.push(center);
        ls.buf
    };
    let tex = BrushParams { texture: TexKind::Paper, texture_scale: 1.0, texture_depth: 1.0, ..Default::default() };
    let plain = paint(BrushParams::default(), [60.0, 60.0]);
    let textured = paint(tex, [60.0, 60.0]);
    // La textura quita pintura y deja variaciones dentro de la punta.
    assert!(ink(&textured) < ink(&plain) * 9 / 10);
    let (mut lo, mut hi) = (255u8, 0u8);
    for y in 50..70 {
        for x in 50..70 {
            let a = alpha_at(&textured, x, y);
            lo = lo.min(a);
            hi = hi.max(a);
        }
    }
    assert!(hi > lo + 80, "variación {lo}..{hi}");
    // Con profundidad 0 no hay efecto.
    assert_eq!(paint(BrushParams { texture_depth: 0.0, ..tex }, [60.0, 60.0]).data, plain.data);
    // Restar deja huecos (alfa 0) donde la textura es oscura; multiplicar no llega a 0 en el centro de un mosaico claro.
    let sub = paint(BrushParams { texture_mode: TexMode::Subtract, ..tex }, [60.0, 60.0]);
    assert!(ink(&sub) < ink(&plain));
    let dark = paint(BrushParams { texture_mode: TexMode::Darken, ..tex }, [60.0, 60.0]);
    assert!(ink(&dark) < ink(&plain));
    // Continua: un píxel hondo en dos marcas desplazadas da el mismo valor. Por marca, no.
    let probe = |params: BrushParams| {
        let a = paint(params, [60.0, 60.0]);
        let b = paint(params, [64.0, 62.0]);
        (alpha_at(&a, 63, 61) as i32, alpha_at(&b, 63, 61) as i32)
    };
    let (ca, cb) = probe(tex);
    assert_eq!(ca, cb, "textura continua: debe depender del lienzo");
    let (pa, pb) = probe(BrushParams { texture_per_tip: true, ..tex });
    let any_diff = (0..6).any(|k| {
        let params = BrushParams { texture_per_tip: true, ..tex };
        let a = paint(params, [60.0 + k as f32, 60.0]);
        let b = paint(params, [64.0 + k as f32, 62.0]);
        alpha_at(&a, 63, 61) != alpha_at(&b, 63, 61)
    });
    assert!(pa != pb || any_diff, "textura por marca: debe moverse con la marca");
}

#[test]
fn image_textures_and_tips_use_the_loaded_gray_image() {
    // Textura de imagen: mosaico de 2x1, un píxel de pintura y uno vacío.
    let tex = Arc::new(Gray { w: 2, h: 1, data: vec![255, 0] });
    let params = BrushParams { texture: TexKind::Image, texture_scale: 1.0, texture_depth: 1.0, spacing: 100.0, ..Default::default() };
    let mut ls = LiveStroke::new(80, 80, spec(50.0, params).with_images(None, Some(tex)));
    ls.push([40.0, 40.0]);
    assert!(ink(&ls.buf) > 0);
    // Sin la imagen cargada la textura de imagen no cambia nada.
    let mut plain = LiveStroke::new(80, 80, spec(50.0, params));
    plain.push([40.0, 40.0]);
    let mut none = LiveStroke::new(80, 80, spec(50.0, BrushParams { texture: TexKind::None, ..params }));
    none.push([40.0, 40.0]);
    assert_eq!(plain.buf.data, none.buf.data);
    assert!(ink(&ls.buf) < ink(&plain.buf));

    // Punta de imagen: un cuadrado lleno pinta las esquinas que una punta redonda deja vacías.
    let square = Arc::new(Gray { w: 8, h: 8, data: vec![255; 64] });
    let round = {
        let mut r = LiveStroke::new(100, 100, spec(60.0, BrushParams::default()));
        r.push([50.0, 50.0]);
        r.buf
    };
    let sq = {
        let mut r = LiveStroke::new(100, 100, spec(60.0, BrushParams { use_tip_image: true, ..Default::default() }).with_images(Some(square.clone()), None));
        r.push([50.0, 50.0]);
        r.buf
    };
    assert_eq!(alpha_at(&round, 25, 25), 0);
    assert!(alpha_at(&sq, 25, 25) > 200);
    // Sin marcar «usar punta de imagen», la imagen se ignora.
    let ignored = {
        let mut r = LiveStroke::new(100, 100, spec(60.0, BrushParams::default()).with_images(Some(square), None));
        r.push([50.0, 50.0]);
        r.buf
    };
    assert_eq!(ignored.data, round.data);
}

#[test]
fn gray_from_rgba_uses_alpha_or_darkness() {
    // Con transparencia manda el alfa.
    let g = Gray::from_rgba(2, 1, &[255, 255, 255, 255, 0, 0, 0, 0]).unwrap();
    assert_eq!(g.data, vec![255, 0]);
    // Sin transparencia, lo oscuro es pintura.
    let g = Gray::from_rgba(2, 1, &[0, 0, 0, 255, 255, 255, 255, 255]).unwrap();
    assert_eq!(g.data, vec![255, 0]);
    assert!(Gray::from_rgba(2, 2, &[0; 4]).is_none());
    // Muestreo con repetición.
    let g = Gray { w: 2, h: 1, data: vec![0, 255] };
    assert!((g.sample(0.0, 0.0, true) - 0.0).abs() < 1e-5);
    assert!((g.sample(1.0, 0.0, true) - 1.0).abs() < 1e-5);
    assert!((g.sample(2.0, 0.0, true) - 0.0).abs() < 1e-5);
}

#[test]
fn load_gray_reads_png_and_rejects_garbage() {
    let dir = std::env::temp_dir().join(format!("leontoon_brush_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let png = dir.join("punta.png");
    let mut pm = tiny_skia::Pixmap::new(4, 4).unwrap();
    pm.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 255));
    pm.save_png(&png).unwrap();
    let g = load_gray(&png).unwrap();
    assert_eq!((g.w, g.h), (4, 4));
    assert!(g.data.iter().all(|v| *v == 255));
    let bad = dir.join("no.png");
    std::fs::write(&bad, b"esto no es una imagen").unwrap();
    assert!(load_gray(&bad).is_err());
    assert!(load_gray(&dir.join("falta.png")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dual_brush_cuts_the_primary_tip() {
    let paint = |params: BrushParams| {
        let mut ls = LiveStroke::new(120, 120, spec(70.0, BrushParams { spacing: 100.0, ..params }));
        ls.push([60.0, 60.0]);
        ls.buf
    };
    let plain = paint(BrushParams::default());
    let dual = paint(BrushParams { dual: true, dual_size: 0.2, dual_count: 14, ..Default::default() });
    // El segundo pincel recorta: queda menos pintura, con huecos dentro de la punta, y nunca fuera de ella.
    assert!(ink(&dual) < ink(&plain) / 2, "{} vs {}", ink(&dual), ink(&plain));
    assert!(ink(&dual) > 0);
    let holes = (45..75).flat_map(|y| (45..75).map(move |x| (x, y))).filter(|(x, y)| alpha_at(&dual, *x, *y) < 30).count();
    assert!(holes > 50, "huecos: {holes}");
    assert_eq!(alpha_at(&dual, 5, 5), 0);
    // Más marcas del segundo pincel dejan más pintura.
    let many = paint(BrushParams { dual: true, dual_size: 0.2, dual_count: 40, ..Default::default() });
    assert!(ink(&many) > ink(&dual));
    // Repetible con la misma semilla.
    assert_eq!(paint(BrushParams { dual: true, ..Default::default() }).data, paint(BrushParams { dual: true, ..Default::default() }).data);
}

#[test]
fn color_dynamics_vary_hue_and_blend_with_secondary() {
    let colors = |params: BrushParams, c2: [u8; 3]| {
        let mut ls = LiveStroke::new(260, 60, BrushSpec { seed: 4, ..spec(10.0, BrushParams { spacing: 100.0, ..params }).with_color2(c2) });
        for i in 0..=20 {
            ls.push([15.0 + i as f32 * 11.0, 30.0]);
        }
        let mut v: Vec<[u8; 3]> = (0..230).filter(|x| alpha_at(&ls.buf, 10 + *x, 30) > 250).map(|x| rgb_at(&ls.buf, 10 + x, 30)).collect();
        v.sort();
        v.dedup();
        v
    };
    // Sin variación todas las marcas son del mismo color.
    assert_eq!(colors(BrushParams::default(), [0, 0, 255]).len(), 1);
    // Con variación de tono salen colores distintos.
    assert!(colors(BrushParams { hue_jitter: 1.0, ..Default::default() }, [0, 0, 255]).len() > 8);
    assert!(colors(BrushParams { bri_jitter: 1.0, ..Default::default() }, [0, 0, 255]).len() > 8);
    assert!(colors(BrushParams { sat_jitter: 1.0, ..Default::default() }, [0, 0, 255]).len() > 8);
    // Con variación entre principal y secundario, el azul del secundario aparece en las marcas.
    let mix = colors(BrushParams { fg_bg_jitter: 1.0, ..Default::default() }, [0, 0, 255]);
    assert!(mix.len() > 8);
    assert!(mix.iter().all(|c| c[2] >= 30), "todos entre el rojo y el azul: {mix:?}");
    assert!(mix.iter().any(|c| c[2] > 120));
    // Nada de variación: HSV de ida y vuelta conserva el color.
    for c in [[0.8, 0.1, 0.1], [0.2, 0.6, 0.9], [0.5, 0.5, 0.5], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]] {
        let back = hsv_to_rgb(rgb_to_hsv(c));
        assert!(back.iter().zip(c).all(|(a, b)| (a - b).abs() < 1e-4), "{c:?} -> {back:?}");
    }
}

#[test]
fn wet_edges_darken_the_rim_and_lighten_the_center() {
    let paint = |wet: f32| {
        let mut s = spec(60.0, BrushParams { hardness: 0.3, spacing: 100.0, wet_edges: wet, ..Default::default() });
        s.opacity = 1.0;
        let mut ls = LiveStroke::new(100, 100, s);
        ls.push([50.0, 50.0]);
        let mut out = Raster::new(100, 100);
        ls.finish_into(&mut out);
        out
    };
    let (dry, wet) = (paint(0.0), paint(1.0));
    // El centro se aligera y la zona del borde se marca más.
    assert!(alpha_at(&wet, 50, 50) < alpha_at(&dry, 50, 50) - 60, "centro {} vs {}", alpha_at(&wet, 50, 50), alpha_at(&dry, 50, 50));
    let rim = (0..30).map(|d| alpha_at(&wet, 50 + d, 50) as i32 - alpha_at(&dry, 50 + d, 50) as i32).max().unwrap();
    assert!(rim > 20, "borde {rim}");
    // La vista previa en vivo muestra lo mismo que se compondrá.
    let mut ls = LiveStroke::new(100, 100, spec(60.0, BrushParams { hardness: 0.3, spacing: 100.0, wet_edges: 1.0, ..Default::default() }));
    ls.push([50.0, 50.0]);
    let region = ls.region_rgba([20, 20, 80, 80]);
    let at = ((30 * 60 + 30) * 4 + 3) as usize;
    assert_eq!(region[at], alpha_at(&wet, 50, 50));
    // wet_alpha: sin efecto en 0 y en los extremos.
    assert_eq!(wet_alpha(0.7, 0.0), 0.7);
    assert_eq!(wet_alpha(0.0, 1.0), 0.0);
    assert!(wet_alpha(1.0, 1.0) < 1.0);
}

// ---------------- Compatibilidad ----------------

#[test]
fn params_roundtrip_and_old_presets_use_defaults() {
    let p = BrushParams {
        hardness: 0.3,
        spacing: 40.0,
        flow: 0.5,
        roundness: 0.7,
        angle: 45.0,
        size_control: Control::Pressure,
        flow_jitter: 0.4,
        scatter: 2.0,
        scatter_count: 5,
        texture: TexKind::Canvas,
        texture_mode: TexMode::Subtract,
        dual: true,
        hue_jitter: 0.3,
        wet_edges: 0.6,
        ..Default::default()
    };
    let back: BrushParams = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(back, p);
    // Un preset de la Fase 1 o 2 no trae las dinámicas ni los efectos: quedan apagados.
    let old: BrushParams = serde_json::from_str(r#"{"hardness":0.5,"spacing":20.0,"flow":1.0,"roundness":1.0,"angle":0.0,"size_jitter":0.2}"#).unwrap();
    assert_eq!(old.hardness, 0.5);
    assert_eq!(old.size_jitter, 0.2);
    assert!(!old.has_effects());
    assert_eq!(old.scatter_count, 1);
    assert_eq!(BrushParams { spacing: 9999.0, roundness: 0.0, size_jitter: 5.0, ..p }.clamped().spacing, 200.0);
    assert_eq!(BrushParams { size_jitter: 5.0, ..p }.clamped().size_jitter, 1.0);
    assert_eq!(BrushParams { scatter_count: 200, dual_count: 0, scatter: 99.0, ..p }.clamped().scatter_count, 16);
    assert_eq!(BrushParams { dual_count: 0, ..p }.clamped().dual_count, 1);
    assert!(BrushParams { flow_control: Control::Pressure, ..Default::default() }.has_dynamics());
    assert!(BrushParams { dual: true, ..Default::default() }.has_effects());
    assert!(!BrushParams::default().has_effects() && !BrushParams::default().has_dynamics());
}

#[test]
fn preview_paints_something_inside_the_image_and_shows_pressure() {
    let r = preview(spec(14.0, BrushParams::default()), 200, 56);
    assert!(!r.is_blank());
    assert_eq!(alpha_at(&r, 0, 0), 0);
    // Con control de tamaño por presión, la muestra es más fina en los extremos que en el centro.
    let p = preview(spec(20.0, BrushParams { size_control: Control::Pressure, ..Default::default() }), 240, 60);
    let col_height = |x: u32| (0..p.h).filter(|y| alpha_at(&p, x, *y) > 8).count();
    let ends = col_height(24).max(col_height(1).max(col_height(238)));
    assert!(col_height(120) > ends, "centro {} vs extremos {}", col_height(120), ends);
}
