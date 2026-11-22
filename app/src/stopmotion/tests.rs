//! Pruebas de la capa Stop Motion (Issue #59): almacenamiento, integridad, recuperación y referencia del proyecto.
//! Usan fotos sintéticas (JPEG y PNG creados aquí) y carpetas temporales; no necesitan cámara.

use super::store::{probe_image, read_exif, sha256_hex};
use super::*;
use std::io::Cursor;
use std::path::{Path, PathBuf};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("leontoon_sm_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn finish(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// Imagen de colores distintos según `seed`, para que cada foto tenga un hash distinto.
fn picture(w: u32, h: u32, seed: u8) -> image::RgbImage {
    image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x as u8).wrapping_add(seed.wrapping_mul(37)), (y as u8).wrapping_mul(2), seed.wrapping_mul(91)]))
}

fn jpeg(w: u32, h: u32, seed: u8) -> Vec<u8> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90).encode_image(&picture(w, h, seed)).unwrap();
    out
}

fn png(w: u32, h: u32, seed: u8) -> Vec<u8> {
    let mut out = Vec::new();
    picture(w, h, seed).write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

/// Un JPEG con datos EXIF (fabricante y fecha) metidos a mano justo después del inicio del archivo.
fn jpeg_with_exif(seed: u8) -> Vec<u8> {
    let mut tiff: Vec<u8> = vec![0x49, 0x49, 0x2A, 0x00, 0x08, 0, 0, 0, 2, 0];
    tiff.extend([0x0F, 0x01, 2, 0, 6, 0, 0, 0, 38, 0, 0, 0]); // Make = «Canon»
    tiff.extend([0x32, 0x01, 2, 0, 20, 0, 0, 0, 44, 0, 0, 0]); // DateTime
    tiff.extend([0, 0, 0, 0]);
    tiff.extend(b"Canon\0");
    tiff.extend(b"2026:11:22 20:14:03\0");
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend(tiff);
    let base = jpeg(64, 48, seed);
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    out.extend(((app1.len() + 2) as u16).to_be_bytes());
    out.extend(app1);
    out.extend(&base[2..]);
    out
}

fn new_folder(name: &str) -> (PathBuf, SeqFolder) {
    let dir = temp_dir(name);
    let f = SeqFolder::create(&dir.join("seq"), "Prueba", 12).unwrap();
    (dir, f)
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    v.sort();
    v
}

// ---------- Carpeta y archivo de la secuencia ----------

#[test]
fn creating_a_sequence_makes_the_folders_and_a_valid_file() {
    let (dir, f) = new_folder("crear");
    for sub in [SOURCES, THUMBNAILS, PROXIES, JOURNAL] {
        assert!(f.dir.join(sub).is_dir(), "falta la carpeta {sub}");
    }
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(f.dir.join(SEQUENCE_FILE)).unwrap()).unwrap();
    assert_eq!((json["format"].as_str(), json["version"].as_u64()), (Some(FORMAT), Some(1)));
    assert_eq!(json["id"].as_str().map(str::len), Some(16));
    assert_eq!((json["name"].as_str(), json["fps"].as_u64()), (Some("Prueba"), Some(12)));
    // No pisa una secuencia que ya está ahí.
    assert!(SeqFolder::create(&f.dir, "Otra", 24).unwrap_err().contains("ya tiene una secuencia"));
    let (back, note) = SeqFolder::open(&f.dir).unwrap();
    assert_eq!((back.seq.id.clone(), note), (f.seq.id.clone(), OpenNote::Ok));
    assert_ne!(Sequence::new("a", 12).id, Sequence::new("a", 12).id, "dos secuencias nunca comparten identificador");
    finish(&dir);
}

#[test]
fn opening_a_folder_without_a_sequence_says_so() {
    let dir = temp_dir("sin_secuencia");
    assert!(SeqFolder::open(&dir).unwrap_err().contains("No hay una secuencia"));
    assert!(SeqFolder::open(&dir.join("no_existe")).is_err());
    finish(&dir);
}

#[test]
fn the_sequence_file_is_written_safely_and_can_be_recovered() {
    let (dir, mut f) = new_folder("archivo");
    f.add_photo_bytes(&jpeg(64, 48, 1), None, None).unwrap();
    f.save().unwrap();
    let first = std::fs::read(f.dir.join(SEQUENCE_FILE)).unwrap();
    f.seq.name = "Cambiada".into();
    f.save().unwrap();
    assert_eq!(std::fs::read(f.dir.join("sequence.json.bak")).unwrap(), first, "la copia anterior no coincide");
    assert!(!f.dir.join("sequence.json.tmp").exists());
    // sequence.json dañado: se abre la copia anterior y se avisa.
    std::fs::write(f.dir.join(SEQUENCE_FILE), b"{ esto esta roto").unwrap();
    let (back, note) = SeqFolder::open(&f.dir).unwrap();
    assert_eq!(note, OpenNote::FromBackup);
    assert_eq!((back.seq.name.as_str(), back.seq.sources.len()), ("Prueba", 1));
    // Sin copia que sirva: error claro.
    std::fs::write(f.dir.join("sequence.json.bak"), b"tambien roto").unwrap();
    assert!(SeqFolder::open(&f.dir).unwrap_err().contains("dañado"));
    finish(&dir);
}

