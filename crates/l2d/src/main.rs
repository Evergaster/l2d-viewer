//! `l2d` — CLI del visor independiente.
//!
//!   l2d inspect <archivo.moc3>     cabecera, canvas, conteos y secciones
//!   l2d model3  <archivo.json>     manifiesto del modelo
//!   l2d ver     <archivo.moc3> <salida.png>  dibuja la pose con el motor propio, sin Core
//!   l2d pose    <archivo.moc3>     evalúa la pose con el motor propio (opcional --oraculo)
//!   l2d demo    [salida.png]       rasteriza una malla deformada (prueba del pipeline)

use std::error::Error;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("inspect") => match args.get(1) {
            Some(p) => inspect(p),
            None => Err("falta la ruta del .moc3".into()),
        },
        Some("model3") => match args.get(1) {
            Some(p) => model3(p),
            None => Err("falta la ruta del .model3.json".into()),
        },
        Some("demo") => demo(args.get(1).map(String::as_str).unwrap_or("demo.png")),
        Some("params") => match args.get(1) {
            Some(f) => params(f),
            _ => Err("uso: l2d params <archivo.moc3>".into()),
        },
        Some("oracle") => match (args.get(1), args.get(2), args.get(3)) {
            (Some(j), Some(t), Some(o)) => {
                let zoom = args.get(4).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.3);
                render_oracle(j, t, o, zoom)
            }
            _ => Err("uso: l2d oracle <dump.json> <textura.png> <salida.png> [zoom]".into()),
        },
        Some("body") => match args.get(1) {
            Some(p) => body_cmd(p),
            None => Err("falta la ruta del .moc3".into()),
        },
        Some("render") => match (args.get(1), args.get(2), args.get(3)) {
            (Some(moc), Some(tex), Some(out)) => {
                let scale = args.get(4).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.5);
                render(moc, tex, out, scale)
            }
            _ => Err("uso: l2d render <modelo.moc3> <textura.png> <salida.png> [escala]".into()),
        },
        Some("ver") => ver_cmd(&args[1..]),
        Some("pose") => pose_cmd(&args[1..]),
        Some("edit") => match (args.get(1), args.get(2)) {
            (Some(l2d), Some(moc)) => edit_cmd(l2d, moc),
            _ => Err("uso: l2d edit <archivo.l2d> <modelo.moc3>".into()),
        },
        Some("-h") | Some("--help") | None => {
            println!("{}", usage());
            Ok(())
        }
        Some(other) => Err(format!("subcomando desconocido: {other}").into()),
    };

    if let Err(e) = r {
        eprintln!("l2d: {e}");
        std::process::exit(1);
    }
}

fn usage() -> String {
    "l2d — visor independiente de modelos Live2D\n\
     \n\
     uso:\n  \
     l2d inspect <archivo.moc3>            contenedor: cabecera, canvas, conteos, secciones\n  \
     l2d body    <archivo.moc3>            cuerpo: partes, mallas, parámetros, keyforms\n  \
     l2d params  <archivo.moc3>            parámetros: id, mínimo, máximo, defecto\n  \
     l2d edit    <archivo.l2d> <modelo.moc3>\n                                        \
     valida el modelo editado contra su .moc3 (huella, mallas, vértices)\n  \
     l2d pose    <modelo.moc3> [--oraculo dump.json] [--malla ID]\n  \
     evalúa la pose con el motor propio y mide la desviación\n  \
     l2d oracle  <dump.json> <tex_dir> <salida.png> [zoom]\n                                        \
     render con geometría del Core oficial\n  \
     l2d model3  <archivo.model3.json>     manifiesto del modelo\n  \
     l2d render  <modelo.moc3> <tex.png> <salida.png> [zoom]\n                                        \
     dibuja la pose de reposo (defecto zoom 0.5)\n  \
     l2d demo    [salida.png]              malla deformada de prueba\n\
     \n\
     variables:\n  \
     L2D_KF=primero|centro|mediana         elección del keyform de reposo (por defecto mediana)"
        .to_string()
}

// ---------------------------------------------------------------- ver

/// Dibuja una pose con el motor propio (`pose` + `view`) y la guarda como PNG.
///
/// Es el visor independiente: no carga el Core de Live2D en ningún momento.
fn ver_cmd(args: &[String]) -> Result<(), Box<dyn Error>> {
    let Some(ruta) = args.first() else {
        return Err("uso: l2d ver <modelo.moc3> <salida.png> [--ancho N] [--param Nombre=valor]".into());
    };
    let Some(salida) = args.get(1) else {
        return Err("falta la ruta de salida (.png)".into());
    };
    let mut ancho = 700u32;
    let mut valores: Vec<(String, f32)> = Vec::new();
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--ancho" => {
                ancho = args.get(i + 1).ok_or("--ancho necesita un número")?.parse()?;
                i += 2;
            }
            "--param" => {
                if let Some(par) = args.get(i + 1) {
                    let (nombre, valor) = par.split_once('=').ok_or("--param espera Nombre=valor")?;
                    valores.push((nombre.to_string(), valor.parse::<f32>()?));
                }
                i += 2;
            }
            otro => return Err(format!("opción desconocida: {otro}").into()),
        }
    }

    let t0 = std::time::Instant::now();
    let vista = view::Vista::cargar(ruta)?;
    let t1 = std::time::Instant::now();
    let mut v = vista.valores_iniciales();
    for (nombre, valor) in &valores {
        let p = vista
            .parametros
            .iter()
            .position(|x| &x.id == nombre)
            .ok_or_else(|| format!("parámetro «{nombre}» no existe en el modelo"))?;
        v[p] = *valor;
    }
    let canvas = vista.render(&v, ancho);
    let t2 = std::time::Instant::now();
    canvas.save_png(salida)?;

    println!("modelo       {} ({} mallas, {} texturas)", vista.nombre, vista.body.art_meshes.len(), vista.texturas.len());
    println!("lienzo       {} × {}  →  {} × {}", vista.lienzo.0, vista.lienzo.1, canvas.width, canvas.height);
    println!("carga        {} ms", t1.duration_since(t0).as_millis());
    println!("dibujo       {} ms", t2.duration_since(t1).as_millis());
    println!("salida       {salida}");
    Ok(())
}

