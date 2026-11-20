//! Lectura de bajo nivel de archivos ABR: bytes en orden big-endian, PackBits y descriptores de Photoshop.
//! Nada aquí hace `panic!`: un archivo dañado o recortado da un `Err` con un mensaje claro.

/// Lector de bytes con comprobación de límites.
pub struct R<'a> {
    b: &'a [u8],
    pub p: usize,
}

pub type Res<T> = Result<T, String>;

impl<'a> R<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        R { b, p: 0 }
    }
    pub fn left(&self) -> usize {
        self.b.len().saturating_sub(self.p)
    }
    pub fn bytes(&mut self, n: usize) -> Res<&'a [u8]> {
        let end = self.p.checked_add(n).filter(|e| *e <= self.b.len()).ok_or_else(|| format!("El archivo termina antes de lo esperado (byte {})", self.p))?;
        let s = &self.b[self.p..end];
        self.p = end;
        Ok(s)
    }
    pub fn skip(&mut self, n: usize) -> Res<()> {
        self.bytes(n).map(|_| ())
    }
    pub fn u8(&mut self) -> Res<u8> {
        Ok(self.bytes(1)?[0])
    }
    pub fn u16(&mut self) -> Res<u16> {
        let b = self.bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    pub fn u32(&mut self) -> Res<u32> {
        let b = self.bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn i32(&mut self) -> Res<i32> {
        Ok(self.u32()? as i32)
    }
    pub fn i64(&mut self) -> Res<i64> {
        let b = self.bytes(8)?;
        Ok(i64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }
    pub fn f64(&mut self) -> Res<f64> {
        let b = self.bytes(8)?;
        Ok(f64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }
    /// Cadena Pascal: un byte de largo y los caracteres.
    pub fn pascal(&mut self) -> Res<String> {
        let n = self.u8()? as usize;
        Ok(String::from_utf8_lossy(self.bytes(n)?).into_owned())
    }
    /// Texto Unicode: u32 con el número de caracteres de 16 bits y los caracteres (UTF-16 big-endian).
    pub fn unicode(&mut self) -> Res<String> {
        let n = self.u32()? as usize;
        if n > self.left() / 2 {
            return Err("Texto con un largo imposible (archivo dañado)".into());
        }
        let raw = self.bytes(n * 2)?;
        let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        Ok(String::from_utf16_lossy(&units).trim_end_matches('\0').to_string())
    }
}

/// Descomprime PackBits (RLE de Photoshop) hasta tener `want` bytes. Si los datos acaban antes, rellena con ceros.
pub fn unpackbits(data: &[u8], want: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(want);
    let mut i = 0;
    while i < data.len() && out.len() < want {
        let n = data[i] as i8;
        i += 1;
        if n >= 0 {
            let c = n as usize + 1;
            let end = (i + c).min(data.len());
            out.extend_from_slice(&data[i..end]);
            i = end;
        } else if n != -128 {
            if i >= data.len() {
                break;
            }
            let c = (-(n as i32)) as usize + 1;
            out.extend(std::iter::repeat(data[i]).take(c));
            i += 1;
        }
    }
    out.resize(want, 0);
    out.truncate(want);
    out
}

/// PackBits al revés (para escribir archivos).
pub fn packbits(row: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(row.len() + row.len() / 100 + 2);
    let mut i = 0;
    while i < row.len() {
        // Tramo repetido
        let mut run = 1;
        while i + run < row.len() && row[i + run] == row[i] && run < 128 {
            run += 1;
        }
        if run >= 2 {
            out.push((257 - run as i32) as u8);
            out.push(row[i]);
            i += run;
            continue;
        }
        // Tramo literal hasta el próximo tramo repetido
        let start = i;
        let mut n = 0;
        while i < row.len() && n < 128 && !(i + 1 < row.len() && row[i] == row[i + 1]) {
            i += 1;
            n += 1;
        }
        if n == 0 {
            i += 1;
            n = 1;
        }
        out.push((n - 1) as u8);
        out.extend_from_slice(&row[start..start + n]);
    }
    out
}

/// Valor de un descriptor de Photoshop.
#[derive(Clone, Debug, PartialEq)]
pub enum V {
    Obj(String, Vec<(String, V)>),
    List(Vec<V>),
    Dbl(f64),
    /// Número con unidad (`#Prc` porcentaje, `#Pxl` píxeles, `#Ang` grados…).
    Unit(String, f64),
    Text(String),
    Long(i64),
    Bool(bool),
    Enum(String, String),
    Raw(usize),
}

impl V {
    pub fn get(&self, key: &str) -> Option<&V> {
        match self {
            V::Obj(_, items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn num(&self) -> Option<f64> {
        match self {
            V::Dbl(d) | V::Unit(_, d) => Some(*d),
            V::Long(l) => Some(*l as f64),
            _ => None,
        }
    }
    pub fn bool(&self) -> Option<bool> {
        match self {
            V::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn text(&self) -> Option<&str> {
        match self {
            V::Text(t) => Some(t),
            _ => None,
        }
    }
    pub fn class(&self) -> Option<&str> {
        match self {
            V::Obj(c, _) => Some(c),
            _ => None,
        }
    }
    pub fn list(&self) -> Option<&[V]> {
        match self {
            V::List(l) => Some(l),
            _ => None,
        }
    }
}

/// Identificador de clave o de clase: u32 de largo (0 = cuatro caracteres) y los caracteres.
fn key(r: &mut R) -> Res<String> {
    let n = r.u32()? as usize;
    let n = if n == 0 { 4 } else { n };
    if n > 256 {
        return Err("Clave con un largo imposible (archivo dañado)".into());
    }
    Ok(String::from_utf8_lossy(r.bytes(n)?).into_owned())
}

/// Profundidad máxima de anidación aceptada (contra archivos hechos para desbordar la pila).
const MAX_DEPTH: usize = 24;

fn value(r: &mut R, ty: &str, depth: usize) -> Res<V> {
    if depth > MAX_DEPTH {
        return Err("Descriptor demasiado anidado (archivo dañado)".into());
    }
    Ok(match ty {
        "Objc" | "GlbO" => descriptor(r, depth + 1)?,
        "VlLs" => {
            let n = r.u32()? as usize;
            if n > r.left() {
                return Err("Lista con un largo imposible (archivo dañado)".into());
            }
            let mut v = Vec::with_capacity(n.min(4096));
            for _ in 0..n {
                let t = String::from_utf8_lossy(r.bytes(4)?).into_owned();
                v.push(value(r, &t, depth + 1)?);
            }
            V::List(v)
        }
        "doub" => V::Dbl(r.f64()?),
        "UntF" => {
            let unit = String::from_utf8_lossy(r.bytes(4)?).into_owned();
            V::Unit(unit, r.f64()?)
        }
        "TEXT" => V::Text(r.unicode()?),
        "long" => V::Long(r.i32()? as i64),
        "comp" => V::Long(r.i64()?),
        "bool" => V::Bool(r.u8()? != 0),
        "enum" => {
            let t = key(r)?;
            V::Enum(t, key(r)?)
        }
        "tdta" => {
            let n = r.u32()? as usize;
            r.skip(n)?;
            V::Raw(n)
        }
        other => return Err(format!("Tipo de dato «{other}» no reconocido en el descriptor")),
    })
}

/// Un descriptor: nombre, clase y elementos con clave.
pub fn descriptor(r: &mut R, depth: usize) -> Res<V> {
    let _name = r.unicode()?;
    let class = key(r)?;
    let n = r.u32()? as usize;
    if n > r.left() {
        return Err("Descriptor con demasiados elementos (archivo dañado)".into());
    }
    let mut items = Vec::with_capacity(n.min(512));
    for _ in 0..n {
        let k = key(r)?;
        let ty = String::from_utf8_lossy(r.bytes(4)?).into_owned();
        items.push((k, value(r, &ty, depth)?));
    }
    Ok(V::Obj(class, items))
}
