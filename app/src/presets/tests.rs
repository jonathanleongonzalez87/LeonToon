//! Pruebas de los pinceles guardados.

use super::*;
use crate::brush::{BrushSpec, LiveStroke};

fn setup(size: f32) -> BrushSetup {
    BrushSetup { size, params: BrushParams { hardness: 0.4, scatter: 1.5, wet_edges: 0.5, ..Default::default() }, ..Default::default() }
}

fn key(c: &str, n: &str) -> PresetKey {
    (c.to_string(), n.to_string())
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("leontoon_presets_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn builtin_presets_are_valid_unique_and_paint() {
    let all = builtin();
    assert!(all.len() >= 10);
    let cats = categories(&[]);
    assert_eq!(cats, vec!["Básicos", "Entintado", "Pintura", "Efectos"]);
    let mut keys: Vec<PresetKey> = all.iter().map(|p| p.key()).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), all.len(), "nombres repetidos");
    for p in &all {
        assert!(p.builtin);
        // Ya están dentro de los rangos.
        assert_eq!(p.setup.clone().clamped(), p.setup, "{}", p.name);
        assert!(p.setup.tip.is_none() && p.setup.texture.is_none());
        // Y pintan algo.
        let spec = BrushSpec::new(p.setup.size, p.setup.opacity, [200, 40, 40], p.setup.params, 3);
        let mut ls = LiveStroke::new(200, 120, spec);
        for i in 0..=30 {
            ls.push([20.0 + i as f32 * 5.0, 60.0]);
        }
        assert!(ls.buf.data.chunks_exact(4).any(|px| px[3] > 0), "{} no pinta", p.name);
    }
}

#[test]
fn save_duplicate_rename_move_and_delete() {
    let mut user: Vec<BrushPreset> = Vec::new();
    // Guardar uno nuevo y que el nombre repetido reciba un número.
    let k1 = save_new(&mut user, "  Mi   pincel ", "Tinta", setup(20.0));
    assert_eq!(k1, key("Tinta", "Mi pincel"));
    let k2 = save_new(&mut user, "Mi pincel", "Tinta", setup(21.0));
    assert_eq!(k2, key("Tinta", "Mi pincel 2"));
    // Sin nombre ni categoría se usan los de por defecto.
    assert_eq!(save_new(&mut user, "", "  ", setup(5.0)), key(USER_CATEGORY, "Pincel"));
    // No se puede ocupar el nombre de un pincel incluido.
    assert_eq!(save_new(&mut user, "Redondo duro", "Básicos", setup(5.0)), key("Básicos", "Redondo duro 2"));
    // Duplicar uno incluido lo copia a «Mis pinceles»; uno propio se copia a su categoría.
    let d1 = duplicate(&mut user, &key("Pintura", "Acuarela")).unwrap();
    assert_eq!(d1, key(USER_CATEGORY, "Acuarela (copia)"));
    assert_eq!(find(&user, &d1).unwrap().setup, find(&user, &key("Pintura", "Acuarela")).unwrap().setup);
    assert!(!find(&user, &d1).unwrap().builtin);
    let d2 = duplicate(&mut user, &k1).unwrap();
    assert_eq!(d2, key("Tinta", "Mi pincel (copia)"));
    // Renombrar.
    let r = rename(&mut user, &k1, "Plumón").unwrap();
    assert_eq!(r, key("Tinta", "Plumón"));
    assert!(find(&user, &k1).is_none());
    assert!(rename(&mut user, &r, "Mi pincel 2").is_err(), "nombre ya ocupado en la categoría");
    assert!(rename(&mut user, &r, "   ").is_err());
    // Cambiar de categoría; no se pisa a otro del mismo nombre.
    let m = set_category(&mut user, &r, "Entintado").unwrap();
    assert_eq!(m, key("Entintado", "Plumón"));
    let _ = save_new(&mut user, "Plumón", "Tinta", setup(2.0));
    assert!(set_category(&mut user, &m, "Tinta").is_err());
    assert!(categories(&user).contains(&"Tinta".to_string()));
    // Actualizar y eliminar.
    update(&mut user, &m, setup(77.0)).unwrap();
    assert_eq!(find(&user, &m).unwrap().setup.size, 77.0);
    remove(&mut user, &m).unwrap();
    assert!(find(&user, &m).is_none());
    assert!(remove(&mut user, &m).is_err(), "ya no existe");
}

#[test]
fn builtin_presets_cannot_be_changed() {
    let mut user: Vec<BrushPreset> = Vec::new();
    let k = key("Básicos", "Lápiz");
    assert!(rename(&mut user, &k, "Otro").unwrap_err().contains("incluidos"));
    assert!(set_category(&mut user, &k, "Otra").is_err());
    assert!(remove(&mut user, &k).is_err());
    assert!(update(&mut user, &k, setup(9.0)).is_err());
    assert!(user.is_empty());
    assert!(duplicate(&mut user, &key("Nada", "Nada")).is_err());
}