// ---------------------------------------------------------------- pose

/// Keyform de mayor peso de una lista de slots.
fn kf_dominante(slots: &[pose::Slot]) -> (usize, f32) {
    slots
        .iter()
        .copied()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((0, 1.0))
}

/// Evalúa una pose con el motor propio (`crates/pose`) y, si se le pasa un volcado del
/// Core, mide la desviación por malla.
///
/// Es el marcador del visor independiente: hoy acierta las cadenas cortas y falla las que
/// pasan de una rotación a un warp (`docs/13` §5.21). El número que imprime es el que hay.
fn pose_cmd(args: &[String]) -> Result<(), Box<dyn Error>> {
    let Some(ruta) = args.first() else {
        return Err("uso: l2d pose <modelo.moc3> [--oraculo dump.json] [--malla ID] [--param Nombre=valor]".into());
    };
    let mut oraculo: Option<String> = None;
    let mut solo_malla: Option<String> = None;
    let mut valores: Vec<(String, f32)> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--oraculo" => {
                oraculo = args.get(i + 1).cloned();
                i += 2;
            }
            "--malla" => {
                solo_malla = args.get(i + 1).cloned();
                i += 2;
            }
            "--param" => {
                if let Some(par) = args.get(i + 1) {
                    let (nombre, valor) = par
                        .split_once('=')
                        .ok_or("--param espera Nombre=valor")?;
                    valores.push((nombre.to_string(), valor.parse::<f32>()?));
                }
                i += 2;
            }
            otro => return Err(format!("opción desconocida: {otro}").into()),
        }
    }

    let bytes = std::fs::read(ruta)?;
    let moc = moc3::Moc3::parse(&bytes)?;
    let modelo = pose::cargar(&moc);
    let bnd = pose::Bindings::cargar(&moc);

    let mut pose_valores = bnd.defectos.clone();
    for (nombre, valor) in &valores {
        let p = bnd
            .parametros
            .iter()
            .position(|x| x == nombre)
            .ok_or_else(|| format!("parámetro «{nombre}» no existe en el modelo"))?;
        pose_valores[p] = *valor;
    }
    let sel = pose::Seleccion::con_valores(&modelo, &bnd, &pose_valores);
    let comp = pose::componer(&modelo, &sel);

    println!("modelo       {ruta}");
    println!(
        "pose         {} parámetros{}",
        pose_valores.len(),
        if valores.is_empty() { " (por defecto)" } else { "" }
    );
    for (nombre, valor) in &valores {
        println!("  {nombre:<20} {valor}");
    }

    // volcado del Core: id → vértices
    let mut esperado: std::collections::HashMap<String, Vec<[f32; 2]>> =
        std::collections::HashMap::new();
    if let Some(r) = &oraculo {
        let texto = std::fs::read_to_string(r)?;
        let json: serde_json::Value = serde_json::from_str(&texto)?;
        let arr = json
            .get("drawables")
            .and_then(|d| d.as_array())
            .ok_or("el volcado no trae «drawables»")?;
        for d in arr {
            let id = d.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let pos = d
                .get("pos")
                .and_then(|x| x.as_array())
                .map(|v| {
                    v.chunks(2)
                        .filter(|c| c.len() == 2)
                        .map(|c| [c[0].as_f64().unwrap_or(0.0) as f32, c[1].as_f64().unwrap_or(0.0) as f32])
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            esperado.insert(id, pos);
        }
        println!("oráculo      {r} ({} drawables)", esperado.len());
    }

    let mut filas: Vec<(String, usize, f32)> = Vec::new();
    let mut detalle: Option<(String, Vec<[f32; 2]>)> = None;
    for malla in &modelo.mallas {
        if let Some(solo) = &solo_malla {
            if &malla.id != solo {
                continue;
            }
        }
        let Some(p) = pose::evaluar(&modelo, &comp, &sel, malla.indice) else {
            continue;
        };
        if let Some(o) = esperado.get(&malla.id) {
            if o.len() != p.len() {
                eprintln!(
                    "  {}: el oráculo trae {} vértices y el modelo {}",
                    malla.id,
                    o.len(),
                    p.len()
                );
                continue;
            }
            // misma métrica que los experimentos de docs/13: la mayor diferencia por
            // coordenada (Chebyshev), para que los números sean comparables entre sí
            let e = p
                .iter()
                .zip(o.iter())
                .map(|(a, b)| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()))
                .fold(0.0f32, f32::max);
            filas.push((malla.id.clone(), modelo.cadena(malla.indice).len(), e));
            if solo_malla.is_some() {
                detalle = Some((malla.id.clone(), p));
            }
        } else if solo_malla.is_some() {
            detalle = Some((malla.id.clone(), p));
        }
    }

    if let Some((id, p)) = &detalle {
        // diagnóstico: qué keyform usa cada eslabón y con qué campos
        if let Some(m) = modelo.mallas.iter().find(|x| &x.id == id) {
            let (kf_m, w_m) = kf_dominante(&sel.malla[m.indice]);
            println!(
                "\n{id}: keyform de malla {kf_m} de {} (peso {w_m:.2})",
                m.keyform_count
            );
            for d in modelo.cadena(m.indice) {
                let def = &modelo.deformadores[d];
                let (k, w) = kf_dominante(&sel.deformador[d]);
                match def.tipo {
                    pose::Tipo::Rotacion => {
                        let r = &modelo.rotaciones[def.specific];
                        let c = &modelo.rot_campos[r.keyform_begin + k];
                        println!(
                            "  {:<12} rotación  kf {}/{} (w {:.2})  ángulo {:+.2}  escala {:.6}  origen [{:+.4}, {:+.4}]  refl {}{}",
                            def.id, k, r.keyform_count, w, c.angulo, c.escala,
                            c.origen[0], c.origen[1],
                            if c.reflect_x { "x" } else { "" },
                            if c.reflect_y { "y" } else { "" }
                        );
                    }
                    pose::Tipo::Warp => {
                        let warp = &modelo.warps[def.specific];
                        println!(
                            "  {:<12} warp      kf {}/{} (w {:.2})  rejilla {}×{} celdas",
                            def.id, k, warp.keyform_count, w, warp.filas, warp.columnas
                        );
                    }
                }
            }
        }
        if let Some(m) = modelo.mallas.iter().find(|x| &x.id == id) {
            if let Some(pasos) = pose::evaluar_pasos(&modelo, &comp, &sel, m.indice) {
                println!("\n  paso a paso (bbox en el espacio del Core):");
                for (nombre, v) in pasos {
                    let (mut x0, mut x1) = (f32::MAX, f32::MIN);
                    let (mut y0, mut y1) = (f32::MAX, f32::MIN);
                    for q in &v {
                        x0 = x0.min(q[0]);
                        x1 = x1.max(q[0]);
                        y0 = y0.min(q[1]);
                        y1 = y1.max(q[1]);
                    }
                    println!("    {nombre:<20} x[{x0:+.4} {x1:+.4}] y[{y0:+.4} {y1:+.4}]");
                }
            }
        }
        println!("\n{id}: {} vértices en el espacio del Core", p.len());
        for v in p.iter().take(6) {
            println!("  [{:+.5}, {:+.5}]", v[0], v[1]);
        }
    }

    if esperado.is_empty() {
        println!("\n(sin oráculo: no hay con qué comparar; pasa --oraculo dump.json)");
        return Ok(());
    }
    if filas.is_empty() {
        return Err("ninguna malla comparable con el oráculo".into());
    }

    filas.sort_by(|a, b| a.2.total_cmp(&b.2));
    let exactas = filas.iter().filter(|f| f.2 < 1e-3).count();
    let dos_px = filas.iter().filter(|f| f.2 < 6.7e-4).count();
    let mediana = filas[filas.len() / 2].2;
    println!(
        "\nmallas < 1e-3: {}/{}   < 6,7e-4 (2 px): {}   mediana {:.2e}   máximo {:.4}",
        exactas,
        filas.len(),
        dos_px,
        mediana,
        filas.last().map(|f| f.2).unwrap_or(0.0)
    );
    println!("\n{} mejores:", 12.min(filas.len()));
    for (id, eslabones, e) in filas.iter().take(12) {
        println!("  {id:<12} eslabones={eslabones:<2} error={e:.6}");
    }
    println!("{} peores:", 5.min(filas.len()));
    for (id, eslabones, e) in filas.iter().rev().take(5) {
        println!("  {id:<12} eslabones={eslabones:<2} error={e:.6}");
    }
    Ok(())
}

