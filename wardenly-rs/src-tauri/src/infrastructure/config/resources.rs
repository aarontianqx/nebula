use crate::domain::model::{Scene, Script};
use std::fs;
use std::path::Path;

/// Load all scene definitions from the resources/scenes directory
pub fn load_scenes() -> anyhow::Result<Vec<Scene>> {
    load_yaml_resources::<Scene>("resources/scenes")
}

/// Load all script definitions from the resources/scripts directory
pub fn load_scripts() -> anyhow::Result<Vec<Script>> {
    load_yaml_resources::<Script>("resources/scripts")
}

/// Generic YAML resource loader
fn load_yaml_resources<T: serde::de::DeserializeOwned>(dir_path: &str) -> anyhow::Result<Vec<T>> {
    let path = Path::new(dir_path);
    let mut resources = Vec::new();

    if !path.exists() {
        tracing::warn!("Resource directory not found: {}", dir_path);
        return Ok(resources);
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_path = entry.path();

        if !file_path.is_file() {
            continue;
        }

        let extension = file_path.extension().and_then(|e| e.to_str());
        if !matches!(extension, Some("yaml") | Some("yml")) {
            continue;
        }

        match load_yaml_file::<T>(&file_path) {
            Ok(resource) => {
                tracing::debug!("Loaded resource from {:?}", file_path);
                resources.push(resource);
            }
            Err(e) => {
                tracing::error!("Failed to load resource {:?}: {}", file_path, e);
            }
        }
    }

    tracing::info!("Loaded {} resources from {}", resources.len(), dir_path);
    Ok(resources)
}

/// Load a single YAML file
fn load_yaml_file<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let content = fs::read_to_string(path)?;
    let resource: T = serde_yaml::from_str(&content)?;
    Ok(resource)
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

