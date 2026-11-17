//! Fórmulas de deformación **verificadas contra el Core** (`docs/13` §1).
//!
//! - Rotación: `p' = escala · R(ángulo + base) · p + origen`, ángulos en grados, `R`
//!   antihoraria, `reflect_x/y` aplicados **antes**. Verificada exacta: 5/5 cadenas de sólo
//!   rotaciones con error 3·10⁻⁶, y aislada entre warps con residuo 9·10⁻⁶ (`docs/13` §5.25).
//! - Warp: bilineal por celda con `u = 5·p.x`, `v = 5·p.y` sobre el keyform elegido.
//!   Exacto en cadenas de hasta 4 eslabones y en cinco warps consecutivos; **falla** cuando
//!   una rotación alimenta a un warp (`docs/13` §5.21): ese es el hueco conocido, no se
//!   disimula aquí.
//!
//! El resultado sale en el **espacio del Core**, con la `y` negada respecto del archivo.

use crate::seleccion::Bindings;
use crate::{Modelo, Tipo};

/// Campos de un keyform de rotación, tal como están en el archivo.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CamposRotacion {
    /// Grados.
    pub angulo: f32,
    /// Traslación final (no es pivote: medido, `docs/13` §1).
    pub origen: [f32; 2],
    /// Escala uniforme.
    pub escala: f32,
    pub reflect_x: bool,
    pub reflect_y: bool,
}

/// Rotación de un punto con los campos de un keyform.
pub fn rotacion(p: [f32; 2], c: &CamposRotacion, base_grados: f32) -> [f32; 2] {
    let x = if c.reflect_x { -p[0] } else { p[0] };
    let y = if c.reflect_y { -p[1] } else { p[1] };
    let a = (c.angulo + base_grados).to_radians();
    let (s, co) = a.sin_cos();
    [
        c.escala * (co * x - s * y) + c.origen[0],
        c.escala * (s * x + co * y) + c.origen[1],
    ]
}

/// Interpolación bilineal por celda de una rejilla de `(filas+1)×(columnas+1)` puntos.
///
/// `p` se interpreta en `[0,1]²` (la hipótesis de la especificación): `u = 5·p.x`. Los
/// valores fuera se recortan al borde.
pub fn warp(rejilla: &[[f32; 2]], filas: usize, columnas: usize, p: [f32; 2]) -> [f32; 2] {
    if filas == 0 || columnas == 0 || rejilla.len() < (filas + 1) * (columnas + 1) {
        return p;
    }
    // `p` en `[0,1]²`: una celda mide `1/columnas` en x y `1/filas` en y.
    let cu = (p[0] * columnas as f32).clamp(0.0, columnas as f32 - 1e-6);
    let cv = (p[1] * filas as f32).clamp(0.0, filas as f32 - 1e-6);
    let i = (cu.floor() as usize).min(columnas - 1);
    let j = (cv.floor() as usize).min(filas - 1);
    let fu = cu - i as f32;
    let fv = cv - j as f32;

    let ancho = columnas + 1;
    let a = rejilla[j * ancho + i];
    let b = rejilla[j * ancho + i + 1];
    let c = rejilla[(j + 1) * ancho + i];
    let d = rejilla[(j + 1) * ancho + i + 1];

    let mut out = [0.0f32; 2];
    for k in 0..2 {
        out[k] = (1.0 - fu) * (1.0 - fv) * a[k]
            + fu * (1.0 - fv) * b[k]
            + (1.0 - fu) * fv * c[k]
            + fu * fv * d[k];
    }
    out
}

/// Qué keyform usa cada malla y cada deformador de una pose concreta.
#[derive(Debug, Clone, Default)]
pub struct Seleccion {
    pub malla: Vec<usize>,
    pub deformador: Vec<usize>,
}

impl Seleccion {
    /// Selección con los valores **por defecto** del modelo (la pose de reposo).
    pub fn reposo(modelo: &Modelo, bnd: &Bindings) -> Self {
        Self::con_valores(modelo, bnd, &[])
    }

    /// Selección con valores de parámetro concretos (`valores[p]`, vacío = defectos).
    pub fn con_valores(modelo: &Modelo, bnd: &Bindings, valores: &[f32]) -> Self {
        let malla = modelo
            .mallas
            .iter()
            .enumerate()
            .map(|(m, x)| {
                let banda = modelo.bandas_malla.get(m).copied().unwrap_or(0);
                bnd.indice_con_valores(banda, x.keyform_count.max(1), valores)
            })
            .collect();
        let deformador = modelo
            .deformadores
            .iter()
            .enumerate()
            .map(|(i, _d)| {
                let banda = modelo.bandas_deformador.get(i).copied().unwrap_or(0);
                let n = modelo.keyforms_deformador(i).unwrap_or(1).max(1);
                bnd.indice_con_valores(banda, n, valores)
            })
            .collect();
        Self { malla, deformador }
    }
}