// ---------------------------------------------------------------- edit

/// Valida un `l2d-model/1` contra su `.moc3` y aplica los desplazamientos sobre la geometría de
/// reposo del archivo. No abre navegador: es la prueba de que el formato se sostiene en Rust.
fn edit_cmd(ruta_l2d: &str, ruta_moc: &str) -> Result<(), Box<dyn Error>> {
    let texto = std::fs::read_to_string(ruta_l2d)?;
    let bytes = std::fs::read(ruta_moc)?;
    let m = moc3::Moc3::parse(&bytes)?;
    let b = m.body()?;
    let doc = edit::Documento::desde_json(&texto)?;

    println!("documento    {ruta_l2d}");
    println!("formato      {}", doc.formato);
    println!(
        "modelo       {}",
        if doc.modelo.is_empty() { "(sin nombre)" } else { doc.modelo.as_str() }
    );
    // El origen va primero: sin él los índices de vértice no significan nada.
    doc.verificar_origen(&bytes)?;
    println!(
        "origen       {ruta_moc} ({} bytes, huella {}) — coincide",
        doc.origen.bytes, doc.origen.huella
    );
    doc.validar(&b)?;

    if doc.esta_vacio() {
        println!("\nsin cambios: el documento no toca el modelo");
    } else {
        println!("\nparámetros   {} fuera de reposo", doc.parametros.len());
        for (id, v) in &doc.parametros {
            println!("  {id:<20} {v:.4}");
        }
        println!("partes       {}", doc.partes.len());
        for (id, visible) in &doc.partes {
            println!("  {id:<20} {}", if *visible { "visible" } else { "oculta" });
        }
        println!(
            "vértices     {} desplazamientos en {} mallas",
            doc.vertices.len(),
            doc.por_malla().len()
        );
        for (malla, n) in doc.por_malla() {
            println!("  {malla:<20} {n} vértice(s)");
        }
    }

    let mut posiciones: Vec<Vec<f32>> = b
        .art_meshes
        .iter()
        .map(|malla| vec![0.0; malla.vertex_count as usize * 2])
        .collect();
    let n = doc.aplicar(&b, &mut posiciones)?;
    println!("\naplicado     {n} vértice(s) sobre la geometría de reposo, sin error");
    Ok(())
}

