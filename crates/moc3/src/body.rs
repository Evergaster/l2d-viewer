//! Cuerpo del `.moc3`: lectura de secciones a partir de la tabla de offsets.
//!
//! El archivo guarda, en `0x40`, un array de offsets (SOT) con una entrada por bloque:
//! `sot[0]` = conteos, `sot[1]` = canvas, `sot[2+i]` = inicio de la sección `i` de
//! [`LAYOUT`]. Cada sección es un array plano de elementos del mismo tipo; los ids son
//! campos fijos de 64 bytes con el texto al principio.
//!
//! Orden y tipos verificados contra los 8 modelos de muestra del SDK.

use crate::{Error, Moc3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elem {
    I32,
    F32,
    I16,
    U8,
    Bool,
    /// Campo fijo de 64 bytes, texto UTF-8 terminado en cero.
    Id,
    /// Espacio de runtime: no se lee.
    Runtime,
}

pub struct Sec {
    pub name: &'static str,
    pub elem: Elem,
    pub count: &'static str,
}

const fn sec(name: &'static str, elem: Elem, count: &'static str) -> Sec {
    Sec { name, elem, count }
}

// Orden exacto del archivo. Cualquier inserción aquí desplaza todas las secciones
// posteriores: se toca con cuidado y se valida contra los modelos.
pub const LAYOUT: &[Sec] = &[
    // parts
    sec("part.runtime_space", Elem::Runtime, "parts"),
    sec("part.ids", Elem::Id, "parts"),
    sec("part.keyform_binding_band_indices", Elem::I32, "parts"),
    sec("part.keyform_begin_indices", Elem::I32, "parts"),
    sec("part.keyform_counts", Elem::I32, "parts"),
    sec("part.visibles", Elem::Bool, "parts"),
    sec("part.enables", Elem::Bool, "parts"),
    sec("part.parent_part_indices", Elem::I32, "parts"),
    // deformers
    sec("deformer.runtime_space", Elem::Runtime, "deformers"),
    sec("deformer.ids", Elem::Id, "deformers"),
    sec("deformer.keyform_binding_band_indices", Elem::I32, "deformers"),
    sec("deformer.visibles", Elem::Bool, "deformers"),
    sec("deformer.enables", Elem::Bool, "deformers"),
    sec("deformer.parent_part_indices", Elem::I32, "deformers"),
    sec("deformer.parent_deformer_indices", Elem::I32, "deformers"),
    sec("deformer.types", Elem::I32, "deformers"),
    sec("deformer.specific_indices", Elem::I32, "deformers"),
    // warp deformers
    sec("warp_deformer.keyform_binding_band_indices", Elem::I32, "warp_deformers"),
    sec("warp_deformer.keyform_begin_indices", Elem::I32, "warp_deformers"),
    sec("warp_deformer.keyform_counts", Elem::I32, "warp_deformers"),
    sec("warp_deformer.vertex_counts", Elem::I32, "warp_deformers"),
    sec("warp_deformer.rows", Elem::I32, "warp_deformers"),
    sec("warp_deformer.cols", Elem::I32, "warp_deformers"),
    // rotation deformers
    sec("rotation_deformer.keyform_binding_band_indices", Elem::I32, "rotation_deformers"),
    sec("rotation_deformer.keyform_begin_indices", Elem::I32, "rotation_deformers"),
    sec("rotation_deformer.keyform_counts", Elem::I32, "rotation_deformers"),
    sec("rotation_deformer.base_angles", Elem::F32, "rotation_deformers"),
    // art meshes
    sec("art_mesh.runtime_space_0", Elem::Runtime, "art_meshes"),
    sec("art_mesh.runtime_space_1", Elem::Runtime, "art_meshes"),
    sec("art_mesh.runtime_space_2", Elem::Runtime, "art_meshes"),
    sec("art_mesh.runtime_space_3", Elem::Runtime, "art_meshes"),
    sec("art_mesh.ids", Elem::Id, "art_meshes"),
    sec("art_mesh.keyform_binding_band_indices", Elem::I32, "art_meshes"),
    sec("art_mesh.keyform_begin_indices", Elem::I32, "art_meshes"),
    sec("art_mesh.keyform_counts", Elem::I32, "art_meshes"),
    sec("art_mesh.visibles", Elem::Bool, "art_meshes"),
    sec("art_mesh.enables", Elem::Bool, "art_meshes"),
    sec("art_mesh.parent_part_indices", Elem::I32, "art_meshes"),
    sec("art_mesh.parent_deformer_indices", Elem::I32, "art_meshes"),
    sec("art_mesh.texture_indices", Elem::I32, "art_meshes"),
    sec("art_mesh.drawable_flags", Elem::U8, "art_meshes"),
    sec("art_mesh.position_index_counts", Elem::I32, "art_meshes"),
    sec("art_mesh.uv_begin_indices", Elem::I32, "art_meshes"),
    sec("art_mesh.position_index_begin_indices", Elem::I32, "art_meshes"),
    sec("art_mesh.vertex_counts", Elem::I32, "art_meshes"),
    sec("art_mesh.mask_begin_indices", Elem::I32, "art_meshes"),
    sec("art_mesh.mask_counts", Elem::I32, "art_meshes"),
    // parameters
    sec("parameter.runtime_space", Elem::Runtime, "parameters"),
    sec("parameter.ids", Elem::Id, "parameters"),
    sec("parameter.max_values", Elem::F32, "parameters"),
    sec("parameter.min_values", Elem::F32, "parameters"),
    sec("parameter.default_values", Elem::F32, "parameters"),
    sec("parameter.repeats", Elem::Bool, "parameters"),
    sec("parameter.decimal_places", Elem::I32, "parameters"),
    sec("parameter.keyform_binding_begin_indices", Elem::I32, "parameters"),
    sec("parameter.keyform_binding_counts", Elem::I32, "parameters"),
    // keyforms
    sec("part_keyform.draw_orders", Elem::F32, "part_keyforms"),
    sec("warp_deformer_keyform.opacities", Elem::F32, "warp_deformer_keyforms"),
    sec("warp_deformer_keyform.keyform_position_begin_indices", Elem::I32, "warp_deformer_keyforms"),
    sec("rotation_deformer_keyform.opacities", Elem::F32, "rotation_deformer_keyforms"),
    sec("rotation_deformer_keyform.angles", Elem::F32, "rotation_deformer_keyforms"),
    sec("rotation_deformer_keyform.origin_xs", Elem::F32, "rotation_deformer_keyforms"),
    sec("rotation_deformer_keyform.origin_ys", Elem::F32, "rotation_deformer_keyforms"),
    sec("rotation_deformer_keyform.scales", Elem::F32, "rotation_deformer_keyforms"),
    sec("rotation_deformer_keyform.reflect_xs", Elem::Bool, "rotation_deformer_keyforms"),
    sec("rotation_deformer_keyform.reflect_ys", Elem::Bool, "rotation_deformer_keyforms"),
    sec("art_mesh_keyform.opacities", Elem::F32, "art_mesh_keyforms"),
    sec("art_mesh_keyform.draw_orders", Elem::F32, "art_mesh_keyforms"),
    sec("art_mesh_keyform.keyform_position_begin_indices", Elem::I32, "art_mesh_keyforms"),
    sec("keyform_position.xys", Elem::F32, "keyform_positions"),
    sec("keyform_binding_index.indices", Elem::I32, "parameter_binding_indices"),
    sec("keyform_binding_band.begin_indices", Elem::I32, "keyform_bindings"),
    sec("keyform_binding_band.counts", Elem::I32, "keyform_bindings"),
    sec("keyform_binding.keys_begin_indices", Elem::I32, "parameter_bindings"),
    sec("keyform_binding.keys_counts", Elem::I32, "parameter_bindings"),
    sec("keys.values", Elem::F32, "keys"),
    sec("uv.xys", Elem::F32, "uvs"),
    sec("position_index.indices", Elem::I16, "position_indices"),
    sec("drawable_mask.art_mesh_indices", Elem::I32, "drawable_masks"),
    sec("draw_order_group.object_begin_indices", Elem::I32, "draw_order_groups"),
    sec("draw_order_group.object_counts", Elem::I32, "draw_order_groups"),
    sec("draw_order_group.object_total_counts", Elem::I32, "draw_order_groups"),
    sec("draw_order_group.min_draw_orders", Elem::I32, "draw_order_groups"),
    sec("draw_order_group.max_draw_orders", Elem::I32, "draw_order_groups"),
    sec("draw_order_group_object.types", Elem::I32, "draw_order_group_objects"),
    sec("draw_order_group_object.indices", Elem::I32, "draw_order_group_objects"),
    sec("draw_order_group_object.group_indices", Elem::I32, "draw_order_group_objects"),
    sec("glue.runtime_space", Elem::Runtime, "glue"),
    sec("glue.ids", Elem::Id, "glue"),
    sec("glue.keyform_binding_band_indices", Elem::I32, "glue"),
    sec("glue.keyform_begin_indices", Elem::I32, "glue"),
    sec("glue.keyform_counts", Elem::I32, "glue"),
    sec("glue.art_mesh_index_as", Elem::I32, "glue"),
    sec("glue.art_mesh_index_bs", Elem::I32, "glue"),
    sec("glue.info_begin_indices", Elem::I32, "glue"),
    sec("glue.info_counts", Elem::I32, "glue"),
    sec("glue_info.weights", Elem::F32, "glue_info"),
    sec("glue_info.position_indices", Elem::I16, "glue_info"),
    sec("glue_keyform.intensities", Elem::F32, "glue_keyforms"),
];

// ---------------------------------------------------------------- estructuras

#[derive(Debug, Clone)]
pub struct ArtMesh {
    pub id: String,
    pub texture_index: u32,
    pub vertex_count: u32,
    pub uv_begin: u32,
    pub index_begin: u32,
    pub index_count: u32,
    pub keyform_begin: u32,
    pub keyform_count: u32,
    pub parent_part: i32,
    pub parent_deformer: i32,
    pub visible: bool,
    pub mask_begin: u32,
    pub mask_count: u32,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub id: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Body {
    pub part_ids: Vec<String>,
    pub part_visible: Vec<bool>,
    /// Draw order del keyform de reposo de cada parte.
    pub part_draw_order: Vec<f32>,
    /// Índice del keyform de reposo de cada parte (dentro de `part_keyform.*`).
    pub part_keyform_begin: Vec<u32>,
    pub art_meshes: Vec<ArtMesh>,
    pub parameters: Vec<Parameter>,
    /// Pares de coordenadas (x, y) por keyform, planos.
    pub keyform_positions: Vec<f32>,
    /// Referencia de cada keyform dentro de `keyform_positions` (en floats).
    pub art_mesh_keyform_positions: Vec<u32>,
    pub art_mesh_keyform_draw_orders: Vec<f32>,
    pub art_mesh_keyform_opacities: Vec<f32>,
    /// Coordenadas de textura (u, v) por pares, planas.
    pub uvs: Vec<f32>,
    pub indices: Vec<i16>,
    /// Índices de malla que actúan como máscara (`drawable_mask.art_mesh_indices`).
    pub drawable_masks: Vec<i32>,
}

// ---------------------------------------------------------------- lectura

impl Moc3 {
    /// Lee las secciones del cuerpo. Requiere que [`Moc3::parse`] haya ido bien.
    pub fn body(&self) -> Result<Body, Error> {
        let data = self.data();
        let sob = self.section_offsets();

        let mut part_ids = Vec::new();
        let mut part_visible: Vec<bool> = Vec::new();
        let mut part_keyform_begin: Vec<u32> = Vec::new();
        let mut art_meshes = Vec::new();
        let mut parameters = Vec::new();

        for (i, sec) in LAYOUT.iter().enumerate() {
            let Some(offset) = sob.get(2 + i).copied() else {
                break;
            };
            let offset = offset as usize;
            let count = self.counts.get_or_zero(sec.count) as usize;
            if count == 0 {
                continue;
            }
            if offset == 0 || offset > data.len() {
                return Err(Error::OffsetOutOfRange {
                    what: sec.name,
                    offset: offset as u32,
                });
            }

            match sec.elem {
                Elem::Id => {
                    let ids = read_ids(data, offset, count)?;
                    match sec.name {
                        "part.ids" => part_ids = ids,
                        "parameter.ids" => {
                            parameters = ids
                                .into_iter()
                                .map(|id| Parameter {
                                    id,
                                    min: 0.0,
                                    max: 0.0,
                                    default: 0.0,
                                })
                                .collect();
                        }
                        "art_mesh.ids" => {
                            art_meshes = ids
                                .into_iter()
                                .map(|id| ArtMesh {
                                    id,
                                    texture_index: 0,
                                    vertex_count: 0,
                                    uv_begin: 0,
                                    index_begin: 0,
                                    index_count: 0,
                                    keyform_begin: 0,
                                    keyform_count: 0,
                                    parent_part: -1,
                                    parent_deformer: -1,
                                    visible: true,
                                    mask_begin: 0,
                                    mask_count: 0,
                                })
                                .collect();
                        }
                        _ => {}
                    }
                }
                Elem::I32 if sec.name == "art_mesh.vertex_counts" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].vertex_count = v.max(0) as u32;
                    }
                }
                Elem::I32 if sec.name == "art_mesh.texture_indices" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].texture_index = v.max(0) as u32;
                    }
                }
                Elem::I32 if sec.name == "art_mesh.uv_begin_indices" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].uv_begin = v.max(0) as u32;
                    }
                }
                Elem::I32 if sec.name == "art_mesh.position_index_begin_indices" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].index_begin = v.max(0) as u32;
                    }
                }
                Elem::I32 if sec.name == "art_mesh.position_index_counts" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].index_count = v.max(0) as u32;
                    }
                }
                Elem::I32 if sec.name == "art_mesh.keyform_begin_indices" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].keyform_begin = v.max(0) as u32;
                    }
                }
                Elem::I32 if sec.name == "art_mesh.keyform_counts" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].keyform_count = v.max(0) as u32;
                    }
                }
                Elem::I32 if sec.name == "art_mesh.parent_part_indices" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].parent_part = v;
                    }
                }
                Elem::I32 if sec.name == "art_mesh.mask_begin_indices" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].mask_begin = v.max(0) as u32;
                    }
                }
                Elem::I32 if sec.name == "art_mesh.mask_counts" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].mask_count = v.max(0) as u32;
                    }
                }
                Elem::I32 if sec.name == "art_mesh.parent_deformer_indices" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].parent_deformer = v;
                    }
                }
                Elem::I32 if sec.name == "part.keyform_begin_indices" => {
                    part_keyform_begin = read_i32(data, offset, count)?
                        .into_iter()
                        .map(|v| v.max(0) as u32)
                        .collect();
                }
                Elem::Bool if sec.name == "part.visibles" => {
                    part_visible = read_i32(data, offset, count)?
                        .into_iter()
                        .map(|v| v != 0)
                        .collect();
                }
                Elem::Bool if sec.name == "art_mesh.visibles" => {
                    for (m, v) in read_i32(data, offset, count)?.into_iter().enumerate() {
                        art_meshes[m].visible = v != 0;
                    }
                }
                Elem::F32 if sec.name == "parameter.max_values" => {
                    for (p, v) in read_f32(data, offset, count)?.into_iter().enumerate() {
                        parameters[p].max = v;
                    }
                }
                Elem::F32 if sec.name == "parameter.min_values" => {
                    for (p, v) in read_f32(data, offset, count)?.into_iter().enumerate() {
                        parameters[p].min = v;
                    }
                }
                Elem::F32 if sec.name == "parameter.default_values" => {
                    for (p, v) in read_f32(data, offset, count)?.into_iter().enumerate() {
                        parameters[p].default = v;
                    }
                }
                _ => {}
            }
        }

        let keyform_positions = self.read_f32_section("keyform_position.xys")?;
        let art_mesh_keyform_positions = self.read_i32_section("art_mesh_keyform.keyform_position_begin_indices")?;
        let art_mesh_keyform_draw_orders = self.read_f32_section("art_mesh_keyform.draw_orders")?;
        let art_mesh_keyform_opacities = self.read_f32_section("art_mesh_keyform.opacities")?;
        let uvs = self.read_f32_section("uv.xys")?;
        let indices = self.read_i16_section("position_index.indices")?;
        let drawable_masks = self.read_i32_section("drawable_mask.art_mesh_indices")?;

        let part_draw_order = self.read_f32_section("part_keyform.draw_orders")?;

        Ok(Body {
            part_ids,
            part_visible,
            part_draw_order,
            part_keyform_begin,
            art_meshes,
            parameters,
            keyform_positions,
            art_mesh_keyform_positions: art_mesh_keyform_positions
                .into_iter()
                .map(|v| v.max(0) as u32)
                .collect(),
            art_mesh_keyform_draw_orders,
            art_mesh_keyform_opacities,
            uvs,
            indices,
            drawable_masks,
        })
    }

    fn section_data(&self, name: &str) -> Result<(&[u8], usize), Error> {
        let idx = LAYOUT
            .iter()
            .position(|s| s.name == name)
            .ok_or(Error::OffsetOutOfRange {
                what: "sección desconocida",
                offset: 0,
            })?;
        let sec = &LAYOUT[idx];
        let offset = *self
            .section_offsets()
            .get(2 + idx)
            .ok_or(Error::OffsetOutOfRange {
                what: sec.name,
                offset: 0,
            })? as usize;
        let count = self.counts.get_or_zero(sec.count) as usize;
        if count > 0 && (offset == 0 || offset > self.data().len()) {
            return Err(Error::OffsetOutOfRange {
                what: sec.name,
                offset: offset as u32,
            });
        }
        Ok((self.data(), offset))
    }

    pub fn read_f32_section(&self, name: &str) -> Result<Vec<f32>, Error> {
        let (data, offset) = self.section_data(name)?;
        let count = self.section_count(name);
        if count == 0 {
            return Ok(Vec::new());
        }
        read_f32(data, offset, count)
    }

    pub fn read_i32_section(&self, name: &str) -> Result<Vec<i32>, Error> {
        let (data, offset) = self.section_data(name)?;
        let count = self.section_count(name);
        if count == 0 {
            return Ok(Vec::new());
        }
        read_i32(data, offset, count)
    }

    pub fn read_i16_section(&self, name: &str) -> Result<Vec<i16>, Error> {
        let (data, offset) = self.section_data(name)?;
        let count = self.section_count(name);
        if count == 0 {
            return Ok(Vec::new());
        }
        if offset + count * 2 > data.len() {
            return Err(Error::TooShort {
                need: offset + count * 2,
                have: data.len(),
            });
        }
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let o = offset + i * 2;
            out.push(i16::from_le_bytes([data[o], data[o + 1]]));
        }
        Ok(out)
    }

    pub fn read_ids_section(&self, name: &str) -> Result<Vec<String>, Error> {
        let (data, offset) = self.section_data(name)?;
        let count = self.section_count(name);
        if count == 0 {
            return Ok(Vec::new());
        }
        read_ids(data, offset, count)
    }

    /// Sección de bytes (`Bool32` del formato): un `bool` por elemento.
    pub fn read_bool_section(&self, name: &str) -> Result<Vec<bool>, Error> {
        let (data, offset) = self.section_data(name)?;
        let count = self.section_count(name);
        if count == 0 {
            return Ok(Vec::new());
        }
        if offset + count > data.len() {
            return Err(Error::TooShort {
                need: offset + count,
                have: data.len(),
            });
        }
        Ok(data[offset..offset + count].iter().map(|b| *b != 0).collect())
    }

    fn section_count(&self, name: &str) -> usize {
        LAYOUT
            .iter()
            .find(|s| s.name == name)
            .map(|s| self.counts.get_or_zero(s.count) as usize)
            .unwrap_or(0)
    }
}

