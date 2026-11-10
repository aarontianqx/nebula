//! JSON contract between `MacroDocument` and the frontend's hand-written TypeScript types.
//!
//! The UI edits the document over IPC as JSON, so the serde shape *is* the protocol. This test
//! serialises a "kitchen sink" document that uses **every** action and condition variant to
//! `src/lib/__fixtures__/kitchen_sink.document.json`; the vitest suite reads the same file and
//! checks the TypeScript types against it. Drift on either side fails a test.
//!
//! Regenerate after an intentional model change:
//!
//! ```text
//! UPDATE_FIXTURES=1 cargo test -p tap-core --test document_json_contract
//! ```

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use serde_json::Value;
use tap_core::{
    CoordinateSpace, DslAction, DslCondition, DslMouseButton, DslRunConfig, DslTargetWindow,
    DslTimedAction, DslValue, MacroDocument, VariableDefinition, VariableType,
};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../src/lib/__fixtures__/kitchen_sink.document.json")
}

fn at(at_ms: u64, action: DslAction) -> DslTimedAction {
    DslTimedAction {
        at_ms,
        action,
        enabled: true,
        note: None,
    }
}

fn int(n: i64) -> DslValue {
    DslValue::Int(n)
}

fn expr(s: &str) -> DslValue {
    DslValue::String(s.into())
}

/// Every condition variant, nested where the model allows it.
fn conditions() -> Vec<DslCondition> {
    vec![
        DslCondition::WindowFocused {
            title: Some("Notepad".into()),
            process: Some("notepad.exe".into()),
        },
        DslCondition::WindowExists {
            title: Some("Calculator".into()),
            process: Some("calc.exe".into()),
        },
        DslCondition::PixelColor {
            x: 10,
            y: 20,
            color: "#FF8800".into(),
            tolerance: 12,
        },
        DslCondition::Counter {
            key: "i".into(),
            op: ">=".into(),
            value: 3,
        },
        DslCondition::Always,
        DslCondition::Never,
        DslCondition::And(vec![DslCondition::Always, DslCondition::Never]),
        DslCondition::Or(vec![DslCondition::Never, DslCondition::Always]),
        DslCondition::Not {
            condition: Box::new(DslCondition::Always),
        },
    ]
}

/// Every action variant. Control flow carries conditions; values cover int, float, expression and
/// bool literals.
fn actions() -> Vec<DslAction> {
    let button = DslMouseButton::Right;
    let mut args = HashMap::new();
    args.insert("who".to_string(), expr("{{ user }}"));
    vec![
        DslAction::Click {
            x: int(100),
            y: expr("{{ i * 10 }}"),
            button,
        },
        DslAction::DoubleClick {
            x: int(1),
            y: int(2),
            button: DslMouseButton::Left,
        },
        DslAction::MouseDown {
            x: int(3),
            y: int(4),
            button: DslMouseButton::Middle,
        },
        DslAction::MouseUp {
            x: int(3),
            y: int(4),
            button: DslMouseButton::Middle,
        },
        DslAction::MouseMove {
            x: DslValue::Float(1.5),
            y: int(6),
        },
        DslAction::Drag {
            from_x: int(1),
            from_y: int(2),
            to_x: int(3),
            to_y: int(4),
            duration_ms: 250,
        },
        DslAction::Scroll {
            delta_x: int(0),
            delta_y: int(-3),
        },
        DslAction::KeyTap {
            key: "Space".into(),
        },
        DslAction::KeyDown {
            key: "Shift".into(),
        },
        DslAction::KeyUp {
            key: "Shift".into(),
        },
        DslAction::TextInput {
            text: expr("hello {{ name }}"),
        },
        DslAction::Wait { ms: 500 },
        DslAction::WaitUntil {
            condition: DslCondition::Counter {
                key: "n".into(),
                op: "==".into(),
                value: 1,
            },
            timeout_ms: Some(5000),
            poll_interval_ms: 50,
        },
        DslAction::Conditional {
            condition: DslCondition::Always,
            then_action: Box::new(DslAction::Exit),
            else_action: Some(Box::new(DslAction::Wait { ms: 1 })),
        },
        DslAction::SetCounter {
            key: "i".into(),
            value: int(0),
        },
        DslAction::IncrCounter { key: "i".into() },
        DslAction::DecrCounter { key: "i".into() },
        DslAction::ResetCounter { key: "i".into() },
        DslAction::Exit,
        DslAction::CallMacro {
            name: "login".into(),
            args,
        },
    ]
}