#[test]
fn a_newer_or_foreign_sequence_file_is_refused_and_not_modified() {
    let (dir, f) = new_folder("version");
    let path = f.dir.join(SEQUENCE_FILE);
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    v["version"] = serde_json::json!(99);
    let bytes = serde_json::to_vec(&v).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert!(SeqFolder::open(&f.dir).unwrap_err().contains("versión más nueva"));
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "no se debe tocar un archivo de una versión más nueva");
    v["version"] = serde_json::json!(1);
    v["format"] = serde_json::json!("otra-cosa");
    std::fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(SeqFolder::open(&f.dir).is_err());
    finish(&dir);
}

// ---------- Fotos originales ----------

#[test]
fn a_photo_is_kept_byte_for_byte_with_its_hash_size_and_metadata() {
    let (dir, mut f) = new_folder("foto");
    let bytes = jpeg(160, 90, 3);
    let out = f.add_photo_bytes(&bytes, Some(Device { name: "Trust Full HD Webcam".into(), provider: "uvc".into() }), Some(std::time::SystemTime::now())).unwrap();
    assert!(!out.reused);
    let s = f.seq.source(&out.id).unwrap().clone();
    assert_eq!(std::fs::read(f.dir.join(&s.file)).unwrap(), bytes, "el original cambió al guardarlo");
    assert_eq!((s.width, s.height, s.format.as_str(), s.bytes), (160, 90, "jpeg", bytes.len() as u64));
    assert_eq!(s.hash, sha256_hex(&bytes));
    assert!(s.hash.starts_with("sha256:") && s.hash.len() == "sha256:".len() + 64);
    assert!(s.captured_at.as_deref().map_or(false, |d| d.ends_with('Z') && d.len() == 20), "{:?}", s.captured_at);
    assert_eq!(s.device.as_ref().map(|d| d.provider.as_str()), Some("uvc"));
    assert_eq!(s.file, "Sources/frame_000001.jpg");
    finish(&dir);
}

#[test]
fn exif_is_read_for_display_and_stays_inside_the_original() {
    let (dir, mut f) = new_folder("exif");
    let bytes = jpeg_with_exif(5);
    assert_eq!(read_exif(&bytes), Some(ExifInfo { make: Some("Canon".into()), model: None, date_time: Some("2026-11-22T20:14:03".into()) }));
    assert_eq!(read_exif(&jpeg(32, 32, 1)), None, "una foto sin EXIF no inventa datos");
    let out = f.add_photo_bytes(&bytes, None, None).unwrap();
    let s = f.seq.source(&out.id).unwrap();
    assert_eq!(s.exif.as_ref().and_then(|e| e.make.as_deref()), Some("Canon"));
    assert_eq!(s.captured_at.as_deref(), Some("2026-11-22T20:14:03"), "la fecha del EXIF manda sobre la del archivo");
    let stored = std::fs::read(f.dir.join(&s.file)).unwrap();
    assert!(stored.windows(5).any(|w| w == b"Canon"), "el EXIF debe seguir dentro del archivo original");
    finish(&dir);
}

#[test]
fn jpeg_and_png_are_accepted_and_bad_files_are_refused_with_a_reason() {
    let (dir, mut f) = new_folder("formatos");
    assert!(f.add_photo_bytes(&jpeg(40, 30, 1), None, None).is_ok());
    let p = f.add_photo_bytes(&png(40, 30, 2), None, None).unwrap();
    assert_eq!(f.seq.source(&p.id).unwrap().format, "png");
    assert!(f.seq.source(&p.id).unwrap().file.ends_with(".png"));
    assert!(f.add_photo_bytes(b"esto no es una foto", None, None).unwrap_err().contains("no es una foto"));
    assert!(f.add_photo_bytes(&[], None, None).is_err());
    let j = jpeg(200, 150, 3);
    assert!(f.add_photo_bytes(&j[..j.len() / 2], None, None).unwrap_err().contains("incompleta"), "un JPEG cortado no debe entrar");
    let n = png(100, 80, 4);
    assert!(f.add_photo_bytes(&n[..n.len() - 20], None, None).unwrap_err().contains("incompleta"), "un PNG cortado no debe entrar");
    // Lo rechazado no deja nada registrado ni en disco.
    assert_eq!(f.seq.sources.len(), 2);
    assert_eq!(files_in(&f.dir.join(SOURCES)).len(), 2);
    assert_eq!(probe_image(&jpeg(10, 20, 1)).unwrap(), ("jpeg", 10, 20));
    finish(&dir);
}

