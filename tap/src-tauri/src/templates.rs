//! Built-in macro templates.
//!
//! Templates are embedded into the binary at compile time so they ship inside
//! the packaged app with no external resource files to locate at runtime. Each
//! template is a canonical `MacroDocument` YAML; applying one replaces the
//! current session document (variables/metadata included).

use serde::Serialize;
use tap_core::parse_yaml;

struct BuiltinTemplate {
    id: &'static str,
    yaml: &'static str,
}

const TEMPLATES: &[BuiltinTemplate] = &[
    BuiltinTemplate {
        id: "simple_click",
        yaml: include_str!("../../templates/simple_click.yaml"),
    },
    BuiltinTemplate {
        id: "auto_type",
        yaml: include_str!("../../templates/auto_type.yaml"),
    },
    BuiltinTemplate {
        id: "loop_with_counter",
        yaml: include_str!("../../templates/loop_with_counter.yaml"),
    },
    BuiltinTemplate {
        id: "wait_for_color",
        yaml: include_str!("../../templates/wait_for_color.yaml"),
    },
];

/// Template summary for the frontend browse list.
#[derive(Debug, Clone, Serialize)]
pub struct TemplateInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

/// List all built-in templates (parsed for their display name/description).
pub fn list_templates() -> Vec<TemplateInfo> {
    TEMPLATES
        .iter()
        .filter_map(|t| {
            parse_yaml(t.yaml).ok().map(|doc| TemplateInfo {
                id: t.id.to_string(),
                name: doc.name,
                description: doc.description,
            })
        })
        .collect()
}

/// Return the raw YAML for a template id, if it exists.
pub fn template_yaml(id: &str) -> Option<&'static str> {
    TEMPLATES.iter().find(|t| t.id == id).map(|t| t.yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_templates_parse_validate_and_are_listed() {
        for t in TEMPLATES {
            let doc = tap_core::parse_yaml(t.yaml)
                .unwrap_or_else(|e| panic!("template `{}` failed to parse: {e}", t.id));
            tap_core::validate_profile(&doc).unwrap_or_else(|errs| {
                panic!(
                    "template `{}` failed validation: {}",
                    t.id,
                    errs.iter()
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            });
        }

        let listed = list_templates();
        assert_eq!(
            listed.len(),
            TEMPLATES.len(),
            "every embedded template must parse as a MacroDocument"
        );
        for info in &listed {
            assert!(
                !info.name.trim().is_empty(),
                "template name must be present"
            );
            assert!(
                template_yaml(&info.id).is_some(),
                "listed template id must resolve to YAML"
            );
        }
    }

    #[test]
    fn unknown_template_returns_none() {
        assert!(template_yaml("does_not_exist").is_none());
    }
}