// ---------------------------------------------------------------- inspect

fn inspect(path: &str) -> Result<(), Box<dyn Error>> {
    let bytes = std::fs::read(path)?;
    let m = moc3::Moc3::parse(&bytes)?;

    println!("archivo      {path} ({:.1} KiB)", bytes.len() as f32 / 1024.0);
    println!("formato      {}", m.version.label());
    println!("endianness   {:?}", m.endianness);
    println!(
        "canvas       {:.0} × {:.0} px  origen ({:.1}, {:.1})  ppu {:.1}  reverse_y={}",
        m.canvas.width, m.canvas.height, m.canvas.origin_x, m.canvas.origin_y,
        m.canvas.pixels_per_unit, m.canvas.reverse_y
    );
    println!("conteos      {} campos", m.counts.len());

    println!("\nconteos no nulos:");
    for (name, value) in m.counts.iter() {
        if value > 0 {
            println!("  {name:<34} {value}");
        }
    }

    let nonempty: Vec<_> = m.sections.iter().filter(|s| s.count > 0).collect();
    println!("\nsecciones con datos: {}", nonempty.len());
    for s in nonempty {
        println!("  {:<44} {:>6} elems  @ {:#08x}", s.name, s.count, s.offset);
    }
    Ok(())
}

// ---------------------------------------------------------------- model3

fn model3(path: &str) -> Result<(), Box<dyn Error>> {
    let json = std::fs::read_to_string(path)?;
    let m = model3::Model3::parse(&json)?;

    println!("manifiesto   {path}");
    println!("moc          {}", m.file_references.moc.as_deref().unwrap_or("(ninguno)"));
    println!("texturas     {}", m.file_references.textures.len());
    for t in &m.file_references.textures {
        println!("  - {t}");
    }
    if let Some(p) = &m.file_references.physics {
        println!("física       {p}");
    }
    if let Some(p) = &m.file_references.pose {
        println!("pose         {p}");
    }
    println!("expresiones  {}", m.file_references.expressions.len());
    println!("movimientos  {}", m.motion_count());
    for (group, motions) in &m.file_references.motions {
        println!("  {group:<10} {}", motions.len());
    }
    if !m.groups.is_empty() {
        println!("grupos       {}", m.groups.len());
        for g in &m.groups {
            println!("  {} ({} ids)", g.name, g.ids.len());
        }
    }
    if !m.hit_areas.is_empty() {
        println!("hit areas    {}", m.hit_areas.join(", "));
    }
    println!("assets       {} archivos referenciados", m.asset_paths().len());
    Ok(())
}

// ---------------------------------------------------------------- body

fn body_cmd(path: &str) -> Result<(), Box<dyn Error>> {
    let bytes = std::fs::read(path)?;
    let m = moc3::Moc3::parse(&bytes)?;
    let b = m.body()?;

    println!("partes       {}", b.part_ids.len());
    for (i, id) in b.part_ids.iter().enumerate().take(6) {
        println!("  [{i:2}] {id}");
    }
    println!("mallas       {}", b.art_meshes.len());
    let total_v: u32 = b.art_meshes.iter().map(|m| m.vertex_count).sum();
    let total_i: u32 = b.art_meshes.iter().map(|m| m.index_count).sum();
    println!("  vértices   {total_v}   índices {total_i}");
    for (i, mesh) in b.art_meshes.iter().enumerate().take(6) {
        println!(
            "  [{i:2}] {:<16} v={:<4} idx={:<4} uv={:<5} kf={} tex={}",
            mesh.id, mesh.vertex_count, mesh.index_count, mesh.uv_begin, mesh.keyform_begin,
            mesh.texture_index
        );
    }
    println!("parámetros   {}", b.parameters.len());
    for p in b.parameters.iter().take(6) {
        println!("  {:<18} {:.1} .. {:.1}  (def {:.1})", p.id, p.min, p.max, p.default);
    }
    println!(
        "keyform pos  {} floats, uvs {} floats, índices {}",
        b.keyform_positions.len(),
        b.uvs.len(),
        b.indices.len()
    );
    Ok(())
}

// ---------------------------------------------------------------- render

