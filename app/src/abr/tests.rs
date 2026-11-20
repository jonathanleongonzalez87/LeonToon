//! Pruebas de ABR. Las que usan el archivo real de Photoshop se saltan si no está instalado en el equipo.

use super::reader::*;
use super::*;
use crate::brush::TexKind;
use crate::presets::BrushPreset;

/// ABR real de Photoshop v6.2 que trae Substance 3D Painter. No se copia ni se guarda en el repositorio.
fn real_file() -> Option<std::path::PathBuf> {
    let p = std::path::PathBuf::from(r"C:\Program Files\Adobe\Adobe Substance 3D Painter\resources\starter_assets\presets\brushes\Kyle Brush Presets.abr");
    p.is_file().then_some(p)
}

/// Archivo con extensión .abr del mismo programa que NO es un ABR (son metadatos suyos).
fn fake_file() -> Option<std::path::PathBuf> {
    let p = std::path::PathBuf::from(r"C:\Program Files\Adobe\Adobe Substance 3D Painter\resources\starter_assets\presets\brushes\.alg_meta\Kyle Brush Presets.abr");
    p.is_file().then_some(p)
}

// ---------------- Constructores de archivos de prueba ----------------

fn be32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}
fn be16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_be_bytes());
}
fn key_w(v: &mut Vec<u8>, k: &str) {
    if k.len() == 4 {
        be32(v, 0);
    } else {
        be32(v, k.len() as u32);
    }
    v.extend_from_slice(k.as_bytes());
}
fn uni(v: &mut Vec<u8>, s: &str) {
    let u: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    be32(v, u.len() as u32);
    for x in u {
        be16(v, x);
    }
}

/// Constructor de descriptores de Photoshop.
#[derive(Default)]
struct D {
    buf: Vec<u8>,
    n: u32,
}

impl D {
    fn item(&mut self, k: &str, ty: &str) {
        key_w(&mut self.buf, k);
        self.buf.extend_from_slice(ty.as_bytes());
        self.n += 1;
    }
    fn bool(mut self, k: &str, b: bool) -> Self {
        self.item(k, "bool");
        self.buf.push(b as u8);
        self
    }
    fn unit(mut self, k: &str, u: &str, f: f64) -> Self {
        self.item(k, "UntF");
        self.buf.extend_from_slice(u.as_bytes());
        self.buf.extend_from_slice(&f.to_be_bytes());
        self
    }
    fn long(mut self, k: &str, l: i32) -> Self {
        self.item(k, "long");
        be32(&mut self.buf, l as u32);
        self
    }
    fn dbl(mut self, k: &str, f: f64) -> Self {
        self.item(k, "doub");
        self.buf.extend_from_slice(&f.to_be_bytes());
        self
    }
    fn text(mut self, k: &str, t: &str) -> Self {
        self.item(k, "TEXT");
        uni(&mut self.buf, t);
        self
    }
    fn obj(mut self, k: &str, class: &str, child: D) -> Self {
        self.item(k, "Objc");
        let b = child.bytes(class);
        self.buf.extend(b);
        self
    }
    /// Un `brVr` (control dinámico): tipo de control, variación y mínimo.
    fn dynamic(self, k: &str, ctl: i32, jitter: f64, min: f64) -> Self {
        self.obj(k, "brVr", D::default().long("bVTy", ctl).long("fStp", 25).unit("jitter", "#Prc", jitter).unit("Mnm ", "#Prc", min))
    }
    fn bytes(&self, class: &str) -> Vec<u8> {
        let mut v = Vec::new();
        be32(&mut v, 1);
        be16(&mut v, 0); // nombre vacío (una unidad nula)
        key_w(&mut v, class);
        be32(&mut v, self.n);
        v.extend_from_slice(&self.buf);
        v
    }
}

/// Lista de pinceles dentro de la raíz `null`, como la guarda Photoshop.
fn desc_section(presets: Vec<D>) -> Vec<u8> {
    let mut v = Vec::new();
    be32(&mut v, 16); // versión del descriptor
    be32(&mut v, 1);
    be16(&mut v, 0);
    key_w(&mut v, "null");
    be32(&mut v, 1);
    key_w(&mut v, "Brsh");
    v.extend_from_slice(b"VlLs");
    be32(&mut v, presets.len() as u32);
    for p in presets {
        v.extend_from_slice(b"Objc");
        v.extend(p.bytes("brushPreset"));
    }
    v
}