fn read_f32(data: &[u8], offset: usize, count: usize) -> Result<Vec<f32>, Error> {
    if offset + count * 4 > data.len() {
        return Err(Error::TooShort {
            need: offset + count * 4,
            have: data.len(),
        });
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * 4;
        out.push(f32::from_le_bytes([
            data[o],
            data[o + 1],
            data[o + 2],
            data[o + 3],
        ]));
    }
    Ok(out)
}

fn read_i32(data: &[u8], offset: usize, count: usize) -> Result<Vec<i32>, Error> {
    if offset + count * 4 > data.len() {
        return Err(Error::TooShort {
            need: offset + count * 4,
            have: data.len(),
        });
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * 4;
        out.push(i32::from_le_bytes([
            data[o],
            data[o + 1],
            data[o + 2],
            data[o + 3],
        ]));
    }
    Ok(out)
}

fn read_ids(data: &[u8], offset: usize, count: usize) -> Result<Vec<String>, Error> {
    const ID_SIZE: usize = 64;
    if offset + count * ID_SIZE > data.len() {
        return Err(Error::TooShort {
            need: offset + count * ID_SIZE,
            have: data.len(),
        });
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let start = offset + i * ID_SIZE;
        let slot = &data[start..start + ID_SIZE];
        let end = slot.iter().position(|&b| b == 0).unwrap_or(ID_SIZE);
        out.push(String::from_utf8_lossy(&slot[..end]).into_owned());
    }
    Ok(out)
}