/// Stable names for every variant. The matches are deliberately exhaustive: adding a variant to
/// the model breaks this file's compilation, which is the cue to extend the fixture above and the
/// TypeScript types.
fn action_kind(action: &DslAction) -> &'static str {
    match action {
        DslAction::Click { .. } => "click",
        DslAction::DoubleClick { .. } => "double_click",
        DslAction::MouseDown { .. } => "mouse_down",
        DslAction::MouseUp { .. } => "mouse_up",
        DslAction::MouseMove { .. } => "mouse_move",
        DslAction::Drag { .. } => "drag",
        DslAction::Scroll { .. } => "scroll",
        DslAction::KeyTap { .. } => "key_tap",
        DslAction::KeyDown { .. } => "key_down",
        DslAction::KeyUp { .. } => "key_up",
        DslAction::TextInput { .. } => "text_input",
        DslAction::Wait { .. } => "wait",
        DslAction::WaitUntil { .. } => "wait_until",
        DslAction::Conditional { .. } => "conditional",
        DslAction::SetCounter { .. } => "set_counter",
        DslAction::IncrCounter { .. } => "incr_counter",
        DslAction::DecrCounter { .. } => "decr_counter",
        DslAction::ResetCounter { .. } => "reset_counter",
        DslAction::Exit => "exit",
        DslAction::CallMacro { .. } => "call_macro",
    }
}

fn condition_kind(condition: &DslCondition) -> &'static str {
    match condition {
        DslCondition::WindowFocused { .. } => "window_focused",
        DslCondition::WindowExists { .. } => "window_exists",
        DslCondition::PixelColor { .. } => "pixel_color",
        DslCondition::Counter { .. } => "counter",
        DslCondition::Always => "always",
        DslCondition::Never => "never",
        DslCondition::And(_) => "and",
        DslCondition::Or(_) => "or",
        DslCondition::Not { .. } => "not",
    }
}

const ALL_ACTION_KINDS: [&str; 20] = [
    "click",
    "double_click",
    "mouse_down",
    "mouse_up",
    "mouse_move",
    "drag",
    "scroll",
    "key_tap",
    "key_down",
    "key_up",
    "text_input",
    "wait",
    "wait_until",
    "conditional",
    "set_counter",
    "incr_counter",
    "decr_counter",
    "reset_counter",
    "exit",
    "call_macro",
];

const ALL_CONDITION_KINDS: [&str; 9] = [
    "window_focused",
    "window_exists",
    "pixel_color",
    "counter",
    "always",
    "never",
    "and",
    "or",
    "not",
];

fn kitchen_sink() -> MacroDocument {
    let mut timeline: Vec<DslTimedAction> = actions()
        .into_iter()
        .enumerate()
        .map(|(i, action)| at(i as u64 * 100, action))
        .collect();
    // One step per condition variant, hosted by `wait_until`.
    timeline.extend(conditions().into_iter().enumerate().map(|(i, condition)| {
        at(
            3000 + i as u64 * 100,
            DslAction::WaitUntil {
                condition,
                timeout_ms: None,
                poll_interval_ms: 100,
            },
        )
    }));
    // Non-default step flags.
    timeline.push(DslTimedAction {
        at_ms: 9000,
        action: DslAction::Wait { ms: 1 },
        enabled: false,
        note: Some("disabled on purpose".into()),
    });

    let mut variables = HashMap::new();
    variables.insert(
        "user".to_string(),
        VariableDefinition {
            var_type: VariableType::String,
            default: Some(Value::String("alice".into())),
            description: Some("who to log in as".into()),
        },
    );

    MacroDocument {
        name: "kitchen sink".into(),
        description: Some("every action and condition".into()),
        version: "1.0".into(),
        author: Some("tests".into()),
        tags: vec!["fixture".into(), "contract".into()],
        variables,
        target_window: Some(DslTargetWindow {
            title: Some("Notepad".into()),
            process: Some("notepad.exe".into()),
            pause_when_unfocused: false,
            coordinates: CoordinateSpace::Window,
        }),
        timeline,
        run: DslRunConfig {
            repeat: 3,
            start_delay_ms: 2000,
            speed: 1.5,
        },
    }
}

/// Rebuild every object with keys inserted in sorted order, so the file does not depend on
/// `HashMap` iteration order or on whether `serde_json/preserve_order` is unified in.
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|k| (k.clone(), canonical(&map[k])))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

