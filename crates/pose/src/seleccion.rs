//! Selección de keyform por valor de parámetro.
//!
//! Implementa la regla medida contra el Core en `docs/13` §2, que en Hiyori cuadra con el
//! número de keyforms declarado en **238/238** elementos (134 mallas + 50 warps +
//! 54 rotaciones):
//!
//! - `keyform_binding_index.indices[s]` **no** es una banda: es el índice de la lista de
//!   claves del slot `s`.
//! - La banda `b` ocupa los slots `[begin[b], begin[b] + count[b])`.
//! - El índice de keyform es el **radix** de la posición del valor en cada lista, en el
//!   orden de la banda: `kf = Σ_i pos_i · Π_{j<i} len(claves_j)`.
//!
//! Límite conocido (no oculto): con parámetros movidos la regla **no** se sostiene
//! (`docs/13` §5.7: 1/14 en `ParamAngleZ=10`). Aquí se implementa tal cual está medida,
//! y el CLI de `pose` imprime la discrepancia en vez de esconderla.

use moc3::Moc3;

/// Tablas de `keyform_binding.*` más los valores por defecto de los parámetros.
pub struct Bindings {
    pub parametros: Vec<String>,
    pub defectos: Vec<f32>,
    /// `keyform_binding_index.indices`: slot → índice de la lista de claves.
    kbi: Vec<i32>,
    b_ini: Vec<i32>,
    b_cnt: Vec<i32>,
    k_ini: Vec<i32>,
    k_cnt: Vec<i32>,
    keys: Vec<f32>,
    /// slot → parámetro al que pertenece (`None` si el slot no está en ninguna banda… o no
    /// se pudo resolver).
    slot_a_param: Vec<Option<usize>>,
}

impl Bindings {
    /// Lee las tablas del `.moc3` ya parseado.
    pub fn cargar(moc: &Moc3) -> Self {
        let parametros = moc.read_ids_section("parameter.ids").unwrap_or_default();
        let defectos = moc
            .read_f32_section("parameter.default_values")
            .unwrap_or_default();
        let kbi = moc
            .read_i32_section("keyform_binding_index.indices")
            .unwrap_or_default();
        let p_ini = moc
            .read_i32_section("parameter.keyform_binding_begin_indices")
            .unwrap_or_default();
        let p_cnt = moc
            .read_i32_section("parameter.keyform_binding_counts")
            .unwrap_or_default();
        let b_ini = moc
            .read_i32_section("keyform_binding_band.begin_indices")
            .unwrap_or_default();
        let b_cnt = moc
            .read_i32_section("keyform_binding_band.counts")
            .unwrap_or_default();
        let k_ini = moc
            .read_i32_section("keyform_binding.keys_begin_indices")
            .unwrap_or_default();
        let k_cnt = moc
            .read_i32_section("keyform_binding.keys_counts")
            .unwrap_or_default();
        let keys = moc.read_f32_section("keys.values").unwrap_or_default();

        let mut slot_a_param = vec![None; kbi.len()];
        for (p, id) in parametros.iter().enumerate() {
            let ini = p_ini.get(p).copied().unwrap_or(0).max(0) as usize;
            let cnt = p_cnt.get(p).copied().unwrap_or(0).max(0) as usize;
            for s in ini..(ini + cnt).min(slot_a_param.len()) {
                slot_a_param[s] = Some(p);
                let _ = id;
            }
        }

        Self {
            parametros,
            defectos,
            kbi,
            b_ini,
            b_cnt,
            k_ini,
            k_cnt,
            keys,
            slot_a_param,
        }
    }

