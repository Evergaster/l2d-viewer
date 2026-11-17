//! `l2d-model/1` — la capa de cambios del editor sobre un `.moc3`.
//!
//! El `.moc3` **no se reescribe** (escribirlo está pospuesto, `docs/09`). Este documento guarda
//! solo lo que el usuario cambió: parámetros fuera de reposo, visibilidad de partes y
//! desplazamientos de vértices. Especificado en `spec/02-formato-modelo.md`.
//!
//! El documento se valida **contra un `.moc3` concreto**: la huella del origen ata los índices de
//! vértice a ese archivo, porque `ArtMesh23` de un modelo no tiene nada que ver con el de otro.

use moc3::Body;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Identificador del formato.
pub const FORMATO: &str = "l2d-model/1";

/// Offset y primo de FNV-1a de 64 bits.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIMO: u64 = 0x0000_0100_0000_01b3;

/// Error del documento. El texto dice el motivo concreto y los valores en conflicto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

fn fallo<T>(msg: impl Into<String>) -> Result<T, Error> {
    Err(Error(msg.into()))
}

/// FNV-1a de 64 bits sobre los bytes, en hexadecimal minúsculas de 16 cifras.
///
/// Es el mismo algoritmo que usa el editor en JavaScript; `huella(b"")` da
/// `cbf29ce484222325`. No es criptográfico: sirve para distinguir dos modelos, no para firmarlos.
pub fn huella(bytes: &[u8]) -> String {
    let mut h = FNV_OFFSET;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIMO);
    }
    format!("{h:016x}")
}

/// Archivo del que salió el modelo editado.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Origen {
    /// Nombre del archivo, solo informativo.
    pub archivo: String,
    /// Tamaño en bytes del `.moc3`.
    pub bytes: u64,
    /// [`huella`] del `.moc3` completo.
    pub huella: String,
}

/// Un vértice movido, en unidades del modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Desplazamiento {
    /// Id del drawable (`ArtMesh23`), no su índice: el índice cambia entre versiones.
    pub malla: String,
    /// Índice 0-based del vértice dentro de esa malla.
    pub vertice: u32,
    pub dx: f32,
    pub dy: f32,
}

/// Documento `l2d-model/1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Documento {
    pub formato: String,
    pub modelo: String,
    pub origen: Origen,
    /// Parámetros que **no** están en su valor por defecto.
    pub parametros: BTreeMap<String, f32>,
    /// Partes cuya visibilidad cambia respecto a la de arranque. `false` = oculta.
    pub partes: BTreeMap<String, bool>,
    /// Desplazamientos de vértices sueltos.
    pub vertices: Vec<Desplazamiento>,
}

impl Documento {
    /// Nuevo documento vacío, con el formato ya puesto.
    pub fn nuevo(modelo: impl Into<String>) -> Self {
        Self {
            formato: FORMATO.to_string(),
            modelo: modelo.into(),
            ..Default::default()
        }
    }

    /// Lee un `l2d-model/1` desde texto.
    pub fn desde_json(texto: &str) -> Result<Self, Error> {
        let doc: Self =
            serde_json::from_str(texto).map_err(|e| Error(format!("json inválido ({e})")))?;
        doc.exigir_formato()?;
        doc.exigir_origen_declarado()?;
        Ok(doc)
    }

    /// Serializa el documento con sangría, redondeando los flotantes a 6 decimales.
    pub fn a_json(&self) -> String {
        let mut copia = self.clone();
        for v in &mut copia.parametros {
            *v.1 = redondear(*v.1);
        }
        for d in &mut copia.vertices {
            d.dx = redondear(d.dx);
            d.dy = redondear(d.dy);
        }
        serde_json::to_string_pretty(&copia).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }

    fn exigir_formato(&self) -> Result<(), Error> {
        if self.formato != FORMATO {
            return fallo(format!("formato «{}»: esperaba {FORMATO}", self.formato));
        }
        Ok(())
    }

    fn exigir_origen_declarado(&self) -> Result<(), Error> {
        if self.origen.huella.is_empty() {
            return fallo("el archivo no declara el .moc3 de origen (origen.huella vacía)");
        }
        Ok(())
    }

    /// Comprueba que el documento se hizo sobre estos bytes de `.moc3`.
    pub fn verificar_origen(&self, bytes: &[u8]) -> Result<(), Error> {
        self.exigir_origen_declarado()?;
        let n = bytes.len() as u64;
        if self.origen.bytes != n {
            return fallo(format!(
                "el archivo declara {} bytes y el .moc3 tiene {n}",
                self.origen.bytes
            ));
        }
        let h = huella(bytes);
        if !self.origen.huella.eq_ignore_ascii_case(&h) {
            return fallo(format!(
                "huella del .moc3 «{h}», el archivo declara «{}»: es otro modelo",
                self.origen.huella
            ));
        }
        Ok(())
    }

