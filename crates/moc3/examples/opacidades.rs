//! ¿Hay opacidades fuera de la malla que el visor no esté aplicando?
//!
//! El visor multiplica solo la opacidad del keyform de la **malla**. El Core aplica además la de
//! los deformadores por los que pasa y la de su **parte** (`docs/21`). Esta sonda dice si esas
//! secciones tienen valores distintos de 1 en el modelo: si todas son 1, la hipótesis muere y no
//! hay nada que arreglar.
//!
//! ```bash
//! cargo run --release -p moc3 --example opacidades -- <modelo.moc3>
//! ```

fn main() {
    let ruta = std::env::args().nth(1).unwrap_or_default();
    if ruta.is_empty() {
        eprintln!("uso: opacidades <modelo.moc3>");
        std::process::exit(1);
    }
    let bytes = std::fs::read(&ruta).expect("leer el archivo");
    let moc = moc3::Moc3::parse(&bytes).expect("parsear el .moc3");

    for (etiqueta, seccion) in [
        ("mallas", "art_mesh_keyform.opacities"),
        ("partes", "part_keyform.opacities"),
        ("warp", "warp_deformer_keyform.opacities"),
        ("rotación", "rotation_deformer_keyform.opacities"),
    ] {
        match moc.read_f32_section(seccion) {
            Ok(v) => {
                let mut menores = 0;
                let mut min = f32::INFINITY;
                let mut max = f32::NEG_INFINITY;
                for x in &v {
                    if *x < 0.999 && *x > 0.001 {
                        menores += 1;
                    }
                    min = min.min(*x);
                    max = max.max(*x);
                }
                println!(
                    "{etiqueta:<9} {seccion:<38} {} valores · rango [{min:.3}, {max:.3}] · {} fuera de 1",
                    v.len(),
                    menores
                );
                if menores > 0 && menores <= 12 {
                    let ejemplos: Vec<String> = v
                        .iter()
                        .enumerate()
                        .filter(|(_, x)| **x < 0.999 && **x > 0.001)
                        .map(|(i, x)| format!("[{i}]={x:.3}"))
                        .collect();
                    println!("          ejemplos: {}", ejemplos.join(" "));
                }
            }
            Err(e) => println!("{etiqueta:<9} {seccion:<38} no se pudo leer: {e}"),
        }
    }
}
