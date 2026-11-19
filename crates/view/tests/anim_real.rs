//! Prueba de punta a punta: modelo real de muestra + `l2d-anim/1` + dibujo.
//!
//! Los modelos no se versionan (licencia de Live2D), así que si no están la prueba se salta en
//! vez de fallar. Cuando están, comprueba lo que de verdad importa: que la animación mueve los
//! parámetros del modelo y que la imagen **cambia** con el tiempo.

use view::anim::Animacion;

const MODELO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../models/samples/Samples/Resources/Hiyori/Hiyori.moc3"
);
/// Animación de ejemplo versionada (3 claves sobre 5 parámetros reales de Hiyori).
const ANIM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/anim-ejemplo.json");

fn hash(c: &raster::Canvas) -> u64 {
    // FNV-1a 64 sobre los píxeles: dos poses distintas deben dar hashes distintos.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in &c.pixels {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[test]
fn la_animacion_mueve_el_modelo_real() {
    if !std::path::Path::new(MODELO).exists() {
        eprintln!("sin modelo de muestra; prueba saltada");
        return;
    }
    let vista = view::Vista::cargar(MODELO).expect("cargar el modelo");
    let ids: Vec<String> = vista.parametros.iter().map(|p| p.id.clone()).collect();

    let anim = if std::path::Path::new(ANIM).exists() {
        Animacion::cargar(ANIM, &ids).expect("cargar la animación de prueba")
    } else {
        // Si falta el archivo, se arma la misma animación aquí para no depender de `out/`.
        let json = r#"{"formato":"l2d-anim/1","modelo":"Hiyori","duracion":3,
            "parametros":["ParamAngleX","ParamEyeLOpen"],
            "claves":[{"t":0,"valores":[0,1]},{"t":1.5,"valores":[30,0]},{"t":3,"valores":[-30,1.2]}]}"#;
        Animacion::desde_json(json, &ids).expect("animación en línea")
    };

    let ocultas = vista.ocultas_por_defecto();
    let mut valores = vista.valores_iniciales();
    let ix = ids.iter().position(|p| p == "ParamAngleX").unwrap();
    let reposo = valores[ix];

    anim.aplicar(0.0, &mut valores);
    let c0 = vista.render(&valores, 320, &ocultas);
    let h0 = hash(&c0);

    anim.aplicar(1.5, &mut valores);
    let c1 = vista.render(&valores, 320, &ocultas);
    let h1 = hash(&c1);

    anim.aplicar(3.0, &mut valores);
    let c2 = vista.render(&valores, 320, &ocultas);
    let h2 = hash(&c2);

    // La pose de t=0 sí mueve el parámetro respecto del reposo del modelo
    assert_ne!(c0.pixels, c2.pixels, "t=0 y t=3 deben ser poses distintas");
    assert_ne!(h0, h1, "t=0 y t=1.5 deben dibujar distinto");
    assert_ne!(h1, h2, "t=1.5 y t=3 deben dibujar distinto");

    // y el valor aplicado en el parámetro es el de la clave, no el de reposo
    assert!(
        (valores[ix] - (-30.0)).abs() < 1e-3,
        "AngleX={} en t=3",
        valores[ix]
    );
    assert_ne!(reposo, valores[ix]);

    // Ida y vuelta: evaluar en un instante intermedio coincide con la clave exacta en t=1.5
    anim.aplicar(1.5, &mut valores);
    assert!(
        (valores[ix] - 30.0).abs() < 1e-3,
        "AngleX={} en t=1.5",
        valores[ix]
    );

    // El bucle no se sale del rango y el dibujo sigue siendo el de una pose válida
    let t = anim.envolver(4.2);
    assert!((t - 1.2).abs() < 1e-4, "envolver(4.2)={t}");
    anim.aplicar(t, &mut valores);
    let _ = vista.render(&valores, 320, &ocultas);

    // PNG de cada pose para mirar: out/anim-t{0,15,30}.png (en la raíz del repo)
    let out = format!("{}/../../out", env!("CARGO_MANIFEST_DIR"));
    let _ = std::fs::create_dir_all(&out);
    c0.save_png(format!("{out}/anim-t0.png")).unwrap();
    c1.save_png(format!("{out}/anim-t15.png")).unwrap();
    c2.save_png(format!("{out}/anim-t30.png")).unwrap();
}

/// Un archivo de otro modelo se rechaza antes de tocar nada.
#[test]
fn animacion_de_otro_modelo_se_rechaza() {
    if !std::path::Path::new(MODELO).exists() {
        return;
    }
    let vista = view::Vista::cargar(MODELO).expect("cargar el modelo");
    let ids: Vec<String> = vista.parametros.iter().map(|p| p.id.clone()).collect();
    let json = r#"{"formato":"l2d-anim/1","duracion":1,
        "parametros":["ParamQueNoExiste"],"claves":[{"t":0,"valores":[0]}]}"#;
    let e = Animacion::desde_json(json, &ids).unwrap_err();
    assert!(e.contains("ParamQueNoExiste"), "{e}");
}