#[test]
fn photo_numbers_are_never_reused() {
    let (dir, mut f) = new_folder("numeros");
    for i in 0..3 {
        f.add_photo_bytes(&jpeg(32, 32, i), None, None).unwrap();
    }
    // Se borra la última foto y su registro: la siguiente NO reutiliza el número 3.
    let last = f.seq.sources.pop().unwrap();
    std::fs::remove_file(f.dir.join(&last.file)).unwrap();
    let out = f.add_photo_bytes(&jpeg(32, 32, 9), None, None).unwrap();
    assert_eq!(out.id, "s000004");
    // Aunque el contador se perdiera, los archivos que hay en disco lo corrigen.
    f.seq.next_source = 1;
    let out = f.add_photo_bytes(&jpeg(32, 32, 10), None, None).unwrap();
    assert_eq!(out.id, "s000005");
    assert!(f.dir.join("Sources/frame_000005.jpg").is_file());
    finish(&dir);
}

#[test]
fn a_failed_write_leaves_no_trace_and_no_reference() {
    let (dir, mut f) = new_folder("fallo");
    // No se puede escribir en Sources/ (hay un archivo con ese nombre): falla, sin referencias ni temporales.
    std::fs::remove_dir_all(f.dir.join(SOURCES)).unwrap();
    std::fs::write(f.dir.join(SOURCES), b"soy un archivo, no una carpeta").unwrap();
    let err = f.add_photo_bytes(&jpeg(32, 32, 1), None, None).unwrap_err();
    assert!(!err.is_empty());
    assert!(f.seq.sources.is_empty(), "no debe quedar una referencia a una foto que no se guardó");
    assert_eq!(f.seq.next_source, 1, "el contador no avanza si la foto no se guardó");
    assert!(!f.import_files(&[dir.join("no_existe.jpg")]).failed.is_empty());
    std::fs::remove_file(f.dir.join(SOURCES)).unwrap();
    std::fs::create_dir_all(f.dir.join(SOURCES)).unwrap();
    // Un archivo que ya estaba con el nombre que tocaba no se pisa: la foto toma el número siguiente.
    std::fs::write(f.dir.join("Sources/frame_000001.jpg"), b"otro contenido de la persona").unwrap();
    let out = f.add_photo_bytes(&jpeg(32, 32, 2), None, None).unwrap();
    assert_eq!(out.id, "s000002");
    assert_eq!(std::fs::read(f.dir.join("Sources/frame_000001.jpg")).unwrap(), b"otro contenido de la persona");
    assert!(files_in(&f.dir.join(SOURCES)).iter().all(|n| !n.ends_with(".tmp")), "no quedan temporales");
    finish(&dir);
}
#[test]
fn the_same_photo_is_registered_once() {
    let (dir, mut f) = new_folder("duplicada");
    let bytes = jpeg(48, 36, 7);
    let a = f.add_photo_bytes(&bytes, None, None).unwrap();
    let b = f.add_photo_bytes(&bytes, None, None).unwrap();
    assert!(!a.reused && b.reused);
    assert_eq!(a.id, b.id);
    assert_eq!(files_in(&f.dir.join(SOURCES)).len(), 1, "no se copia otra vez el mismo contenido");
    finish(&dir);
}

// ---------- La secuencia: una foto, muchas posiciones ----------

#[test]
fn one_photo_can_be_used_many_times_without_copying_the_file() {
    let (dir, mut f) = new_folder("posiciones");
    let id = f.add_photo_bytes(&jpeg(64, 48, 1), None, None).unwrap().id;
    let other = f.add_photo_bytes(&jpeg(64, 48, 2), None, None).unwrap().id;
    let hash_before = f.seq.source(&id).unwrap().hash.clone();
    let a = f.seq.push_item(&id, 1).unwrap();
    f.seq.push_item(&other, 2).unwrap();
    f.seq.push_item(&id, 4).unwrap();
    assert_eq!(f.seq.total_frames(), 7);
    assert_eq!((f.seq.start_frame(0), f.seq.start_frame(1), f.seq.start_frame(2)), (0, 1, 3));
    assert_eq!((f.seq.item_at_frame(0), f.seq.item_at_frame(2), f.seq.item_at_frame(3), f.seq.item_at_frame(6), f.seq.item_at_frame(7)), (Some(0), Some(1), Some(2), Some(2), None));
    // Repetir, cambiar la exposición y quitar una posición no tocan los archivos.
    let copy = f.seq.duplicate_item(&a).unwrap();
    assert_eq!(f.seq.items[1].id, copy, "la copia queda justo después");
    assert!(f.seq.set_exposure(&copy, 3));
    assert_eq!(f.seq.total_frames(), 10);
    assert!(f.seq.remove_item(&a));
    assert!(!f.seq.remove_item(&a));
    assert_eq!(files_in(&f.dir.join(SOURCES)).len(), 2, "usar una foto varias veces no duplica su archivo");
    assert_eq!(f.seq.source(&id).unwrap().hash, hash_before);
    assert!(f.verify(true).is_empty(), "los originales no cambian al editar la secuencia");
    // Una exposición imposible se corrige; una foto desconocida se rechaza.
    assert!(f.seq.set_exposure(&copy, 0));
    assert_eq!(f.seq.items.iter().find(|i| i.id == copy).unwrap().exposure, 1);
    assert!(f.seq.push_item("s999999", 1).is_err());
    f.save().unwrap();
    let (back, _) = SeqFolder::open(&f.dir).unwrap();
    assert_eq!(back.seq, f.seq, "la secuencia vuelve igual al abrirla");
    finish(&dir);
}