fn samp_entry(minor: u16, uuid: &str, g: &Gray, comp: u8) -> Vec<u8> {
    let header: usize = if minor <= 1 { 47 } else { 301 };
    let mut b = Vec::new();
    b.push(uuid.len() as u8);
    b.extend_from_slice(uuid.as_bytes());
    b.resize(header, 0);
    for v in [0i32, 0, g.h as i32, g.w as i32] {
        b.extend_from_slice(&v.to_be_bytes());
    }
    be16(&mut b, 8);
    b.push(comp);
    if comp == 0 {
        b.extend_from_slice(&g.data);
    } else {
        let rows: Vec<Vec<u8>> = g.data.chunks(g.w as usize).map(packbits).collect();
        for r in &rows {
            be16(&mut b, r.len() as u16);
        }
        for r in &rows {
            b.extend_from_slice(r);
        }
    }
    let mut e = Vec::new();
    be32(&mut e, b.len() as u32);
    e.extend_from_slice(&b);
    while e.len() % 4 != 0 {
        e.push(0);
    }
    e
}

fn section(key: &str, body: &[u8]) -> Vec<u8> {
    let mut v = b"8BIM".to_vec();
    v.extend_from_slice(key.as_bytes());
    be32(&mut v, body.len() as u32);
    v.extend_from_slice(body);
    v
}

fn v6_file(minor: u16, samps: Vec<u8>, desc: Vec<u8>, patt: &[u8]) -> Vec<u8> {
    let mut f = Vec::new();
    be16(&mut f, 6);
    be16(&mut f, minor);
    f.extend(section("samp", &samps));
    if !patt.is_empty() {
        f.extend(section("patt", patt));
    }
    if !desc.is_empty() {
        f.extend(section("desc", &desc));
    }
    f
}

fn gradient(w: u32, h: u32) -> Gray {
    Gray { w, h, data: (0..w * h).map(|i| ((i * 7) % 256) as u8).collect() }
}

fn sampled_preset(name: &str, uuid: &str) -> D {
    D::default().text("Nm  ", name).obj(
        "Brsh",
        "sampledBrush",
        D::default().unit("Dmtr", "#Pxl", 80.0).unit("Angl", "#Ang", 15.0).unit("Rndn", "#Prc", 60.0).text("Nm  ", "forma").unit("Spcn", "#Prc", 12.0).bool("flipX", false).bool("flipY", false).text("sampledData", uuid),
    )
}

// ---------------- PackBits ----------------

#[test]
fn packbits_roundtrips_and_unpacking_never_panics() {
    let cases: Vec<Vec<u8>> = vec![
        vec![],
        vec![5],
        vec![7; 300],
        (0..=255).collect(),
        vec![1, 1, 2, 3, 3, 3, 4, 5, 5],
        (0..500).map(|i| if i % 50 < 25 { 9 } else { (i % 251) as u8 }).collect(),
    ];
    for c in cases {
        assert_eq!(unpackbits(&packbits(&c), c.len()), c, "ida y vuelta de {} bytes", c.len());
    }
    // Datos cortados o sin sentido: rellena con ceros y no se cae.
    assert_eq!(unpackbits(&[], 5), vec![0; 5]);
    assert_eq!(unpackbits(&[0xFE], 3).len(), 3);
    assert_eq!(unpackbits(&[0x80, 0x80, 0x80], 4), vec![0; 4]);
    let mut seed = 12345u32;
    for _ in 0..200 {
        let junk: Vec<u8> = (0..64)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 24) as u8
            })
            .collect();
        assert_eq!(unpackbits(&junk, 100).len(), 100);
    }
}

// ---------------- Versión 6 (Photoshop CS y posteriores) ----------------

