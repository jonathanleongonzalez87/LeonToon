//! Papeles de estudio de la Mesa de luz (Issue #52): catálogo de papeles de animación y grano procedural.
//!
//! Cada papel tiene un color base y un tipo de grano. El grano es una textura de 512×512 px **sin costuras**,
//! generada en código (no hay archivos de imagen), con valores de -1 (fibra oscura) a +1 (fibra clara). Se guarda
//! como blanco y negro con transparencia, de modo que dibujarla sobre la hoja equivale a mezclar con Overlay.
//!
//! Para que no se note que la textura se repite, el grano es orgánico (ruido deformado, fibras curvas, manchas de
//! densidad, guijarros irregulares) y sobre la hoja se mezclan tres capas de la misma textura con tamaños
//! incomensurables, giradas y desplazadas (`GRAIN_LAYERS`): el patrón conjunto no se repite a la vista.
//! Su fuerza depende de la intensidad de la luz de la mesa: con más luz el papel se «retroilumina» y la fibra se ve más.
//!
//! Los tonos, la fuerza del grano y la translucidez son aproximaciones: el Issue describe cada papel con palabras.

/// Tipo de fibra del papel.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Grain {
    /// Bond antiguo: grano irregular y poroso.
    BondRugoso,
    /// Fibra de algodón satinada.
    AlgodonSatinado,
    /// Grano fino y uniforme.
    Fino,
    /// Celulosa estándar.
    Celulosa,
    /// Fibra lisa, casi sin relieve.
    Liso,
    /// Prensado en frío (cold-press): relieve visible.
    PrensadoFrio,
}

impl Grain {
    pub const ALL: [Grain; 6] = [Grain::BondRugoso, Grain::AlgodonSatinado, Grain::Fino, Grain::Celulosa, Grain::Liso, Grain::PrensadoFrio];
}

/// Un papel del catálogo.
pub struct Paper {
    pub id: &'static str,
    pub name: &'static str,
    pub era: &'static str,
    pub note: &'static str,
    pub rgb: [u8; 3],
    pub grain: Grain,
    /// Cuánto se nota el grano (0 a 1).
    pub strength: f32,
    /// Cuánta luz atraviesa el papel (0 opaco, 1 casi transparente).
    pub translucency: f32,
}

impl Paper {
    /// Texto del tooltip: nombre, época y la descripción del Issue.
    pub fn tooltip(&self) -> String {
        format!("{} — {}\n{}", self.name, self.era, self.note)
    }

    /// Opacidad del grano (0 a ~0,34) con la luz de la mesa (0 a 1). Con luz baja el grano se suaviza hasta un
    /// mínimo; los papeles más translúcidos tienen un mínimo menor, así que responden más a la luz.
    pub fn grain_alpha(&self, light: f32) -> f32 {
        let light = light.clamp(0.0, 1.0);
        let floor = 0.05 + 0.3 * (1.0 - self.translucency.clamp(0.0, 1.0));
        0.34 * self.strength * (floor + (1.0 - floor) * light)
    }
}