    /// Comprueba que cada nombre y cada índice existen en el modelo.
    pub fn validar(&self, body: &Body) -> Result<(), Error> {
        let mut faltan: Vec<&str> = self
            .parametros
            .keys()
            .filter(|id| !body.parameters.iter().any(|p| &p.id == *id))
            .map(String::as_str)
            .collect();
        if !faltan.is_empty() {
            faltan.sort_unstable();
            return fallo(format!(
                "parámetros que el modelo no tiene: {}",
                lista(&faltan)
            ));
        }

        let mut faltan: Vec<&str> = self
            .partes
            .keys()
            .filter(|id| !body.part_ids.iter().any(|p| p == *id))
            .map(String::as_str)
            .collect();
        if !faltan.is_empty() {
            faltan.sort_unstable();
            return fallo(format!("partes que el modelo no tiene: {}", lista(&faltan)));
        }

        for d in &self.vertices {
            let malla = match body.art_meshes.iter().find(|m| m.id == d.malla) {
                Some(m) => m,
                None => return fallo(format!("malla «{}» no existe en el modelo", d.malla)),
            };
            if d.vertice >= malla.vertex_count {
                return fallo(format!(
                    "«{}» tiene {} vértices y el archivo mueve el {}",
                    d.malla, malla.vertex_count, d.vertice
                ));
            }
            if !d.dx.is_finite() || !d.dy.is_finite() {
                return fallo(format!(
                    "«{}» vértice {}: desplazamiento no finito",
                    d.malla, d.vertice
                ));
            }
        }
        Ok(())
    }

    /// Aplica los desplazamientos sobre las posiciones de cada malla (pares x,y planos).
    ///
    /// `posiciones[m]` corresponde a `body.art_meshes[m]`; el orden es el del archivo.
    /// Devuelve cuántos vértices se movieron.
    pub fn aplicar(&self, body: &Body, posiciones: &mut [Vec<f32>]) -> Result<usize, Error> {
        self.validar(body)?;
        if posiciones.len() != body.art_meshes.len() {
            return fallo(format!(
                "hay {} mallas de geometría y {} en el modelo",
                posiciones.len(),
                body.art_meshes.len()
            ));
        }
        let mut movidos = 0;
        for d in &self.vertices {
            let m = body
                .art_meshes
                .iter()
                .position(|m| m.id == d.malla)
                .expect("validar() ya comprobó la malla");
            let v = d.vertice as usize;
            posiciones[m][v * 2] += d.dx;
            posiciones[m][v * 2 + 1] += d.dy;
            movidos += 1;
        }
        Ok(movidos)
    }

    /// Cuántos desplazamientos hay por malla, ordenado por id.
    pub fn por_malla(&self) -> BTreeMap<&str, usize> {
        let mut m: BTreeMap<&str, usize> = BTreeMap::new();
        for d in &self.vertices {
            *m.entry(d.malla.as_str()).or_insert(0) += 1;
        }
        m
    }

    /// `true` si no cambia nada respecto al modelo base.
    pub fn esta_vacio(&self) -> bool {
        self.parametros.is_empty() && self.partes.is_empty() && self.vertices.is_empty()
    }
}

fn redondear(x: f32) -> f32 {
    (x * 1e6).round() / 1e6
}