#[test]
fn counters_and_values_are_repaired_when_the_file_was_edited() {
    let mut s = Sequence::new("x", 500);
    assert_eq!(s.fps, 120);
    s.sources.push(SourceFrame { id: "s000007".into(), ..Default::default() });
    s.items.push(Item { id: "i000009".into(), source: "s000007".into(), exposure: 0, visible: true, note: None });
    s.items.push(Item { id: "i000010".into(), source: "no_existe".into(), exposure: 5, visible: true, note: None });
    s.next_source = 1;
    s.next_item = 1;
    s.sanitize();
    assert_eq!(s.items.len(), 1, "una posición sin foto se descarta");
    assert_eq!(s.items[0].exposure, 1);
    assert_eq!((s.next_source, s.next_item), (8, 10), "los contadores superan a los números ya usados");
}

// ---------- Importar ----------

#[test]
fn importing_copies_the_photos_and_never_touches_the_users_files() {
    let (dir, mut f) = new_folder("importar");
    let origin = dir.join("origen");
    std::fs::create_dir_all(&origin).unwrap();
    let good: Vec<PathBuf> = (0..2).map(|i| origin.join(format!("foto{i}.jpg"))).collect();
    for (i, p) in good.iter().enumerate() {
        std::fs::write(p, jpeg(80, 60, i as u8)).unwrap();
    }
    let text = origin.join("notas.txt");
    std::fs::write(&text, "no soy una foto").unwrap();
    let cut = origin.join("cortada.jpg");
    let j = jpeg(200, 150, 9);
    std::fs::write(&cut, &j[..j.len() / 3]).unwrap();
    let before: Vec<Vec<u8>> = [&good[0], &good[1], &text, &cut].iter().map(|p| std::fs::read(p).unwrap()).collect();

    let report = f.import_files(&[good[0].clone(), text.clone(), good[1].clone(), cut.clone(), origin.join("no_existe.jpg")]);
    assert_eq!(report.added.len(), 2);
    assert_eq!(report.failed.len(), 3, "{:?}", report.failed);
    assert!(report.failed.iter().any(|(p, e)| p == &text && e.contains("no es una foto")));
    assert!(report.failed.iter().any(|(p, e)| p == &cut && e.contains("incompleta")));
    assert!(report.failed.iter().any(|(p, e)| p.ends_with("no_existe.jpg") && e.contains("no se pudo leer")));
    let after: Vec<Vec<u8>> = [&good[0], &good[1], &text, &cut].iter().map(|p| std::fs::read(p).unwrap()).collect();
    assert_eq!(before, after, "los archivos de la persona no se tocan");
    assert_eq!((f.seq.sources.len(), f.seq.items.len(), f.seq.total_frames()), (2, 2, 2));
    // Quedó guardado en disco y en el diario.
    let (back, _) = SeqFolder::open(&f.dir).unwrap();
    assert_eq!(back.seq.items.len(), 2);
    assert!(std::fs::read_to_string(f.dir.join("Journal/registro.txt")).unwrap().contains("Importadas 2 fotos"));
    // Importar otra vez las mismas fotos reutiliza los archivos.
    let again = f.import_files(&good);
    assert!(again.added.iter().all(|(_, o)| o.reused));
    assert_eq!(files_in(&f.dir.join(SOURCES)).len(), 2);
    assert_eq!(f.seq.items.len(), 4, "cada importación añade posiciones, aunque la foto ya estuviera");
    finish(&dir);
}

// ---------- Integridad y recuperación ----------

#[test]
fn verification_finds_missing_changed_and_damaged_photos() {
    let (dir, mut f) = new_folder("verificar");
    let ids: Vec<String> = (0..4).map(|i| f.add_photo_bytes(&jpeg(64, 48, i), None, None).unwrap().id).collect();
    assert!(f.verify(true).is_empty() && f.verify(false).is_empty());
    let file = |id: &str| f.dir.join(&f.seq.source(id).unwrap().file);
    std::fs::remove_file(file(&ids[0])).unwrap();
    let mut changed = std::fs::read(file(&ids[1])).unwrap();
    let mid = changed.len() / 2;
    changed[mid] ^= 0xFF;
    std::fs::write(file(&ids[1]), &changed).unwrap(); // mismo tamaño, otro contenido
    let short = std::fs::read(file(&ids[2])).unwrap();
    std::fs::write(file(&ids[2]), &short[..short.len() - 10]).unwrap(); // más corta
    let quick = f.verify(false);
    assert!(quick.contains(&Problem::Missing { source: ids[0].clone(), file: "Sources/frame_000001.jpg".into() }));
    assert!(quick.iter().any(|p| matches!(p, Problem::Damaged { source, reason, .. } if source == &ids[2] && reason.contains("tamaño"))));
    assert_eq!(quick.len(), 2, "la revisión rápida no lee el contenido: {quick:?}");
    let deep = f.verify(true);
    assert_eq!(deep.len(), 3, "{deep:?}");
    assert!(deep.iter().any(|p| matches!(p, Problem::Damaged { source, reason, .. } if source == &ids[1] && reason.contains("contenido"))));
    finish(&dir);
}