pub const PAPERS: [Paper; 9] = [
    Paper { id: "bond16", name: "Bond Genérico 16 lb", era: "Era Pionera (1910s)", note: "Blanco hueso cálido, grano irregular pronunciado, alta porosidad", rgb: [242, 234, 214], grain: Grain::BondRugoso, strength: 0.95, translucency: 0.45 },
    Paper { id: "ingram24", name: "Ingram Bond 24 lb", era: "Edad de Oro (1930s–70s)", note: "Marfil suave, fibra de algodón satinada, baja translucidez", rgb: [238, 229, 203], grain: Grain::AlgodonSatinado, strength: 0.5, translucency: 0.25 },
    Paper { id: "chromacolour", name: "Chromacolour Pro-Grade (75 g/m²)", era: "Estándar Occidental (1980s–90s)", note: "Blanco neutro calibrado, grano fino uniforme, translucidez media-alta", rgb: [246, 246, 243], grain: Grain::Fino, strength: 0.35, translucency: 0.7 },
    Paper { id: "cartoon20", name: "Cartoon Colour 20 lb", era: "Estándar Escolar/TV (1980s)", note: "Blanco ligeramente frío, textura celulosa estándar", rgb: [243, 247, 251], grain: Grain::Celulosa, strength: 0.55, translucency: 0.5 },
    Paper { id: "genga", name: "Papel Genga (60 g/m²)", era: "Anime - Genga (Japón)", note: "Blanco/crema ultra ligero, fibra lisa de alta transparencia", rgb: [251, 249, 241], grain: Grain::Liso, strength: 0.22, translucency: 0.95 },
    Paper { id: "douga-amarillo", name: "Papel Douga / Sakkan — Amarillo pálido", era: "Anime - Corrección/Intercalado (Japón)", note: "Tono pastel funcional, grano fino", rgb: [252, 246, 196], grain: Grain::Fino, strength: 0.3, translucency: 0.8 },
    Paper { id: "douga-menta", name: "Papel Douga / Sakkan — Verde menta", era: "Anime - Corrección/Intercalado (Japón)", note: "Tono pastel funcional, grano fino", rgb: [212, 240, 222], grain: Grain::Fino, strength: 0.3, translucency: 0.8 },
    Paper { id: "douga-rosa", name: "Papel Douga / Sakkan — Rosa tenue", era: "Anime - Corrección/Intercalado (Japón)", note: "Tono pastel funcional, grano fino", rgb: [250, 222, 230], grain: Grain::Fino, strength: 0.3, translucency: 0.8 },
    Paper { id: "tmk", name: "TMK Poster Paper", era: "Fondos Tradicionales (Ghibli)", note: "Blanco natural con textura prensada en frío (cold-press) visible", rgb: [250, 248, 242], grain: Grain::PrensadoFrio, strength: 0.85, translucency: 0.3 },
];

/// El papel de estudio cuyo color es el de la hoja, si lo hay. Así el grano se apaga solo si se elige otro color.
pub fn for_background(bg: [u8; 4]) -> Option<&'static Paper> {
    PAPERS.iter().find(|p| p.rgb == [bg[0], bg[1], bg[2]])
}

/// Lado de la textura de grano, en píxeles.
pub const TILE: usize = 512;

/// Tamaño de un píxel de la textura sobre la hoja, en píxeles de la escena, con la escala al 100 %.
pub const TEXEL: f32 = 0.75;

/// Escala del grano (porcentaje) que se puede elegir.
pub const SCALE_MIN: f32 = 40.0;
pub const SCALE_MAX: f32 = 400.0;
pub const SCALE_DEFAULT: f32 = 100.0;

/// Una capa de grano sobre la hoja: tamaño relativo, giro en grados, desplazamiento (en teselas) y peso.
pub struct GrainLayer {
    pub size: f32,
    pub angle: f32,
    pub offset: [f32; 2],
    pub weight: f32,
}

/// Tres capas con tamaños que no son múltiplos entre sí y giros distintos: el patrón combinado no se repite a la vista.
pub const GRAIN_LAYERS: [GrainLayer; 3] = [
    GrainLayer { size: 1.0, angle: 0.0, offset: [0.0, 0.0], weight: 1.0 },
    GrainLayer { size: 0.713, angle: 33.0, offset: [0.37, 0.61], weight: 0.7 },
    GrainLayer { size: 1.311, angle: -21.0, offset: [0.83, 0.29], weight: 0.5 },
];

/// Cuánto del grano se ve según el tamaño de un píxel de textura en pantalla: muy pequeño se funde (evita el moaré).
pub fn grain_fade(texel_px: f32) -> f32 {
    ((texel_px - 0.3) / 0.25).clamp(0.0, 1.0)
}

/// Peso total de las capas, para repartir la opacidad (cada capa se dibuja con `weight / suma`·1,25).
pub fn layers_norm() -> f32 {
    GRAIN_LAYERS.iter().map(|l| l.weight * l.weight).sum::<f32>().sqrt()
}

fn hash(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263) ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    h as f32 / u32::MAX as f32 * 2.0 - 1.0
}

