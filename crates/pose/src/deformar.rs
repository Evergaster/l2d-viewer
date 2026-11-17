//! Deformación **compuesta**, tal como la describen una especificación no oficial y una
//! implementación libre (nunca el SDK):
//!
//! - `AyagamiDev/docs · model/moc3_file_format.md`, § «Deformer chaining»: cada deformador
//!   deforma a **sus hijos inmediatos**, y el caso «rotación hija de un warp» es especial: su
//!   escala se multiplica por la de la rotación ancestro, su traslación se deforma por el warp
//!   padre y su ángulo se corrige sondeando `(0, −0.1)` a través de ese mismo padre.
//! - `mocari` 0.4.1 (Rust, MIT): la composición completa (`moc3/compose.rs`) y la
//!   interpolación bilineal con extrapolación fuera de `[0,1)²` (`core/deformers.rs`).
//!
//! Con las dos piezas —composición + selección con pesos (`seleccion.rs`)— el motor reproduce
//! el Core en **134/134 mallas de Hiyori y 34/34 de Wanko**, mediana 5·10⁻⁶, y también en las
//! poses movidas (`tests/marcador.rs`). Antes, con la cadena aplicada punto a punto, eran
//! 12/134: el error no estaba en las fórmulas, estaba en **qué** se compone.
//!
//! El resultado sale en el **espacio del Core**, con la `y` negada respecto del archivo.

use crate::seleccion::{Bindings, Slot};
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

/// Transformación de rejilla: bilineal por celda dentro de `[0,1)²`, extrapolación afín fuera.
///
/// La base afín sale de las cuatro esquinas de la rejilla (media de las dos diagonales), que
/// es la aproximación documentada para los puntos exteriores. La especificación afina además
/// el perímetro entre 1 y 3 unidades; en estos modelos los puntos exteriores son 9 y con la
/// base afín el error queda por debajo de 1e-3, así que queda anotado como pendiente menor.
pub fn warp(rejilla: &[[f32; 2]], filas: usize, columnas: usize, p: [f32; 2]) -> [f32; 2] {
    let ancho = columnas + 1;
    if filas == 0 || columnas == 0 || rejilla.len() < ancho * (filas + 1) {
        return p;
    }
    let (x, y) = (p[0], p[1]);
    if (0.0..1.0).contains(&x) && (0.0..1.0).contains(&y) {
        let cu = x * columnas as f32;
        let cv = y * filas as f32;
        let i = (cu as usize).min(columnas - 1);
        let j = (cv as usize).min(filas - 1);
        let fu = cu - i as f32;
        let fv = cv - j as f32;
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
        return out;
    }

    // fuera: base afín de las esquinas
    let c00 = rejilla[0];
    let c10 = rejilla[columnas];
    let c01 = rejilla[filas * ancho];
    let c11 = rejilla[filas * ancho + columnas];
    let d11_00 = [c11[0] - c00[0], c11[1] - c00[1]];
    let d10_01 = [c10[0] - c01[0], c10[1] - c01[1]];
    let dpdu = [0.5 * (d11_00[0] - d10_01[0]), 0.5 * (d11_00[1] - d10_01[1])];
    let dpdv = [0.5 * (d10_01[0] + d11_00[0]), 0.5 * (d10_01[1] + d11_00[1])];
    let centro = [
        0.25 * (c00[0] + c10[0] + c01[0] + c11[0]) - 0.5 * d11_00[0],
        0.25 * (c00[1] + c10[1] + c01[1] + c11[1]) - 0.5 * d11_00[1],
    ];
    [
        centro[0] + dpdv[0] * x + dpdu[0] * y,
        centro[1] + dpdv[1] * x + dpdu[1] * y,
    ]
}

/// Un deformador ya compuesto con toda su ascendencia.
#[derive(Debug, Clone, PartialEq)]
pub enum Compuesto {
    Warp {
        rejilla: Vec<[f32; 2]>,
        columnas: usize,
        filas: usize,
        /// Escala heredada de la rotación ancestro (sólo la usan las rotaciones hijas).
        escala: f32,
    },
    Rotacion {
        angulo: f32,
        origen: [f32; 2],
        escala: f32,
        reflect_x: bool,
        reflect_y: bool,
    },
}

impl Compuesto {
    /// Aplica el deformador a un punto.
    pub fn aplicar(&self, p: [f32; 2]) -> [f32; 2] {
        match self {
            Self::Warp {
                rejilla,
                columnas,
                filas,
                ..
            } => warp(rejilla, *filas, *columnas, p),
            Self::Rotacion {
                angulo,
                origen,
                escala,
                reflect_x,
                reflect_y,
            } => rotacion(
                p,
                &CamposRotacion {
                    angulo: *angulo,
                    origen: *origen,
                    escala: *escala,
                    reflect_x: *reflect_x,
                    reflect_y: *reflect_y,
                },
                0.0,
            ),
        }
    }