#[test]
fn v6_file_maps_shape_dynamics_scatter_transfer_and_color() {
    let tip = gradient(12, 8);
    let rich = sampled_preset("Pincel completo", "uuid-1")
        .bool("useTipDynamics", true)
        .dynamic("szVr", 2, 30.0, 0.0)
        .unit("minimumDiameter", "#Prc", 25.0)
        .dynamic("angleDynamics", 0, 40.0, 0.0)
        .dynamic("roundnessDynamics", 0, 20.0, 0.0)
        .unit("minimumRoundness", "#Prc", 35.0)
        .bool("useScatter", true)
        .dynamic("scatterDynamics", 0, 150.0, 0.0)
        .dbl("Cnt ", 4.0)
        .bool("bothAxes", true)
        .dynamic("countDynamics", 0, 50.0, 0.0)
        .bool("usePaintDynamics", true)
        .dynamic("opVr", 2, 10.0, 51.0)
        .dynamic("prVr", 3, 5.0, 20.0)
        .bool("useColorDynamics", true)
        .unit("H   ", "#Prc", 12.0)
        .unit("Strt", "#Prc", 8.0)
        .unit("Brgh", "#Prc", 6.0)
        .dynamic("clVr", 0, 30.0, 0.0)
        .bool("Wtdg", true)
        .obj("toolOptions", "PbTl", D::default().long("flow", 58).long("Opct", 80).bool("smoothing", false));
    let file = v6_file(2, samp_entry(2, "uuid-1", &tip, 1), desc_section(vec![rich]), &[]);
    let abr = read_abr_bytes(&file).unwrap();
    assert_eq!(abr.version, (6, 2));
    assert_eq!(abr.brushes.len(), 1);
    assert_eq!(abr.tips_found, 1);
    let b = &abr.brushes[0];
    assert_eq!(b.name, "Pincel completo");
    // Forma: llega idéntica, con el tamaño, el espaciado, el ángulo y la redondez del archivo.
    assert_eq!(b.tip.as_ref().unwrap().data, tip.data);
    let (s, p) = (&b.setup, &b.setup.params);
    assert!(p.use_tip_image);
    assert_eq!(s.size, 80.0);
    assert_eq!(p.spacing, 12.0);
    assert_eq!(p.angle, 15.0);
    assert!((p.roundness - 0.6).abs() < 1e-6);
    // Dinámica de forma.
    assert_eq!(p.size_control, Control::Pressure);
    assert!((p.size_jitter - 0.3).abs() < 1e-6 && (p.min_size - 0.25).abs() < 1e-6);
    assert!((p.angle_jitter - 0.4).abs() < 1e-6);
    assert!((p.roundness_jitter - 0.2).abs() < 1e-6 && (p.min_roundness - 0.35).abs() < 1e-6);
    // Dispersión.
    assert!((p.scatter - 1.5).abs() < 1e-6 && p.scatter_both && p.scatter_count == 4 && (p.count_jitter - 0.5).abs() < 1e-6);
    // Transferencia, con sus mínimos.
    assert_eq!((p.opacity_control, p.flow_control), (Control::Pressure, Control::Tilt));
    assert!((p.opacity_jitter - 0.1).abs() < 1e-6 && (p.opacity_min - 0.51).abs() < 1e-6);
    assert!((p.flow_jitter - 0.05).abs() < 1e-6 && (p.flow_min - 0.2).abs() < 1e-6);
    // Color, bordes húmedos, flujo y opacidad de la herramienta.
    assert!((p.hue_jitter - 0.12).abs() < 1e-6 && (p.sat_jitter - 0.08).abs() < 1e-6 && (p.bri_jitter - 0.06).abs() < 1e-6 && (p.fg_bg_jitter - 0.3).abs() < 1e-6);
    assert_eq!(p.wet_edges, 1.0);
    assert!((p.flow - 0.58).abs() < 1e-6 && (s.opacity - 0.8).abs() < 1e-6 && s.smoothing == 0.0);
    // Lo único que no cabe es el máximo de la lista: wet edges avisa que Photoshop no guarda la cantidad.
    assert!(b.notes.iter().any(|n| n.contains("Bordes húmedos")), "{:?}", b.notes);
    assert!(b.notes.iter().any(|n| n.contains("Ángulo") && n.contains("control por lápiz")) == false, "ángulo sin control no debe avisar");
}