#[test]
fn photos_left_behind_by_a_crash_are_found_and_recovered() {
    let (dir, mut f) = new_folder("huerfanas");
    f.add_photo_bytes(&jpeg(64, 48, 1), None, None).unwrap();
    f.seq.push_item("s000001", 2).unwrap();
    f.save().unwrap();
    // Cierre inesperado entre guardar la foto y guardar la secuencia: la foto está en disco pero no en sequence.json.
    f.add_photo_bytes(&jpeg(64, 48, 2), None, None).unwrap();
    // Además, quedó un temporal a medio escribir y un archivo que no es foto.
    std::fs::write(f.dir.join("Sources/frame_000009.tmp"), b"a medias").unwrap();
    std::fs::write(f.dir.join("Sources/notas.txt"), b"hola").unwrap();
    drop(f);

    let (mut f, _) = SeqFolder::open(&SeqFolder::open(&dir.join("seq")).unwrap().0.dir).unwrap();
    assert_eq!(f.seq.sources.len(), 1, "la foto sin registrar no está en la secuencia");
    let names: Vec<String> = f.orphans().iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
    assert_eq!(names, vec!["frame_000002.jpg".to_string(), "notas.txt".to_string()], "el temporal no cuenta como foto huérfana");
    let (recovered, skipped) = f.recover_orphans().unwrap();
    assert_eq!(recovered, 1);
    assert_eq!(skipped.len(), 1, "notas.txt no es una foto");
    assert_eq!(f.seq.items.last().map(|i| i.source.as_str()), Some("s000002"), "se añade al final");
    assert!(f.verify(true).is_empty());
    assert_eq!(f.clean_temp(), 1);
    assert!(!f.dir.join("Sources/frame_000009.tmp").exists());
    let (back, _) = SeqFolder::open(&f.dir).unwrap();
    assert_eq!(back.seq.sources.len(), 2);
    assert!(std::fs::read_to_string(back.dir.join("Journal/registro.txt")).unwrap().contains("huérfanas"));
    finish(&dir);
}

#[test]
fn capturing_appends_the_photo_and_saves_immediately() {
    let (dir, mut f) = new_folder("captura");
    let out = f.append_photo(&jpeg(64, 48, 1), Some(Device { name: "Cámara".into(), provider: "uvc".into() })).unwrap();
    assert!(!out.reused);
    let (back, _) = SeqFolder::open(&f.dir).unwrap();
    assert_eq!((back.seq.sources.len(), back.seq.items.len()), (1, 1), "la captura queda guardada en el acto");
    assert!(f.append_photo(b"basura", None).is_err());
    assert_eq!(SeqFolder::open(&f.dir).unwrap().0.seq.items.len(), 1, "un fallo no deja nada a medias");
    finish(&dir);
}

// ---------- Miniaturas ----------

#[test]
fn thumbnails_are_derived_regenerable_and_leave_the_original_alone() {
    let (dir, mut f) = new_folder("miniaturas");
    let id = f.add_photo_bytes(&jpeg(1200, 800, 1), None, None).unwrap().id;
    let hash = f.seq.source(&id).unwrap().hash.clone();
    let t = f.ensure_thumbnail(&id).unwrap();
    assert!(t.starts_with(f.dir.join(THUMBNAILS)));
    assert_eq!(image::image_dimensions(&t).unwrap(), (320, 213));
    std::fs::remove_file(&t).unwrap();
    assert!(f.ensure_thumbnail(&id).unwrap().is_file(), "una miniatura borrada se regenera");
    assert_eq!(files_in(&f.dir.join(THUMBNAILS)), vec![format!("{id}.jpg")]);
    assert!(f.verify(true).is_empty() && f.seq.source(&id).unwrap().hash == hash, "el original no cambió");
    assert!(f.ensure_thumbnail("s999999").is_err());
    // Una foto pequeña no se agranda.
    let small = f.add_photo_bytes(&jpeg(100, 50, 2), None, None).unwrap().id;
    assert_eq!(image::image_dimensions(f.ensure_thumbnail(&small).unwrap()).unwrap(), (100, 50));
    finish(&dir);
}

// ---------- Referencia del proyecto ----------

