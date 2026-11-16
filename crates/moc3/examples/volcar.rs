//! Vuelca a JSON las secciones del `.moc3` que hacen falta para el motor de deformación.
//!
//! Sirve para investigar la semántica (orden de deformadores, mallas base, keyforms) fuera
//! del binario: el JSON se compara contra el oráculo del Core en `tools/`.
//!
//! ```text
//! cargo run -q -p moc3 --example volcar -- models/samples/Samples/Resources/Hiyori/Hiyori.moc3 > /tmp/hiyori.json
//! ```

use std::fmt::Write as _;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("uso: volcar <modelo.moc3>");
        std::process::exit(2);
    });
    let bytes = std::fs::read(&path).expect("no pude leer el archivo");
    let moc = moc3::Moc3::parse(&bytes).expect("no pude parsear el .moc3");

    let mut out = String::with_capacity(1 << 22);
    out.push_str("{\n");

    // conteos
    out.push_str("\"conteos\": {");
    for (i, (nombre, valor)) in moc.counts.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "\"{nombre}\":{valor}");
    }
    out.push_str("},\n");

    let f32s = [
        "keyform_position.xys",
        "uv.xys",
        "keys.values",
        "rotation_deformer.base_angles",
        "rotation_deformer_keyform.angles",
        "rotation_deformer_keyform.origin_xs",
        "rotation_deformer_keyform.origin_ys",
        "rotation_deformer_keyform.scales",
        "parameter.min_values",
        "parameter.max_values",
        "parameter.default_values",
        "part_keyform.draw_orders",
        "art_mesh_keyform.draw_orders",
        "art_mesh_keyform.opacities",
        "warp_deformer_keyform.opacities",
        "rotation_deformer_keyform.opacities",
    ];
    let i32s = [
        "deformer.types",
        "deformer.parent_deformer_indices",
        "deformer.specific_indices",
        "deformer.keyform_binding_band_indices",
        "warp_deformer.rows",
        "warp_deformer.cols",
        "warp_deformer.vertex_counts",
        "warp_deformer.keyform_begin_indices",
        "warp_deformer.keyform_counts",
        "warp_deformer.keyform_binding_band_indices",
        "warp_deformer_keyform.keyform_position_begin_indices",
        "rotation_deformer.keyform_begin_indices",
        "rotation_deformer.keyform_counts",
        "rotation_deformer.keyform_binding_band_indices",
        "art_mesh.parent_deformer_indices",
        "art_mesh.parent_part_indices",
        "art_mesh.keyform_begin_indices",
        "art_mesh.keyform_counts",
        "art_mesh.vertex_counts",
        "art_mesh.keyform_binding_band_indices",
        "art_mesh_keyform.keyform_position_begin_indices",
        "art_mesh.uv_begin_indices",
        "art_mesh.position_index_begin_indices",
        "art_mesh.position_index_counts",
        "art_mesh.texture_indices",
        "art_mesh.mask_begin_indices",
        "art_mesh.mask_counts",
        "parameter.keyform_binding_begin_indices",
        "parameter.keyform_binding_counts",
        "keyform_binding_index.indices",
        "keyform_binding_band.begin_indices",
        "keyform_binding_band.counts",
        "keyform_binding.keys_begin_indices",
        "keyform_binding.keys_counts",
    ];
    let ids = ["deformer.ids", "art_mesh.ids", "parameter.ids", "part.ids"];

    for nombre in i32s {
        match moc.read_i32_section(nombre) {
            Ok(v) => {
                out.push('"');
                out.push_str(nombre);
                out.push_str("\":[");
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    let _ = write!(out, "{x}");
                }
                out.push_str("],\n");
            }
            Err(e) => {
                out.push('"');
                out.push_str(nombre);
                let _ = writeln!(out, "\": null, // {e}");
            }
        }
    }
    for nombre in f32s {
        match moc.read_f32_section(nombre) {
            Ok(v) => {
                out.push('"');
                out.push_str(nombre);
                out.push_str("\":[");
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    // redondeo a 6 decimales: sobra para comparar contra el oráculo
                    let y = (*x as f64 * 1e6).round() / 1e6;
                    let _ = write!(out, "{y}");
                }
                out.push_str("],\n");
            }
            Err(e) => {
                out.push('"');
                out.push_str(nombre);
                let _ = writeln!(out, "\": null, // {e}");
            }
        }
    }
    for nombre in ids {
        match moc.read_ids_section(nombre) {
            Ok(v) => {
                out.push('"');
                out.push_str(nombre);
                out.push_str("\":[");
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    let _ = write!(out, "\"{}\"", x.replace('\\', "\\\\").replace('"', "\\\""));
                }
                out.push_str("],\n");
            }
            Err(e) => {
                out.push('"');
                out.push_str(nombre);
                let _ = writeln!(out, "\": null, // {e}");
            }
        }
    }
    // uv.xys ya se leyó como i32 (mismo error): se relee como f32 si quedó null
    out.push_str("\"_fin\": true\n}\n");
    print!("{out}");
}