#[test]
fn v6_file_reports_everything_it_cannot_represent() {
    let tip = gradient(6, 6);
    let poor = sampled_preset("Con extras", "u")
        .bool("useTexture", true)
        .obj("Txtr", "Ptrn", D::default().text("Nm  ", "Papel rugoso").text("Idnt", "xyz"))
        .obj("dualBrush", "dualBrush", D::default().bool("useDualBrush", true))
        .bool("Nose", true)
        .bool("Rpt ", true)
        .bool("brushProjection", true)
        .bool("useBrushPose", true)
        .bool("useTipDynamics", true)
        .dynamic("szVr", 1, 0.0, 0.0)
        .dynamic("angleDynamics", 3, 10.0, 0.0)
        .bool("useColorDynamics", true)
        .unit("purity", "#Prc", 30.0);
    let patt = vec![0u8; 3_000_000];
    let mut file = v6_file(2, samp_entry(2, "u", &tip, 0), desc_section(vec![poor]), &patt);
    file.extend(section("phry", &[0; 8]));
    let abr = read_abr_bytes(&file).unwrap();
    let n = &abr.brushes[0].notes;
    for needle in ["Textura de patrón «Papel rugoso»", "Doble pincel", "Ruido", "Aerógrafo", "Proyección", "Postura", "«Fundido»", "Ángulo: el control por lápiz", "Pureza"] {
        assert!(n.iter().any(|x| x.contains(needle)), "falta el aviso «{needle}» en {n:?}");
    }
    assert!(abr.warnings.iter().any(|w| w.contains("patrones") && w.contains("3 MB")), "{:?}", abr.warnings);
    assert!(abr.warnings.iter().any(|w| w.contains("carpetas")), "{:?}", abr.warnings);
}

#[test]
fn v6_samp_versions_compression_and_damage() {
    let g = gradient(9, 5);
    // 6.1 y 6.2 (cabeceras de 47 y 301 bytes), con y sin compresión, llegan iguales.
    for (minor, comp) in [(1, 0), (1, 1), (2, 0), (2, 1)] {
        let desc = desc_section(vec![sampled_preset("A", "k")]);
        let abr = read_abr_bytes(&v6_file(minor, samp_entry(minor, "k", &g, comp), desc, &[])).unwrap();
        assert_eq!(abr.brushes[0].tip.as_ref().unwrap().data, g.data, "v6.{minor} compresión {comp}");
    }
    // Una forma que el pincel nombra pero no está en el archivo: se avisa y queda una punta redonda.
    let abr = read_abr_bytes(&v6_file(2, samp_entry(2, "otra", &g, 1), desc_section(vec![sampled_preset("Huérfano", "no_esta")]), &[])).unwrap();
    assert!(abr.brushes[0].tip.is_none() && !abr.brushes[0].setup.params.use_tip_image);
    assert!(abr.brushes[0].notes.iter().any(|n| n.contains("no está en el archivo")));
    // Sin descriptores, cada forma se vuelve un pincel con valores por defecto, y se avisa.
    let abr = read_abr_bytes(&v6_file(2, samp_entry(2, "solo", &g, 1), vec![], &[])).unwrap();
    assert_eq!(abr.brushes.len(), 1);
    assert!(abr.warnings.iter().any(|w| w.contains("no trae parámetros")));
    // Profundidad de 16 bits: se queda con el byte alto.
    let mut e16 = Vec::new();
    e16.push(1u8);
    e16.push(b'z');
    e16.resize(301, 0);
    for v in [0i32, 0, 1, 2] {
        e16.extend_from_slice(&v.to_be_bytes());
    }
    be16(&mut e16, 16);
    e16.push(0);
    e16.extend_from_slice(&[200, 9, 100, 9]);
    let mut samp = Vec::new();
    be32(&mut samp, e16.len() as u32);
    samp.extend(e16);
    while samp.len() % 4 != 0 {
        samp.push(0);
    }
    let abr = read_abr_bytes(&v6_file(2, samp, vec![], &[])).unwrap();
    assert_eq!(abr.brushes[0].tip.as_ref().unwrap().data, vec![200, 100]);
}