#[test]
fn setup_roundtrip_clamps_and_keeps_every_property() {
    let s = BrushSetup {
        params: BrushParams { hardness: 0.2, scatter: 3.0, texture: TexKind::Image, dual: true, hue_jitter: 0.4, wet_edges: 0.7, use_tip_image: true, size_control: Control::Pressure, ..Default::default() },
        size: 55.0,
        opacity: 0.8,
        smoothing: 0.5,
        tip: Some("aaaa".into()),
        texture: Some("bbbb".into()),
    };
    let p = BrushPreset { name: "x".into(), category: "y".into(), setup: s.clone(), builtin: false, notes: vec!["Textura no importada".into()] };
    let back: BrushPreset = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(back, p);
    // Valores imposibles se corrigen.
    let wild = BrushSetup { size: 9999.0, opacity: -3.0, smoothing: 7.0, ..s.clone() }.clamped();
    assert_eq!((wild.size, wild.opacity, wild.smoothing), (1000.0, 0.05, 0.85));
    assert_eq!(BrushSetup { size: f32::NAN, ..s }.clamped().size, 8.0);
    // Un preset con solo un nombre y categoría (archivo antiguo o a mano) trae el pincel por defecto.
    let bare: BrushPreset = serde_json::from_str(r#"{"name":"a","category":"b"}"#).unwrap();
    assert_eq!(bare.setup, BrushSetup::default());
    assert!(!bare.builtin);
}

#[test]
fn images_roundtrip_exactly_including_opaque_and_partial_masks() {
    for data in [vec![255u8; 16], vec![0, 64, 128, 250, 251, 252, 253, 254, 255, 1, 2, 3, 4, 5, 6, 7], vec![0u8; 16]] {
        let g = Gray { w: 4, h: 4, data };
        let im = BrushImage::from_gray("punta.png", &g).unwrap();
        assert_eq!((im.w, im.h), (4, 4));
        assert_eq!(im.id, gray_id(&g));
        let back = im.to_gray().unwrap();
        assert_eq!(back.data, g.data, "la máscara debe volver igual");
        // El PNG pasa por JSON sin cambios.
        let j: BrushImage = serde_json::from_str(&serde_json::to_string(&im).unwrap()).unwrap();
        assert_eq!(j, im);
    }
    // Imágenes distintas, identificadores distintos; la misma, el mismo.
    let a = Gray { w: 2, h: 1, data: vec![1, 2] };
    let b = Gray { w: 1, h: 2, data: vec![1, 2] };
    assert_ne!(gray_id(&a), gray_id(&b));
    assert_eq!(gray_id(&a), gray_id(&a.clone()));
    // Datos dañados dan error, no un cierre del programa.
    let bad = BrushImage { id: "x".into(), name: "x".into(), w: 2, h: 2, png: "%%%".into() };
    assert!(bad.to_gray().is_err());
    let bad2 = BrushImage { png: crate::raster::base64_encode(b"no es un png"), ..bad };
    assert!(bad2.to_gray().is_err());
}

#[test]
fn project_keeps_presets_images_and_state_and_old_projects_still_open() {
    let g = Gray { w: 3, h: 3, data: (0..9).map(|v| v * 30).collect() };
    let im = BrushImage::from_gray("hoja.png", &g).unwrap();
    let mut s = setup(33.0);
    s.tip = Some(im.id.clone());
    s.params.use_tip_image = true;
    let mut p = crate::model::Project::blank(320, 180, 24, 10);
    let k = save_new(&mut p.brush_presets, "Hoja", "Follaje", s.clone());
    p.brush_images.push(im.clone());
    p.brush_state = Some(BrushState { setup: s.clone(), preset: Some(k.clone()) });
    let back: crate::model::Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(back.brush_presets, p.brush_presets);
    assert_eq!(back.brush_images, p.brush_images);
    assert_eq!(back.brush_state, p.brush_state);
    assert_eq!(back.brush_images[0].to_gray().unwrap().data, g.data);
    // Un proyecto guardado antes de existir los pinceles guardados no trae esos campos.
    let mut v: serde_json::Value = serde_json::to_value(&crate::model::Project::blank(320, 180, 24, 10)).unwrap();
    let o = v.as_object_mut().unwrap();
    o.remove("brush_presets");
    o.remove("brush_images");
    o.remove("brush_state");
    let old: crate::model::Project = serde_json::from_value(v).unwrap();
    assert!(old.brush_presets.is_empty() && old.brush_images.is_empty() && old.brush_state.is_none());
}

#[test]
fn sanitize_repairs_ranges_duplicates_and_missing_images() {
    let g = Gray { w: 2, h: 2, data: vec![9; 4] };
    let im = BrushImage::from_gray("a", &g).unwrap();
    let mut images = vec![im.clone(), im.clone(), BrushImage { id: "gigante".into(), name: "g".into(), w: 99_999, h: 3, png: String::new() }];
    let wild = BrushSetup { size: 1e9, params: BrushParams { scatter: 999.0, flow: 0.0, ..Default::default() }, ..Default::default() };
    let missing = BrushSetup {
        tip: Some("no_existe".into()),
        texture: Some("tampoco".into()),
        params: BrushParams { use_tip_image: true, texture: TexKind::Image, ..Default::default() },
        ..Default::default()
    };
    let with_img = BrushSetup { tip: Some(im.id.clone()), params: BrushParams { use_tip_image: true, ..Default::default() }, ..Default::default() };
    let mk = |n: &str, c: &str, s: BrushSetup| BrushPreset { name: n.into(), category: c.into(), setup: s, builtin: true, notes: vec![] };
    let mut user = vec![
        mk("Salvaje", "Prueba", wild),
        mk("Sin imagen", "Prueba", missing),
        mk("Con imagen", "Prueba", with_img),
        mk("Salvaje", "Prueba", BrushSetup::default()),     // repetido: se descarta
        mk("  ", "Prueba", BrushSetup::default()),          // sin nombre: se descarta
        mk("Redondo duro", "Básicos", BrushSetup::default()), // choca con uno incluido: se descarta
        mk("Sin categoría", "", BrushSetup::default()),
    ];
    sanitize(&mut user, &mut images);
    assert_eq!(images.len(), 1, "sin repetidas ni gigantes");
    let names: Vec<&str> = user.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["Salvaje", "Sin imagen", "Con imagen", "Sin categoría"]);
    assert!(user.iter().all(|p| !p.builtin), "los guardados nunca son incluidos");
    assert_eq!(user[0].setup.size, 1000.0);
    assert_eq!(user[0].setup.params.scatter, 10.0);
    assert_eq!(user[0].setup.params.flow, 0.01);
    // Las imágenes que faltan se quitan y la configuración deja de pedirlas.
    assert!(user[1].setup.tip.is_none() && user[1].setup.texture.is_none());
    assert!(!user[1].setup.params.use_tip_image);
    assert_eq!(user[1].setup.params.texture, TexKind::None);
    // La que sí existe se conserva.
    assert_eq!(user[2].setup.tip.as_deref(), Some(im.id.as_str()));
    assert!(user[2].setup.params.use_tip_image);
    assert_eq!(user[3].category, USER_CATEGORY);
    // Lo mismo para el pincel en uso.
    let mut state = Some(BrushState { setup: BrushSetup { tip: Some("nada".into()), size: 5000.0, ..Default::default() }, preset: None });
    sanitize_state(&mut state, &images);
    let st = state.unwrap().setup;
    assert!(st.tip.is_none() && st.size == 1000.0);
}