#[test]
fn relative_paths_work_in_both_directions() {
    let a = Path::new(if cfg!(windows) { "C:\\proyectos\\peli" } else { "/proyectos/peli" });
    let b = Path::new(if cfg!(windows) { "C:\\proyectos\\fotos\\escena1" } else { "/proyectos/fotos/escena1" });
    assert_eq!(relative_path(a, b).as_deref(), Some("../fotos/escena1"));
    assert_eq!(resolve_relative(a, "../fotos/escena1"), b);
    assert_eq!(relative_path(a, a).as_deref(), Some("."));
    assert_eq!(relative_path(a, &a.join("dentro/mas")).as_deref(), Some("dentro/mas"));
    assert_eq!(resolve_relative(a, "dentro/mas"), a.join("dentro").join("mas"));
    if cfg!(windows) {
        assert_eq!(relative_path(Path::new("C:\\x"), Path::new("D:\\y")), None, "unidades distintas: no hay ruta relativa");
        assert_eq!(relative_path(Path::new("C:\\Proy"), Path::new("c:\\proy\\a")).as_deref(), Some("a"), "Windows no distingue mayúsculas");
    }
}

#[test]
fn the_project_finds_its_folder_after_moving_and_can_relocate_it() {
    let root = temp_dir("referencia");
    let project = root.join("proyecto");
    std::fs::create_dir_all(&project).unwrap();
    let mut f = SeqFolder::create(&project.join("fotos").join("escena1"), "Escena 1", 12).unwrap();
    f.add_photo_bytes(&jpeg(40, 30, 1), None, None).unwrap();
    f.seq.push_item("s000001", 3).unwrap();
    f.save().unwrap();
    let r = SmRef::new(&f, Some(&project));
    assert_eq!(r.rel.as_deref(), Some("fotos/escena1"));
    assert_eq!((r.photos, r.frames, r.fps, r.name.as_str()), (1, 3, 12, "Escena 1"));
    assert_eq!(r.locate(Some(&project)), Located::Found(f.dir.clone()));
    assert_eq!(r.locate(None), Located::Found(f.dir.clone()), "sin carpeta de proyecto sirve la ruta absoluta");

    // Se mueve el proyecto entero (con su carpeta de fotos dentro): la ruta absoluta ya no sirve pero la relativa sí.
    let moved = root.join("proyecto_movido");
    std::fs::rename(&project, &moved).unwrap();
    let Located::Found(dir) = r.locate(Some(&moved)) else { panic!("no se encontró tras mover el proyecto") };
    assert_eq!(dir, moved.join("fotos").join("escena1"));
    let mut r2 = r.clone();
    r2.refresh_paths(Some(&moved));
    assert_eq!(PathBuf::from(&r2.abs), dir, "al guardar se actualiza la ruta absoluta");

    // Se mueve solo la carpeta de fotos a otro sitio: falta; se vuelve a localizar.
    let elsewhere = root.join("discoB").join("escena1_nueva");
    std::fs::create_dir_all(elsewhere.parent().unwrap()).unwrap();
    std::fs::rename(&dir, &elsewhere).unwrap();
    assert_eq!(r2.locate(Some(&moved)), Located::Missing);
    // Una carpeta vacía o con otra secuencia no se acepta.
    let empty = root.join("vacia");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(r2.relocate(&empty, Some(&moved)).unwrap_err().contains("no tiene una secuencia"));
    let other = SeqFolder::create(&root.join("otra"), "Otra", 24).unwrap();
    assert!(r2.relocate(&other.dir, Some(&moved)).unwrap_err().contains("otra secuencia"));
    assert_eq!(r2.locate(Some(&moved)), Located::Missing, "un intento fallido no cambia la referencia");
    // La carpeta correcta sí.
    r2.relocate(&elsewhere, Some(&moved)).unwrap();
    assert_eq!(r2.locate(Some(&moved)), Located::Found(elsewhere.clone()));
    assert_eq!(r2.rel.as_deref(), Some("../discoB/escena1_nueva"));
    // Si en la ruta guardada hay ahora OTRA secuencia, se avisa de cuál es.
    let mut stale = r2.clone();
    stale.abs = other.dir.to_string_lossy().to_string();
    stale.rel = None;
    assert_eq!(stale.locate(Some(&moved)), Located::WrongSequence("Otra".into()));
    finish(&root);
}

// ---------- Forma del archivo y rendimiento ----------

#[test]
fn the_sequence_file_has_the_documented_shape() {
    let (dir, mut f) = new_folder("forma");
    f.add_photo_bytes(&jpeg(32, 24, 1), Some(Device { name: "Trust".into(), provider: "uvc".into() }), None).unwrap();
    f.seq.push_item("s000001", 2).unwrap();
    f.seq.markers.push(Marker { at: 0, kind: "pose".into(), label: "Contacto".into() });
    f.save().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(f.dir.join(SEQUENCE_FILE)).unwrap()).unwrap();
    for k in ["format", "version", "id", "name", "fps", "sources", "items", "markers"] {
        assert!(v.get(k).is_some(), "falta el campo {k}");
    }
    let s = &v["sources"][0];
    for k in ["id", "file", "width", "height", "format", "bytes", "hash", "captured_at", "device"] {
        assert!(s.get(k).is_some(), "falta el campo {k} de la foto");
    }
    assert_eq!((v["items"][0]["source"].as_str(), v["items"][0]["exposure"].as_u64()), (Some("s000001"), Some(2)));
    assert_eq!(v["markers"][0]["label"], "Contacto");
    assert!(!v["sources"][0]["file"].as_str().unwrap().contains('\\'), "las rutas guardadas usan /");
    finish(&dir);
}

