//! Banco del dibujo: cuántos milisegundos cuesta una pose a cada ancho.
//!
//! ```bash
//! cargo run --release -p view --example banco -- models/samples/Samples/Resources/Hiyori/Hiyori.moc3
//! ```
//!
//! Sirve para decidir dos cosas con números: a qué ancho puede reproducir la ventana sin comerse
//! un núcleo, y si vale la pena reutilizar el canvas de máscara.

use std::time::Instant;

fn main() {
    let ruta = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "models/samples/Samples/Resources/Hiyori/Hiyori.moc3".to_string());
    let vista = match view::Vista::cargar(&ruta) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("no pude abrir «{ruta}»: {e}");
            std::process::exit(1);
        }
    };
    let valores = vista.valores_iniciales();
    let ocultas = vista.ocultas_por_defecto();
    println!(
        "{} · {} mallas · {} partes · lienzo {:.0}×{:.0}",
        vista.nombre,
        vista.body.art_meshes.len(),
        vista.partes.len(),
        vista.lienzo.0,
        vista.lienzo.1
    );
    println!(
        "{:<8} {:>10} {:>10} {:>10} {:>14}",
        "ancho", "1ª (ms)", "media", "peor", "cuadros/s"
    );
    for ancho in [320u32, 420, 480, 560, 700, 1400] {
        // una pasada de calentamiento (cachés de textura, primer toque de memoria)
        let _ = vista.render(&valores, ancho, &ocultas);
        let n = 6;
        let mut tiempos = Vec::with_capacity(n);
        for _ in 0..n {
            let t0 = Instant::now();
            let _ = vista.render(&valores, ancho, &ocultas);
            tiempos.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        let media = tiempos.iter().sum::<f64>() / n as f64;
        let peor = tiempos.iter().cloned().fold(0.0, f64::max);
        println!(
            "{:<8} {:>10.1} {:>10.1} {:>10.1} {:>14.0}",
            ancho,
            tiempos[0],
            media,
            peor,
            1000.0 / media
        );
    }
}
