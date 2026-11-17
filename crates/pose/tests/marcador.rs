//! Marcador del motor propio contra el volcado del Core.
//!
//! No es una prueba de «todo bien»: es un **suelo**. Hoy el motor acierta las cadenas
//! cortas y falla las que pasan una rotación a un warp (`docs/13` §5.21), y estos números
//! son los que hay. Si alguien arregla el dominio del warp, la prueba sigue pasando; si
//! alguien lo rompe, falla.
//!
//! Los modelos no se versionan (licencia de Live2D): si no están, las pruebas se saltan.

use std::collections::HashMap;

use pose::{Bindings, Modelo, Seleccion};

fn cargar(
    ruta_moc: &str,
    ruta_oraculo: &str,
) -> Option<(Modelo, Bindings, HashMap<String, Vec<[f32; 2]>>)> {
    let bytes = std::fs::read(ruta_moc).ok()?;
    let moc = moc3::Moc3::parse(&bytes).ok()?;
    let modelo = pose::cargar(&moc);
    let bnd = Bindings::cargar(&moc);

    let texto = std::fs::read_to_string(ruta_oraculo).ok()?;
    let json: serde_json::Value = serde_json::from_str(&texto).ok()?;
    let mut oraculo = HashMap::new();
    for d in json.get("drawables")?.as_array()? {
        let id = d.get("id")?.as_str()?.to_string();
        let pos: Vec<[f32; 2]> = d
            .get("pos")?
            .as_array()?
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| {
                [
                    c[0].as_f64().unwrap_or(0.0) as f32,
                    c[1].as_f64().unwrap_or(0.0) as f32,
                ]
            })
            .collect();
        oraculo.insert(id, pos);
    }
    Some((modelo, bnd, oraculo))
}

/// `(mallas exactas, mediana, máximo)` con la métrica de `docs/13` (mayor diferencia por
/// coordenada).
fn marcador(
    modelo: &Modelo,
    bnd: &Bindings,
    oraculo: &HashMap<String, Vec<[f32; 2]>>,
) -> (usize, f32, f32) {
    let sel = Seleccion::reposo(modelo, bnd);
    let mut errores = Vec::new();
    for malla in &modelo.mallas {
        let (Some(p), Some(o)) = (
            pose::evaluar(modelo, &sel, malla.indice),
            oraculo.get(&malla.id),
        ) else {
            continue;
        };
        assert_eq!(
            p.len(),
            o.len(),
            "{}: el oráculo y el modelo no coinciden en vértices",
            malla.id
        );
        let e = p
            .iter()
            .zip(o.iter())
            .map(|(a, b)| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()))
            .fold(0.0f32, f32::max);
        errores.push(e);
    }
    errores.sort_by(f32::total_cmp);
    if errores.is_empty() {
        return (0, 0.0, 0.0);
    }
    let exactas = errores.iter().filter(|e| **e < 1e-3).count();
    (
        exactas,
        errores[errores.len() / 2],
        errores[errores.len() - 1],
    )
}

const HIYORI: (&str, &str) = (
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../models/samples/Samples/Resources/Hiyori/Hiyori.moc3"
    ),
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../models/samples/hiyori.oracle.json"
    ),
);

const WANKO: (&str, &str) = (
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../models/samples/Samples/Resources/Wanko/Wanko.moc3"
    ),
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../models/samples/wanko.oracle.json"
    ),
);

#[test]
fn hiyori_no_empeora() {
    let Some((modelo, bnd, oraculo)) = cargar(HIYORI.0, HIYORI.1) else {
        return; // modelos no versionados
    };
    let (exactas, mediana, maximo) = marcador(&modelo, &bnd, &oraculo);
    assert!(exactas >= 12, "mallas exactas: {exactas} (el suelo es 12)");
    assert!(mediana <= 0.034, "mediana: {mediana} (el suelo es 0,033)");
    assert!(maximo <= 0.093, "máximo: {maximo} (el suelo es 0,0921)");
}

#[test]
fn wanko_no_empeora() {
    let Some((modelo, bnd, oraculo)) = cargar(WANKO.0, WANKO.1) else {
        return;
    };
    let (exactas, mediana, maximo) = marcador(&modelo, &bnd, &oraculo);
    assert!(exactas >= 12, "mallas exactas: {exactas} (el suelo es 12)");
    assert!(mediana <= 0.211, "mediana: {mediana} (el suelo es 0,2102)");
    assert!(maximo <= 0.845, "máximo: {maximo} (el suelo es 0,8437)");
}