fn collect_condition_kinds(condition: &DslCondition, out: &mut BTreeSet<&'static str>) {
    out.insert(condition_kind(condition));
    match condition {
        DslCondition::And(children) | DslCondition::Or(children) => {
            children
                .iter()
                .for_each(|c| collect_condition_kinds(c, out));
        }
        DslCondition::Not { condition } => collect_condition_kinds(condition, out),
        _ => {}
    }
}

fn collect_kinds(
    action: &DslAction,
    actions: &mut BTreeSet<&'static str>,
    conditions: &mut BTreeSet<&'static str>,
) {
    actions.insert(action_kind(action));
    match action {
        DslAction::WaitUntil { condition, .. } => collect_condition_kinds(condition, conditions),
        DslAction::Conditional {
            condition,
            then_action,
            else_action,
        } => {
            collect_condition_kinds(condition, conditions);
            collect_kinds(then_action, actions, conditions);
            if let Some(other) = else_action {
                collect_kinds(other, actions, conditions);
            }
        }
        _ => {}
    }
}

#[test]
fn the_kitchen_sink_uses_every_action_and_condition_variant() {
    let doc = kitchen_sink();
    let (mut seen_actions, mut seen_conditions) = (BTreeSet::new(), BTreeSet::new());
    for step in &doc.timeline {
        collect_kinds(&step.action, &mut seen_actions, &mut seen_conditions);
    }

    let expected_actions: BTreeSet<&str> = ALL_ACTION_KINDS.into_iter().collect();
    let expected_conditions: BTreeSet<&str> = ALL_CONDITION_KINDS.into_iter().collect();
    assert_eq!(seen_actions, expected_actions);
    assert_eq!(seen_conditions, expected_conditions);
}

#[test]
fn the_json_fixture_matches_the_model() {
    let doc = kitchen_sink();
    let value = canonical(&serde_json::to_value(&doc).expect("document serialises"));
    let actual = format!("{}\n", serde_json::to_string_pretty(&value).unwrap());
    let path = fixture_path();

    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\nrun: UPDATE_FIXTURES=1 cargo test -p tap-core --test document_json_contract",
            path.display()
        )
    });
    assert_eq!(
        expected, actual,
        "the JSON shape of MacroDocument changed; if intentional run:\n\
         UPDATE_FIXTURES=1 cargo test -p tap-core --test document_json_contract\n\
         and update the TypeScript types in src/lib/types.ts"
    );
}

#[test]
fn the_json_fixture_deserialises_back_to_the_same_document() {
    // What the UI sends back over IPC must round-trip losslessly.
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        return; // the fixture is being rewritten by its sibling test right now
    }
    let text = std::fs::read_to_string(fixture_path())
        .expect("fixture exists (generate it with UPDATE_FIXTURES=1)");
    let parsed: MacroDocument = serde_json::from_str(&text).expect("fixture deserialises");
    assert_eq!(parsed, kitchen_sink());
}

#[test]
fn the_documents_the_ui_sends_are_accepted() {
    // The UI omits defaults, but also spells some `Option`s as explicit `null` (a cleared note, an
    // unset window title). Both must deserialize; `tags`, `variables` and `enabled` are never null.
    let sent = r#"{
        "name": "ui",
        "version": "1.0",
        "timeline": [
            { "at_ms": 0, "action": { "click": { "x": 1, "y": 2, "button": "left" } }, "enabled": true, "note": null },
            { "at_ms": 10, "action": { "wait_until": { "condition": "always", "timeout_ms": null, "poll_interval_ms": 100 } } },
            { "at_ms": 20, "action": { "conditional": { "condition": "never", "then_action": "exit", "else_action": null } } }
        ],
        "run": { "repeat": 0, "start_delay_ms": 3000, "speed": 1 },
        "target_window": { "title": null, "process": "calc.exe", "pause_when_unfocused": true }
    }"#;
    let doc: MacroDocument = serde_json::from_str(sent).expect("UI payload deserialises");

    assert_eq!(doc.timeline.len(), 3);
    assert_eq!(doc.timeline[0].note, None);
    assert!(doc.timeline[1].enabled, "absent `enabled` means enabled");
    let target = doc.target_window.expect("target window");
    assert_eq!(
        (target.title, target.process.as_deref()),
        (None, Some("calc.exe"))
    );
}
