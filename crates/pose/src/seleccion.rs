//! Selección de keyform por valor de parámetro.
//!
//! Dos piezas, las dos verificadas contra el Core (y contra `mocari`, Rust/MIT, y la
//! especificación no oficial de AyagamiDev — nunca contra el SDK):
//!
//! 1. **Qué banda de claves usa cada elemento** (`*_keyform_binding_band_indices`) y qué
//!    parámetro es dueño de cada binding (`parameter.keyform_binding_begin_indices`: un
//!    parámetro puede tener varios bindings y manda el dueño del rango).
//! 2. **Slots con peso**: el Core no elige «el keyform más cercano», **interpola** entre los
//!    dos vecinos de cada eje. Con `t = (v − k_i)/(k_{i+1} − k_i)`, el keyform activo por eje
//!    es `k_i + (t ≠ 0)` y el peso se reparte `(1−t)`/`t`, multiplicando entre ejes. Cuando el
//!    valor cae en una clave queda un solo keyform con peso 1; cuando cae en la **última**,
//!    el activo es el de la derecha — la excepción de la banda 7 de Hiyori que costó
//!    `docs/13` §5.7 y que con esta regla deja de ser una excepción.

use moc3::Moc3;

/// Un keyform activo y cuánto pesa en la mezcla.
pub type Slot = (usize, f32);

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
    /// Binding → parámetro dueño.
    duenio: Vec<Option<usize>>,
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
        let p_ini = moc
            .read_i32_section("parameter.keyform_binding_begin_indices")
            .unwrap_or_default();

        // Dueño de cada binding: el parámetro cuyo `begin` lo cubre hasta el siguiente
        // `begin` estrictamente mayor (un parámetro sin binding repite el anterior y no
        // reclama nada nuevo).
        let n_bind = k_ini.len();
        let mut duenio = vec![None; n_bind];
        for (p, ini) in p_ini.iter().enumerate() {
            let ini = *ini;
            if ini < 0 || ini as usize >= n_bind {
                continue;
            }
            let fin = p_ini[p + 1..]
                .iter()
                .filter(|s| **s > ini)
                .copied()
                .min()
                .map(|s| s as usize)
                .unwrap_or(n_bind);
            for slot in duenio.iter_mut().take(fin).skip(ini as usize) {
                if slot.is_none() {
                    *slot = Some(p);
                }
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
            duenio,
        }
    }

    /// Listas de claves de una banda, en el orden del radix: `(parámetro, valores)`.
    pub fn claves_de(&self, banda: usize) -> Vec<(Option<usize>, Vec<f32>)> {
        let ini = self.b_ini.get(banda).copied().unwrap_or(0).max(0) as usize;
        let cnt = self.b_cnt.get(banda).copied().unwrap_or(0).max(0) as usize;
        let mut out = Vec::with_capacity(cnt);
        for s in ini..ini + cnt {
            let Some(&slot) = self.kbi.get(s) else {
                continue;
            };
            let binding = slot.max(0) as usize;
            let ki = self.k_ini.get(binding).copied().unwrap_or(0).max(0) as usize;
            let kc = self.k_cnt.get(binding).copied().unwrap_or(0).max(0) as usize;
            let valores = self
                .keys
                .get(ki..(ki + kc).min(self.keys.len()))
                .unwrap_or(&[])
                .to_vec();
            out.push((self.duenio.get(binding).copied().flatten(), valores));
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

    /// Keyforms activos y sus pesos para una banda con estos valores de parámetro.
    ///
    /// El primero de la lista es siempre la combinación «todo a la izquierda»: de ahí salen
    /// los flags de espejo, porque la especificación dice que se usan los del primer form.
    pub fn slots(&self, banda: usize, n_keyforms: usize, valores: &[f32]) -> Vec<Slot> {
        if n_keyforms == 0 {
            return Vec::new();
        }
        if n_keyforms == 1 {
            return vec![(0, 1.0)];
        }
        let ejes = self.claves_de(banda);
        if ejes.is_empty() {
            return vec![(0, 1.0)];
        }

        let mut tramos: Vec<(usize, f32, usize)> = Vec::with_capacity(ejes.len());
        let mut stride = 1usize;
        for (parametro, claves) in &ejes {
            if claves.is_empty() {
                continue;
            }
            let v = parametro.map(|p| self.valor(p, valores)).unwrap_or(0.0);
            let (izq, t) = if claves.len() < 2 {
                (0, 0.0)
            } else if v <= claves[0] {
                (0, 0.0)
            } else if v >= claves[claves.len() - 1] {
                (claves.len() - 2, 1.0)
            } else {
                let mut i = 0usize;
                while i + 2 < claves.len() && v >= claves[i + 1] {
                    i += 1;
                }
                let ancho = claves[i + 1] - claves[i];
                let t = if ancho.abs() < 1e-12 {
                    0.0
                } else {
                    (v - claves[i]) / ancho
                };
                (i, t.clamp(0.0, 1.0))
            };
            tramos.push((izq, t, stride));
            stride *= claves.len();
        }

        let mut slots = Vec::new();
        for mascara in 0..(1usize << tramos.len()) {
            let mut indice = 0usize;
            let mut peso = 1.0f32;
            for (eje, &(izq, t, st)) in tramos.iter().enumerate() {
                if (mascara >> eje) & 1 == 1 {
                    if t == 0.0 {
                        peso = 0.0;
                        break;
                    }
                    indice += (izq + 1) * st;
                    peso *= t;
                } else {
                    indice += izq * st;
                    peso *= 1.0 - t;
                }
            }
            if peso > 0.0 && indice < n_keyforms {
                slots.push((indice, peso));
            }
        }
        if slots.is_empty() {
            slots.push((0, 1.0));
        }
        slots
    }

    /// Keyform de mayor peso para esta banda (el «de reposo» del elemento).
    pub fn indice(&self, banda: usize, n_keyforms: usize, valores: &[f32]) -> usize {
        self.slots(banda, n_keyforms, valores)
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn un_eje() -> Bindings {
        // banda de un eje con claves [-10, 0, 10]
        Bindings {
            parametros: vec!["A".into()],
            defectos: vec![0.0],
            kbi: vec![0],
            b_ini: vec![0],
            b_cnt: vec![1],
            k_ini: vec![0],
            k_cnt: vec![3],
            keys: vec![-10.0, 0.0, 10.0],
            duenio: vec![Some(0)],
        }
    }

    #[test]
    fn un_solo_keyform_no_necesita_banda() {
        let b = un_eje();
        assert_eq!(b.slots(0, 1, &[]), vec![(0, 1.0)]);
    }

    #[test]
    fn el_peso_se_reparte_entre_dos_claves() {
        let b = un_eje();
        // en la clave del medio: un solo slot, el 1
        assert_eq!(b.slots(0, 3, &[0.0]), vec![(1, 1.0)]);
        // a mitad entre 0 y 10: mitades de los keyforms 1 y 2
        let s = b.slots(0, 3, &[5.0]);
        assert_eq!(s.len(), 2, "{s:?}");
        assert!(s[0].0 == 1 && (s[0].1 - 0.5).abs() < 1e-6, "{s:?}");
        assert!(s[1].0 == 2 && (s[1].1 - 0.5).abs() < 1e-6, "{s:?}");
        let total: f32 = s.iter().map(|x| x.1).sum();
        assert!((total - 1.0).abs() < 1e-5);
    }

    #[test]
    fn en_la_ultima_clave_gana_la_de_la_derecha() {
        let b = un_eje();
        assert_eq!(b.slots(0, 3, &[10.0]), vec![(2, 1.0)]);
        assert_eq!(b.slots(0, 3, &[99.0]), vec![(2, 1.0)]);
        assert_eq!(b.slots(0, 3, &[-99.0]), vec![(0, 1.0)]);
    }

    #[test]
    fn dos_ejes_producen_cuatro_slots_que_suman_uno() {
        let b = Bindings {
            parametros: vec!["A".into(), "B".into()],
            defectos: vec![0.0, 0.0],
            kbi: vec![0, 1],
            b_ini: vec![0],
            b_cnt: vec![2],
            k_ini: vec![0, 2],
            k_cnt: vec![2, 2],
            keys: vec![-1.0, 1.0, -1.0, 1.0],
            duenio: vec![Some(0), Some(1)],
        };
        let s = b.slots(0, 4, &[0.0, 0.0]);
        assert_eq!(s.len(), 4, "{s:?}");
        assert_eq!(s[0].0, 0);
        assert!((s[0].1 - 0.25).abs() < 1e-5, "{s:?}");
        let total: f32 = s.iter().map(|x| x.1).sum();
        assert!((total - 1.0).abs() < 1e-5, "los pesos suman {total}");
    }
}
