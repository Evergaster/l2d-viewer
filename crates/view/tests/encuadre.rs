//! El encuadre (zoom y centro) no puede cambiar el dibujo cuando está en 1:1.
//!
//! Si los modelos de muestra no están, las pruebas que los necesitan se saltan (no se versionan).

use view::{Encuadre, Vista};

const MODELO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../models/samples/Samples/Resources/Hiyori/Hiyori.moc3"
);

fn vista() -> Option<Vista> {
    if !std::path::Path::new(MODELO).exists() {
        eprintln!("sin modelo de muestra; prueba saltada");
        return None;
    }
    Some(Vista::cargar(MODELO).expect("cargar el modelo"))
}

fn opacos(c: &raster::Canvas) -> usize {
    c.pixels.chunks_exact(4).filter(|p| p[3] > 0).count()
}

/// El encuadre por defecto es el dibujo de siempre, píxel a píxel.
#[test]
fn entero_es_identico_a_render() {
    let Some(v) = vista() else { return };
    let valores = v.valores_iniciales();
    let ocultas = v.ocultas_por_defecto();
    let a = v.render(&valores, 300, &ocultas);
    let b = v.render_encuadre(&valores, 300, &ocultas, Encuadre::entero(&v));
    assert_eq!(a.width, b.width);
    assert_eq!(a.height, b.height);
    assert_eq!(
        a.pixels, b.pixels,
        "el encuadre 1:1 debe ser el mismo dibujo"
    );
}

/// Ampliar no mueve el centro: el punto central del lienzo es el mismo en 1× y en 3×.
#[test]
fn el_zoom_conserva_el_centro() {
    let Some(v) = vista() else { return };
    let valores = v.valores_iniciales();
    let ocultas = v.ocultas_por_defecto();
    let base = v.render(&valores, 300, &ocultas);
    for zoom in [1.5f32, 2.0, 3.0] {
        let e = Encuadre::entero(&v).con_zoom(zoom);
        let c = v.render_encuadre(&valores, 300, &ocultas, e);
        assert_eq!((c.width, c.height), (base.width, base.height));
        let centro = |x: &raster::Canvas| x.pixel(x.width / 2, x.height / 2);
        let (a, b) = (centro(&base), centro(&c));
        // No es igualdad exacta: al cambiar la escala, el muestreo bilineal de la textura cae en
        // otro submúsculo del téxel y el redondeo a 8 bits mueve ±1. Un borde de triángulo daría
        // una diferencia de decenas.
        for k in 0..4 {
            let d = (a[k] as i32 - b[k] as i32).abs();
            assert!(
                d <= 3,
                "con zoom {zoom} el centro cambió demasiado: {a:?} vs {b:?}"
            );
        }
    }
}

/// Ampliar llena más el lienzo: el personaje ocupa más píxeles.
#[test]
fn el_zoom_agranda_el_personaje() {
    let Some(v) = vista() else { return };
    let valores = v.valores_iniciales();
    let ocultas = v.ocultas_por_defecto();
    let base = opacos(&v.render(&valores, 300, &ocultas));
    let dos =
        opacos(&v.render_encuadre(&valores, 300, &ocultas, Encuadre::entero(&v).con_zoom(2.0)));
    assert!(
        dos > base,
        "a 2× debería haber más píxeles del modelo: {dos} vs {base}"
    );
}

/// Mover el centro cambia qué se ve: centrar arriba a la izquierda deja la cabeza sola.
#[test]
fn el_centro_mueve_la_camara() {
    let Some(v) = vista() else { return };
    let valores = v.valores_iniciales();
    let ocultas = v.ocultas_por_defecto();
    let (cw, ch) = v.lienzo;
    let base = v.render(&valores, 300, &ocultas);
    let arriba = v.render_encuadre(
        &valores,
        300,
        &ocultas,
        Encuadre::entero(&v)
            .con_zoom(3.0)
            .centrado_en(&v, cw * 0.5, ch * 0.15),
    );
    assert_ne!(base.pixels, arriba.pixels);
    assert!(
        opacos(&arriba) > 0,
        "centrar en la cabeza con 3× tiene que dejar algo dibujado"
    );
}

/// Zoom absurdo o centro fuera del lienzo no revientan: se recortan.
#[test]
fn entradas_absurdas_se_recortan() {
    let Some(v) = vista() else { return };
    let valores = v.valores_iniciales();
    let ocultas = v.ocultas_por_defecto();
    let (cw, ch) = v.lienzo;
    let mut e = Encuadre::entero(&v);
    e.zoom = f32::NAN;
    let c = v.render_encuadre(&valores, 200, &ocultas, e);
    assert_eq!(opacos(&c), opacos(&v.render(&valores, 200, &ocultas)));

    let e = Encuadre::entero(&v)
        .con_zoom(-4.0)
        .centrado_en(&v, -900.0, 1e9);
    let _ = ch;
    assert!(e.zoom > 0.0, "el zoom negativo se recorta: {}", e.zoom);
    // `centrado_en` recibe y de imagen (arriba-izquierda): y enorme cae en el borde de arriba,
    // que en el eje del modelo es `y = 0`.
    assert_eq!(e.centro, [0.0, 0.0], "el centro se recorta al lienzo");
    let _ = cw;
    let c = v.render_encuadre(&valores, 200, &ocultas, e);
    assert!(c.width == 200 && c.height > 0);
}