#[test]
fn damaged_and_foreign_files_give_clear_errors_and_never_panic() {
    // Un texto cualquiera con extensión .abr.
    let e = read_abr_bytes(b"Esto no es un archivo ABR de Photoshop, es texto.").unwrap_err();
    assert!(e.contains("no es un ABR"), "{e}");
    assert!(read_abr_bytes(&[]).is_err());
    assert!(read_abr_bytes(&[0, 6]).is_err());
    // Recorte en cada punto de un archivo v6 y de uno v2: nunca se cae.
    let g = gradient(10, 10);
    let v6 = v6_file(2, samp_entry(2, "k", &g, 1), desc_section(vec![sampled_preset("A", "k"), sampled_preset("B", "k")]), &[0; 64]);
    let exported = export_v2(&[(BrushPreset { name: "forma".into(), category: "c".into(), setup: Default::default(), builtin: false, notes: vec![] }, None)]).unwrap().0;
    for file in [&v6, &exported] {
        for cut in 0..file.len() {
            let _ = read_abr_bytes(&file[..cut]);
        }
    }
    // Una sección que miente sobre su largo.
    let mut lie = v6.clone();
    lie[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(read_abr_bytes(&lie).unwrap_err().contains("recortado o dañado"));
    // Descriptor con un largo imposible y con anidación excesiva.
    let mut bad = desc_section(vec![D::default().text("Nm  ", "x")]);
    let n = bad.len();
    bad[n - 12..n - 8].copy_from_slice(&u32::MAX.to_be_bytes());
    let abr = read_abr_bytes(&v6_file(2, samp_entry(2, "k", &g, 1), bad, &[])).unwrap();
    assert!(abr.warnings.iter().any(|w| w.contains("No se pudieron leer los parámetros")) || abr.brushes.len() <= 1);
    // Bytes al azar detrás de una cabecera válida.
    let mut seed = 99u32;
    for ver in [1u16, 2, 6] {
        for _ in 0..60 {
            let mut f = Vec::new();
            be16(&mut f, ver);
            be16(&mut f, 2);
            for _ in 0..200 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                f.push((seed >> 24) as u8);
            }
            let _ = read_abr_bytes(&f);
        }
    }
}

// ---------------- Versión 2 y exportación ----------------

fn preset(name: &str, setup: BrushSetup) -> BrushPreset {
    BrushPreset { name: name.into(), category: "Prueba".into(), setup, builtin: false, notes: vec![] }
}

#[test]
fn export_v2_roundtrips_round_and_sampled_brushes() {
    let g = gradient(14, 9);
    let round = BrushSetup { size: 50.0, params: BrushParams { hardness: 0.6, spacing: 25.0, roundness: 0.7, angle: 30.0, ..Default::default() }, ..Default::default() };
    let sampled = BrushSetup { size: 40.0, params: BrushParams { use_tip_image: true, spacing: 18.0, ..Default::default() }, ..Default::default() };
    let (bytes, rep) = export_v2(&[(preset("Redondo", round), None), (preset("Hoja ñandú", sampled), Some(g.clone()))]).unwrap();
    assert_eq!(rep.written, 2);
    let back = read_abr_bytes(&bytes).unwrap();
    assert_eq!(back.version, (2, 0));
    assert_eq!(back.brushes.len(), 2);
    let (a, b) = (&back.brushes[0], &back.brushes[1]);
    // Redondo: diámetro, dureza, espaciado, redondez y ángulo.
    assert_eq!(a.setup.size, 50.0);
    assert!((a.setup.params.hardness - 0.6).abs() < 1e-6 && a.setup.params.spacing == 25.0);
    assert!((a.setup.params.roundness - 0.7).abs() < 1e-6 && a.setup.params.angle == 30.0);
    assert!(a.tip.is_none());
    // Con forma: nombre (con eñe), espaciado y la imagen exacta.
    assert_eq!(b.name, "Hoja ñandú");
    assert_eq!(b.setup.params.spacing, 18.0);
    assert_eq!(b.tip.as_ref().unwrap().data, g.data);
    assert_eq!((b.tip.as_ref().unwrap().w, b.tip.as_ref().unwrap().h), (14, 9));
    assert!(b.setup.params.use_tip_image);
}