/// Evalúa una malla con su selección de keyforms y devuelve sus vértices en el espacio del
/// Core (con la `y` negada).
///
/// El recorrido es **hoja→raíz**: el primer deformador que actúa es el padre directo de la
/// malla. Raíz→hoja se midió y falla (`docs/13` §5.2: 719/1438 de error).
pub fn evaluar(modelo: &Modelo, sel: &Seleccion, malla: usize) -> Option<Vec<[f32; 2]>> {
    Some(evaluar_pasos(modelo, sel, malla)?.pop()?.1)
}

/// Igual que [`evaluar`], pero devuelve el estado tras cada eslabón: `(id, posiciones)`.
///
/// Las posiciones van en el espacio del archivo hasta el último paso y en el del Core (con la
/// `y` negada) en todos, para que se puedan comparar con el volcado del Core directamente.
pub fn evaluar_pasos(
    modelo: &Modelo,
    sel: &Seleccion,
    malla: usize,
) -> Option<Vec<(String, Vec<[f32; 2]>)>> {
    let kf_malla = *sel.malla.get(malla)?;
    let mut p: Vec<[f32; 2]> = modelo.vertices(malla, kf_malla)?.to_vec();
    let mut pasos = vec![(
        format!("malla kf {kf_malla}"),
        p.iter().map(|q| [q[0], -q[1]]).collect(),
    )];

    for d in modelo.cadena(malla).into_iter().rev() {
        let def = modelo.deformadores.get(d)?;
        let k = *sel.deformador.get(d).unwrap_or(&0);
        match def.tipo {
            Tipo::Warp => {
                let w = modelo.warps.get(def.specific)?;
                let rejilla = modelo.puntos_warp(def.specific, k)?;
                p = p
                    .into_iter()
                    .map(|v| warp(rejilla, w.filas, w.columnas, v))
                    .collect();
            }
            Tipo::Rotacion => {
                let r = modelo.rotaciones.get(def.specific)?;
                let campos = modelo.rot_campos.get(r.keyform_begin + k)?;
                p = p
                    .into_iter()
                    .map(|v| rotacion(v, campos, r.base_angle))
                    .collect();
            }
        }
        pasos.push((
            modelo
                .deformadores
                .get(d)
                .map(|x| x.id.clone())
                .unwrap_or_default(),
            p.iter().map(|q| [q[0], -q[1]]).collect(),
        ));
    }
    Some(pasos)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_rotacion_es_una_traslacion_cuando_el_angulo_es_cero() {
        let c = CamposRotacion {
            angulo: 0.0,
            origen: [0.5, 0.25],
            escala: 0.5,
            reflect_x: false,
            reflect_y: false,
        };
        let r = rotacion([0.2, -0.4], &c, 0.0);
        assert!(
            (r[0] - 0.6).abs() < 1e-6 && (r[1] - 0.05).abs() < 1e-6,
            "{r:?}"
        );
    }

    #[test]
    fn noventa_grados_giran_antihorario() {
        let c = CamposRotacion {
            angulo: 90.0,
            origen: [0.0, 0.0],
            escala: 1.0,
            reflect_x: false,
            reflect_y: false,
        };
        let r = rotacion([1.0, 0.0], &c, 0.0);
        assert!((r[0]).abs() < 1e-6 && (r[1] - 1.0).abs() < 1e-6, "{r:?}");
    }

    #[test]
    fn el_reflejo_se_aplica_antes_de_rotar() {
        let c = CamposRotacion {
            angulo: 0.0,
            origen: [0.0, 0.0],
            escala: 1.0,
            reflect_x: true,
            reflect_y: false,
        };
        let r = rotacion([0.3, 0.7], &c, 0.0);
        assert!(
            (r[0] + 0.3).abs() < 1e-6 && (r[1] - 0.7).abs() < 1e-6,
            "{r:?}"
        );
    }

    #[test]
    fn el_warp_devuelve_la_rejilla_en_sus_esquinas() {
        // rejilla 2×2 celdas (3×3 puntos) que es la identidad sobre [0,1]²
        let mut rejilla = Vec::new();
        for j in 0..3 {
            for i in 0..3 {
                rejilla.push([i as f32 / 2.0, j as f32 / 2.0]);
            }
        }
        for (p, esperado) in [
            ([0.0, 0.0], [0.0, 0.0]),
            ([0.5, 0.5], [0.5, 0.5]),
            ([1.0, 1.0], [1.0, 1.0]),
            ([0.25, 0.75], [0.25, 0.75]),
        ] {
            let r = warp(&rejilla, 2, 2, p);
            assert!(
                (r[0] - esperado[0]).abs() < 1e-6 && (r[1] - esperado[1]).abs() < 1e-6,
                "{p:?} → {r:?}, esperaba {esperado:?}"
            );
        }
    }

    #[test]
    fn fuera_de_la_rejilla_se_recorta_al_borde() {
        let mut rejilla = Vec::new();
        for j in 0..3 {
            for i in 0..3 {
                rejilla.push([i as f32 / 2.0, j as f32 / 2.0]);
            }
        }
        let r = warp(&rejilla, 2, 2, [-0.5, 5.0]);
        assert!((r[0]).abs() < 1e-6 && (r[1] - 1.0).abs() < 1e-6, "{r:?}");
    }
}
