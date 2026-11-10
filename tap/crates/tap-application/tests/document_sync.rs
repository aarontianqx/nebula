//! The UI edits the canonical document directly (no lossy `Profile` side copy), so an edit can
//! never flatten parameterized steps. These tests pin that down, including the macro from the
//! original "rename silently destroys call_macro" bug report.

use tap_application::SessionStore;
use tap_core::{parse_yaml, MacroDocument};

/// A `call_macro` with an argument, a constant expression, a counter-driven expression and a
/// conditional. Plus metadata, a target window and a declared variable so every document-level
/// field is exercised.
const PARAMETERIZED: &str = r#"
name: parameterized
description: exercises every document-level field
author: tester
tags: [a, b]
variables:
  step:
    type: number
    default: 10
target_window:
  title: Notepad
  pause_when_unfocused: false
timeline:
- at_ms: 0
  action: !click
    x: 100
    y: 200
- at_ms: 500
  action: !call_macro
    name: login_sequence
    args:
      user: '{{ step }}'
- at_ms: 900
  action: !click
    x: '{{ 40 + 60 }}'
    y: 200
- at_ms: 1200
  action: !incr_counter
    key: i
- at_ms: 1500
  enabled: false
  note: counter driven
  action: !click
    x: '{{ i * 10 }}'
    y: 300
- at_ms: 2000
  action: !conditional
    condition: !counter
      key: i
      op: '>='
      value: 3
    then_action: !wait
      ms: 250
run:
  repeat: 3
  speed: 2.0
"#;

fn parameterized() -> MacroDocument {
    parse_yaml(PARAMETERIZED).expect("fixture should parse")
}

#[test]
fn an_update_stores_the_document_exactly_as_given() {
    let doc = parameterized();
    let mut session = SessionStore::new();

    let issues = session.update_document(doc.clone());

    assert!(issues.is_empty(), "unexpected findings: {issues:?}");
    assert_eq!(session.document(), &doc);
}

#[test]
fn renaming_a_parameterized_macro_keeps_every_step_and_expression() {
    // The original bug: any edit (even a rename) round-tripped through the resolved view, which
    // dropped `call_macro` steps and flattened `{{ }}` expressions into literals.
    let original = parameterized();
    let mut session = SessionStore::with_document(original.clone());

    let mut edited = session.document().clone();
    edited.name = "renamed".into();
    session.update_document(edited);

    let mut expected = original;
    expected.name = "renamed".into();
    assert_eq!(session.document(), &expected);
    assert_eq!(session.document().timeline.len(), 6);
}

#[test]
fn intermediate_editing_states_are_stored_and_reported_not_rejected() {
    let mut session = SessionStore::with_document(parameterized());

    let mut blank = session.document().clone();
    blank.name = String::new();
    blank.timeline.clear();
    let issues = session.update_document(blank.clone());

    assert_eq!(
        session.document(),
        &blank,
        "an invalid edit must still be stored"
    );
    let paths: Vec<&str> = issues.iter().map(|i| i.path.as_str()).collect();
    assert!(paths.contains(&"name"), "{paths:?}");
    assert!(paths.contains(&"timeline"), "{paths:?}");
}

#[test]
fn findings_describe_the_stored_document_not_the_previous_one() {
    let mut session = SessionStore::with_document(parameterized());

    let mut broken = session.document().clone();
    broken.name = String::new();
    assert!(!session.update_document(broken).is_empty());

    assert!(session.update_document(parameterized()).is_empty());
}