#[test]
fn export_v2_reports_every_property_it_cannot_store() {
    let rich = BrushSetup {
        size: 30.0,
        opacity: 0.6,
        smoothing: 0.35,
        params: BrushParams {
            hardness: 0.5,
            flow: 0.4,
            size_control: Control::Pressure,
            angle_jitter: 0.3,
            opacity_control: Control::Pressure,
            scatter: 1.0,
            texture: TexKind::Paper,
            dual: true,
            hue_jitter: 0.2,
            wet_edges: 0.5,
            ..Default::default()
        },
        ..Default::default()
    };
    let (_, rep) = export_v2(&[(preset("Rico", rich), None), (preset("Simple", BrushSetup { smoothing: 0.0, ..Default::default() }), None)]).unwrap();
    let rico = &rep.per_brush[0].1;
    for needle in ["nombre", "opacidad de 60 %", "flujo de 40 %", "suavizado", "dinámica de forma", "transferencia", "dispersión", "textura", "doble pincel", "dinámica de color", "bordes húmedos"] {
        assert!(rico.iter().any(|n| n.contains(needle)), "falta «{needle}» en {rico:?}");
    }
    // Un pincel redondo simple solo pierde el nombre.
    assert_eq!(rep.per_brush[1].1.len(), 1);
    assert!(rep.general.iter().any(|g| g.contains("no se ha abierto en Photoshop")));
    // Una forma de imagen cuenta que pierde el tamaño, y una forma que falta se avisa.
    let tipped = BrushSetup { params: BrushParams { use_tip_image: true, hardness: 0.3, ..Default::default() }, ..Default::default() };
    let (_, rep) = export_v2(&[(preset("Sello", tipped.clone()), Some(gradient(4, 4))), (preset("Sin forma", tipped), None)]).unwrap();
    assert!(rep.per_brush[0].1.iter().any(|n| n.contains("tamaño")) && rep.per_brush[0].1.iter().any(|n| n.contains("dureza")));
    assert!(rep.per_brush[1].1.iter().any(|n| n.contains("no está disponible")));
    // Casos que no se pueden escribir.
    assert!(export_v2(&[]).is_err());
    let huge = Gray { w: 2600, h: 2, data: vec![0; 5200] };
    assert!(export_v2(&[(preset("Enorme", BrushSetup { params: BrushParams { use_tip_image: true, ..Default::default() }, ..Default::default() }), Some(huge))]).is_err());
}

#[test]
fn classic_v1_brushes_without_name_are_read() {
    // ABR v1: un pincel calculado y una forma sin nombre (el nombre solo existe desde la versión 2).
    let g = gradient(5, 4);
    let mut f = Vec::new();
    be16(&mut f, 1);
    be16(&mut f, 2);
    // Calculado: misc, espaciado, diámetro, redondez, dureza, ángulo.
    let mut c = Vec::new();
    be32(&mut c, 0);
    for v in [25u16, 40, 100, 80, 0] {
        be16(&mut c, v);
    }
    be16(&mut f, 1);
    be32(&mut f, c.len() as u32);
    f.extend(c);
    // Muestreado sin nombre.
    let mut s = Vec::new();
    be32(&mut s, 0);
    be16(&mut s, 30);
    s.push(1);
    for v in [0u16, 0, 4, 5] {
        be16(&mut s, v);
    }
    for v in [0i32, 0, 4, 5] {
        s.extend_from_slice(&v.to_be_bytes());
    }
    be16(&mut s, 8);
    s.push(0);
    s.extend_from_slice(&g.data);
    be16(&mut f, 2);
    be32(&mut f, s.len() as u32);
    f.extend(s);
    let abr = read_abr_bytes(&f).unwrap();
    assert_eq!(abr.version.0, 1);
    assert_eq!(abr.brushes.len(), 2);
    assert_eq!((abr.brushes[0].setup.size, abr.brushes[0].setup.params.spacing), (40.0, 25.0));
    assert!((abr.brushes[0].setup.params.hardness - 0.8).abs() < 1e-6);
    assert_eq!(abr.brushes[1].tip.as_ref().unwrap().data, g.data);
    assert!(abr.brushes[1].name.starts_with("Muestra"));
}

// ---------------- Archivos reales ----------------