fn hash01(x: u32, y: u32, seed: u32) -> f32 {
    (hash(x, y, seed) + 1.0) * 0.5
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Ruido de valores sin costuras: `u` y `v` dan la vuelta (cualquier valor) y la retícula tiene periodo `freq`.
fn value_noise(u: f32, v: f32, freq: u32, seed: u32) -> f32 {
    let (u, v) = (u.rem_euclid(1.0), v.rem_euclid(1.0));
    let (x, y) = (u * freq as f32, v * freq as f32);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (smooth(x - x0), smooth(y - y0));
    let (ix, iy) = (x0 as u32 % freq, y0 as u32 % freq);
    let (jx, jy) = ((ix + 1) % freq, (iy + 1) % freq);
    let (a, b) = (hash(ix, iy, seed), hash(jx, iy, seed));
    let (c, d) = (hash(ix, jy, seed), hash(jx, jy, seed));
    let top = a + (b - a) * fx;
    let bot = c + (d - c) * fx;
    top + (bot - top) * fy
}

/// Distancia al punto de rejilla más cercano (ruido celular sin costuras): da guijarros irregulares.
fn worley(u: f32, v: f32, freq: u32, seed: u32) -> f32 {
    let (u, v) = (u.rem_euclid(1.0), v.rem_euclid(1.0));
    let (x, y) = (u * freq as f32, v * freq as f32);
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let mut best = f32::MAX;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (cx, cy) = (xi + dx, yi + dy);
            let (wx, wy) = (cx.rem_euclid(freq as i32) as u32, cy.rem_euclid(freq as i32) as u32);
            let px = cx as f32 + hash01(wx, wy, seed);
            let py = cy as f32 + hash01(wx, wy, seed ^ 0x55aa);
            let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            best = best.min(d);
        }
    }
    best
}

struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Rng {
        Rng(seed.wrapping_mul(2_654_435_761) ^ 0x9e37_79b9)
    }

    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Suma fibras curvas (caminos que giran suavemente y dan la vuelta por los bordes, para que la textura siga sin costuras).
fn add_fibers(buf: &mut [f32], count: usize, len: f32, amp: f32, seed: u32) {
    let mut rng = Rng::new(seed);
    let n = TILE as i32;
    for _ in 0..count {
        let (mut x, mut y) = (rng.next() * TILE as f32, rng.next() * TILE as f32);
        let mut ang = rng.next() * std::f32::consts::TAU;
        let bend = (rng.next() - 0.5) * 0.06;
        let l = len * (0.4 + 1.2 * rng.next());
        let a = amp * (rng.next() * 2.0 - 1.0) * (0.5 + rng.next());
        for _ in 0..l.ceil() as usize {
            let (ix, iy) = (x.round() as i32, y.round() as i32);
            for (dx, dy, w) in [(0, 0, 1.0), (1, 0, 0.35), (-1, 0, 0.35), (0, 1, 0.35), (0, -1, 0.35)] {
                let px = (ix + dx).rem_euclid(n) as usize;
                let py = (iy + dy).rem_euclid(n) as usize;
                buf[py * TILE + px] += a * w;
            }
            ang += bend + (rng.next() - 0.5) * 0.08;
            x += ang.cos();
            y += ang.sin();
        }
    }
}

/// Parámetros de cada tipo de grano. Las frecuencias son para una textura de 512 px (el doble que la anterior de 256,
/// para conservar el tamaño del detalle).
struct Recipe {
    /// Octavas de ruido: (frecuencia, peso).
    octaves: &'static [(u32, f32)],
    /// Cuánto se deforma el dominio (0 = nada): vuelve el ruido orgánico en vez de regular.
    warp: f32,
    /// Fibras: cantidad, largo y fuerza.
    fibers: (usize, f32, f32),
    /// Guijarros (ruido celular): frecuencia y peso; 0 = no hay.
    pebbles: (u32, f32),
    /// Relieve: simula luz rasante.
    emboss: bool,
    /// Manchas de densidad (variación lenta de la fuerza del grano).
    clumps: f32,
    /// Ruido fino por píxel.
    speckle: f32,
}