    /// Listas de claves de una banda, en el orden del radix: `(parámetro, valores)`.
    pub fn claves_de(&self, banda: usize) -> Vec<(Option<usize>, Vec<f32>)> {
        let ini = self.b_ini.get(banda).copied().unwrap_or(0).max(0) as usize;
        let cnt = self.b_cnt.get(banda).copied().unwrap_or(0).max(0) as usize;
        let mut out = Vec::with_capacity(cnt);
        for s in ini..ini + cnt {
            let Some(&lista) = self.kbi.get(s) else {
                continue;
            };
            let lista = lista.max(0) as usize;
            let ki = self.k_ini.get(lista).copied().unwrap_or(0).max(0) as usize;
            let kc = self.k_cnt.get(lista).copied().unwrap_or(0).max(0) as usize;
            let valores = self
                .keys
                .get(ki..(ki + kc).min(self.keys.len()))
                .unwrap_or(&[])
                .to_vec();
            out.push((self.slot_a_param.get(s).copied().flatten(), valores));
        }
        out
    }

    /// Valor de un parámetro: el que se pide en `valores` o, si no está, su defecto.
    fn valor(&self, p: usize, valores: &[f32]) -> f32 {
        valores
            .get(p)
            .copied()
            .or_else(|| self.defectos.get(p).copied())
            .unwrap_or(0.0)
    }

    /// Índice de keyform de una banda con los valores por defecto del modelo.
    pub fn indice(&self, banda: usize, n_keyforms: usize) -> usize {
        self.indice_con_valores(banda, n_keyforms, &[])
    }

    /// Índice de keyform de una banda con valores concretos de parámetro.
    ///
    /// `valores[p]` manda sobre el valor por defecto de `p`. Un valor fuera del rango de la
    /// lista se recorta a la clave extrema (no se extrapola el índice).
    pub fn indice_con_valores(&self, banda: usize, n_keyforms: usize, valores: &[f32]) -> usize {
        if n_keyforms == 0 {
            return 0;
        }
        let lista = self.claves_de(banda);
        let mut pos = 0usize;
        let mut mul = 1usize;
        for (parametro, claves) in lista {
            if claves.is_empty() {
                continue;
            }
            let v = parametro.map(|p| self.valor(p, valores)).unwrap_or(0.0);
            let mut mejor = 0usize;
            let mut dist = f32::INFINITY;
            for (i, k) in claves.iter().enumerate() {
                let d = (k - v).abs();
                if d < dist {
                    dist = d;
                    mejor = i;
                }
            }
            pos += mejor * mul;
            mul *= claves.len();
        }
        pos.min(n_keyforms - 1)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_radix_es_el_producto_de_las_listas() {
        // una banda con listas de 3 y 2 claves → 6 keyforms, en orden de archivo
        let b = Bindings {
            parametros: vec!["A".into(), "B".into()],
            defectos: vec![0.0, 0.0],
            kbi: vec![0, 1],
            b_ini: vec![0],
            b_cnt: vec![2],
            k_ini: vec![0, 3],
            k_cnt: vec![3, 2],
            keys: vec![-10.0, 0.0, 10.0, 0.0, 1.0],
            slot_a_param: vec![Some(0), Some(1)],
        };
        // A=-10 (i=0) y B=0 (i=0) → 0·1 + 0·3 = 0
        assert_eq!(b.indice_con_valores(0, 6, &[-10.0, 0.0]), 0);
        // A=-10, B=1 → 0 + 1·3 = 3
        assert_eq!(b.indice_con_valores(0, 6, &[-10.0, 1.0]), 3);
        // A=0 (i=1), B=0 → 1
        assert_eq!(b.indice_con_valores(0, 6, &[0.0, 0.0]), 1);
        // A=10 (i=2), B=1 → 2 + 3 = 5 (el último)
        assert_eq!(b.indice_con_valores(0, 6, &[10.0, 1.0]), 5);
        // fuera de rango: cada valor cae en su clave extrema (A→i=2, B→i=0) → 2
        assert_eq!(b.indice_con_valores(0, 6, &[999.0, -5.0]), 2);
        // sin valores: usa los defectos (A=0, B=0) → 1
        assert_eq!(b.indice(0, 6), 1);
    }
}
