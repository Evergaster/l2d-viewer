//! Lector del contenedor `.moc3`.
//!
//! Implementación propia a partir de la especificación pública del formato
//! (OpenL2D, «MOC3 Format Specification 2.1b»). No contiene código de Live2D.
//!
//! Alcance de este módulo: **el contenedor**. Es decir cabecera, información de canvas,
//! tabla de conteos y tabla de offsets de sección. La evaluación de keyforms
//! (deformadores, mallas, curvas) es otra fase y todavía no está aquí.
//!
//! ```
//! # fn synth() -> Vec<u8> { unimplemented!() }
//! // let m = moc3::Moc3::parse(&bytes)?;
//! // println!("{} — {}×{}", m.version.label(), m.canvas.width, m.canvas.height);
//! ```

use std::fmt;

pub mod body;

pub use body::{ArtMesh, Body, Parameter};

pub const MAGIC: [u8; 4] = *b"MOC3";
pub const HEADER_SIZE: usize = 0x40;
pub const CANVAS_INFO_SIZE: usize = 0x40;
/// La tabla de offsets empieza justo después de la cabecera.
///
/// Verificado contra los 8 modelos de muestra oficiales del SDK (versiones 1, 3, 5 y 6):
/// `u32@0x40` = offset de los conteos, `u32@0x44` = offset del bloque de canvas, y desde
/// `0x48` empiezan las tablas de offsets por sección. **No** hay relleno de 0x280 antes.
pub const TABLE_START: usize = HEADER_SIZE;

// ---------------------------------------------------------------- errores

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    TooShort { need: usize, have: usize },
    BadMagic([u8; 4]),
    UnsupportedEndianness(u8),
    UnknownVersion(u8),
    OffsetOutOfRange { what: &'static str, offset: u32 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TooShort { need, have } => {
                write!(f, "archivo truncado: hacen falta {need} bytes y hay {have}")
            }
            Error::BadMagic(m) => write!(f, "no es un .moc3 (magic {:?})", String::from_utf8_lossy(m)),
            Error::UnsupportedEndianness(b) => write!(f, "endianness {b} no soportada"),
            Error::UnknownVersion(v) => write!(f, "versión de formato {v} desconocida"),
            Error::OffsetOutOfRange { what, offset } => {
                write!(f, "offset fuera de rango en {what}: {offset}")
            }
        }
    }
}

impl std::error::Error for Error {}

// ---------------------------------------------------------------- tipos

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version(pub u8);

impl Version {
    pub fn label(self) -> &'static str {
        match self.0 {
            1 => "MOC3 3.0.0 (v1)",
            2 => "MOC3 3.3.0 (v2)",
            3 => "MOC3 4.0.0 (v3)",
            4 => "MOC3 4.2.0 (v4)",
            5 => "MOC3 5.0.0 (v5)",
            6 => "MOC3 v6 (5.3+, sin especificación pública)",
            _ => "versión desconocida",
        }
    }

    pub fn has_blend_shapes(self) -> bool {
        self.0 >= 4
    }

    pub fn has_v5_keyforms(self) -> bool {
        self.0 >= 5
    }
}

/// Bloque de canvas: **verificado contra el Core oficial** en 5 modelos
/// (Wanko, Hiyori, Rice, Mark, Mao). El orden es
/// `[pixelsPerUnit, originX, originY, ancho, alto]`.
///
/// La versión anterior de este archivo lo interpretaba al revés (ancho primero);
/// el oráculo del Core lo desmintió.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Canvas {
    pub pixels_per_unit: f32,
    pub origin_x: f32,
    pub origin_y: f32,
    pub width: f32,
    pub height: f32,
    pub reverse_y: bool,
}

