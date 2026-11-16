//! Lector de `model3.json`: el manifiesto que dice qué archivos componen un modelo.
//!
//! Estructura según la documentación pública del formato. Los campos opcionales se
//! modelan como `Option` para aceptar tanto exportaciones mínimas como completas.

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct Model3 {
    #[serde(default)]
    pub version: Option<serde_json::Value>,
    #[serde(default)]
    pub file_references: FileReferences,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub hit_areas: Vec<String>,
    #[serde(default)]
    pub layout: Option<Layout>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct FileReferences {
    #[serde(default)]
    pub moc: Option<String>,
    #[serde(default)]
    pub textures: Vec<String>,
    #[serde(default)]
    pub physics: Option<String>,
    #[serde(default)]
    pub pose: Option<String>,
    #[serde(default)]
    pub display_info: Option<String>,
    #[serde(default)]
    pub user_data: Option<String>,
    #[serde(default)]
    pub expressions: Vec<ExpressionRef>,
    #[serde(default)]
    pub motions: BTreeMap<String, Vec<MotionRef>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpressionRef {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "File")]
    pub file: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MotionRef {
    #[serde(rename = "File")]
    pub file: String,
    #[serde(rename = "FadeInTime", default)]
    pub fade_in_time: Option<f32>,
    #[serde(rename = "FadeOutTime", default)]
    pub fade_out_time: Option<f32>,
    #[serde(rename = "Sound", default)]
    pub sound: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Group {
    #[serde(rename = "Target", default)]
    pub target: Option<String>,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Ids", default)]
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct Layout {
    #[serde(default)]
    pub center_x: f32,
    #[serde(default)]
    pub center_y: f32,
    #[serde(default)]
    pub width: f32,
}

impl Model3 {
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Todos los archivos que el manifiesto referencia, en orden de carga razonable.
    pub fn asset_paths(&self) -> Vec<String> {
        let mut out = Vec::new();
        let r = &self.file_references;
        if let Some(m) = &r.moc {
            out.push(m.clone());
        }
        out.extend(r.textures.iter().cloned());
        if let Some(p) = &r.physics {
            out.push(p.clone());
        }
        if let Some(p) = &r.pose {
            out.push(p.clone());
        }
        if let Some(d) = &r.display_info {
            out.push(d.clone());
        }
        if let Some(u) = &r.user_data {
            out.push(u.clone());
        }
        out.extend(r.expressions.iter().map(|e| e.file.clone()));
        for motions in r.motions.values() {
            out.extend(motions.iter().map(|m| m.file.clone()));
        }
        out
    }

    pub fn motion_count(&self) -> usize {
        self.file_references.motions.values().map(Vec::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "Version": 3,
      "FileReferences": {
        "Moc": "haru.moc3",
        "Textures": ["haru.2048/texture_00.png"],
        "Physics": "haru.physics3.json",
        "Pose": "haru.pose3.json",
        "DisplayInfo": "haru.cdi3.json",
        "Expressions": [
          { "Name": "f01", "File": "expressions/f01.exp3.json" }
        ],
        "Motions": {
          "Idle": [
            { "File": "motions/idle_01.motion3.json", "FadeInTime": 0.5, "FadeOutTime": 1.0 }
          ],
          "Tap": [
            { "File": "motions/tap_01.motion3.json", "Sound": "sounds/tap.wav" }
          ]
        }
      },
      "Groups": [
        { "Target": "Parameter", "Name": "EyeBlink", "Ids": ["ParamEyeLOpen", "ParamEyeROpen"] }
      ],
      "HitAreas": ["Head", "Body"]
    }"#;

    #[test]
    fn parsea_manifiesto_completo() {
        let m = Model3::parse(SAMPLE).expect("parse");
        assert_eq!(m.file_references.moc.as_deref(), Some("haru.moc3"));
        assert_eq!(m.file_references.textures.len(), 1);
        assert_eq!(m.groups.len(), 1);
        assert_eq!(m.groups[0].name, "EyeBlink");
        assert_eq!(m.groups[0].ids.len(), 2);
        assert_eq!(m.hit_areas, vec!["Head", "Body"]);
    }

    #[test]
    fn cuenta_movimientos_y_assets() {
        let m = Model3::parse(SAMPLE).expect("parse");
        assert_eq!(m.motion_count(), 2);
        let paths = m.asset_paths();
        assert!(paths.contains(&"haru.moc3".to_string()));
        assert!(paths.contains(&"haru.physics3.json".to_string()));
        assert!(paths.contains(&"expressions/f01.exp3.json".to_string()));
        // 1 moc + 1 textura + physics + pose + displayinfo + 1 expresión + 2 motions
        assert_eq!(paths.len(), 8);
    }

    #[test]
    fn acepta_manifiesto_minimo() {
        let m = Model3::parse(r#"{"FileReferences":{"Moc":"a.moc3","Textures":[]}}"#).expect("min");
        assert_eq!(m.file_references.moc.as_deref(), Some("a.moc3"));
        assert!(m.file_references.physics.is_none());
        assert_eq!(m.motion_count(), 0);
    }
}