fn render(moc: &str, tex: &str, out: &str, zoom: f32) -> Result<(), Box<dyn Error>> {
    let m = moc3::Moc3::parse(&std::fs::read(moc)?)?;
    let b = m.body()?;
    // Hiyori (y casi todos los modelos) usan varios atlas: texture_00, texture_01...
    // Se cargan todos los del directorio y cada malla elige por su índice.
    let tex_dir = std::path::Path::new(tex)
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or("ruta de textura inválida")?;
    let mut rutas: Vec<std::path::PathBuf> = std::fs::read_dir(&tex_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("texture") && n.ends_with(".png"))
                .unwrap_or(false)
        })
        .collect();
    rutas.sort();
    let mut textures: Vec<raster::Texture> = Vec::new();
    for r in &rutas {
        textures.push(raster::Texture::from_png(r)?);
    }
    if textures.is_empty() {
        textures.push(raster::Texture::from_png(tex)?);
    }

    let (cw, ch) = (m.canvas.width, m.canvas.height);
    if cw <= 0.0 || ch <= 0.0 {
        return Err("canvas sin dimensiones".into());
    }
    let iw = (cw * zoom).round().max(1.0) as u32;
    let ih = (ch * zoom).round().max(1.0) as u32;

    #[derive(Clone)]
    struct Ready {
        verts: Vec<([f32; 2], [f32; 2])>,
        indices: Vec<i16>,
        opacity: f32,
        masks: Vec<usize>,
    }

    // Pre-paso: centroide mediano de todos los keyforms del modelo. Sirve para elegir,
    // por malla, el keyform que cae más cerca del cuerpo (heurística de reposo).
    // Calibrado contra el Core oficial: el keyform de reposo es el primero
    // (penalización mediana 0; ver tools/calibrar-keyform.py).
    let kf_mode = std::env::var("L2D_KF").unwrap_or_else(|_| "primero".to_string());
    let median: [f32; 2] = {
        let mut xs: Vec<f32> = Vec::new();
        let mut ys: Vec<f32> = Vec::new();
        for mesh in &b.art_meshes {
            let n_idx = mesh.vertex_count as usize;
            if n_idx == 0 || !mesh.visible {
                continue;
            }
            let ib = mesh.index_begin as usize;
            if ib + n_idx > b.indices.len() {
                continue;
            }
            let tri = &b.indices[ib..ib + n_idx];
            let n_verts = tri.iter().map(|v| (*v).max(0) as usize).max().unwrap_or(0) + 1;
            if n_verts == 0 {
                continue;
            }
            for kf in mesh.keyform_begin..(mesh.keyform_begin + mesh.keyform_count.max(1)) {
                if let Some(pb) = b.art_mesh_keyform_positions.get(kf as usize) {
                    let pb = *pb as usize;
                    if pb + 2 * n_verts > b.keyform_positions.len() {
                        continue;
                    }
                    let (mut sx, mut sy) = (0.0f32, 0.0f32);
                    for k in 0..n_verts {
                        sx += b.keyform_positions[pb + 2 * k];
                        sy += b.keyform_positions[pb + 2 * k + 1];
                    }
                    xs.push(sx / n_verts as f32);
                    ys.push(sy / n_verts as f32);
                }
            }
        }
        if xs.is_empty() {
            [0.0, 0.0]
        } else {
            xs.sort_by(|a, c| a.partial_cmp(c).unwrap());
            ys.sort_by(|a, c| a.partial_cmp(c).unwrap());
            [xs[xs.len() / 2], ys[ys.len() / 2]]
        }
    };

    let mut skipped_bad = 0usize;
    let mut ready: Vec<Option<Ready>> = vec![None; b.art_meshes.len()];
    let mut keys: Vec<f32> = vec![0.0; b.art_meshes.len()];
    let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);

    // Mallas usadas como fuente de máscara: geometría auxiliar, no se dibujan.
    let mut es_mascara = vec![false; b.art_meshes.len()];
    for mesh in &b.art_meshes {
        let mb = mesh.mask_begin as usize;
        let mc = mesh.mask_count as usize;
        for k in mb..(mb + mc).min(b.drawable_masks.len()) {
            let idx = b.drawable_masks[k];
            if idx >= 0 && (idx as usize) < es_mascara.len() {
                es_mascara[idx as usize] = true;
            }
        }
    }

    for (i, mesh) in b.art_meshes.iter().enumerate() {
        let n_idx = mesh.vertex_count as usize; // entradas de índice (3 por triángulo)
        if n_idx == 0 || !mesh.visible || es_mascara[i] {
            continue;
        }
        let parent = mesh.parent_part;
        if parent >= 0 && b.part_visible.get(parent as usize).copied() == Some(false) {
            continue;
        }
        let ib = mesh.index_begin as usize;
        if ib + n_idx > b.indices.len() {
            continue;
        }
        let tri = &b.indices[ib..ib + n_idx];
        let n_verts = tri.iter().map(|v| (*v).max(0) as usize).max().unwrap_or(0) + 1;

        if std::env::var("L2D_DEBUG").is_ok() {
            let pb0 = b.art_mesh_keyform_positions.get(mesh.keyform_begin as usize).copied().unwrap_or(0) as usize;
            let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
            for k in 0..n_verts {
                if pb0 + 2 * k + 1 < b.keyform_positions.len() {
                    let (x, y) = (b.keyform_positions[pb0 + 2 * k], b.keyform_positions[pb0 + 2 * k + 1]);
                    x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x); y1 = y1.max(y);
                }
            }
            if (x1 - x0) * (y1 - y0) > 0.5 {
                eprintln!("malla {i:>3} ancho {:.2} alto {:.2}  x[{x0:.2},{x1:.2}] y[{y0:.2},{y1:.2}]", x1 - x0, y1 - y0);
            }
        }

        // Keyform de reposo: según L2D_KF (mediana | centro | primero).
        let kf = {
            let end = mesh.keyform_begin + mesh.keyform_count.max(1);
            match kf_mode.as_str() {
                "primero" => mesh.keyform_begin,
                "centro" => mesh.keyform_begin + mesh.keyform_count / 2,
                _ => {
                    let mut best = mesh.keyform_begin;
                    let mut best_d = f32::INFINITY;
                    for cand in mesh.keyform_begin..end {
                        if let Some(pb) = b.art_mesh_keyform_positions.get(cand as usize) {
                            let pb = *pb as usize;
                            if pb + 2 * n_verts > b.keyform_positions.len() {
                                continue;
                            }
                            let (mut sx, mut sy) = (0.0f32, 0.0f32);
                            for k in 0..n_verts {
                                sx += b.keyform_positions[pb + 2 * k];
                                sy += b.keyform_positions[pb + 2 * k + 1];
                            }
                            let c = [sx / n_verts as f32, sy / n_verts as f32];
                            let d = (c[0] - median[0]).powi(2) + (c[1] - median[1]).powi(2);
                            if d < best_d {
                                best_d = d;
                                best = cand;
                            }
                        }
                    }
                    best
                }
            }
        };
        let pos_begin = b
            .art_mesh_keyform_positions
            .get(kf as usize)
            .copied()
            .unwrap_or(0) as usize;
        let uv_begin = mesh.uv_begin as usize;
        if pos_begin + 2 * n_verts > b.keyform_positions.len()
            || uv_begin + 2 * n_verts > b.uvs.len()
        {
            continue;
        }
        let opacity = b
            .art_mesh_keyform_opacities
            .get(kf as usize)
            .copied()
            .unwrap_or(1.0);
        if opacity <= 0.0 {
            continue;
        }

        let mut verts = Vec::with_capacity(n_verts);
        let mut sane = true;
        for k in 0..n_verts {
            let p = [
                b.keyform_positions[pos_begin + 2 * k],
                b.keyform_positions[pos_begin + 2 * k + 1],
            ];
            let uv = [b.uvs[uv_begin + 2 * k], b.uvs[uv_begin + 2 * k + 1]];
            if p[0].abs() > 20000.0 || p[1].abs() > 20000.0 {
                sane = false;
            }
            min[0] = min[0].min(p[0]);
            min[1] = min[1].min(p[1]);
            max[0] = max[0].max(p[0]);
            max[1] = max[1].max(p[1]);
            verts.push((p, uv));
        }

        if !sane {
            skipped_bad += 1;
            continue;
        }

        let part_order = if parent >= 0 {
            b.part_keyform_begin
                .get(parent as usize)
                .and_then(|k| b.part_draw_order.get(*k as usize))
                .copied()
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let mesh_order = b
            .art_mesh_keyform_draw_orders
            .get(kf as usize)
            .copied()
            .unwrap_or(0.0);
        keys[i] = part_order + mesh_order / 10000.0;

        let mb = mesh.mask_begin as usize;
        let mc = mesh.mask_count as usize;
        let masks = if mc > 0 && mb + mc <= b.drawable_masks.len() {
            b.drawable_masks[mb..mb + mc]
                .iter()
                .map(|v| (*v).max(0) as usize)
                .filter(|v| *v < b.art_meshes.len())
                .collect()
        } else {
            Vec::new()
        };

        ready[i] = Some(Ready {
            verts,
            indices: tri.to_vec(),
            opacity,
            masks,
        });
    }

    if !min[0].is_finite() {
        return Err("ninguna malla dibujable".into());
    }

    // Mapeo verificado con el Core oficial:
    //   px = origin + pos * pixelsPerUnit  (el archivo guarda posiciones en unidades)
    let ppu = if m.canvas.pixels_per_unit > 0.0 { m.canvas.pixels_per_unit } else { cw };
    let (ox, oy) = (m.canvas.origin_x, m.canvas.origin_y);
    let to_screen = |p: [f32; 2]| -> [f32; 2] {
        [
            (ox + p[0]) * zoom,
            ih as f32 - (oy + p[1]) * zoom,
        ]
    };

    fn paint(
        canvas: &mut raster::Canvas,
        r: &Ready,
        mask: Option<&raster::Canvas>,
        tex: Option<&raster::Texture>,
        to_screen: &dyn Fn([f32; 2]) -> [f32; 2],
    ) {
        for tri in r.indices.chunks_exact(3) {
            let (a, c, d) = (
                tri[0].max(0) as usize,
                tri[1].max(0) as usize,
                tri[2].max(0) as usize,
            );
            if a >= r.verts.len() || c >= r.verts.len() || d >= r.verts.len() {
                continue;
            }
            let mk = |k: usize| {
                let (p, uv) = r.verts[k];
                let sp = to_screen(p);
                raster::Vertex::new(sp[0], sp[1], uv[0], uv[1])
                    .with_color(1.0, 1.0, 1.0, r.opacity)
            };
            canvas.triangle_masked([mk(a), mk(c), mk(d)], tex, raster::Blend::Normal, mask);
        }
    }

    let mut canvas = raster::Canvas::new(iw, ih);
    let fondo = std::env::var("L2D_BG").is_ok();
    canvas.clear(if fondo { [120, 120, 120, 255] } else { [0, 0, 0, 0] });

    let mut order: Vec<usize> = (0..ready.len()).filter(|i| ready[*i].is_some()).collect();
    // En Cubism, draw order mayor = más al frente.
    order.sort_by(|a, c| keys[*c].partial_cmp(&keys[*a]).unwrap_or(std::cmp::Ordering::Equal));

    let mut drawn = 0usize;
    let mut masked = 0usize;
    let total_meshes = order.len();
    for idx in order.clone() {
        let r = ready[idx].as_ref().unwrap();
        let mask_canvas = if r.masks.is_empty() {
            None
        } else {
            let mut mc = raster::Canvas::new(iw, ih);
            mc.clear([0, 0, 0, 0]);
            for mi in &r.masks {
                if let Some(mr) = ready[*mi].as_ref() {
                    paint(&mut mc, mr, None, None, &to_screen);
                }
            }
            masked += 1;
            Some(mc)
        };
        paint(&mut canvas, r, mask_canvas.as_ref(), textures.first(), &to_screen);
        drawn += r.indices.len() / 3;
    }

    if let Some(dir) = std::path::Path::new(out).parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    canvas.save_png(out)?;
    if skipped_bad > 0 {
        eprintln!("  ({skipped_bad} mallas descartadas por coordenadas fuera de rango)");
    }
    println!(
        "render {out}  {iw}x{ih}  {} mallas, {drawn} triángulos, {masked} con máscara  (canvas {:.0}x{:.0}, ppu {:.0})",
        total_meshes,
        cw,
        ch,
        ppu
    );
    Ok(())
}