impl Canvas {
    /// Convierte una coordenada del modelo (unidades) a píxeles de canvas.
    pub fn to_pixels(&self, p: [f32; 2]) -> [f32; 2] {
        [
            self.origin_x + p[0] * self.pixels_per_unit,
            self.origin_y + p[1] * self.pixels_per_unit,
        ]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    fields: Vec<(&'static str, u32)>,
}

impl Counts {
    pub fn get(&self, name: &str) -> Option<u32> {
        self.fields.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
    }

    pub fn get_or_zero(&self, name: &str) -> u32 {
        self.get(name).unwrap_or(0)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'static str, u32)> + '_ {
        self.fields.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// Una sección del cuerpo: su nombre, dónde empieza y cuántos elementos tiene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: &'static str,
    pub offset: u32,
    pub count: u32,
}

#[derive(Debug, Clone)]
pub struct Moc3 {
    pub version: Version,
    pub endianness: Endianness,
    pub canvas: Canvas,
    pub counts: Counts,
    /// Tabla de offsets tal cual: `[0]` conteos, `[1]` canvas, `[2+i]` sección `i`.
    pub sot: Vec<u32>,
    pub sections: Vec<Section>,
    size: usize,
    data: Vec<u8>,
}

impl Moc3 {
    /// Bytes completos del archivo analizado.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Tabla de offsets de sección.
    pub fn section_offsets(&self) -> &[u32] {
        &self.sot
    }
}

// ---------------------------------------------------------------- nombres

const BASE_COUNTS: [&str; 23] = [
    "parts",
    "deformers",
    "warp_deformers",
    "rotation_deformers",
    "art_meshes",
    "parameters",
    "part_keyforms",
    "warp_deformer_keyforms",
    "rotation_deformer_keyforms",
    "art_mesh_keyforms",
    "keyform_positions",
    "parameter_binding_indices",
    "keyform_bindings",
    "parameter_bindings",
    "keys",
    "uvs",
    "position_indices",
    "drawable_masks",
    "draw_order_groups",
    "draw_order_group_objects",
    "glue",
    "glue_info",
    "glue_keyforms",
];

const V4_COUNTS: [&str; 9] = [
    "keyform_multiply_colors",
    "keyform_screen_colors",
    "blend_shape_parameter_bindings",
    "blend_shape_keyform_bindings",
    "blend_shapes_warp_deformers",
    "blend_shapes_art_meshes",
    "blend_shape_constraint_indices",
    "blend_shape_constraints",
    "blend_shape_constraint_values",
];

const V5_COUNTS: [&str; 3] = [
    "blend_shapes_parts",
    "blend_shapes_rotation_deformers",
    "blend_shapes_glue",
];

/// Secciones base y el contador del que depende su longitud.

// ---------------------------------------------------------------- utilidades

fn read_u32(b: &[u8], off: usize, endian: Endianness) -> u32 {
    let s = &b[off..off + 4];
    match endian {
        Endianness::Little => u32::from_le_bytes([s[0], s[1], s[2], s[3]]),
        Endianness::Big => u32::from_be_bytes([s[0], s[1], s[2], s[3]]),
    }
}

fn read_f32(b: &[u8], off: usize, endian: Endianness) -> f32 {
    f32::from_bits(read_u32(b, off, endian))
}

// ---------------------------------------------------------------- parseo

impl Moc3 {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < TABLE_START + 8 {
            return Err(Error::TooShort {
                need: TABLE_START + 8,
                have: bytes.len(),
            });
        }

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        if magic != MAGIC {
            return Err(Error::BadMagic(magic));
        }

        let version = Version(bytes[4]);
        if !(1..=6).contains(&version.0) {
            return Err(Error::UnknownVersion(version.0));
        }

        let endian = match bytes[5] {
            0 => Endianness::Little,
            other => return Err(Error::UnsupportedEndianness(other)),
        };

        let sot_len = 2 + body::LAYOUT.len();
        if TABLE_START + sot_len * 4 > bytes.len() {
            return Err(Error::TooShort {
                need: TABLE_START + sot_len * 4,
                have: bytes.len(),
            });
        }
        let sot: Vec<u32> = (0..sot_len)
            .map(|i| read_u32(bytes, TABLE_START + i * 4, endian))
            .collect();

        let count_info_off = sot[0];
        let canvas_off = sot[1];

        let canvas = Self::read_canvas(bytes, canvas_off, endian)?;
        let counts = Self::read_counts(bytes, count_info_off, version, endian)?;
        let sections = Self::build_sections(&sot, &counts);

        Ok(Moc3 {
            version,
            endianness: endian,
            canvas,
            counts,
            sot,
            sections,
            size: bytes.len(),
            data: bytes.to_vec(),
        })
    }

    fn read_canvas(bytes: &[u8], off: u32, endian: Endianness) -> Result<Canvas, Error> {
        let off = off as usize;
        if off == 0 || off + CANVAS_INFO_SIZE > bytes.len() {
            return Err(Error::OffsetOutOfRange {
                what: "canvas_info",
                offset: off as u32,
            });
        }
        Ok(Canvas {
            pixels_per_unit: read_f32(bytes, off, endian),
            origin_x: read_f32(bytes, off + 4, endian),
            origin_y: read_f32(bytes, off + 8, endian),
            width: read_f32(bytes, off + 12, endian),
            height: read_f32(bytes, off + 16, endian),
            reverse_y: (bytes[off + 20] & 0b1) != 0,
        })
    }

    fn read_counts(
        bytes: &[u8],
        count_info_off: u32,
        version: Version,
        endian: Endianness,
    ) -> Result<Counts, Error> {
        let base = count_info_off as usize;
        if base == 0 {
            return Err(Error::OffsetOutOfRange {
                what: "count_info",
                offset: 0,
            });
        }

        let mut names: Vec<&'static str> = BASE_COUNTS.to_vec();
        if version.has_blend_shapes() {
            names.extend(V4_COUNTS);
        }
        if version.has_v5_keyforms() {
            names.extend(V5_COUNTS);
        }

        // En los archivos reales los conteos empiezan justo en el offset indicado:
        // sin relleno previo, en todas las versiones 1..6 comprobadas.
        let start = base;
        let need = start + names.len() * 4;
        if need > bytes.len() {
            return Err(Error::TooShort {
                need,
                have: bytes.len(),
            });
        }

        let fields = names
            .into_iter()
            .enumerate()
            .map(|(i, name)| (name, read_u32(bytes, start + i * 4, endian)))
            .collect();

        Ok(Counts { fields })
    }

    fn build_sections(sot: &[u32], counts: &Counts) -> Vec<Section> {
        body::LAYOUT
            .iter()
            .enumerate()
            .map(|(i, s)| Section {
                name: s.name,
                offset: sot.get(2 + i).copied().unwrap_or(0),
                count: counts.get_or_zero(s.count),
            })
            .collect()
    }

    /// Tamaño del archivo analizado.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Resumen de una línea, para logs.
    pub fn summary(&self) -> String {
        let size = format!(
            "{:.0}×{:.0} px @ {:.0} px/u",
            self.canvas.width, self.canvas.height, self.canvas.pixels_per_unit
        );
        format!(
            "{} · canvas {} · {} partes · {} mallas · {} parámetros",
            self.version.label(),
            size,
            self.counts.get_or_zero("parts"),
            self.counts.get_or_zero("art_meshes"),
            self.counts.get_or_zero("parameters"),
        )
    }
}

// ---------------------------------------------------------------- pruebas

#[cfg(test)]
mod tests {
    use super::*;

    fn write_u32(v: &mut [u8], off: usize, val: u32) {
        v[off..off + 4].copy_from_slice(&val.to_le_bytes());
    }

    fn write_f32(v: &mut [u8], off: usize, val: f32) {
        write_u32(v, off, val.to_bits());
    }

    /// Construye un `.moc3` sintético válido para el contenedor.
    fn synth(version: u8) -> Vec<u8> {
        let mut v = vec![0u8; 0x4000];
        v[0..4].copy_from_slice(b"MOC3");
        v[4] = version;
        v[5] = 0;

        let count_info_off = 0x1000u32;
        let canvas_off = 0x2000u32;
        write_u32(&mut v, TABLE_START, count_info_off);
        write_u32(&mut v, TABLE_START + 4, canvas_off);

        // [ppu, originX, originY, ancho, alto]
        let c = canvas_off as usize;
        write_f32(&mut v, c, 2.0);
        write_f32(&mut v, c + 4, 400.0);
        write_f32(&mut v, c + 8, 300.0);
        write_f32(&mut v, c + 12, 800.0);
        write_f32(&mut v, c + 16, 600.0);
        v[c + 20] = 1; // reverse Y

        let n = match version {
            1 | 2 | 3 => 23,
            4 => 32,
            _ => 35,
        };
        for i in 0..n {
            write_u32(&mut v, count_info_off as usize + i * 4, (i as u32) + 1);
        }
        v
    }

    #[test]
    fn header_y_canvas() {
        let m = Moc3::parse(&synth(3)).expect("parse");
        assert_eq!(m.version, Version(3));
        assert_eq!(m.endianness, Endianness::Little);
        assert_eq!(m.canvas.width, 800.0);
        assert_eq!(m.canvas.height, 600.0);
        assert_eq!([m.canvas.origin_x, m.canvas.origin_y], [400.0, 300.0]);
        assert_eq!(m.canvas.pixels_per_unit, 2.0);
        assert!(m.canvas.reverse_y);
    }

    #[test]
    fn conteos_por_version() {
        let v1 = Moc3::parse(&synth(1)).expect("v1");
        assert_eq!(v1.counts.len(), 23);
        assert_eq!(v1.counts.get("art_meshes"), Some(5)); // índice 4 → 5

        let v4 = Moc3::parse(&synth(4)).expect("v4");
        assert_eq!(v4.counts.len(), 32);
        assert!(v4.counts.get("blend_shapes_art_meshes").is_some());

        let v5 = Moc3::parse(&synth(5)).expect("v5");
        assert_eq!(v5.counts.len(), 35);
        assert!(v5.counts.get("blend_shapes_glue").is_some());

        // v6 existe en modelos reales (Ren.moc3) y no tiene especificación pública:
        // se parsea con la forma de v5.
        let v6 = Moc3::parse(&synth(6)).expect("v6");
        assert_eq!(v6.counts.len(), 35);
    }

    #[test]
    fn secciones_cubren_todo_el_layout() {
        let m = Moc3::parse(&synth(4)).unwrap();
        assert_eq!(m.sections.len(), body::LAYOUT.len());
        assert!(m.sections.iter().any(|s| s.name == "part.ids"));
        assert!(m.sections.iter().any(|s| s.name == "art_mesh.ids"));
        assert!(m.sections.iter().any(|s| s.name == "keyform_position.xys"));
        // el archivo sintético no trae datos de sección: los offsets quedan en cero
        assert!(m.sections.iter().all(|s| s.offset == 0));
    }

    #[test]
    fn rechaza_lo_que_no_es_moc3() {
        let mut v = synth(3);
        v[0] = b'X';
        assert!(matches!(Moc3::parse(&v), Err(Error::BadMagic(_))));
        assert!(matches!(Moc3::parse(&[0u8; 8]), Err(Error::TooShort { .. })));
    }

    #[test]
    fn rechaza_version_desconocida() {
        let mut v = synth(3);
        v[4] = 9;
        assert!(matches!(Moc3::parse(&v), Err(Error::UnknownVersion(9))));
    }
}
