//! Dibujo de una pose **sin el Core**: `pose` calcula la geometría, `raster` la pinta.
//!
//! Es la pieza que faltaba para tener un visor independiente: hasta ahora la geometría exacta
//! (`docs/16`) y el rasterizador CPU existían por separado y nadie los había juntado.
//!
//! Convenciones, las mismas que el editor del navegador (verificadas contra el Core):
//! - `px = origen + posición · ppu`, con la posición en el espacio del Core (la `y` ya negada
//!   respecto del archivo por `pose::evaluar`).
//! - UV con `v` invertida (`v' = 1 − v`).
//! - Orden de dibujo: `part_draw_order` del keyform de la parte, con el `draw_order` de la malla
//!   como desempate, pintando **de menor a mayor** (mayor = más al frente, así que se pinta al
//!   final). Al revés el modelo sale de espaldas.

use std::path::{Path, PathBuf};

use moc3::{Body, Moc3};
use pose::{Bindings, Modelo, Seleccion};

/// Un parámetro del modelo, tal como lo ve una interfaz.
#[derive(Debug, Clone)]
pub struct Parametro {
    pub id: String,
    pub min: f32,
    pub max: f32,
    pub defecto: f32,
}

/// Modelo listo para dibujar.
pub struct Vista {
    pub modelo: Modelo,
    pub bnd: Bindings,
    pub body: Body,
    pub texturas: Vec<raster::Texture>,
    /// Tamaño del lienzo del modelo, en píxeles.
    pub lienzo: (f32, f32),
    pub origen: [f32; 2],
    pub ppu: f32,
    pub parametros: Vec<Parametro>,
    /// Ids de las partes (capas), en el orden del archivo.
    pub partes: Vec<String>,
    /// Nombre del modelo (el del archivo).
    pub nombre: String,
}

