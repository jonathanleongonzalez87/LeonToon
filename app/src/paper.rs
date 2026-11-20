//! Papeles de estudio de la Mesa de luz (Issue #52): catálogo de papeles de animación y grano procedural.
//!
//! Cada papel tiene un color base y un tipo de grano. El grano es una textura de 256×256 px **sin costuras**,
//! generada en código (no hay archivos de imagen), con valores de -1 (fibra oscura) a +1 (fibra clara). Se guarda
//! como blanco y negro con transparencia, de modo que dibujarla sobre la hoja equivale a mezclar con Overlay.
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
pub const TILE: usize = 256;

fn hash(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263) ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    h as f32 / u32::MAX as f32 * 2.0 - 1.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Ruido de valores sin costuras: `u` y `v` van de 0 a 1 y la retícula da la vuelta con periodo `freq`.
fn value_noise(u: f32, v: f32, freq: u32, seed: u32) -> f32 {
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

struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Suma fibras cortas (segmentos que dan la vuelta por los bordes, para que la textura siga sin costuras).
fn add_fibers(buf: &mut [f32], count: usize, len: f32, amp: f32, seed: u32) {
    let mut rng = Rng(seed ^ 0x9e37_79b9);
    let n = TILE as i32;
    for _ in 0..count {
        let (x0, y0) = (rng.next() * TILE as f32, rng.next() * TILE as f32);
        let ang = rng.next() * std::f32::consts::TAU;
        let l = len * (0.5 + rng.next());
        let a = amp * (rng.next() * 2.0 - 1.0);
        let steps = l.ceil() as usize;
        for s in 0..steps {
            let t = s as f32;
            let x = ((x0 + ang.cos() * t).round() as i32).rem_euclid(n) as usize;
            let y = ((y0 + ang.sin() * t).round() as i32).rem_euclid(n) as usize;
            buf[y * TILE + x] += a;
        }
    }
}

/// Textura de grano en RGBA premultiplicado (TILE × TILE × 4): fibra clara = blanco con transparencia, oscura = negro.
pub fn grain_image(kind: Grain) -> Vec<u8> {
    let (octaves, fibers, emboss): (&[(u32, f32)], (usize, f32, f32), bool) = match kind {
        Grain::BondRugoso => (&[(6, 0.45), (12, 0.3), (24, 0.2), (64, 0.12)], (90, 14.0, 0.35), false),
        Grain::AlgodonSatinado => (&[(16, 0.25), (48, 0.25), (128, 0.2)], (60, 28.0, 0.25), false),
        Grain::Fino => (&[(64, 0.25), (128, 0.35), (256, 0.4)], (0, 0.0, 0.0), false),
        Grain::Celulosa => (&[(24, 0.3), (64, 0.35), (128, 0.3)], (70, 8.0, 0.3), false),
        Grain::Liso => (&[(128, 0.3), (256, 0.3)], (24, 40.0, 0.2), false),
        Grain::PrensadoFrio => (&[(5, 0.5), (10, 0.5), (20, 0.35), (40, 0.2)], (0, 0.0, 0.0), true),
    };
    let seed = 7 + kind as u32 * 31;
    let mut v = vec![0.0f32; TILE * TILE];
    for y in 0..TILE {
        for x in 0..TILE {
            let (u, w) = (x as f32 / TILE as f32, y as f32 / TILE as f32);
            v[y * TILE + x] = octaves.iter().enumerate().map(|(i, (f, a))| a * value_noise(u, w, *f, seed + i as u32 * 101)).sum();
        }
    }
    if emboss {
        // Relieve: diferencia con el vecino en diagonal (con vuelta), como una hoja prensada iluminada de lado.
        let base = v.clone();
        let k = 3;
        for y in 0..TILE {
            for x in 0..TILE {
                let a = base[((y + k) % TILE) * TILE + (x + k) % TILE];
                let b = base[((y + TILE - k) % TILE) * TILE + (x + TILE - k) % TILE];
                v[y * TILE + x] = base[y * TILE + x] * 0.4 + (a - b) * 1.6;
            }
        }
    }
    if fibers.0 > 0 {
        add_fibers(&mut v, fibers.0, fibers.1, fibers.2, seed);
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
    fn grain_is_deterministic_and_seamless() {
        for kind in Grain::ALL {
            let a = grain_image(kind);
            assert_eq!(a.len(), TILE * TILE * 4);
            assert_eq!(a, grain_image(kind), "{kind:?} no es determinista");
            // Sin costuras: el salto entre la última columna y la primera (y entre última y primera fila) es como el de columnas vecinas.
            let (mut seam, mut inner) = (0.0f32, 0.0f32);
            for i in 0..TILE {
                seam += (signed(&a, TILE - 1, i) - signed(&a, 0, i)).abs() + (signed(&a, i, TILE - 1) - signed(&a, i, 0)).abs();
                inner += (signed(&a, 100, i) - signed(&a, 101, i)).abs() + (signed(&a, i, 100) - signed(&a, i, 101)).abs();
            }
            assert!(seam < inner * 1.5, "{kind:?}: costura visible ({seam} frente a {inner})");
        }
    }

    #[test]
    fn grain_kinds_look_different() {
        let energy = |k: Grain| {
            let a = grain_image(k);
            (0..TILE * TILE).map(|i| (a[i * 4 + 3] as f32 / 255.0).powi(2)).sum::<f32>() / (TILE * TILE) as f32
        };
        // Cada tipo usa texturas distintas.
        let imgs: Vec<Vec<u8>> = Grain::ALL.iter().map(|k| grain_image(*k)).collect();
        for i in 0..imgs.len() {
            for j in i + 1..imgs.len() {
                assert_ne!(imgs[i], imgs[j]);
            }
        }
        assert!(energy(Grain::BondRugoso) > 0.02 && energy(Grain::PrensadoFrio) > 0.02, "el grano pronunciado tiene relieve");
        // Premultiplicado válido: ningún canal supera la transparencia.
        for img in &imgs {
            assert!(img.chunks(4).all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]));
        }
    }
}
