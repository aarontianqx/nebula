use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// `None` when the position cannot be read.
pub fn cursor_position() -> Option<(i32, i32)> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let location = CGEvent::new(source).ok()?.location();
    Some((location.x as i32, location.y as i32))
}
