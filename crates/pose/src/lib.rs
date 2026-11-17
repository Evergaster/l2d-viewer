//! Grafo de deformadores y layout del pool de posiciones de un `.moc3`.
//!
//! Este crate es el principio del **motor de deformación propio**: primero la estructura
//! (qué deformador cuelga de quién, de qué tipo es y dónde están sus puntos de control) y
//! el layout real del pool de posiciones. Las fórmulas de warp y rotación se añaden encima.
//!
//! # El layout del pool (medido, no supuesto)
//!
//! `keyform_position.xys` es **un solo pool** compartido por keyforms de malla y de warp, y
//! cada keyform ocupa un bloque **alineado a 8 puntos**: `ceil(n / 8) * 8`, donde `n` es el
//! número de vértices (36 en un warp 5×5 → bloque de 40).
//!
//! Verificado contra Hiyori: los 134 bloques de malla no se solapan, el total cuadra exacto
//! (271 keyforms de warp × 40 + 1 242 de malla = 40 200 puntos) y con esa regla el bloque de
//! la única malla sin deformador padre coincide con el Core con error de 3·10⁻⁶.
//!
//! # Y se invierte
//!
//! El oráculo del Core entrega la misma geometría que el archivo con **y cambiada de signo**.
//! Por eso [`Modelo::vertices`] devuelve las coordenadas tal como están en el archivo y
//! [`Modelo::vertices_en_espacio_del_core`] aplica el cambio de signo.

use moc3::Moc3;

pub mod deformar;
pub mod seleccion;

pub use deformar::{evaluar, evaluar_pasos, rotacion, warp, CamposRotacion, Seleccion};
pub use seleccion::Bindings;

/// Alineación de cada bloque de keyform en el pool, en puntos.
pub const ALINEACION: usize = 8;

/// Cuántos puntos ocupa un keyform de `n` vértices en el pool.
pub const fn bloque_puntos(n: usize) -> usize {
    n.div_ceil(ALINEACION) * ALINEACION
}

/// Tipo de deformador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    /// Rejilla (warp): tipo 0 en el archivo.
    Warp,
    /// Rotación con escala y espejo: tipo 1 en el archivo.
    Rotacion,
}

/// Un deformador del grafo.
#[derive(Debug, Clone)]
pub struct Deformador {
    pub indice: usize,
    pub id: String,
    pub tipo: Tipo,
    /// Índice dentro de la tabla de su tipo (`warp_deformer.*` o `rotation_deformer.*`).
    pub specific: usize,
    /// Padre en la cadena; `None` = raíz.
    pub padre: Option<usize>,
}

/// Una malla de arte (drawable) enganchada a su deformador.
#[derive(Debug, Clone)]
pub struct Malla {
    pub indice: usize,
    pub id: String,
    /// Número de **vértices** (no de índices de triángulo).
    pub vertices: usize,
    pub keyform_begin: usize,
    pub keyform_count: usize,
    pub padre: Option<usize>,
}

/// Deformador de rejilla.
#[derive(Debug, Clone)]
pub struct Warp {
    pub filas: usize,
    pub columnas: usize,
    /// Índice del primer keyform en la tabla `warp_deformer_keyform.keyform_position_begin_indices`
    /// (que tiene una entrada por **keyform**, no por deformador).
    pub keyform_begin: usize,
    pub keyform_count: usize,
}

/// Deformador de rotación.
#[derive(Debug, Clone)]
pub struct Rotacion {
    pub keyform_begin: usize,
    pub keyform_count: usize,
    pub base_angle: f32,
}

/// Modelo con el grafo y el pool listos para evaluar.
pub struct Modelo {
    pub pool: Vec<[f32; 2]>,
    pub deformadores: Vec<Deformador>,
    pub mallas: Vec<Malla>,
    pub warps: Vec<Warp>,
    pub rotaciones: Vec<Rotacion>,
    /// Campos de cada keyform de rotación, indexados por el índice global de keyform
    /// (`rotation_deformer.keyform_begin_indices[specific] + k`).
    pub rot_campos: Vec<CamposRotacion>,
    /// Banda de claves de cada deformador (`deformer.keyform_binding_band_indices`).
    pub bandas_deformador: Vec<usize>,
    /// Banda de claves de cada malla (`art_mesh.keyform_binding_band_indices`).
    pub bandas_malla: Vec<usize>,
    /// Offsets de inicio (en floats) de la tabla de keyforms de cada malla.
    offsets_malla: Vec<i32>,
    /// Offsets de inicio (en floats) de la tabla de keyforms de warp (una entrada por keyform).
    offsets_warp: Vec<i32>,
}

