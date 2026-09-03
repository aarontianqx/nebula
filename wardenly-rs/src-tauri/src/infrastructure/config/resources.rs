use crate::domain::model::{ProtocolScript, Scene, Script, Task};
use include_dir::{include_dir, Dir};
use serde::Deserialize;

// Embed the entire scenes and scripts directories at compile time
static SCENES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/resources/scenes");
static SCRIPTS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/resources/scripts");
static PROTOCOLS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/resources/protocols");
static TASKS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/resources/tasks");

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

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        match file.contents_utf8() {
            Some(content) => match parse_scene_content(content) {
                Ok(scenes) => {
                    tracing::debug!("Loaded {} scenes from {}", scenes.len(), file_name);
                    all_scenes.extend(scenes);
                }
                Err(e) => {
                    tracing::error!("Failed to parse scene {}: {}", file_name, e);
                }
            },
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

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        match file.contents_utf8() {
            Some(content) => match serde_yaml::from_str::<Script>(content) {
                Ok(script) => {
                    tracing::debug!("Loaded script: {}", file_name);
                    scripts.push(script);
                }
                Err(e) => {
                    tracing::error!("Failed to parse script {}: {}", file_name, e);
                }
            },
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

/// Load all task definitions (unified schema v2) from embedded resources
/// Automatically discovers all .yaml files in the tasks directory
pub fn load_tasks() -> anyhow::Result<Vec<Task>> {
    let mut tasks = Vec::new();

    for file in TASKS_DIR.files() {
        let path = file.path();
        let extension = path.extension().and_then(|e| e.to_str());

        if !matches!(extension, Some("yaml") | Some("yml")) {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        match file.contents_utf8() {
            Some(content) => match serde_yaml::from_str::<Task>(content) {
                Ok(task) => {
                    tracing::debug!("Loaded task: {}", file_name);
                    tasks.push(task);
                }
                Err(e) => {
                    tracing::error!("Failed to parse task {}: {}", file_name, e);
                }
            },
            None => {
                tracing::error!("Task file {} is not valid UTF-8", file_name);
            }
        }
    }

    tracing::info!("Loaded {} tasks total", tasks.len());
    Ok(tasks)
}

/// Find a task by name
pub fn find_task<'a>(tasks: &'a [Task], name: &str) -> Option<&'a Task> {
    tasks.iter().find(|t| t.name == name)
}

/// Load all protocol script definitions from embedded resources
/// Automatically discovers all .yaml files in the protocols directory
pub fn load_protocol_scripts() -> anyhow::Result<Vec<ProtocolScript>> {
    let mut scripts = Vec::new();

    for file in PROTOCOLS_DIR.files() {
        let path = file.path();
        let extension = path.extension().and_then(|e| e.to_str());

        if !matches!(extension, Some("yaml") | Some("yml")) {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        match file.contents_utf8() {
            Some(content) => match serde_yaml::from_str::<ProtocolScript>(content) {
                Ok(script) => {
                    tracing::debug!("Loaded protocol script: {}", file_name);
                    scripts.push(script);
                }
                Err(e) => {
                    tracing::error!("Failed to parse protocol script {}: {}", file_name, e);
                }
            },
            None => {
                tracing::error!("Protocol script file {} is not valid UTF-8", file_name);
            }
        }
    }

    tracing::info!("Loaded {} protocol scripts total", scripts.len());
    Ok(scripts)
}

/// Find a protocol script by name
pub fn find_protocol_script<'a>(
    scripts: &'a [ProtocolScript],
    name: &str,
) -> Option<&'a ProtocolScript> {
    scripts.iter().find(|s| s.name == name)
}

/// Protocol name → id registry extracted from a game bundle.
/// Used to validate protocol scripts at load time; the bridge itself resolves
/// names in-page, so a stale registry only affects validation, not execution.
#[derive(Debug, Clone, Deserialize)]
pub struct ProtocolRegistry {
    /// Game bundle version this registry was extracted from
    pub bundle_version: String,
    /// Protocol name → numeric id
    pub protocols: std::collections::HashMap<String, u32>,
}

impl ProtocolRegistry {
    pub fn contains(&self, name: &str) -> bool {
        self.protocols.contains_key(name)
    }
}

/// Load the protocol registry (resources/protocols/registry.json).
/// Returns None when missing or unparsable — validation is then skipped.
pub fn load_protocol_registry() -> Option<ProtocolRegistry> {
    let file = PROTOCOLS_DIR.get_file("registry.json")?;
    let content = file.contents_utf8()?;
    match serde_json::from_str::<ProtocolRegistry>(content) {
        Ok(registry) => {
            tracing::info!(
                "Loaded protocol registry (bundle {}, {} protocols)",
                registry.bundle_version,
                registry.protocols.len()
            );
            Some(registry)
        }
        Err(e) => {
            tracing::error!("Failed to parse protocol registry: {}", e);
            None
        }
    }
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

    /// Every protocol name referenced by embedded task templates must exist
    /// in the registry — catches typos at build time instead of at task start
    /// on a user's machine.
    #[test]
    fn task_templates_reference_known_protocols() {
        use crate::domain::model::TaskAction;
        let registry = load_protocol_registry().expect("registry must load");
        let tasks = load_tasks().expect("tasks must load");
        assert!(!tasks.is_empty(), "expected at least one embedded task");

        fn collect(actions: &[TaskAction], out: &mut Vec<String>) {
            for action in actions {
                match action {
                    TaskAction::SendProtocol { protocol, .. } => out.push(protocol.clone()),
                    TaskAction::Request {
                        protocol,
                        expect,
                        expect_any,
                        ..
                    } => {
                        out.push(protocol.clone());
                        out.extend(expect.iter().cloned());
                        out.extend(expect_any.iter().cloned());
                    }
                    TaskAction::WaitProtocol { protocol, .. } => out.push(protocol.clone()),
                    TaskAction::Loop { actions, .. } => collect(actions, out),
                    _ => {}
                }
            }
        }

        for task in tasks {
            let mut names = Vec::new();
            for step in &task.steps {
                collect(&step.actions, &mut names);
            }
            let unknown: Vec<_> = names
                .into_iter()
                .filter(|n| !registry.contains(n))
                .collect();
            assert!(
                unknown.is_empty(),
                "task '{}' references unknown protocols: {:?}",
                task.name,
                unknown
            );
        }
    }
}