// ---------------------------------------------------------------- params

/// Lista los parámetros del modelo: es la base de cualquier editor/animador.
fn params(path: &str) -> Result<(), Box<dyn Error>> {
    let bytes = std::fs::read(path)?;
    let m = moc3::Moc3::parse(&bytes)?;
    let b = m.body()?;
    println!("{:<28} {:>7} {:>7} {:>8}", "parámetro", "mín", "máx", "defecto");
    println!("{}", "-".repeat(54));
    for p in &b.parameters {
        println!("{:<28} {:>7.3} {:>7.3} {:>8.3}", p.id, p.min, p.max, p.default);
    }
    println!("\n{} parámetros", b.parameters.len());
    Ok(())
}

// ---------------------------------------------------------------- oracle

/// Renderiza a partir de un dump del Core oficial (geometría ya deformada).
/// Sirve para validar el rasterizador y como puente hasta que el parser propio
/// resuelva keyforms y deformadores.
fn render_oracle(json_path: &str, tex: &str, out: &str, zoom: f32) -> Result<(), Box<dyn Error>> {
    let json = std::fs::read_to_string(json_path)?;
    let v: serde_json::Value = serde_json::from_str(&json)?;
    let c = &v["canvas"];
    let (cw, ch, ox, oy, ppu) = (
        c["w"].as_f64().unwrap_or(0.0) as f32,
        c["h"].as_f64().unwrap_or(0.0) as f32,
        c["ox"].as_f64().unwrap_or(0.0) as f32,
        c["oy"].as_f64().unwrap_or(0.0) as f32,
        c["ppu"].as_f64().unwrap_or(1.0) as f32,
    );
    // Hiyori (y casi todos los modelos) usan varios atlas: texture_00, texture_01...
    // Se cargan todos los del directorio y cada malla elige por su índice.
    let tex_dir = std::path::Path::new(tex)
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or("ruta de textura inválida")?;
    let mut rutas: Vec<std::path::PathBuf> = std::fs::read_dir(&tex_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("texture") && n.ends_with(".png"))
                .unwrap_or(false)
        })
        .collect();
    rutas.sort();
    let mut textures: Vec<raster::Texture> = Vec::new();
    for r in &rutas {
        textures.push(raster::Texture::from_png(r)?);
    }
    if textures.is_empty() {
        textures.push(raster::Texture::from_png(tex)?);
    }
    let iw = (cw * zoom).round().max(1.0) as u32;
    let ih = (ch * zoom).round().max(1.0) as u32;

    struct M {
        pos: Vec<f32>,
        uv: Vec<f32>,
        idx: Vec<i32>,
        opacity: f32,
        order: i64,
        masks: Vec<usize>,
        texture: usize,
    }
    let mut meshes: Vec<M> = Vec::new();
    let arr = v["drawables"].as_array().ok_or("sin drawables")?;
    for d in arr {
        let f = |k: &str| d[k].as_array().cloned().unwrap_or_default();
        meshes.push(M {
            pos: f("pos").iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect(),
            uv: f("uv").iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect(),
            idx: f("idx").iter().filter_map(|x| x.as_i64()).map(|x| x as i32).collect(),
            opacity: d["opacity"].as_f64().unwrap_or(1.0) as f32,
            order: d["renderOrder"].as_i64().unwrap_or(0),
            masks: f("masks").iter().filter_map(|x| x.as_i64()).map(|x| x as usize).collect(),
            texture: d["texture"].as_i64().unwrap_or(0).max(0) as usize,
        });
    }

    let noflip = std::env::var("L2D_NOFLIP").is_ok();
    let plana = std::env::var("L2D_FLAT").is_ok();
    // Live2D guarda las UV con origen arriba-izquierda; el muestreo del atlas
    // necesita V invertida. Verificado: sin esto el modelo aparece "despiezado".
    let flipv = std::env::var("L2D_NOFLIPV").is_err();
    let to_screen = |x: f32, y: f32| -> [f32; 2] {
        let yy = (oy + y * ppu) * zoom;
        [(ox + x * ppu) * zoom, if noflip { yy } else { ih as f32 - yy }]
    };
    let paint = |canvas: &mut raster::Canvas, m: &M, mask: Option<&raster::Canvas>| {
        let texture = &textures[m.texture.min(textures.len() - 1)];
        let n = m.pos.len() / 2;
        for tri in m.idx.chunks_exact(3) {
            let mut vs = [raster::Vertex::new(0.0, 0.0, 0.0, 0.0); 3];
            let mut ok = true;
            for k in 0..3 {
                let i = tri[k].max(0) as usize;
                if i >= n || 2 * i + 1 >= m.pos.len() {
                    ok = false;
                    break;
                }
                let sp = to_screen(m.pos[2 * i], m.pos[2 * i + 1]);
                let (u, vv) = if 2 * i + 1 < m.uv.len() {
                    (m.uv[2 * i], if flipv { 1.0 - m.uv[2 * i + 1] } else { m.uv[2 * i + 1] })
                } else {
                    (0.0, 0.0)
                };
                vs[k] = raster::Vertex::new(sp[0], sp[1], u, vv)
                    .with_color(1.0, 1.0, 1.0, m.opacity);
            }
            if ok {
                let t = if plana { None } else { Some(texture) };
                canvas.triangle_masked(vs, t, raster::Blend::Normal, mask);
            }
        }
    };

    let mut canvas = raster::Canvas::new(iw, ih);
    let fondo = std::env::var("L2D_BG").is_ok();
    canvas.clear(if fondo { [120, 120, 120, 255] } else { [0, 0, 0, 0] });

    // Las mallas que sirven de máscara en Cubism son geometría auxiliar:
    // recortan a otras y NO se dibujan como arte.
    let mut es_mascara = vec![false; meshes.len()];
    for m in &meshes {
        for mi in &m.masks {
            if *mi < es_mascara.len() {
                es_mascara[*mi] = true;
            }
        }
    }
    let todas = std::env::var("L2D_ALL").is_ok();
    let mut order: Vec<usize> = (0..meshes.len())
        .filter(|i| !es_mascara[*i] && meshes[*i].opacity > 0.01)
        .filter(|i| todas || meshes[*i].opacity > 0.01)
        .collect();
    let al_reves = std::env::var("L2D_REVORDER").is_ok();
    order.sort_by_key(|i| if al_reves { -meshes[*i].order } else { meshes[*i].order });

    let mut drawn = 0;
    let mut masked = 0;
    for i in order {
        let m = &meshes[i];
        let sin_mascara = std::env::var("L2D_NOMASK").is_ok();
        let mask_canvas = if m.masks.is_empty() || sin_mascara {
            None
        } else {
            let mut mc = raster::Canvas::new(iw, ih);
            mc.clear([0, 0, 0, 0]);
            for mi in &m.masks {
                if let Some(mm) = meshes.get(*mi) {
                    paint(&mut mc, mm, None);
                }
            }
            masked += 1;
            Some(mc)
        };
        paint(&mut canvas, m, mask_canvas.as_ref());
        drawn += m.idx.len() / 3;
    }

    if let Some(dir) = std::path::Path::new(out).parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    canvas.save_png(out)?;
    println!("render(oracle) {out}  {iw}x{ih}  {drawn} triángulos, {masked} con máscara");
    Ok(())
}

