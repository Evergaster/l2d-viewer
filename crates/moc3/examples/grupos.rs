//! Vuelca los grupos de dibujo de un `.moc3` y compara el orden declarado con el que usa el
//! visor hoy (`profundidad de la parte + profundidad de la malla / 10000`, empates por índice).
//!
//! ```bash
//! cargo run --release -p moc3 --example grupos -- modelos/.../Hiyori.moc3
//! ```
//!
//! Sirve para decidir con datos si vale la pena ordenar por el árbol de grupos: si el orden
//! declarado coincide con el que ya se pinta, no hay nada que arreglar.

fn main() {
    let ruta = std::env::args().nth(1).unwrap_or_default();
    if ruta.is_empty() {
        eprintln!("uso: grupos <modelo.moc3>");
        std::process::exit(1);
    }
    let bytes = std::fs::read(&ruta).expect("leer el archivo");
    let moc = moc3::Moc3::parse(&bytes).expect("parsear el .moc3");
    let body = moc.body().expect("leer el cuerpo");

    println!(
        "{} mallas · {} partes · {} grupos · {} items",
        body.art_meshes.len(),
        body.part_ids.len(),
        body.grupos.len(),
        body.objetos_grupo.len()
    );
    for (i, g) in body.grupos.iter().enumerate() {
        println!(
            "  grupo {i:>2}: items {:?} · total_mallas {} · depth {}..{}",
            g.objetos, g.total_mallas, g.min_depth, g.max_depth
        );
    }

    // Qué tipos aparecen: se asume 0 = malla, pero se mide en vez de suponerlo.
    let mut tipos = std::collections::BTreeMap::new();
    for o in &body.objetos_grupo {
        *tipos.entry(o.tipo).or_insert(0usize) += 1;
    }
    println!("  tipos de item: {tipos:?}");

    // Aplanado del árbol desde la raíz (grupo 0), tal como lo describe la spec: en cada nodo los
    // items van de fondo a frente, y un grupo hijo se expande en su sitio.
    let mut orden = Vec::new();
    let mut vistos = vec![0usize; body.art_meshes.len()];
    aplanar(&body, 0, &mut orden, &mut vistos, 0);

    let cubiertas = vistos.iter().filter(|v| **v == 1).count();
    let repetidas = vistos.iter().filter(|v| **v > 1).count();
    println!(
        "  árbol: {} mallas listadas · {} distintas · {} repetidas · {} sin referenciar",
        orden.len(),
        cubiertas,
        repetidas,
        body.art_meshes.len() - cubiertas
    );

    // Orden del visor: parte + malla/10000, estable por índice de malla.
    let mut actual: Vec<usize> = (0..body.art_meshes.len()).collect();
    actual.sort_by(|a, b| {
        let clave = |i: usize| {
            let malla = &body.art_meshes[i];
            let parte = if malla.parent_part >= 0 {
                body.part_keyform_begin
                    .get(malla.parent_part as usize)
                    .and_then(|k| body.part_draw_order.get(*k as usize))
                    .copied()
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            let m = body
                .art_mesh_keyform_draw_orders
                .get(malla.keyform_begin as usize)
                .copied()
                .unwrap_or(0.0);
            parte + m / 10000.0
        };
        clave(*a)
            .partial_cmp(&clave(*b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // ¿En qué posiciones difieren?
    let solo_arbol: Vec<usize> = orden.iter().copied().filter(|m| vistos[*m] == 1).collect();
    let mut dif = 0;
    for (i, m) in solo_arbol.iter().enumerate() {
        if actual.get(i) != Some(m) {
            dif += 1;
        }
    }
    println!(
        "  orden del árbol vs orden del visor: {dif} de {} posiciones distintas",
        solo_arbol.len()
    );
    println!(
        "  árbol (primeras 20): {:?}",
        &solo_arbol[..solo_arbol.len().min(20)]
    );
    println!(
        "  visor (primeras 20): {:?}",
        &actual[..actual.len().min(20)]
    );
}

fn aplanar(
    body: &moc3::Body,
    grupo: usize,
    salida: &mut Vec<usize>,
    vistos: &mut [usize],
    hondura: usize,
) {
    if hondura > 32 {
        return; // un ciclo en el archivo no debe colgar la herramienta
    }
    let Some(g) = body.grupos.get(grupo) else {
        return;
    };
    for i in g.objetos.0..g.objetos.1 {
        let Some(o) = body.objetos_grupo.get(i) else {
            continue;
        };
        if let Some(hijo) = o.grupo_hijo {
            aplanar(body, hijo, salida, vistos, hondura + 1);
        } else if o.tipo == 0 && o.indice >= 0 {
            let m = o.indice as usize;
            if let Some(v) = vistos.get_mut(m) {
                *v += 1;
            }
            salida.push(m);
        }
    }
}
