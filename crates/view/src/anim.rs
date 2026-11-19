//! Animación `l2d-anim/1`: lectura, validación e interpolación (sin Core).
//!
//! El formato vive en `spec/01-formato-animacion.md`. Una clave guarda **todos** los parámetros
//! que anima el archivo en un instante; aquí se mapean por nombre a los índices del modelo al
//! cargar, así que evaluar no busca cadenas en cada cuadro.
//!
//! Decisiones sobre el spec (documentadas en `docs/18-animacion.md`):
//!
//! 1. **Un parámetro del archivo que el modelo no tiene es error**, no un aviso: casi siempre es
//!    señal de que la animación es de otro modelo.
//! 2. **Un parámetro del modelo que el archivo no anima queda como esté** (en reposo, si el
//!    llamador no lo tocó). El editor exporta la lista completa, pero una animación de tres
//!    parámetros es legítima y útil para probar.
//! 3. **Fuera del rango de claves no se extrapola**: se mantiene la clave extrema.

use std::path::Path;

/// Identificador del formato, tal como aparece en el JSON.
pub const FORMATO: &str = "l2d-anim/1";

/// Un instante de la animación: los valores de los parámetros animados.
#[derive(Debug, Clone, PartialEq)]
pub struct Clave {
    pub t: f32,
    pub valores: Vec<f32>,
}

/// Una animación cargada y atada a los parámetros de un modelo concreto.
#[derive(Debug, Clone)]
pub struct Animacion {
    /// Nombre del archivo (`modelo` en el JSON, si viene).
    pub nombre: String,
    pub duracion: f32,
    pub parametros: Vec<String>,
    pub claves: Vec<Clave>,
    /// Índice en el modelo de cada entrada de `parametros`.
    indices: Vec<usize>,
}

impl Animacion {
    /// Carga desde disco. El mensaje de error ya nombra el archivo.
    pub fn cargar(ruta: impl AsRef<Path>, parametros_modelo: &[String]) -> Result<Self, String> {
        let ruta = ruta.as_ref();
        let texto = std::fs::read_to_string(ruta)
            .map_err(|e| format!("no pude leer «{}»: {e}", ruta.display()))?;
        let mut anim = Self::desde_json(&texto, parametros_modelo)
            .map_err(|e| format!("«{}»: {e}", ruta.display()))?;
        if anim.nombre.is_empty() {
            anim.nombre = ruta
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("animación")
                .to_string();
        }
        Ok(anim)
    }

    /// Valida y construye. Rechaza el archivo completo antes de devolver nada a medias.
    pub fn desde_json(texto: &str, parametros_modelo: &[String]) -> Result<Self, String> {
        let json: serde_json::Value =
            serde_json::from_str(texto).map_err(|e| format!("JSON inválido: {e}"))?;

        let formato = json
            .get("formato")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if formato != FORMATO {
            return Err(format!("formato «{formato}», se esperaba «{FORMATO}»"));
        }

        let duracion = json
            .get("duracion")
            .and_then(|v| v.as_f64())
            .ok_or("falta «duracion»")? as f32;
        if !(duracion > 0.0) || !duracion.is_finite() {
            return Err(format!("duracion inválida: {duracion}"));
        }

        let lista = json
            .get("parametros")
            .and_then(|v| v.as_array())
            .ok_or("falta la lista «parametros»")?;
        let mut parametros = Vec::with_capacity(lista.len());
        for (i, v) in lista.iter().enumerate() {
            let id = v
                .as_str()
                .ok_or_else(|| format!("parametros[{i}] no es texto"))?;
            if parametros.iter().any(|p: &String| p == id) {
                return Err(format!("parámetro repetido: «{id}»"));
            }
            parametros.push(id.to_string());
        }
        if parametros.is_empty() {
            return Err("la lista «parametros» está vacía".to_string());
        }

        // nombre -> índice del modelo; un nombre desconocido es un archivo de otro modelo.
        let mut indices = Vec::with_capacity(parametros.len());
        for id in &parametros {
            let Some(j) = parametros_modelo.iter().position(|p| p == id) else {
                return Err(format!(
                    "el modelo no tiene el parámetro «{id}» (¿animación de otro modelo?)"
                ));
            };
            indices.push(j);
        }

        let crudas = json
            .get("claves")
            .and_then(|v| v.as_array())
            .ok_or("falta la lista «claves»")?;
        if crudas.is_empty() {
            return Err("la animación no tiene claves".to_string());
        }
        let mut claves = Vec::with_capacity(crudas.len());
        for (i, c) in crudas.iter().enumerate() {
            let t = c
                .get("t")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| format!("claves[{i}] sin «t»"))? as f32;
            if !t.is_finite() || t < 0.0 {
                return Err(format!("claves[{i}]: t inválido ({t})"));
            }
            let valores = c
                .get("valores")
                .and_then(|v| v.as_array())
                .ok_or_else(|| format!("claves[{i}] sin «valores»"))?;
            if valores.len() != parametros.len() {
                return Err(format!(
                    "claves[{i}] tiene {} valores y hay {} parámetros",
                    valores.len(),
                    parametros.len()
                ));
            }
            let mut v = Vec::with_capacity(valores.len());
            for (k, x) in valores.iter().enumerate() {
                let f = x
                    .as_f64()
                    .ok_or_else(|| format!("claves[{i}].valores[{k}] no es número"))?
                    as f32;
                if !f.is_finite() {
                    return Err(format!("claves[{i}].valores[{k}] no es finito"));
                }
                v.push(f);
            }
            if let Some(prev) = claves.last() as Option<&Clave> {
                if t <= prev.t {
                    return Err(format!(
                        "claves desordenadas: t={t} después de t={} (deben ser crecientes)",
                        prev.t
                    ));
                }
            }
            claves.push(Clave { t, valores: v });
        }