// ---------------------------------------------------------------- demo

fn demo(out: &str) -> Result<(), Box<dyn Error>> {
    let mut canvas = raster::Canvas::new(640, 640);
    canvas.clear([18, 18, 22, 255]);

    // "Textura" del modelo: damero cálido/frío.
    let tex = raster::Texture::checker(128, 16, [235, 120, 90, 255], [60, 70, 110, 255]);

    // Malla 24×24 deformada con una onda: simula el resultado de un deformer warp.
    let (nx, ny) = (24u32, 24u32);
    let (x0, y0, w, h) = (80.0f32, 60.0f32, 480.0f32, 520.0f32);

    let disp = |u: f32, v: f32| -> [f32; 2] {
        let dx = (v * 6.0).sin() * 14.0 + (u * 9.0).cos() * 6.0;
        let dy = (u * 5.0).sin() * 12.0;
        [u * w + dx, v * h + dy]
    };

    for j in 0..ny {
        for i in 0..nx {
            let u0 = i as f32 / nx as f32;
            let u1 = (i + 1) as f32 / nx as f32;
            let v0 = j as f32 / ny as f32;
            let v1 = (j + 1) as f32 / ny as f32;

            let vert = |u: f32, v: f32| -> raster::Vertex {
                let p = disp(u, v);
                raster::Vertex::new(x0 + p[0], y0 + p[1], u, v)
            };

            let a = vert(u0, v0);
            let b = vert(u1, v0);
            let c = vert(u1, v1);
            let d = vert(u0, v1);

            canvas.triangle([a, b, c], Some(&tex), raster::Blend::Normal);
            canvas.triangle([a, c, d], Some(&tex), raster::Blend::Normal);
        }
    }

    // Capa aditiva: demuestra que el blending por modo funciona.
    filled_quad(&mut canvas, 0.0, 0.0, 640.0, 640.0, [0.85, 0.35, 0.15, 0.18], raster::Blend::Additive);

    // Marco multiplicativo, para ver el tercer modo.
    filled_quad(&mut canvas, 0.0, 0.0, 640.0, 24.0, [0.6, 0.7, 1.0, 0.9], raster::Blend::Multiplicative);

    if let Some(dir) = Path::new(out).parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    canvas.save_png(out)?;
    println!("demo escrito en {out} (640×640, {} triángulos)", nx * ny * 2);
    Ok(())
}

fn filled_quad(
    c: &mut raster::Canvas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    col: [f32; 4],
    mode: raster::Blend,
) {
    let v = |x: f32, y: f32| raster::Vertex::new(x, y, 0.0, 0.0).with_color(col[0], col[1], col[2], col[3]);
    c.triangle([v(x, y), v(x + w, y), v(x + w, y + h)], None, mode);
    c.triangle([v(x, y), v(x + w, y + h), v(x, y + h)], None, mode);
}
