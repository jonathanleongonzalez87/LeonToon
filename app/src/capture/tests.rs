use super::*;
use std::time::{Duration, Instant};

fn start(m: MockProvider) -> Session {
    let p: Arc<dyn CaptureProvider> = Arc::new(m);
    let dev = p.list().unwrap().remove(0);
    Session::start(p, dev)
}

/// Espera eventos hasta que `pick` devuelve algo (o se acaba el tiempo).
fn wait<T>(s: &Session, mut pick: impl FnMut(Event) -> Option<T>) -> Option<T> {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if let Ok(e) = s.events.recv_timeout(Duration::from_millis(50)) {
            if let Some(v) = pick(e) {
                return Some(v);
            }
        }
    }
    None
}

#[test]
fn la_sesion_abre_y_declara_sus_controles() {
    let s = start(MockProvider::default());
    let (controls, size) = wait(&s, |e| match e {
        Event::Opened { controls, size } => Some((controls, size)),
        _ => None,
    })
    .expect("la cámara debía abrirse");
    assert_eq!(size, (1280, 720));
    assert_eq!(controls.len(), 1);
    assert_eq!(controls[0].label, "Brillo");
}

#[test]
fn un_dispositivo_sin_controles_no_declara_ninguno() {
    let s = start(MockProvider { with_controls: false, ..Default::default() });
    let controls = wait(&s, |e| if let Event::Opened { controls, .. } = e { Some(controls) } else { None }).unwrap();
    assert!(controls.is_empty(), "no deben aparecer controles que el dispositivo no soporta");
    // Pedir un control que no existe avisa y no rompe la sesión.
    s.set_control("Brightness", 10);
    let msg = wait(&s, |e| if let Event::Warning(m) = e { Some(m) } else { None }).expect("debía avisar");
    assert!(msg.contains("Brightness"));
}

#[test]
fn el_live_view_es_pequeno_y_la_foto_tiene_la_resolucion_del_dispositivo() {
    let s = start(MockProvider::default());
    let end = Instant::now() + Duration::from_secs(10);
    let live = loop {
        if let Some(l) = s.take_live() {
            break l;
        }
        assert!(Instant::now() < end, "no llegó el Live View");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(live.width <= LIVE_WIDTH, "el Live View se reduce: {}", live.width);
    assert_eq!(live.full, (1280, 720));
    assert_eq!(live.rgba.len(), (live.width * live.height * 4) as usize);

    s.capture();
    let photo = wait(&s, |e| if let Event::Photo(p) = e { Some(p) } else { None }).expect("debía llegar la foto");
    assert_eq!((photo.width, photo.height), (1280, 720), "la foto es a la resolución del dispositivo, no la del Live View");
    assert!(!photo.png);
    // Es el JPEG que dio la cámara, y se puede abrir con su tamaño completo.
    let img = image::load_from_memory(&photo.bytes).unwrap();
    assert_eq!((img.width(), img.height()), (1280, 720));
}

#[test]
fn una_camara_sin_compresion_da_png_sin_perdidas() {
    let s = start(MockProvider { width: 320, height: 240, jpeg: false, ..Default::default() });
    s.capture();
    let photo = wait(&s, |e| if let Event::Photo(p) = e { Some(p) } else { None }).unwrap();
    assert!(photo.png);
    let img = image::load_from_memory_with_format(&photo.bytes, image::ImageFormat::Png).unwrap();
    assert_eq!((img.width(), img.height()), (320, 240));
}

#[test]
fn si_la_camara_se_desconecta_se_avisa_y_la_sesion_termina() {
    let s = start(MockProvider { fail_after: Some(5), ..Default::default() });
    let msg = wait(&s, |e| if let Event::Failed(m) = e { Some(m) } else { None }).expect("debía avisar de la desconexión");
    assert!(msg.contains("dejó de responder"), "{msg}");
}

#[test]
fn si_no_se_puede_abrir_se_avisa() {
    let p: Arc<dyn CaptureProvider> = Arc::new(MockProvider::default());
    let s = Session::start(p, DeviceInfo { id: "simulada:9".into(), name: "x".into(), provider: "simulada".into() });
    let msg = wait(&s, |e| if let Event::Failed(m) = e { Some(m) } else { None }).unwrap();
    assert!(msg.contains("No se pudo abrir"), "{msg}");
}

#[test]
fn el_proveedor_se_elige_por_el_prefijo_del_id() {
    let ps: Vec<Arc<dyn CaptureProvider>> = vec![Arc::new(WebcamProvider), Arc::new(MockProvider::default())];
    assert_eq!(provider_of(&ps, "simulada:0").unwrap().name(), "simulada");
    assert_eq!(provider_of(&ps, "webcam:1").unwrap().name(), "webcam");
    assert!(provider_of(&ps, "canon:0").is_none());
}

#[test]
fn un_proveedor_que_falla_no_impide_listar_los_demas() {
    struct Broken;
    impl CaptureProvider for Broken {
        fn name(&self) -> &'static str {
            "roto"
        }
        fn list(&self) -> Result<Vec<DeviceInfo>, String> {
            Err("sin permiso".into())
        }
        fn open(&self, _: &str) -> Result<Box<dyn CaptureDevice>, String> {
            Err("no".into())
        }
    }
    let ps: Vec<Arc<dyn CaptureProvider>> = vec![Arc::new(Broken), Arc::new(MockProvider::default())];
    let (devices, errors) = list_all(&ps);
    assert_eq!(devices.len(), 1);
    assert_eq!(errors, vec!["roto: sin permiso".to_string()]);
}
