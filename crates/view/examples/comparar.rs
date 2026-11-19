//! Compara el orden de dibujo del visor (profundidad de parte + malla) con el que declara el
//! archivo en sus grupos de dibujo (`draw_order_group`), píxel a píxel.
//!
//! ```bash
//! cargo run --release -p view --example comparar -- modelos/.../Hiyori.moc3
//! ```
//!
//! Deja `out/orden-profundidad.png` y `out/orden-arbol.png` para mirarlos, y dice cuántos píxeles
//! cambian entre los dos. Si el número es pequeño y está en bordes, el orden no se nota; si está en
//! el pelo o la cara, importa.

fn main() {
    let ruta = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "models/samples/Samples/Resources/Hiyori/Hiyori.moc3".to_string());
    let ancho: u32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(560);
    let vista = match view::Vista::cargar(&ruta) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("no pude abrir «{ruta}»: {e}");
            std::process::exit(1);
        }
    };
    let valores = vista.valores_iniciales();
    let ocultas = vista.ocultas_por_defecto();
    let enc = view::Encuadre::entero(&vista);

    let a = vista.render_orden(&valores, ancho, &ocultas, enc, view::Orden::Profundidad);
    let b = vista.render_orden(&valores, ancho, &ocultas, enc, view::Orden::Arbol);

    let mut distintos = 0usize;
    let mut suma = 0u64;
    let mut max = 0u8;
    for (pa, pb) in a.pixels.chunks_exact(4).zip(b.pixels.chunks_exact(4)) {
        if pa != pb {
            distintos += 1;
            for k in 0..4 {
                let d = (pa[k] as i32 - pb[k] as i32).unsigned_abs() as u64;
                suma += d;
                max = max.max(d.min(255) as u8);
            }
        }
    }
    println!(
        "{} · {}×{} · píxeles distintos: {} de {} ({:.2} %) · diferencia media por canal (sólo los distintos): {:.1} · máxima {}",
        vista.nombre,
        a.width,
        a.height,
        distintos,
        a.width * a.height,
        100.0 * distintos as f64 / (a.width * a.height) as f64,
        suma as f64 / (distintos.max(1) * 4) as f64,
        max
    );
    let out = format!("{}/../../out", env!("CARGO_MANIFEST_DIR"));
    let _ = std::fs::create_dir_all(&out);
    a.save_png(format!("{out}/orden-profundidad.png")).unwrap();
    b.save_png(format!("{out}/orden-arbol.png")).unwrap();
    println!("PNG: {out}/orden-profundidad.png y {out}/orden-arbol.png");
}