        let nombre = json
            .get("modelo")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        Ok(Self {
            nombre,
            duracion,
            parametros,
            claves,
            indices,
        })
    }

    /// Valores de los parámetros animados en el instante `t` (mismo orden que `parametros`).
    ///
    /// Interpolación lineal entre las dos claves que rodean `t`; fuera del rango, la clave extrema.
    pub fn evaluar(&self, t: f32) -> Vec<f32> {
        let n = self.claves.len();
        debug_assert!(n > 0);
        let primera = &self.claves[0];
        let ultima = &self.claves[n - 1];
        if !(t > primera.t) {
            return primera.valores.clone();
        }
        if t >= ultima.t {
            return ultima.valores.clone();
        }
        // Búsqueda lineal: las animaciones tienen decenas de claves, no miles.
        let mut i = 0;
        while i + 1 < n && self.claves[i + 1].t <= t {
            i += 1;
        }
        let (a, b) = (&self.claves[i], &self.claves[i + 1]);
        let tramo = b.t - a.t;
        if tramo <= 0.0 {
            return a.valores.clone();
        }
        let u = (t - a.t) / tramo;
        a.valores
            .iter()
            .zip(&b.valores)
            .map(|(va, vb)| va + (vb - va) * u)
            .collect()
    }

    /// Escribe en `destino` (el vector completo de parámetros del modelo) los valores de `t`.
    /// Los parámetros que la animación no toca se quedan como estaban.
    pub fn aplicar(&self, t: f32, destino: &mut [f32]) {
        for (v, &j) in self.evaluar(t).iter().zip(&self.indices) {
            if let Some(d) = destino.get_mut(j) {
                *d = *v;
            }
        }
    }

    /// `t` dentro de `[0, duracion)`, para reproducir en bucle.
    pub fn envolver(&self, t: f32) -> f32 {
        if self.duracion <= 0.0 {
            return 0.0;
        }
        let mut t = t % self.duracion;
        if t < 0.0 {
            t += self.duracion;
        }
        t
    }

    /// Cuántos parámetros anima y en cuántas claves, para el pie de la ventana.
    pub fn resumen(&self) -> String {
        format!(
            "{} claves · {} parámetros · {:.1} s",
            self.claves.len(),
            self.parametros.len(),
            self.duracion
        )
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn modelo() -> Vec<String> {
        [
            "ParamAngleX",
            "ParamAngleY",
            "ParamEyeLOpen",
            "ParamMouthOpenY",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn json(claves: &str, parametros: &str, duracion: f32) -> String {
        format!(
            r#"{{"formato":"l2d-anim/1","modelo":"Prueba","duracion":{duracion},
                "parametros":{parametros},"claves":{claves}}}"#
        )
    }

    const DOS_PARAMS: &str = r#"["ParamAngleX","ParamAngleY"]"#;
    const DOS_CLAVES: &str = r#"[{"t":0.0,"valores":[0,10]},{"t":2.0,"valores":[20,-10]}]"#;

    #[test]
    fn interpola_lineal_entre_dos_claves() {
        let a = Animacion::desde_json(&json(DOS_CLAVES, DOS_PARAMS, 4.0), &modelo()).unwrap();
        assert_eq!(a.evaluar(0.0), vec![0.0, 10.0]);
        assert_eq!(a.evaluar(1.0), vec![10.0, 0.0]);
        assert_eq!(a.evaluar(2.0), vec![20.0, -10.0]);
    }

    #[test]
    fn fuera_de_rango_mantiene_la_clave_extrema() {
        let a = Animacion::desde_json(&json(DOS_CLAVES, DOS_PARAMS, 4.0), &modelo()).unwrap();
        // sin extrapolar: antes de la primera y después de la última
        assert_eq!(a.evaluar(-5.0), vec![0.0, 10.0]);
        assert_eq!(a.evaluar(99.0), vec![20.0, -10.0]);
        // la cola quieta de `duracion` también repite la última
        assert_eq!(a.evaluar(3.5), vec![20.0, -10.0]);
    }

    #[test]
    fn aplicar_respeta_indices_y_no_toca_lo_demas() {
        let a = Animacion::desde_json(&json(DOS_CLAVES, DOS_PARAMS, 2.0), &modelo()).unwrap();
        // destino en reposo con centinelas distintos de cero
        let mut vals = vec![99.0, 99.0, 0.7, -0.3];
        a.aplicar(1.0, &mut vals);
        assert_eq!(vals, vec![10.0, 0.0, 0.7, -0.3]);
    }

    #[test]
    fn anima_un_solo_parametro_del_medio() {
        let uno = r#"[{"t":0,"valores":[0]},{"t":1,"valores":[1]}]"#;
        let a = Animacion::desde_json(&json(uno, r#"["ParamEyeLOpen"]"#, 1.0), &modelo()).unwrap();
        let mut vals = vec![0.0, 0.0, 0.0, 0.0];
        a.aplicar(0.5, &mut vals);
        assert_eq!(vals, vec![0.0, 0.0, 0.5, 0.0]);
    }

    #[test]
    fn rechaza_formato_ajeno() {
        let malo = json(DOS_CLAVES, DOS_PARAMS, 2.0).replace("l2d-anim/1", "l2d-anim/2");
        let e = Animacion::desde_json(&malo, &modelo()).unwrap_err();
        assert!(e.contains("formato"), "{e}");
    }

    #[test]
    fn rechaza_parametro_que_el_modelo_no_tiene() {
        let e = Animacion::desde_json(&json(DOS_CLAVES, r#"["ParamInexistente"]"#, 2.0), &modelo())
            .unwrap_err();
        assert!(e.contains("ParamInexistente"), "{e}");
    }

    #[test]
    fn rechaza_parametro_repetido() {
        let e = Animacion::desde_json(
            &json(DOS_CLAVES, r#"["ParamAngleX","ParamAngleX"]"#, 2.0),
            &modelo(),
        )
        .unwrap_err();
        assert!(e.contains("repetido"), "{e}");
    }

    #[test]
    fn rechaza_claves_desordenadas_y_t_repetido() {
        let mal = r#"[{"t":2,"valores":[0,0]},{"t":1,"valores":[1,1]}]"#;
        assert!(Animacion::desde_json(&json(mal, DOS_PARAMS, 3.0), &modelo()).is_err());
        let rep = r#"[{"t":1,"valores":[0,0]},{"t":1.0,"valores":[1,1]}]"#;
        assert!(Animacion::desde_json(&json(rep, DOS_PARAMS, 3.0), &modelo()).is_err());
    }

    #[test]
    fn rechaza_longitud_de_valores_que_no_cuadra() {
        let mal = r#"[{"t":0,"valores":[0,0,0]},{"t":1,"valores":[1,1]}]"#;
        let e = Animacion::desde_json(&json(mal, DOS_PARAMS, 2.0), &modelo()).unwrap_err();
        assert!(e.contains("valores"), "{e}");
    }

    #[test]
    fn rechaza_sin_claves_y_duracion_cero() {
        assert!(Animacion::desde_json(&json("[]", DOS_PARAMS, 2.0), &modelo()).is_err());
        assert!(Animacion::desde_json(&json(DOS_CLAVES, DOS_PARAMS, 0.0), &modelo()).is_err());
    }

    #[test]
    fn envolver_hace_bucle_en_ambos_sentidos() {
        let a = Animacion::desde_json(&json(DOS_CLAVES, DOS_PARAMS, 4.0), &modelo()).unwrap();
        assert!((a.envolver(0.5) - 0.5).abs() < 1e-6);
        assert!((a.envolver(4.5) - 0.5).abs() < 1e-6);
        assert!((a.envolver(9.0) - 1.0).abs() < 1e-6);
        assert!((a.envolver(-0.5) - 3.5).abs() < 1e-6);
    }

    /// El archivo que exporta el editor real: todos los parámetros del modelo.
    #[test]
    fn acepta_la_lista_completa_del_modelo() {
        let todos = r#"["ParamAngleX","ParamAngleY","ParamEyeLOpen","ParamMouthOpenY"]"#;
        let claves = r#"[{"t":0,"valores":[0,0,1,0]},{"t":1,"valores":[30,0,0,1]}]"#;
        let a = Animacion::desde_json(&json(claves, todos, 1.0), &modelo()).unwrap();
        assert_eq!(a.parametros.len(), 4);
        assert_eq!(a.evaluar(0.5), vec![15.0, 0.0, 0.5, 0.5]);
    }
}