impl Vista {
    /// Carga un `.moc3` y sus texturas (`texture*.png` en la misma carpeta).
    pub fn cargar(ruta: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let ruta = ruta.as_ref();
        let bytes = std::fs::read(ruta)?;
        let moc = Moc3::parse(&bytes)?;
        let body = moc.body()?;
        let modelo = pose::cargar(&moc);
        let bnd = Bindings::cargar(&moc);

        // Las texturas las declara el `model3.json` del modelo (suelen vivir en una
        // subcarpeta, p. ej. `Hiyori.2048/texture_00.png`). Si no hay manifiesto, se buscan
        // `texture*.png` junto al `.moc3`.
        let mut texturas = Vec::new();
        if let Some(dir) = ruta.parent() {
            let manifiesto = std::fs::read_dir(dir)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .find(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.ends_with(".model3.json"))
                        .unwrap_or(false)
                });
            let mut rutas: Vec<PathBuf> = Vec::new();
            if let Some(m) = manifiesto {
                // Se lee sólo `FileReferences.Textures`. El lector tipado `model3` rechaza
                // este archivo (defecto anotado en NOW.md: «invalid type: map, expected a
                // string» en `Motions`), y aquí no hace falta el resto del manifiesto.
                if let Ok(texto) = std::fs::read_to_string(&m) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&texto) {
                        if let Some(lista) = json
                            .get("FileReferences")
                            .and_then(|f| f.get("Textures"))
                            .and_then(|v| v.as_array())
                        {
                            for rel in lista.iter().filter_map(|v| v.as_str()) {
                                rutas.push(dir.join(rel));
                            }
                        }
                    }
                }
            }
            if rutas.is_empty() {
                let mut sueltas: Vec<PathBuf> = std::fs::read_dir(dir)?
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with("texture") && n.ends_with(".png"))
                            .unwrap_or(false)
                    })
                    .collect();
                sueltas.sort();
                rutas = sueltas;
            }
            for r in rutas {
                if let Ok(t) = raster::Texture::from_png(&r) {
                    texturas.push(t);
                }
            }
        }

        let parametros = bnd
            .parametros
            .iter()
            .enumerate()
            .map(|(i, id)| Parametro {
                id: id.clone(),
                min: body.parameters.get(i).map(|p| p.min).unwrap_or(-1.0),
                max: body.parameters.get(i).map(|p| p.max).unwrap_or(1.0),
                defecto: bnd.defectos.get(i).copied().unwrap_or(0.0),
            })
            .collect();

        let nombre = ruta
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("modelo")
            .to_string();

        let partes = bnd_partes(&body);
        Ok(Self {
            modelo,
            bnd,
            body,
            texturas,
            lienzo: (moc.canvas.width, moc.canvas.height),
            origen: [moc.canvas.origin_x, moc.canvas.origin_y],
            ppu: if moc.canvas.pixels_per_unit > 0.0 {
                moc.canvas.pixels_per_unit
            } else {
                moc.canvas.width
            },
            parametros,
            partes,
            nombre,
        })
    }

    /// Qué capas se ocultan por defecto: las que el archivo marca invisibles y las variantes
    /// mutuamente excluyentes tipo `PartArmA` / `PartArmB` (se deja la A y se oculta la B; sin
    /// esta regla el modelo sale con cuatro brazos — es la misma que usa el editor, `docs/14`).
    pub fn ocultas_por_defecto(&self) -> Vec<bool> {
        let mut ocultas: Vec<bool> = self.body.part_visible.iter().map(|v| !*v).collect();
        ocultas.resize(self.partes.len(), false);
        for (i, id) in self.partes.iter().enumerate() {
            if let Some(base) = id.strip_suffix('A') {
                if let Some(j) = self.partes.iter().position(|x| x == &format!("{base}B")) {
                    if j != i {
                        ocultas[j] = true;
                    }
                }
            }
        }
        ocultas
    }

    /// Valores por defecto de todos los parámetros.
    pub fn valores_iniciales(&self) -> Vec<f32> {
        self.parametros.iter().map(|p| p.defecto).collect()
    }

    /// Dibuja la pose con estos valores de parámetro, a `ancho_px` de ancho.
    ///
    /// `ocultas` es una casilla por parte (`true` = capa oculta). Si va vacía se usa
    /// [`Vista::ocultas_por_defecto`].
    pub fn render(&self, valores: &[f32], ancho_px: u32, ocultas: &[bool]) -> raster::Canvas {
        let (cw, ch) = self.lienzo;
        let k = if cw > 0.0 { ancho_px as f32 / cw } else { 1.0 };
        let alto = (ch * k).round().max(1.0) as u32;
        let ancho = ancho_px.max(1);

        let sel = Seleccion::con_valores(&self.modelo, &self.bnd, valores);
        let comp = pose::componer(&self.modelo, &sel);

        // Convención verificada mirando el render (docs/17 §3): la `y` del Core va al revés
        // respecto del lienzo del archivo, y la `v` del atlas se muestrea **tal cual**. Las dos
        // van en pareja: invertir una sola deja el personaje boca abajo o despiezado.
        let alto_total = ch * k;
        let to_screen = move |p: [f32; 2]| -> [f32; 2] {
            [
                (self.origen[0] + p[0] * self.ppu) * k,
                alto_total - (self.origen[1] + p[1] * self.ppu) * k,
            ]
        };

        // Variantes mutuamente excluyentes: los modelos suelen traer dos juegos completos de
        // brazo (`PartArmA` / `PartArmB`) y sólo uno debe verse; el otro se oculta por defecto
        // (es la misma regla que el editor, docs/14; sin ella aparecen cuatro brazos).
        let por_defecto;
        let partes_ocultas: &[bool] = if ocultas.is_empty() {
            por_defecto = self.ocultas_por_defecto();
            &por_defecto
        } else {
            ocultas
        };

        // qué mallas son fuente de máscara: no se dibujan como arte
        let n = self.body.art_meshes.len();
        let mut es_mascara = vec![false; n];
        for mesh in &self.body.art_meshes {
            let mb = mesh.mask_begin as usize;
            let mc = mesh.mask_count as usize;
            for m in mb..(mb + mc).min(self.body.drawable_masks.len()) {
                let idx = self.body.drawable_masks[m];
                if idx >= 0 && (idx as usize) < n {
                    es_mascara[idx as usize] = true;
                }
            }
        }

        struct Listo {
            verts: Vec<([f32; 2], [f32; 2])>,
            indices: Vec<i16>,
            textura: usize,
            orden: f32,
            mascaras: Vec<usize>,
            opacidad: f32,
        }

        let mut listos: Vec<Option<Listo>> = (0..n).map(|_| None).collect();
        for (i, mesh) in self.body.art_meshes.iter().enumerate() {
            // OJO: las mallas que son fuente de máscara **sí** necesitan geometría (se pintan en
            // el canvas de la máscara); sólo se excluyen del dibujo final.
            if !mesh.visible {
                continue;
            }
            if mesh.parent_part >= 0
                && partes_ocultas
                    .get(mesh.parent_part as usize)
                    .copied()
                    .unwrap_or(false)
            {
                continue;
            }
            let Some(geometria) = pose::evaluar(&self.modelo, &comp, &sel, i) else {
                continue;
            };
            let n_idx = mesh.vertex_count as usize;
            let ib = mesh.index_begin as usize;
            if n_idx == 0 || ib + n_idx > self.body.indices.len() {
                continue;
            }
            let indices = self.body.indices[ib..ib + n_idx].to_vec();
            let uv_begin = mesh.uv_begin as usize;
            let mut verts = Vec::with_capacity(geometria.len());
            let mut sano = true;
            for (v, p) in geometria.iter().enumerate() {
                if uv_begin + 2 * v + 1 >= self.body.uvs.len() {
                    sano = false;
                    break;
                }
                if !p[0].is_finite() || !p[1].is_finite() || p[0].abs() > 1e4 || p[1].abs() > 1e4 {
                    sano = false;
                    break;
                }
                // UV con v invertida: es la convención del formato (origen arriba-izquierda)
                verts.push((
                    *p,
                    [
                        self.body.uvs[uv_begin + 2 * v],
                        self.body.uvs[uv_begin + 2 * v + 1],
                    ],
                ));
            }
            if !sano || verts.is_empty() {
                continue;
            }

            let orden_parte = if mesh.parent_part >= 0 {
                self.body
                    .part_keyform_begin
                    .get(mesh.parent_part as usize)
                    .and_then(|k| self.body.part_draw_order.get(*k as usize))
                    .copied()
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            let orden_malla = sel
                .malla
                .get(i)
                .and_then(|s| s.iter().max_by(|a, b| a.1.total_cmp(&b.1)))
                .and_then(|(kf, _)| self.body.art_mesh_keyform_draw_orders.get(*kf))
                .copied()
                .unwrap_or(0.0);

            let mb = mesh.mask_begin as usize;
            let mc = mesh.mask_count as usize;
            let mascaras = if mc > 0 && mb + mc <= self.body.drawable_masks.len() {
                self.body.drawable_masks[mb..mb + mc]
                    .iter()
                    .map(|v| (*v).max(0) as usize)
                    .filter(|v| *v < n)
                    .collect()
            } else {
                Vec::new()
            };

            let opacidad = sel
                .malla
                .get(i)
                .map(|slots| {
                    slots
                        .iter()
                        .filter_map(|(kf, w)| {
                            self.body.art_mesh_keyform_opacities.get(*kf).map(|o| o * w)
                        })
                        .sum::<f32>()
                })
                .unwrap_or(1.0);

            listos[i] = Some(Listo {
                verts,
                indices,
                textura: mesh.texture_index as usize,
                orden: orden_parte + orden_malla / 10000.0,
                mascaras,
                opacidad: opacidad.clamp(0.0, 1.0),
            });
        }

        let mut canvas = raster::Canvas::new(ancho, alto);
        canvas.clear([0, 0, 0, 0]);

        let pintar = |canvas: &mut raster::Canvas, r: &Listo, mascara: Option<&raster::Canvas>| {
            let tex = self
                .texturas
                .get(r.textura)
                .or_else(|| self.texturas.first());
            for tri in r.indices.chunks_exact(3) {
                let (a, b, c) = (
                    tri[0].max(0) as usize,
                    tri[1].max(0) as usize,
                    tri[2].max(0) as usize,
                );
                if a >= r.verts.len() || b >= r.verts.len() || c >= r.verts.len() {
                    continue;
                }
                let mk = |k: usize| {
                    let (p, uv) = r.verts[k];
                    let sp = to_screen(p);
                    raster::Vertex::new(sp[0], sp[1], uv[0], uv[1])
                        .with_color(1.0, 1.0, 1.0, r.opacidad)
                };
                canvas.triangle_masked([mk(a), mk(b), mk(c)], tex, raster::Blend::Normal, mascara);
            }
        };

        let mut orden: Vec<usize> = (0..n).filter(|i| listos[*i].is_some()).collect();
        orden.sort_by(|a, b| {
            let (oa, ob) = (
                listos[*a].as_ref().map(|x| x.orden).unwrap_or(0.0),
                listos[*b].as_ref().map(|x| x.orden).unwrap_or(0.0),
            );
            oa.partial_cmp(&ob).unwrap_or(std::cmp::Ordering::Equal)
        });

        for i in orden {
            if es_mascara[i] {
                continue; // geometría auxiliar: se usa para recortar, no se dibuja
            }
            let Some(r) = listos[i].as_ref() else {
                continue;
            };
            let mascara = if r.mascaras.is_empty() {
                None
            } else {
                let mut mc = raster::Canvas::new(ancho, alto);
                mc.clear([0, 0, 0, 0]);
                for mi in &r.mascaras {
                    // el canvas de máscara usa la textura para el recorte por alfa
                    let Some(mr) = listos[*mi].as_ref() else {
                        continue;
                    };
                    let tex = self
                        .texturas
                        .get(mr.textura)
                        .or_else(|| self.texturas.first());
                    for tri in mr.indices.chunks_exact(3) {
                        let (a, b, c) = (
                            tri[0].max(0) as usize,
                            tri[1].max(0) as usize,
                            tri[2].max(0) as usize,
                        );
                        if a >= mr.verts.len() || b >= mr.verts.len() || c >= mr.verts.len() {
                            continue;
                        }
                        let mk = |k: usize| {
                            let (p, uv) = mr.verts[k];
                            let sp = to_screen(p);
                            raster::Vertex::new(sp[0], sp[1], uv[0], uv[1])
                                .with_color(1.0, 1.0, 1.0, 1.0)
                        };
                        mc.triangle_masked([mk(a), mk(b), mk(c)], tex, raster::Blend::Normal, None);
                    }
                }
                Some(mc)
            };
            pintar(&mut canvas, r, mascara.as_ref());
        }

        canvas
    }
}

/// Ids de las partes del modelo (capas), para listarlas en una interfaz.
fn bnd_partes(body: &Body) -> Vec<String> {
    body.part_ids.clone()
}