    /// Escala que heredarán las rotaciones hijas.
    pub fn escala_acum(&self) -> f32 {
        match self {
            Self::Warp { escala, .. } => *escala,
            Self::Rotacion { escala, .. } => *escala,
        }
    }
}

/// Qué keyforms usa cada malla y cada deformador, con sus pesos.
#[derive(Debug, Clone, Default)]
pub struct Seleccion {
    pub malla: Vec<Vec<Slot>>,
    pub deformador: Vec<Vec<Slot>>,
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
                bnd.slots(banda, x.keyform_count.max(1), valores)
            })
            .collect();
        let deformador = modelo
            .deformadores
            .iter()
            .enumerate()
            .map(|(i, _d)| {
                let banda = modelo.bandas_deformador.get(i).copied().unwrap_or(0);
                let n = modelo.keyforms_deformador(i).unwrap_or(1).max(1);
                bnd.slots(banda, n, valores)
            })
            .collect();
        Self { malla, deformador }
    }
}

/// Compone cada deformador con su padre, de la raíz a las hojas.
pub fn componer(modelo: &Modelo, sel: &Seleccion) -> Vec<Option<Compuesto>> {
    let n = modelo.deformadores.len();
    let mut profundidad = vec![0usize; n];
    for (i, d) in modelo.deformadores.iter().enumerate() {
        let mut p = d.padre;
        let mut d_paso = 0usize;
        while let Some(x) = p {
            d_paso += 1;
            p = modelo.deformadores.get(x).and_then(|y| y.padre);
            if d_paso > n {
                break;
            }
        }
        profundidad[i] = d_paso;
    }
    let mut orden: Vec<usize> = (0..n).collect();
    orden.sort_by_key(|i| profundidad[*i]);

    let mut comp: Vec<Option<Compuesto>> = vec![None; n];
    for idx in orden {
        let d = &modelo.deformadores[idx];
        let slots = sel.deformador.get(idx).cloned().unwrap_or_default();
        if slots.is_empty() {
            continue;
        }
        let padre = d.padre.and_then(|p| comp.get(p).cloned().flatten());
        match d.tipo {
            Tipo::Warp => {
                let Some(w) = modelo.warps.get(d.specific) else {
                    continue;
                };
                let puntos = (w.filas + 1) * (w.columnas + 1);
                let mut rejilla = vec![[0.0f32; 2]; puntos];
                for (k, peso) in &slots {
                    let Some(grid) = modelo.puntos_warp(d.specific, *k) else {
                        continue;
                    };
                    for (j, v) in grid.iter().enumerate().take(puntos) {
                        rejilla[j][0] += v[0] * peso;
                        rejilla[j][1] += v[1] * peso;
                    }
                }
                if let Some(p) = &padre {
                    for v in rejilla.iter_mut() {
                        *v = p.aplicar(*v);
                    }
                }
                comp[idx] = Some(Compuesto::Warp {
                    rejilla,
                    columnas: w.columnas,
                    filas: w.filas,
                    escala: padre.as_ref().map(|p| p.escala_acum()).unwrap_or(1.0),
                });
            }
            Tipo::Rotacion => {
                let Some(r) = modelo.rotaciones.get(d.specific) else {
                    continue;
                };
                let mut angulo = 0.0f32;
                let mut traslacion = [0.0f32, 0.0f32];
                let mut escala = 0.0f32;
                let mut kf_izq: Option<usize> = None;
                for (k, peso) in &slots {
                    let Some(c) = modelo.rot_campos.get(r.keyform_begin + k) else {
                        continue;
                    };
                    angulo += c.angulo * peso;
                    traslacion[0] += c.origen[0] * peso;
                    traslacion[1] += c.origen[1] * peso;
                    escala += c.escala * peso;
                    if kf_izq.is_none() {
                        kf_izq = Some(*k);
                    }
                }
                angulo += r.base_angle;
                // los flags de espejo son los del primer form del conjunto (el de la izquierda)
                let (fx, fy) = kf_izq
                    .and_then(|k| modelo.rot_campos.get(r.keyform_begin + k))
                    .map(|c| (c.reflect_x, c.reflect_y))
                    .unwrap_or((false, false));

                match &padre {
                    None => {
                        comp[idx] = Some(Compuesto::Rotacion {
                            angulo,
                            origen: traslacion,
                            escala,
                            reflect_x: fx,
                            reflect_y: fy,
                        });
                    }
                    Some(p) => {
                        let origen = p.aplicar(traslacion);
                        let desfase = sondeo(p, origen, traslacion);
                        comp[idx] = Some(Compuesto::Rotacion {
                            angulo: angulo + desfase.to_degrees(),
                            origen,
                            escala: escala * p.escala_acum(),
                            reflect_x: fx,
                            reflect_y: fy,
                        });
                    }
                }
            }
        }
    }
    comp
}

