//! El orden de pintado: el árbol de grupos del archivo manda como desempate.
//!
//! La especificación no oficial dice que dentro de cada grupo los items se ordenan por
//! profundidad, y el orden del archivo desempata. Contra el render del Core (geometría oficial +
//! nuestro rasterizador) eso baja la diferencia media de 38,5 a 19,4 y los píxeles con diferencia
//! > 60 del 13,6 % al 8,6 % — por eso es el orden por defecto.

use view::{Orden, Vista};

fn modelos() -> Vec<String> {
    vec![
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../models/samples/Samples/Resources/Hiyori/Hiyori.moc3"
        )
        .to_string(),
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../models/samples/Samples/Resources/Wanko/Wanko.moc3"
        )
        .to_string(),
    ]
}

fn cargar(ruta: &str) -> Option<Vista> {
    if !std::path::Path::new(ruta).exists() {
        eprintln!("sin modelo de muestra ({ruta}); prueba saltada");
        return None;
    }
    Some(Vista::cargar(ruta).expect("cargar el modelo"))
}

/// Todas las mallas aparecen exactamente una vez en el árbol de grupos.
#[test]
fn el_arbol_cubre_cada_malla_una_vez() {
    for ruta in modelos() {
        let Some(v) = cargar(&ruta) else { continue };
        let n = v.body.art_meshes.len();
        let mut cuenta = vec![0usize; n];
        for m in &v.orden_arbol {
            cuenta[*m] += 1;
        }
        let repetidas = cuenta.iter().filter(|c| **c > 1).count();
        let ausentes = cuenta.iter().filter(|c| **c == 0).count();
        assert_eq!(
            repetidas, 0,
            "{}: {} mallas repetidas en el árbol",
            v.nombre, repetidas
        );
        assert_eq!(
            ausentes, 0,
            "{}: {} mallas sin grupo (el archivo incumple la spec)",
            v.nombre, ausentes
        );
        assert_eq!(v.orden_arbol.len(), n);
    }
}

/// El orden por defecto es el del árbol y no el de la profundidad sola.
#[test]
fn el_orden_por_defecto_es_el_arbol() {
    for ruta in modelos() {
        let Some(v) = cargar(&ruta) else { continue };
        let valores = v.valores_iniciales();
        let ocultas = v.ocultas_por_defecto();
        let enc = view::Encuadre::entero(&v);
        let por_defecto = v.render(&valores, 300, &ocultas);
        let arbol = v.render_orden(&valores, 300, &ocultas, enc, Orden::Arbol);
        let profundidad = v.render_orden(&valores, 300, &ocultas, enc, Orden::Profundidad);
        assert_eq!(
            por_defecto.pixels, arbol.pixels,
            "{}: render() debe ser el orden del árbol",
            v.nombre
        );
        assert_ne!(
            arbol.pixels, profundidad.pixels,
            "{}: si los dos órdenes dieran lo mismo, el desempate no serviría de nada",
            v.nombre
        );
    }
}

/// Cambiar el orden no cambia **qué** se dibuja, solo en qué orden: mismo número de píxeles
/// pintados.
#[test]
fn el_orden_no_cambia_las_mallas_dibujadas() {
    for ruta in modelos() {
        let Some(v) = cargar(&ruta) else { continue };
        let valores = v.valores_iniciales();
        let ocultas = v.ocultas_por_defecto();
        let enc = view::Encuadre::entero(&v);
        let opacos = |c: &raster::Canvas| c.pixels.chunks_exact(4).filter(|p| p[3] > 0).count();
        let a = opacos(&v.render_orden(&valores, 300, &ocultas, enc, Orden::Profundidad));
        let b = opacos(&v.render_orden(&valores, 300, &ocultas, enc, Orden::Arbol));
        // Puede haber diferencias de borde (un píxel queda tapado o no), pero no un cambio de
        // conjunto: si faltara una malla, se notaría en miles de píxeles.
        let dif = a.abs_diff(b);
        assert!(
            dif * 100 < a.max(1),
            "{}: la silueta cambió demasiado entre órdenes ({a} vs {b})",
            v.nombre
        );
    }
}