fn recipe(kind: Grain) -> Recipe {
    match kind {
        Grain::BondRugoso => Recipe { octaves: &[(12, 0.3), (24, 0.32), (48, 0.28), (110, 0.18), (240, 0.12)], warp: 0.06, fibers: (560, 34.0, 0.35), pebbles: (0, 0.0), emboss: false, clumps: 0.3, speckle: 0.12 },
        Grain::AlgodonSatinado => Recipe { octaves: &[(24, 0.2), (72, 0.25), (190, 0.2)], warp: 0.05, fibers: (440, 70.0, 0.22), pebbles: (0, 0.0), emboss: false, clumps: 0.35, speckle: 0.06 },
        Grain::Fino => Recipe { octaves: &[(128, 0.25), (256, 0.35), (512, 0.35)], warp: 0.015, fibers: (0, 0.0, 0.0), pebbles: (0, 0.0), emboss: false, clumps: 0.25, speckle: 0.2 },
        Grain::Celulosa => Recipe { octaves: &[(40, 0.3), (120, 0.35), (280, 0.3)], warp: 0.04, fibers: (480, 20.0, 0.28), pebbles: (0, 0.0), emboss: false, clumps: 0.4, speckle: 0.1 },
        Grain::Liso => Recipe { octaves: &[(190, 0.15), (512, 0.2)], warp: 0.01, fibers: (120, 100.0, 0.15), pebbles: (0, 0.0), emboss: false, clumps: 0.2, speckle: 0.1 },
        Grain::PrensadoFrio => Recipe { octaves: &[(6, 0.3), (14, 0.2)], warp: 0.035, fibers: (0, 0.0, 0.0), pebbles: (30, 1.3), emboss: true, clumps: 0.4, speckle: 0.04 },
    }
}