#[test]
fn a_sequence_with_thousands_of_photos_saves_and_opens_quickly() {
    let (dir, mut f) = new_folder("miles");
    for i in 0..5000u32 {
        f.seq.sources.push(SourceFrame { id: format!("s{:06}", i + 1), file: format!("Sources/frame_{:06}.jpg", i + 1), width: 1920, height: 1080, format: "jpeg".into(), bytes: 300_000, hash: format!("sha256:{i:064}"), ..Default::default() });
        f.seq.items.push(Item { id: format!("i{:06}", i + 1), source: format!("s{:06}", i + 1), exposure: 2, visible: true, note: None });
    }
    f.seq.sanitize();
    let t = std::time::Instant::now();
    f.save().unwrap();
    let (back, _) = SeqFolder::open(&f.dir).unwrap();
    let ms = t.elapsed().as_millis();
    assert_eq!((back.seq.sources.len(), back.seq.total_frames()), (5000, 10_000));
    assert!(ms < 3000, "guardar y abrir 5000 fotos tardó {ms} ms");
    eprintln!("5000 fotos: guardar y abrir sequence.json en {ms} ms");
    // Buscar el cuadro de una posición en una secuencia larga es inmediato.
    let t = std::time::Instant::now();
    for frame in (0..10_000).step_by(37) {
        assert!(back.seq.item_at_frame(frame).is_some());
    }
    assert!(t.elapsed().as_millis() < 500);
    finish(&dir);
}

// ---------- Edición de la secuencia (Fase 3, #61) ----------