fn lista(ids: &[&str]) -> String {
    const MAX: usize = 5;
    let mut s = ids
        .iter()
        .take(MAX)
        .map(|x| format!("«{x}»"))
        .collect::<Vec<_>>()
        .join(", ");
    if ids.len() > MAX {
        s.push_str(&format!(" y {} más", ids.len() - MAX));
    }
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use moc3::body::ArtMesh;

    /// FNV-1a 64 de vectores conocidos: ancla el algoritmo contra la implementación de JS.
    #[test]
    fn la_huella_coincide_con_los_vectores_publicados() {
        assert_eq!(huella(b""), "cbf29ce484222325");
        assert_eq!(huella(b"a"), "af63dc4c8601ec8c");
        assert_eq!(huella(b"foobar"), "85944171f73967e8");
    }

    fn cuerpo() -> Body {
        let malla = |id: &str, vertices: u32| ArtMesh {
            id: id.to_string(),
            texture_index: 0,
            vertex_count: vertices,
            uv_begin: 0,
            index_begin: 0,
            index_count: 0,
            keyform_begin: 0,
            keyform_count: 1,
            parent_part: 0,
            parent_deformer: -1,
            visible: true,
            mask_begin: 0,
            mask_count: 0,
        };
        Body {
            part_ids: vec!["PartArmA".into(), "PartArmB".into()],
            art_meshes: vec![malla("ArtMesh1", 4), malla("ArtMesh2", 3)],
            parameters: vec![
                moc3::body::Parameter {
                    id: "ParamAngleX".into(),
                    min: -30.0,
                    max: 30.0,
                    default: 0.0,
                },
                moc3::body::Parameter {
                    id: "ParamEyeLOpen".into(),
                    min: 0.0,
                    max: 1.0,
                    default: 1.0,
                },
            ],
            ..Default::default()
        }
    }

    fn texto_base() -> String {
        let mut d = Documento::nuevo("Hiyori");
        d.origen = Origen {
            archivo: "Hiyori.moc3".into(),
            bytes: 307123,
            huella: huella(b"").to_string(),
        };
        d.parametros.insert("ParamAngleX".into(), 12.5);
        d.partes.insert("PartArmB".into(), false);
        d.vertices.push(Desplazamiento {
            malla: "ArtMesh1".into(),
            vertice: 2,
            dx: 0.25,
            dy: -0.5,
        });
        d.a_json()
    }

    #[test]
    fn ida_y_vuelta_sin_perdida() {
        let texto = texto_base();
        let a = Documento::desde_json(&texto).expect("debe leer");
        let b = Documento::desde_json(&a.a_json()).expect("debe releer");
        assert_eq!(a, b);
        assert_eq!(a.parametros["ParamAngleX"], 12.5);
        assert_eq!(a.partes["PartArmB"], false);
        assert_eq!(a.vertices[0].dx, 0.25);
    }

    #[test]
    fn rechaza_otro_formato() {
        let texto = texto_base().replace("l2d-model/1", "l2d-model/2");
        let e = Documento::desde_json(&texto).unwrap_err();
        assert!(e.to_string().contains("l2d-model/2"), "{e}");
    }

    #[test]
    fn rechaza_sin_origen() {
        let texto = texto_base().replace(&huella(b""), "");
        let e = Documento::desde_json(&texto).unwrap_err();
        assert!(e.to_string().contains("origen"), "{e}");
    }

    #[test]
    fn el_origen_ata_el_documento_a_su_moc3() {
        let doc = Documento::desde_json(&texto_base()).unwrap();
        // otro tamaño: se detecta antes incluso de hashear
        let e = doc.verificar_origen(b"otro modelo").unwrap_err();
        assert!(e.to_string().contains("bytes"), "{e}");
        // con el tamaño correcto pero otro contenido, la huella delata
        let mut falsos = vec![0u8; 307123];
        falsos[10] = 7;
        let e = doc.verificar_origen(&falsos).unwrap_err();
        assert!(e.to_string().contains("huella"), "{e}");
    }

    #[test]
    fn valida_contra_el_cuerpo() {
        let body = cuerpo();
        let mut doc = Documento::desde_json(&texto_base()).unwrap();
        doc.validar(&body).expect("el documento base es válido");

        let mut malo = doc.clone();
        malo.vertices[0].malla = "ArtMesh99".into();
        assert!(malo
            .validar(&body)
            .unwrap_err()
            .to_string()
            .contains("ArtMesh99"));

        let mut malo = doc.clone();
        malo.vertices[0].vertice = 4;
        let e = malo.validar(&body).unwrap_err().to_string();
        assert!(e.contains("4 vértices"), "{e}");

        doc.parametros.insert("ParamInexistente".into(), 1.0);
        assert!(doc
            .validar(&body)
            .unwrap_err()
            .to_string()
            .contains("ParamInexistente"));

        doc.parametros.remove("ParamInexistente");
        doc.partes.insert("PartCola".into(), false);
        assert!(doc
            .validar(&body)
            .unwrap_err()
            .to_string()
            .contains("PartCola"));
    }

    #[test]
    fn aplica_solo_los_vertices_indicados() {
        let body = cuerpo();
        let doc = Documento::desde_json(&texto_base()).unwrap();
        let mut posiciones = vec![vec![0.0f32; 4 * 2], vec![1.0f32; 3 * 2]];
        let n = doc.aplicar(&body, &mut posiciones).unwrap();
        assert_eq!(n, 1);
        assert_eq!(&posiciones[0][4..6], &[0.25, -0.5]);
        assert_eq!(&posiciones[0][0..4], &[0.0, 0.0, 0.0, 0.0]);
        // la otra malla no se toca
        assert!(posiciones[1].iter().all(|x| *x == 1.0));
    }

    #[test]
    fn un_documento_sin_cambios_lo_dice() {
        let doc = Documento::nuevo("Hiyori");
        assert!(doc.esta_vacio());
        assert!(!Documento::desde_json(&texto_base()).unwrap().esta_vacio());
    }
}