/// Textura de grano en RGBA premultiplicado (TILE × TILE × 4): fibra clara = blanco con transparencia, oscura = negro.
/// `seed` da otra variante del mismo tipo de papel (mismo aspecto, otro dibujo).
pub fn grain_image(kind: Grain, seed: u32) -> Vec<u8> {
    let r = recipe(kind);
    let seed = seed.wrapping_mul(7919).wrapping_add(7 + kind as u32 * 31);
    let tile = TILE as f32;
    let mut v = vec![0.0f32; TILE * TILE];
    for y in 0..TILE {
        for x in 0..TILE {
            let (u, w) = (x as f32 / tile, y as f32 / tile);
            // Deformación del dominio con ruido lento: rompe cualquier regularidad de la retícula.
            let (du, dw) = (r.warp * value_noise(u, w, 6, seed + 500), r.warp * value_noise(u, w, 6, seed + 501));
            let (su, sw) = (u + du, w + dw);
            let mut s: f32 = r.octaves.iter().enumerate().map(|(i, (f, a))| a * value_noise(su, sw, *f, seed + i as u32 * 101)).sum();
            if r.pebbles.0 > 0 {
                // Guijarros irregulares (cold-press): distancia al centro más próximo, centrada en cero.
                s += r.pebbles.1 * (0.45 - worley(su, sw, r.pebbles.0, seed + 900));
            }
            // Manchas de densidad: el grano es más fuerte en unas zonas que en otras.
            let m = 1.0 - r.clumps * 0.5 + r.clumps * 0.5 * value_noise(u, w, 3, seed + 700);
            v[y * TILE + x] = s * m + r.speckle * hash(x as u32, y as u32, seed + 33);
        }
    }
    if r.emboss {
        // Relieve: diferencia con el vecino en diagonal (con vuelta), como una hoja prensada iluminada de lado.
        let base = v.clone();
        let k = 2;
        for y in 0..TILE {
            for x in 0..TILE {
                let a = base[((y + k) % TILE) * TILE + (x + k) % TILE];
                let b = base[((y + TILE - k) % TILE) * TILE + (x + TILE - k) % TILE];
                v[y * TILE + x] = base[y * TILE + x] * 0.55 + (a - b) * 1.1;
            }
        }
    }
    if r.fibers.0 > 0 {
        add_fibers(&mut v, r.fibers.0, r.fibers.1, r.fibers.2, seed);
    }
    // Centrar y normalizar a -1…1.
    let n = v.len() as f32;
    let mean = v.iter().sum::<f32>() / n;
    let std = (v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n).sqrt().max(1e-6);
    let mut out = Vec::with_capacity(TILE * TILE * 4);
    for x in &v {
        let s = ((x - mean) / (2.2 * std)).clamp(-1.0, 1.0);
        let a = (s.abs() * 255.0).round() as u8;
        if s >= 0.0 {
            out.extend_from_slice(&[a, a, a, a]);
        } else {
            out.extend_from_slice(&[0, 0, 0, a]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Valor con signo de un píxel de la textura (-255…255).
    fn signed(img: &[u8], x: usize, y: usize) -> f32 {
        let i = (y * TILE + x) * 4;
        if img[i] > 0 { img[i + 3] as f32 } else { -(img[i + 3] as f32) }
    }

    #[test]
    fn catalog_is_complete_and_distinct() {
        assert_eq!(PAPERS.len(), 9);
        let mut ids: Vec<&str> = PAPERS.iter().map(|p| p.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 9, "ids repetidos");
        for (i, a) in PAPERS.iter().enumerate() {
            assert!(!a.name.is_empty() && !a.era.is_empty() && !a.note.is_empty());
            assert!((0.0..=1.0).contains(&a.strength) && (0.0..=1.0).contains(&a.translucency));
            for b in &PAPERS[i + 1..] {
                assert_ne!(a.rgb, b.rgb, "{} y {} tienen el mismo color: no se podrían distinguir", a.name, b.name);
            }
            // Distinto de los 4 papeles clásicos de la Mesa de luz.
            for c in [[0xfb, 0xfa, 0xf7], [0xf6, 0xef, 0xe2], [0xec, 0xe2, 0xd2], [0xd9, 0xd6, 0xd2]] {
                assert_ne!(a.rgb, c, "{} choca con un papel clásico", a.name);
            }
        }
        // Los siete papeles del Issue, con sus nombres.
        for needle in ["Bond Genérico 16 lb", "Ingram Bond 24 lb", "Chromacolour", "Cartoon Colour 20 lb", "Genga", "Douga", "TMK"] {
            assert!(PAPERS.iter().any(|p| p.name.contains(needle)), "falta {needle}");
        }
        assert!(PAPERS.iter().filter(|p| p.name.contains("Douga")).count() == 3);
    }

    #[test]
    fn paper_is_recognized_by_the_sheet_color() {
        for p in &PAPERS {
            assert_eq!(for_background([p.rgb[0], p.rgb[1], p.rgb[2], 255]).map(|q| q.id), Some(p.id));
        }
        assert!(for_background([0xfb, 0xfa, 0xf7, 255]).is_none(), "un papel clásico no activa grano");
        assert!(for_background([1, 2, 3, 255]).is_none());
    }

    #[test]
    fn tooltip_names_the_paper_and_its_data() {
        let t = PAPERS[2].tooltip();
        assert!(t.contains("Chromacolour Pro-Grade (75 g/m²)") && t.contains("Estándar Occidental") && t.contains("translucidez media-alta"));
    }

    #[test]
    fn grain_follows_the_light() {
        for p in &PAPERS {
            let (a0, a5, a1) = (p.grain_alpha(0.0), p.grain_alpha(0.5), p.grain_alpha(1.0));
            assert!(a0 > 0.0, "con la luz al mínimo el grano sigue siendo superficial");
            assert!(a0 < a5 && a5 < a1, "{}: más luz, más grano ({a0} {a5} {a1})", p.name);
            assert!(a1 <= 0.34 + 1e-6);
        }
        // Un papel muy translúcido responde más a la luz que uno opaco (relación entre luz baja y alta).
        let genga = PAPERS.iter().find(|p| p.id == "genga").unwrap();
        let ingram = PAPERS.iter().find(|p| p.id == "ingram24").unwrap();
        assert!(genga.grain_alpha(0.0) / genga.grain_alpha(1.0) < ingram.grain_alpha(0.0) / ingram.grain_alpha(1.0));
        // Fuera de rango se limita.
        assert_eq!(genga.grain_alpha(5.0), genga.grain_alpha(1.0));
        assert_eq!(genga.grain_alpha(-1.0), genga.grain_alpha(0.0));
    }

    #[test]
    fn grain_is_deterministic_for_a_seed_and_seamless() {
        for kind in Grain::ALL {
            let a = grain_image(kind, 1);
            assert_eq!(a.len(), TILE * TILE * 4);
            assert_eq!(a, grain_image(kind, 1), "{kind:?} no es determinista");
            // Sin costuras: el salto entre la última columna y la primera (y entre última y primera fila) es como el de columnas vecinas.
            let (mut seam, mut inner) = (0.0f32, 0.0f32);
            for i in 0..TILE {
                seam += (signed(&a, TILE - 1, i) - signed(&a, 0, i)).abs() + (signed(&a, i, TILE - 1) - signed(&a, i, 0)).abs();
                inner += (signed(&a, 200, i) - signed(&a, 201, i)).abs() + (signed(&a, i, 200) - signed(&a, i, 201)).abs();
            }
            assert!(seam < inner * 1.5, "{kind:?}: costura visible ({seam} frente a {inner})");
        }
    }

    #[test]
    fn a_new_seed_gives_a_different_pattern_of_the_same_kind() {
        for kind in [Grain::PrensadoFrio, Grain::Fino] {
            let (a, b) = (grain_image(kind, 1), grain_image(kind, 2));
            assert_ne!(a, b, "{kind:?}: la semilla no cambia nada");
            // Misma «energía» (mismo aspecto), distinto dibujo: poca correlación entre las dos.
            let e = |img: &[u8]| (0..TILE * TILE).map(|i| (img[i * 4 + 3] as f32 / 255.0).powi(2)).sum::<f32>() / (TILE * TILE) as f32;
            assert!((e(&a) - e(&b)).abs() < 0.05, "{kind:?}: aspecto distinto con otra semilla");
        }
    }

    #[test]
    fn grain_kinds_look_different() {
        let imgs: Vec<Vec<u8>> = Grain::ALL.iter().map(|k| grain_image(*k, 1)).collect();
        for i in 0..imgs.len() {
            for j in i + 1..imgs.len() {
                assert_ne!(imgs[i], imgs[j]);
            }
        }
        let energy = |a: &[u8]| (0..TILE * TILE).map(|i| (a[i * 4 + 3] as f32 / 255.0).powi(2)).sum::<f32>() / (TILE * TILE) as f32;
        assert!(energy(&imgs[0]) > 0.02 && energy(&imgs[5]) > 0.02, "el grano pronunciado tiene relieve");
        // Premultiplicado válido: ningún canal supera la transparencia.
        for img in &imgs {
            assert!(img.chunks(4).all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]));
        }
    }

    #[test]
    fn grain_has_no_internal_periodicity() {
        // Correlación de la textura consigo misma desplazada: si hubiera un patrón que se repite dentro de la
        // textura, aparecería un pico lejos del desplazamiento cero. Debe quedar claramente por debajo de 1.
        for kind in Grain::ALL {
            let img = grain_image(kind, 1);
            let v: Vec<f32> = (0..TILE * TILE).map(|i| signed(&img, i % TILE, i / TILE)).collect();
            let e0: f32 = v.iter().map(|x| x * x).sum();
            let mut worst = 0.0f32;
            for shift in (48..TILE / 2).step_by(8) {
                for (dx, dy) in [(shift, 0), (0, shift), (shift, shift)] {
                    let c: f32 = (0..TILE * TILE).map(|i| v[i] * v[((i / TILE + dy) % TILE) * TILE + (i % TILE + dx) % TILE]).sum();
                    worst = worst.max(c / e0);
                }
            }
            assert!(worst < 0.45, "{kind:?}: hay un patrón que se repite (correlación {worst:.2})");
        }
    }

    #[test]
    fn layers_never_line_up() {
        // Tamaños sin relación simple entre sí y giros distintos: la combinación no se repite a la vista.
        for (i, a) in GRAIN_LAYERS.iter().enumerate() {
            for b in &GRAIN_LAYERS[i + 1..] {
                let r = a.size / b.size;
                for q in 1..=4 {
                    let p = (r * q as f32).round();
                    assert!((r * q as f32 - p).abs() > 0.04, "tamaños {} y {} casi múltiplos", a.size, b.size);
                }
                assert!((a.angle - b.angle).abs() >= 15.0, "giros demasiado parecidos");
            }
        }
        assert!(layers_norm() > 1.0 && layers_norm() < 2.0);
    }

    #[test]
    fn grain_fades_out_only_when_too_small_on_screen() {
        assert_eq!(grain_fade(0.75), 1.0);
        assert_eq!(grain_fade(0.2), 0.0);
        let mid = grain_fade(0.4);
        assert!(mid > 0.0 && mid < 1.0);
        assert!(grain_fade(0.3) < grain_fade(0.45) && grain_fade(0.45) <= grain_fade(0.6));
    }

    #[test]
    fn generating_a_grain_is_quick() {
        let t0 = std::time::Instant::now();
        let _ = grain_image(Grain::PrensadoFrio, 1);
        let _ = grain_image(Grain::BondRugoso, 1);
        let ms = t0.elapsed().as_millis();
        assert!(ms < 2500, "dos texturas tardaron {ms} ms");
        eprintln!("dos texturas de {TILE}×{TILE}: {ms} ms");
    }
}