fn edit_seq(n_items: usize) -> (PathBuf, SeqFolder, Vec<String>) {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let (dir, mut f) = new_folder(&format!("edit{}", N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    for s in 0..3u8 {
        f.add_photo_bytes(&png(8, 6, s), None, None).unwrap();
    }
    let mut ids = Vec::new();
    for i in 0..n_items {
        ids.push(f.seq.push_item(&format!("s{:06}", i % 3 + 1), 1 + (i as u32 % 3)).unwrap());
    }
    f.save().unwrap();
    (dir, f, ids)
}

fn order(s: &Sequence) -> Vec<String> {
    s.items.iter().map(|i| i.id.clone()).collect()
}

#[test]
fn items_can_be_moved_to_a_slot_one_or_many() {
    let (dir, mut f, ids) = edit_seq(5);
    // Mover la 1.ª después de la 3.ª (hueco 3).
    assert!(f.seq.move_items(&ids[0..1], 3));
    assert_eq!(order(&f.seq), [ids[1].clone(), ids[2].clone(), ids[0].clone(), ids[3].clone(), ids[4].clone()]);
    // Varias a la vez al principio, conservando su orden relativo.
    assert!(f.seq.move_items(&[ids[3].clone(), ids[4].clone()], 0));
    assert_eq!(order(&f.seq), [ids[3].clone(), ids[4].clone(), ids[1].clone(), ids[2].clone(), ids[0].clone()]);
    // Soltar en su mismo sitio no cambia nada.
    assert!(!f.seq.move_items(&ids[3..4], 0));
    assert!(!f.seq.move_items(&ids[3..4], 1));
    // Al final.
    assert!(f.seq.move_items(&ids[3..4], 99));
    assert_eq!(f.seq.items.last().unwrap().id, ids[3]);
    assert_eq!(f.seq.items.len(), 5);
    finish(&dir);
}

#[test]
fn duplicate_remove_replace_and_blank_never_touch_the_files() {
    let (dir, mut f, ids) = edit_seq(3);
    let before = files_in(&f.dir.join(SOURCES));
    let dup = f.seq.duplicate_items(&ids[0..2]);
    assert_eq!(dup.len(), 2);
    assert_eq!(f.seq.items.len(), 5);
    assert_eq!(f.seq.items[1].source, f.seq.items[0].source);
    let blank = f.seq.insert_blank(1, 3);
    assert_eq!(f.seq.items[1].id, blank);
    assert!(f.seq.items[1].source.is_empty());
    assert_eq!(f.seq.replace_source(&ids[2..3], "s000001").unwrap(), 1);
    assert!(f.seq.replace_source(&ids[2..3], "s999").is_err());
    assert_eq!(f.seq.remove_items(&ids[0..1]), 1);
    f.save().unwrap();
    // Un fotograma vacío sobrevive a guardar y abrir, y las fotos siguen todas en su sitio.
    let (again, _) = SeqFolder::open(&f.dir).unwrap();
    assert!(again.seq.items.iter().any(|i| i.source.is_empty()));
    assert_eq!(files_in(&f.dir.join(SOURCES)), before);
    assert!(again.verify(true).is_empty());
    assert_eq!(again.seq.sources.len(), 3);
    finish(&dir);
}

#[test]
fn exposures_holds_and_freezes_stay_in_range() {
    let (dir, mut f, ids) = edit_seq(3);
    assert_eq!(f.seq.set_exposures(&ids, 4), 3);
    f.seq.add_exposure(&ids[0..1], 6);
    assert_eq!(f.seq.items[0].exposure, 10);
    f.seq.add_exposure(&ids[1..2], -99);
    assert_eq!(f.seq.items[1].exposure, 1, "nunca baja de un cuadro");
    f.seq.add_exposure(&ids[2..3], i32::MAX);
    assert_eq!(f.seq.items[2].exposure, MAX_EXPOSURE);
    let total = f.seq.total_frames();
    let fr = f.seq.freeze(&ids[1], 12).unwrap();
    assert_eq!(f.seq.items[f.seq.index_of(&fr).unwrap()].exposure, 12);
    assert_eq!(f.seq.total_frames(), total + 12);
    assert_eq!(f.seq.items[1].source, f.seq.items[2].source);
    finish(&dir);
}

#[test]
fn markers_and_poses_are_kept_sorted_and_saved() {
    let (dir, mut f, _) = edit_seq(4);
    f.seq.add_marker(5, POSE, "contacto");
    f.seq.add_marker(2, MARKER, "");
    f.seq.add_marker(5, POSE, "contacto fuerte");
    assert_eq!(f.seq.markers.len(), 2, "una pose por cuadro: la segunda cambia el texto");
    assert_eq!(f.seq.markers[0].at, 2);
    assert_eq!(f.seq.markers[1].label, "contacto fuerte");
    f.save().unwrap();
    let (again, _) = SeqFolder::open(&f.dir).unwrap();
    assert_eq!(again.seq.markers, f.seq.markers);
    assert!(!again.seq.markers_in(0).is_empty() || again.seq.markers_in(1).len() + again.seq.markers_in(2).len() + again.seq.markers_in(3).len() > 0);
    assert!(f.seq.remove_marker(0) && !f.seq.remove_marker(7));
    finish(&dir);
}

#[test]
fn every_edit_can_be_undone_and_redone() {
    let (dir, mut f, ids) = edit_seq(4);
    let mut h = History::default();
    let start = f.seq.snapshot();
    h.record(&f.seq);
    f.seq.move_items(&ids[0..1], 4);
    h.record(&f.seq);
    f.seq.remove_items(&ids[1..3]);
    h.record(&f.seq);
    f.seq.add_marker(1, POSE, "x");
    let end = f.seq.snapshot();
    assert!(h.undo(&mut f.seq) && f.seq.markers.is_empty());
    assert!(h.undo(&mut f.seq) && f.seq.items.len() == 4);
    assert!(h.undo(&mut f.seq));
    assert_eq!(f.seq.snapshot(), start);
    assert!(!h.undo(&mut f.seq) && h.can_redo());
    assert!(h.redo(&mut f.seq) && h.redo(&mut f.seq) && h.redo(&mut f.seq));
    assert_eq!(f.seq.snapshot(), end);
    assert!(!h.redo(&mut f.seq));
    // Un cambio nuevo borra lo que se podía rehacer.
    h.undo(&mut f.seq);
    h.record(&f.seq);
    assert!(!h.can_redo());
    finish(&dir);
}

#[test]
fn history_is_bounded() {
    let (dir, f, _) = edit_seq(1);
    let mut h = History::default();
    for _ in 0..(History::LIMIT + 50) {
        h.record(&f.seq);
    }
    let mut s = f.seq.clone();
    let mut n = 0;
    while h.undo(&mut s) {
        n += 1;
    }
    assert_eq!(n, History::LIMIT);
    finish(&dir);
}

#[test]
fn a_thousand_photos_are_listed_and_thumbnailed_without_blocking() {
    let (dir, mut f) = new_folder("thousand");
    for i in 0..1000u32 {
        // Tamaños distintos para que cada foto tenga su propio hash.
        f.add_photo_bytes(&jpeg(96 + i % 50, 54 + i / 50, (i % 7) as u8), None, None).unwrap();
    }
    assert_eq!(f.seq.sources.len(), 1000);
    // Pedir 1000 miniaturas solo apunta trabajo al hilo: volver es inmediato.
    let mut loader = Loader::thumbs();
    let t = std::time::Instant::now();
    for s in &f.seq.sources {
        loader.request((f.seq.id.clone(), s.id.clone()), f.dir.clone(), s.clone());
    }
    assert!(t.elapsed().as_millis() < 500, "pedir las miniaturas tardó {:?}", t.elapsed());
    // Y llegan todas, sin fallos.
    let start = std::time::Instant::now();
    let mut done = 0;
    while done < 1000 && start.elapsed().as_secs() < 120 {
        for (_, r) in loader.poll() {
            assert!(r.is_ok());
            done += 1;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(done, 1000);
    finish(&dir);
}
