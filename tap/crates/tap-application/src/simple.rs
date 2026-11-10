//! The "simple repeat" tool: one action repeated at a fixed interval, expressed as an ordinary
//! macro so the engine needs no special mode.

use tap_core::{DslAction, DslRunConfig, DslTimedAction, MacroDocument, DSL_VERSION};

/// A macro that performs `action`, waits `interval_ms`, and repeats (`None` repeats forever).
pub fn simple_repeat_document(
    action: DslAction,
    interval_ms: u64,
    repeat: Option<u32>,
    countdown_ms: u64,
) -> MacroDocument {
    MacroDocument {
        name: "Simple Repeat".into(),
        description: None,
        version: DSL_VERSION.into(),
        author: None,
        tags: Vec::new(),
        variables: Default::default(),
        target_window: None,
        timeline: vec![
            DslTimedAction::new(0, action),
            DslTimedAction::new(0, DslAction::Wait { ms: interval_ms }),
        ],
        run: DslRunConfig {
            repeat: repeat.unwrap_or(0),
            start_delay_ms: countdown_ms,
            ..DslRunConfig::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tap_core::MouseButton;

    #[test]
    fn performs_the_action_then_waits_the_interval() {
        let document = simple_repeat_document(
            DslAction::click(1, 2, MouseButton::Left),
            250,
            Some(3),
            2000,
        );

        assert_eq!(document.timeline.len(), 2);
        assert_eq!(
            document.timeline[0].action,
            DslAction::click(1, 2, MouseButton::Left)
        );
        assert_eq!(document.timeline[1].action, DslAction::Wait { ms: 250 });
        assert_eq!(document.run.repeat, 3);
        assert_eq!(document.run.start_delay_ms, 2000);
    }

    #[test]
    fn no_repeat_count_repeats_forever() {
        let document = simple_repeat_document(DslAction::Exit, 10, None, 0);
        assert_eq!(document.run.repeat, 0);
    }
}
