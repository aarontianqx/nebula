use crate::domain::model::{Scene, Script};
use include_dir::{include_dir, Dir};
use serde::Deserialize;

// Embed the entire scenes and scripts directories at compile time
static SCENES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/resources/scenes");
static SCRIPTS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/resources/scripts");

/// Wrapper for scene files that use wardenly-go nested format
/// Format: { category: "...", scenes: [...] }
#[derive(Debug, Deserialize)]
struct SceneFile {
    category: String,
    scenes: Vec<SceneDefinition>,
}

/// Scene definition within a SceneFile (without category, which comes from parent)
#[derive(Debug, Deserialize)]
struct SceneDefinition {
    name: String,
    points: Vec<crate::domain::model::ColorPoint>,
    #[serde(default)]
    actions: std::collections::HashMap<String, crate::domain::model::SceneAction>,
}

/// Load all scene definitions from embedded resources
/// Automatically discovers all .yaml files in the scenes directory
pub fn load_scenes() -> anyhow::Result<Vec<Scene>> {
    let mut all_scenes = Vec::new();

    for file in SCENES_DIR.files() {
        let path = file.path();
        let extension = path.extension().and_then(|e| e.to_str());
        
        if !matches!(extension, Some("yaml") | Some("yml")) {
            continue;
        }

        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        match file.contents_utf8() {
            Some(content) => {
                match parse_scene_content(content) {
                    Ok(scenes) => {
                        tracing::debug!("Loaded {} scenes from {}", scenes.len(), file_name);
                        all_scenes.extend(scenes);
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse scene {}: {}", file_name, e);
                    }
                }
            }
            None => {
                tracing::error!("Scene file {} is not valid UTF-8", file_name);
            }
        }
    }

    tracing::info!("Loaded {} scenes total", all_scenes.len());
    Ok(all_scenes)
}

/// Parse scene content from YAML string (wardenly-go nested format)
fn parse_scene_content(content: &str) -> anyhow::Result<Vec<Scene>> {
    let scene_file: SceneFile = serde_yaml::from_str(content)?;

    // Convert SceneDefinitions to Scenes, adding category from parent
    let scenes = scene_file
        .scenes
        .into_iter()
        .map(|def| Scene {
            name: def.name,
            category: scene_file.category.clone(),
            points: def.points,
            actions: def.actions,
        })
        .collect();

    Ok(scenes)
}

/// Load all script definitions from embedded resources
/// Automatically discovers all .yaml files in the scripts directory
pub fn load_scripts() -> anyhow::Result<Vec<Script>> {
    let mut scripts = Vec::new();

    for file in SCRIPTS_DIR.files() {
        let path = file.path();
        let extension = path.extension().and_then(|e| e.to_str());
        
        if !matches!(extension, Some("yaml") | Some("yml")) {
            continue;
        }

        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        match file.contents_utf8() {
            Some(content) => {
                match serde_yaml::from_str::<Script>(content) {
                    Ok(script) => {
                        tracing::debug!("Loaded script: {}", file_name);
                        scripts.push(script);
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse script {}: {}", file_name, e);
                    }
                }
            }
            None => {
                tracing::error!("Script file {} is not valid UTF-8", file_name);
            }
        }
    }

    tracing::info!("Loaded {} scripts total", scripts.len());
    Ok(scripts)
}

/// Find a scene by name
pub fn find_scene<'a>(scenes: &'a [Scene], name: &str) -> Option<&'a Scene> {
    scenes.iter().find(|s| s.name == name)
}

/// Find a script by name
pub fn find_script<'a>(scripts: &'a [Script], name: &str) -> Option<&'a Script> {
    scripts.iter().find(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_scene() {
        let scenes = vec![
            Scene {
                name: "test_scene".to_string(),
                category: "test".to_string(),
                points: vec![],
                actions: Default::default(),
            },
            Scene {
                name: "another_scene".to_string(),
                category: "test".to_string(),
                points: vec![],
                actions: Default::default(),
            },
        ];

        assert!(find_scene(&scenes, "test_scene").is_some());
        assert!(find_scene(&scenes, "another_scene").is_some());
        assert!(find_scene(&scenes, "non_existent").is_none());
    }
}