/// Desfase de ángulo que impone el padre: se sondea un punto `0.1` más abajo (`−10` si el
/// padre es una rotación) y se mira cuánto gira el vector al deformarlo (`compose.rs`).
fn sondeo(padre: &Compuesto, origen: [f32; 2], traslacion: [f32; 2]) -> f32 {
    let paso = match padre {
        Compuesto::Warp { .. } => -0.1f32,
        Compuesto::Rotacion { .. } => -10.0f32,
    };
    let sonda = padre.aplicar([traslacion[0], traslacion[1] + paso]);
    let dx = sonda[0] - origen[0];
    let dy = sonda[1] - origen[1];
    if dx == 0.0 && dy == 0.0 {
        return 0.0;
    }
    dy.atan2(dx) - paso.atan2(0.0)
}

/// Vértices de una malla con la mezcla de sus keyforms, antes de aplicar el padre.
fn malla_mezclada(modelo: &Modelo, sel: &Seleccion, malla: usize) -> Option<Vec<[f32; 2]>> {
    let slots = sel.malla.get(malla)?;
    let n = modelo.mallas.get(malla)?.vertices;
    let mut p = vec![[0.0f32; 2]; n];
    for (k, peso) in slots {
        let v = modelo.vertices(malla, *k)?;
        for (j, q) in v.iter().enumerate().take(n) {
            p[j][0] += q[0] * peso;
            p[j][1] += q[1] * peso;
        }
    }
    Some(p)
}

/// Evalúa una malla: mezcla de sus keyforms y **una** aplicación del padre compuesto.
///
/// Los vértices salen en el espacio del Core (con la `y` negada).
pub fn evaluar(
    modelo: &Modelo,
    comp: &[Option<Compuesto>],
    sel: &Seleccion,
    malla: usize,
) -> Option<Vec<[f32; 2]>> {
    let mut p = malla_mezclada(modelo, sel, malla)?;
    if let Some(padre) = modelo.mallas.get(malla).and_then(|m| m.padre) {
        if let Some(c) = comp.get(padre).cloned().flatten() {
            p = p.into_iter().map(|v| c.aplicar(v)).collect();
        }
    }
    Some(p.into_iter().map(|q| [q[0], -q[1]]).collect())
}

/// Igual que [`evaluar`], pero con el estado antes y después del padre: `(nombre, posiciones)`.
pub fn evaluar_pasos(
    modelo: &Modelo,
    comp: &[Option<Compuesto>],
    sel: &Seleccion,
    malla: usize,
) -> Option<Vec<(String, Vec<[f32; 2]>)>> {
    let m = malla_mezclada(modelo, sel, malla)?;
    let antes: Vec<[f32; 2]> = m.iter().map(|q| [q[0], -q[1]]).collect();
    let mut pasos = vec![("malla (mezcla de keyforms)".to_string(), antes)];

    if let Some(i) = modelo.mallas.get(malla).and_then(|x| x.padre) {
        if let Some(c) = comp.get(i).cloned().flatten() {
            let despues: Vec<[f32; 2]> = m
                .into_iter()
                .map(|v| {
                    let q = c.aplicar(v);
                    [q[0], -q[1]]
                })
                .collect();
            let nombre = modelo
                .deformadores
                .get(i)
                .map(|d| d.id.clone())
                .unwrap_or_default();
            pasos.push((format!("{nombre} (compuesto)"), despues));
        }
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
        assert!(r[0].abs() < 1e-6 && (r[1] - 1.0).abs() < 1e-6, "{r:?}");
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

    fn rejilla_identidad(filas: usize, columnas: usize) -> Vec<[f32; 2]> {
        let mut v = Vec::new();
        for j in 0..=filas {
            for i in 0..=columnas {
                v.push([i as f32 / columnas as f32, j as f32 / filas as f32]);
            }
        }
        v
    }

    #[test]
    fn el_warp_devuelve_la_rejilla_en_sus_esquinas_y_su_centro() {
        let rej = rejilla_identidad(2, 2);
        for (p, esperado) in [
            ([0.0, 0.0], [0.0, 0.0]),
            ([0.5, 0.5], [0.5, 0.5]),
            ([0.99, 0.99], [0.99, 0.99]),
            ([0.25, 0.75], [0.25, 0.75]),
        ] {
            let r = warp(&rej, 2, 2, p);
            assert!(
                (r[0] - esperado[0]).abs() < 1e-3 && (r[1] - esperado[1]).abs() < 1e-3,
                "{p:?} → {r:?}, esperaba {esperado:?}"
            );
        }
    }

    #[test]
    fn fuera_de_la_rejilla_se_extrapola_con_la_base_afin() {
        let rej = rejilla_identidad(2, 2);
        // la rejilla es la identidad, así que la extrapolación también lo es
        let r = warp(&rej, 2, 2, [-0.5, 1.5]);
        assert!(
            (r[0] + 0.5).abs() < 1e-3 && (r[1] - 1.5).abs() < 1e-3,
            "{r:?}"
        );
    }
}