impl Modelo {
    /// Cuántos keyforms declara un deformador (`keyform_counts` de su tabla).
    pub fn keyforms_deformador(&self, i: usize) -> Option<usize> {
        let d = self.deformadores.get(i)?;
        match d.tipo {
            Tipo::Warp => self.warps.get(d.specific).map(|w| w.keyform_count),
            Tipo::Rotacion => self.rotaciones.get(d.specific).map(|r| r.keyform_count),
        }
    }

    /// Vertices de la malla `m` en el keyform `k`, tal como están en el archivo.
    pub fn vertices(&self, m: usize, k: usize) -> Option<&[[f32; 2]]> {
        let malla = self.mallas.get(m)?;
        if k >= malla.keyform_count {
            return None;
        }
        let off = *self.offsets_malla.get(malla.keyform_begin + k)?;
        let ini = usize::try_from(off).ok()? / 2;
        let n = malla.vertices;
        self.pool.get(ini..ini + n)
    }

    /// Igual que [`Modelo::vertices`] pero en el espacio del Core (y negada).
    pub fn vertices_en_espacio_del_core(&self, m: usize, k: usize) -> Option<Vec<[f32; 2]>> {
        let v = self.vertices(m, k)?;
        Some(v.iter().map(|p| [p[0], -p[1]]).collect())
    }

    /// Cadena de deformadores de una malla, **de la raíz a la malla**.
    pub fn cadena(&self, m: usize) -> Vec<usize> {
        let mut cadena = Vec::new();
        let mut actual = self.mallas.get(m).and_then(|m| m.padre);
        let mut guarda = 0;
        while let Some(p) = actual {
            cadena.push(p);
            actual = self.deformadores.get(p).and_then(|d| d.padre);
            guarda += 1;
            if guarda > 256 {
                break; // un ciclo en el archivo no nos cuelga
            }
        }
        cadena.reverse();
        cadena
    }

    /// Puntos de control del keyform `k` de un warp, en rejilla fila-mayor.
    pub fn puntos_warp(&self, w: usize, k: usize) -> Option<&[[f32; 2]]> {
        let warp = self.warps.get(w)?;
        if k >= warp.keyform_count {
            return None;
        }
        let n = (warp.filas + 1) * (warp.columnas + 1);
        let off = *self.offsets_warp.get(warp.keyform_begin + k)?;
        let ini = usize::try_from(off).ok()? / 2;
        self.pool.get(ini..ini + n)
    }
}

fn i32s(moc: &Moc3, nombre: &str) -> Vec<i32> {
    moc.read_i32_section(nombre).unwrap_or_default()
}

fn f32s(moc: &Moc3, nombre: &str) -> Vec<f32> {
    moc.read_f32_section(nombre).unwrap_or_default()
}