#[test]
fn real_photoshop_file_is_read_with_its_tips_and_parameters() {
    let Some(p) = real_file() else {
        eprintln!("(saltada: no está el ABR real de Photoshop en este equipo)");
        return;
    };
    let t0 = std::time::Instant::now();
    let abr = read_abr(&p).unwrap();
    assert!(t0.elapsed().as_secs() < 20, "leer 167 MB (saltando los patrones) tardó demasiado: {:?}", t0.elapsed());
    assert_eq!(abr.version, (6, 2));
    assert_eq!(abr.brushes.len(), 102);
    assert_eq!(abr.tips_found, 100);
    assert!(abr.warnings.iter().any(|w| w.contains("patrones") && w.contains("MB")), "{:?}", abr.warnings);
    assert!(abr.warnings.iter().any(|w| w.contains("carpetas")));
    // El primer pincel, comprobado contra lo que muestra el propio archivo.
    let b = &abr.brushes[0];
    assert_eq!(b.name, "Kyle's Real Watercolor - Bleeding Edges");
    let (s, q) = (&b.setup, &b.setup.params);
    assert_eq!(s.size, 125.0);
    assert_eq!(q.spacing, 1.0);
    assert!((q.flow - 0.58).abs() < 1e-6);
    assert!(q.use_tip_image);
    let tip = b.tip.as_ref().unwrap();
    assert_eq!((tip.w, tip.h), (103, 98));
    assert!(tip.data.iter().any(|v| *v > 100) && tip.data.iter().any(|v| *v < 100), "la forma debe tener tinta y fondo");
    assert_eq!(q.size_control, Control::Pressure);
    assert!((q.size_jitter - 0.39).abs() < 1e-6);
    assert_eq!(q.scatter_count, 8);
    assert!(q.scatter_both);
    assert!((q.opacity_min - 0.51).abs() < 1e-6 && q.opacity_control == Control::Pressure);
    assert!((q.min_roundness - 0.40).abs() < 1e-6);
    for needle in ["Textura de patrón «Kyle WC Seamless 1»", "Doble pincel"] {
        assert!(b.notes.iter().any(|n| n.contains(needle)), "falta «{needle}» en {:?}", b.notes);
    }
    // Todos tienen nombre, valores dentro de rango y las formas con tinta.
    for b in &abr.brushes {
        assert!(!b.name.is_empty());
        assert_eq!(b.setup.clone().clamped(), b.setup, "{} fuera de rango", b.name);
        if let Some(t) = &b.tip {
            // Algunas formas son tenues a propósito (por ejemplo «Salt Fine» llega a 75 de 255): basta con que tengan tinta.
            assert!(t.data.iter().any(|v| *v > 0), "{}: forma vacía", b.name);
        }
    }
    // Hay pinceles con dispersión grande (553 %) y diámetros de 430 px que ya caben.
    assert!(abr.brushes.iter().any(|b| b.setup.params.scatter > 5.0));
    assert!(abr.brushes.iter().any(|b| b.setup.size > 400.0));
}

#[test]
fn a_dab_from_a_real_tip_keeps_its_proportions() {
    let Some(p) = real_file() else { return };
    let abr = read_abr(&p).unwrap();
    // «Bleeding Edges» tiene una forma de 103×98; una más alta que ancha comprueba que no se estira.
    let tall = abr.brushes.iter().find(|b| b.tip.as_ref().map_or(false, |t| t.h > t.w + 40)).expect("alguna forma alta");
    let t = tall.tip.clone().unwrap();
    let spec = crate::brush::BrushSpec::new(200.0, 1.0, [0, 0, 0], BrushParams { use_tip_image: true, spacing: 100.0, ..Default::default() }, 1).with_images(Some(std::sync::Arc::new(t.clone())), None);
    let mut ls = crate::brush::LiveStroke::new(300, 300, spec);
    ls.push([150.0, 150.0]);
    let alpha = |x: u32, y: u32| ls.buf.data[((y * 300 + x) * 4 + 3) as usize];
    let cols = (0..300).filter(|x| (0..300).any(|y| alpha(*x, y) > 8)).count() as f32;
    let rows = (0..300).filter(|y| (0..300).any(|x| alpha(x, *y) > 8)).count() as f32;
    let want = t.w as f32 / t.h as f32;
    assert!((cols / rows - want).abs() < 0.12, "proporción de la marca {:.2} frente a la de la imagen {:.2}", cols / rows, want);
    assert!(rows > 150.0, "el lado mayor mide el diámetro");
}

#[test]
fn foreign_abr_extension_file_is_rejected_clearly() {
    let Some(p) = fake_file() else { return };
    let e = read_abr(&p).unwrap_err();
    assert!(e.contains("no es un ABR de Photoshop"), "{e}");
    assert!(e.contains("otros programas"));
}