#[test]
fn library_updates_without_overwriting_and_offers_only_what_the_project_lacks() {
    let g = Gray { w: 2, h: 2, data: vec![255, 0, 0, 255] };
    let im = BrushImage::from_gray("tramado.png", &g).unwrap();
    let spare = BrushImage::from_gray("sobrante.png", &Gray { w: 1, h: 1, data: vec![7] }).unwrap();
    let mut a: Vec<BrushPreset> = Vec::new();
    save_new(&mut a, "Sello", "Tramas", BrushSetup { tip: Some(im.id.clone()), params: BrushParams { use_tip_image: true, ..Default::default() }, ..Default::default() });
    save_new(&mut a, "Liso", "Tramas", setup(10.0));
    let dir = temp_dir("lib");
    // El proyecto A pone sus pinceles; la imagen que no usa ningún pincel no entra.
    update_library(&dir, &a, &[im.clone(), spare], &[]).unwrap();
    let lib = load_library(&dir);
    assert_eq!(lib.presets, a);
    assert_eq!(lib.images, vec![im.clone()]);
    assert!(!dir.join("pinceles.json.tmp").exists(), "no queda el archivo temporal");
    // El proyecto B solo sabe de un pincel suyo y de que se borró «Liso»: no pisa «Sello».
    let mut b: Vec<BrushPreset> = Vec::new();
    save_new(&mut b, "Hojas", "Follaje", setup(55.0));
    update_library(&dir, &b, &[], &[key("Tramas", "Liso")]).unwrap();
    let lib = load_library(&dir);
    let names: Vec<String> = lib.presets.iter().map(|p| format!("{}/{}", p.category, p.name)).collect();
    assert_eq!(names, vec!["Tramas/Sello", "Follaje/Hojas"], "«Sello» sigue y «Liso» se fue");
    assert_eq!(lib.images, vec![im.clone()], "la imagen de «Sello» sigue en uso");
    // Reemplazar uno existente (misma categoría y nombre) cambia su contenido y no lo repite.
    let mut b2 = b.clone();
    b2[0].setup.size = 99.0;
    update_library(&dir, &b2, &[], &[]).unwrap();
    let lib = load_library(&dir);
    assert_eq!(lib.presets.len(), 2);
    assert_eq!(find(&lib.presets, &key("Follaje", "Hojas")).unwrap().setup.size, 99.0);
    // Al quitar el único pincel que usaba la imagen, la imagen se va con él.
    update_library(&dir, &[], &[], &[key("Tramas", "Sello")]).unwrap();
    assert!(load_library(&dir).images.is_empty());
    update_library(&dir, &a, &[im.clone()], &[]).unwrap();

    // Un proyecto nuevo solo recibe lo que pide: se ofrece lo que le falta, con su tamaño aproximado.
    let lib = load_library(&dir);
    let mut proj: Vec<BrushPreset> = Vec::new();
    let mut proj_images: Vec<BrushImage> = Vec::new();
    save_new(&mut proj, "Liso", "Tramas", setup(99.0));
    let offer = library_offer(&proj, &lib);
    let tramas = offer.iter().find(|(c, _, _)| c == "Tramas").unwrap();
    assert_eq!(tramas.1, 1, "solo «Sello»: «Liso» ya lo tiene");
    assert!(tramas.2 > 0, "el tamaño incluye la imagen");
    assert!(offer.iter().any(|(c, n, _)| c == "Follaje" && *n == 1));
    // Traer una categoría trae sus pinceles y sus imágenes, y no pisa los del proyecto.
    let only_tramas = Library { presets: lib.presets.iter().filter(|p| p.category == "Tramas").cloned().collect(), ..lib.clone() };
    assert_eq!(merge_library(&mut proj, &mut proj_images, only_tramas.clone()), 1);
    assert_eq!(find(&proj, &key("Tramas", "Liso")).unwrap().setup.size, 99.0, "el del proyecto no se pisa");
    assert!(find(&proj, &key("Tramas", "Sello")).is_some() && find(&proj, &key("Follaje", "Hojas")).is_none());
    assert_eq!(proj_images.len(), 1);
    assert_eq!(merge_library(&mut proj, &mut proj_images, only_tramas), 0, "una segunda vez no repite");
    assert!(library_offer(&proj, &lib).iter().all(|(c, _, _)| c != "Tramas"));

    // Archivo dañado o ausente: biblioteca vacía, sin fallar; y se puede volver a escribir encima.
    std::fs::write(dir.join("pinceles.json"), "{ esto no es json").unwrap();
    assert!(load_library(&dir).presets.is_empty());
    assert!(load_library(&dir.join("no_existe")).presets.is_empty());
    update_library(&dir, &a, &[im], &[]).unwrap();
    assert_eq!(load_library(&dir).presets.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn presets_reproduce_the_same_stroke_after_a_save_cycle() {
    // Un pincel complejo, guardado y recuperado, pinta exactamente lo mismo.
    let s = BrushSetup {
        params: BrushParams {
            hardness: 0.3,
            size_jitter: 0.4,
            flow_jitter: 0.3,
            scatter: 1.2,
            scatter_count: 3,
            texture: TexKind::Canvas,
            texture_per_tip: true,
            dual: true,
            hue_jitter: 0.5,
            wet_edges: 0.4,
            spacing: 30.0,
            ..Default::default()
        },
        size: 26.0,
        ..Default::default()
    };
    let mut p = crate::model::Project::blank(100, 100, 24, 5);
    let k = save_new(&mut p.brush_presets, "Complejo", "Prueba", s);
    let back: crate::model::Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    let paint = |setup: &BrushSetup| {
        let spec = BrushSpec::new(setup.size, setup.opacity, [30, 90, 200], setup.params, 11);
        let mut ls = LiveStroke::new(200, 100, spec);
        for i in 0..=40 {
            ls.push([15.0 + i as f32 * 4.0, 50.0]);
        }
        let mut out = crate::raster::Raster::new(200, 100);
        ls.finish_into(&mut out);
        out.data
    };
    assert_eq!(paint(&find(&p.brush_presets, &k).unwrap().setup), paint(&find(&back.brush_presets, &k).unwrap().setup));
}