/// Carga el grafo y el pool de un `.moc3` ya parseado.
pub fn cargar(moc: &Moc3) -> Modelo {
    let xys = f32s(moc, "keyform_position.xys");
    let pool: Vec<[f32; 2]> = xys.chunks_exact(2).map(|c| [c[0], c[1]]).collect();

    let ids_df = moc.read_ids_section("deformer.ids").unwrap_or_default();
    let tipos = i32s(moc, "deformer.types");
    let padres_df = i32s(moc, "deformer.parent_deformer_indices");
    let specific = i32s(moc, "deformer.specific_indices");

    let deformadores: Vec<Deformador> = (0..tipos.len())
        .map(|i| Deformador {
            indice: i,
            id: ids_df.get(i).cloned().unwrap_or_default(),
            tipo: if tipos[i] == 0 {
                Tipo::Warp
            } else {
                Tipo::Rotacion
            },
            specific: specific.get(i).copied().unwrap_or(0).max(0) as usize,
            padre: padres_df.get(i).copied().unwrap_or(-1).try_into().ok(),
        })
        .collect();

    let ids_malla = moc.read_ids_section("art_mesh.ids").unwrap_or_default();
    let padres_malla = i32s(moc, "art_mesh.parent_deformer_indices");
    // OJO: en la tabla de secciones los dos nombres están cruzados respecto al contenido
    // (medido: `vertex_counts` suma 10 278 = índices, `position_index_counts` suma 2 822 =
    // vértices, que coincide con los pares UV). Aquí se usan por su significado real.
    let n_vertices = i32s(moc, "art_mesh.position_index_counts");
    let kf_begin = i32s(moc, "art_mesh.keyform_begin_indices");
    let kf_count = i32s(moc, "art_mesh.keyform_counts");
    let offsets_malla = i32s(moc, "art_mesh_keyform.keyform_position_begin_indices");

    let mallas: Vec<Malla> = (0..ids_malla.len())
        .map(|i| Malla {
            indice: i,
            id: ids_malla[i].clone(),
            vertices: n_vertices.get(i).copied().unwrap_or(0).max(0) as usize,
            keyform_begin: kf_begin.get(i).copied().unwrap_or(0).max(0) as usize,
            keyform_count: kf_count.get(i).copied().unwrap_or(0).max(0) as usize,
            padre: padres_malla.get(i).copied().unwrap_or(-1).try_into().ok(),
        })
        .collect();

    let filas = i32s(moc, "warp_deformer.rows");
    let columnas = i32s(moc, "warp_deformer.cols");
    let wkf_begin = i32s(moc, "warp_deformer.keyform_begin_indices");
    let wkf_count = i32s(moc, "warp_deformer.keyform_counts");
    let wkf_pool = i32s(moc, "warp_deformer_keyform.keyform_position_begin_indices");
    let n_warps = moc.counts.get_or_zero("warp_deformers") as usize;
    let warps: Vec<Warp> = (0..n_warps)
        .map(|w| Warp {
            filas: filas.get(w).copied().unwrap_or(0).max(0) as usize,
            columnas: columnas.get(w).copied().unwrap_or(0).max(0) as usize,
            keyform_begin: wkf_begin.get(w).copied().unwrap_or(0).max(0) as usize,
            keyform_count: wkf_count.get(w).copied().unwrap_or(0).max(0) as usize,
        })
        .collect();

    let base = f32s(moc, "rotation_deformer.base_angles");
    let rkf_begin = i32s(moc, "rotation_deformer.keyform_begin_indices");
    let rkf_count = i32s(moc, "rotation_deformer.keyform_counts");
    let n_rot = moc.counts.get_or_zero("rotation_deformers") as usize;
    let rotaciones: Vec<Rotacion> = (0..n_rot)
        .map(|i| Rotacion {
            keyform_begin: rkf_begin.get(i).copied().unwrap_or(0).max(0) as usize,
            keyform_count: rkf_count.get(i).copied().unwrap_or(0).max(0) as usize,
            base_angle: base.get(i).copied().unwrap_or(0.0),
        })
        .collect();

    let bandas_df = i32s(moc, "deformer.keyform_binding_band_indices");
    let bandas_malla_tab = i32s(moc, "art_mesh.keyform_binding_band_indices");
    let bandas_deformador: Vec<usize> = bandas_df.iter().map(|b| (*b).max(0) as usize).collect();
    let bandas_malla: Vec<usize> = bandas_malla_tab
        .iter()
        .map(|b| (*b).max(0) as usize)
        .collect();

    let angulos = f32s(moc, "rotation_deformer_keyform.angles");
    let origenes_x = f32s(moc, "rotation_deformer_keyform.origin_xs");
    let origenes_y = f32s(moc, "rotation_deformer_keyform.origin_ys");
    let escalas = f32s(moc, "rotation_deformer_keyform.scales");
    let reflejos_x = moc
        .read_bool_section("rotation_deformer_keyform.reflect_xs")
        .unwrap_or_default();
    let reflejos_y = moc
        .read_bool_section("rotation_deformer_keyform.reflect_ys")
        .unwrap_or_default();
    let rot_campos: Vec<CamposRotacion> = (0..angulos.len())
        .map(|k| CamposRotacion {
            angulo: angulos[k],
            origen: [
                origenes_x.get(k).copied().unwrap_or(0.0),
                origenes_y.get(k).copied().unwrap_or(0.0),
            ],
            escala: escalas.get(k).copied().unwrap_or(1.0),
            reflect_x: reflejos_x.get(k).copied().unwrap_or(false),
            reflect_y: reflejos_y.get(k).copied().unwrap_or(false),
        })
        .collect();

    Modelo {
        pool,
        deformadores,
        mallas,
        warps,
        rotaciones,
        rot_campos,
        bandas_deformador,
        bandas_malla,
        offsets_malla,
        offsets_warp: wkf_pool,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hiyori() -> Option<Modelo> {
        let ruta = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../models/samples/Samples/Resources/Hiyori/Hiyori.moc3"
        );
        let bytes = std::fs::read(ruta).ok()?;
        let moc = Moc3::parse(&bytes).ok()?;
        Some(cargar(&moc))
    }

    #[test]
    fn bloque_alineado_a_ocho() {
        assert_eq!(bloque_puntos(36), 40); // rejilla 5x5
        assert_eq!(bloque_puntos(26), 32);
        assert_eq!(bloque_puntos(24), 24);
        assert_eq!(bloque_puntos(1), 8);
        assert_eq!(bloque_puntos(0), 0);
    }

    #[test]
    fn el_pool_se_recorre_sin_solapes() {
        let Some(m) = hiyori() else { return };
        let total_esperado: usize = m.pool.len();
        let mut cursor = 0usize;
        let mut suma = 0usize;

        // primero todos los keyforms de warp, en orden de tabla: bloques de 40 puntos contiguos
        for (i, off) in m.offsets_warp.iter().enumerate() {
            let ini = usize::try_from(*off).unwrap() / 2;
            assert_eq!(ini, cursor, "keyform de warp {i} no empieza donde toca");
            cursor += bloque_puntos(36);
            suma += bloque_puntos(36);
        }
        // después los de malla
        for malla in &m.mallas {
            assert!(malla.vertices > 0, "{} sin vértices", malla.id);
            for k in 0..malla.keyform_count {
                let v = m
                    .vertices(malla.indice, k)
                    .unwrap_or_else(|| panic!("{} keyform {k} fuera del pool", malla.id));
                assert_eq!(v.len(), malla.vertices);
                suma += bloque_puntos(malla.vertices);
            }
        }
        assert_eq!(
            suma, total_esperado,
            "el pool no cuadra con la suma de bloques"
        );
        assert_eq!(m.pool.len(), 40_200);
    }

    #[test]
    fn la_unica_malla_sin_deformador_es_hitarea_y_cuadra_con_el_core() {
        let Some(m) = hiyori() else { return };
        let raices: Vec<&str> = m
            .mallas
            .iter()
            .filter(|x| x.padre.is_none())
            .map(|x| x.id.as_str())
            .collect();
        assert_eq!(raices, vec!["HitArea"]);
        let i = m.mallas.iter().position(|x| x.id == "HitArea").unwrap();
        assert_eq!(m.mallas[i].vertices, 4);

        // ancla medida contra el Core: mismas x, y cambiada de signo
        let v = m.vertices_en_espacio_del_core(i, 0).unwrap();
        let esperado = [
            [0.1736_f32, 0.3668],
            [-0.1736, 0.3668],
            [0.1736, -0.1313],
            [-0.1736, -0.1313],
        ];
        for (a, b) in v.iter().zip(esperado.iter()) {
            assert!((a[0] - b[0]).abs() < 5e-4, "{a:?} != {b:?}");
            assert!((a[1] - b[1]).abs() < 5e-4, "{a:?} != {b:?}");
        }
    }

    #[test]
    fn cadenas_coherentes() {
        let Some(m) = hiyori() else { return };
        let mut profundidades = std::collections::BTreeSet::new();
        for malla in &m.mallas {
            let c = m.cadena(malla.indice);
            profundidades.insert(c.len());
            // la cadena empieza en una raíz y todos sus eslabones existen
            for (k, d) in c.iter().enumerate() {
                assert!((*d) < m.deformadores.len());
                if k == 0 {
                    assert!(m.deformadores[*d].padre.is_none());
                }
            }
        }
        // en Hiyori la única malla sin deformador es HitArea (cadena 0); el resto, 3..12
        assert!(profundidades.contains(&0));
        assert_eq!(m.mallas.iter().filter(|x| x.padre.is_none()).count(), 1);
        assert!(profundidades.iter().filter(|&&p| p > 0).all(|&p| p >= 3));
        assert_eq!(
            m.deformadores.iter().filter(|d| d.padre.is_none()).count(),
            3
        );
        assert_eq!(m.warps.len(), 50);
        assert_eq!(m.rotaciones.len(), 54);
    }
}
