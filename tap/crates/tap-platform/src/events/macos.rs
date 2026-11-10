//! macOS-native global event listening.
//!
//! This module provides a custom implementation of global event listening for macOS
//! that avoids the thread-safety issues in rdev's keyboard character resolution.
//!
//! The issue: rdev calls TSMGetInputSourceProperty which must be called on the main thread,
//! but rdev::listen runs its callback in a background thread's CFRunLoop.
//!
//! Our solution: Use CGEventTap directly and skip keyboard character resolution entirely.
//! We only need the keycode (which we can map to key names ourselves), not the actual character.
//!
//! IMPORTANT: This module uses a SINGLETON pattern. Only ONE event listener runs at a time,
//! and multiple subscribers can receive events from it. This avoids issues with multiple
//! CGEventTaps running simultaneously.

use core_foundation::base::TCFType;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop, CFRunLoopSource};
use core_graphics::event::{
    CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::ffi::c_void;
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use tracing::{debug, error, info};

// FFI declarations for functions not exposed by core-graphics crate
type CFMachPortRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFAllocatorRef = *const c_void;
type CFIndex = i64;
type CGEventRef = *mut c_void;
type CGEventFlags = u64;

// CGPoint structure for mouse position
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

// Event field constants
const KEYBOARD_EVENT_KEYCODE: u32 = 9;
const SCROLL_WHEEL_EVENT_DELTA_AXIS_1: u32 = 11;
const SCROLL_WHEEL_EVENT_DELTA_AXIS_2: u32 = 12;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallback,
        user_info: *mut c_void,
    ) -> CFMachPortRef;

    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);

    fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> CGEventFlags;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
}

type CGEventTapCallback = extern "C" fn(
    proxy: *mut c_void,
    event_type: CGEventType,
    cg_event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

/// Raw event types from macOS.
#[derive(Debug, Clone)]
pub enum MacOSEventType {
    MouseMove { x: f64, y: f64 },
    MouseDown { x: f64, y: f64, button: u8 },
    MouseUp { x: f64, y: f64, button: u8 },
    Scroll { delta_x: i64, delta_y: i64 },
    KeyDown { keycode: u16 },
    KeyUp { keycode: u16 },
    FlagsChanged { keycode: u16, flags: u64 },
}

/// A macOS event with timestamp.
#[derive(Debug, Clone)]
pub struct MacOSEvent {
    pub event_type: MacOSEventType,
    pub timestamp_ms: u64,
}

/// Global singleton for the event listener (using OnceLock for safe initialization)
static GLOBAL_LISTENER: OnceLock<Arc<GlobalEventListener>> = OnceLock::new();

/// Thread-safe list of event subscribers
struct GlobalEventListener {
    /// Keep the receiver alive to prevent channel from closing
    _broadcast_rx: Receiver<MacOSEvent>,
    /// List of active subscriber senders
    subscribers: Mutex<Vec<Sender<MacOSEvent>>>,
}

impl GlobalEventListener {
    fn new() -> Arc<Self> {
        let (broadcast_tx, broadcast_rx) = bounded::<MacOSEvent>(2048);

        let listener = Arc::new(Self {
            _broadcast_rx: broadcast_rx.clone(),
            subscribers: Mutex::new(Vec::new()),
        });

        // Spawn the broadcast thread that distributes events to subscribers
        let subscribers_ref = Arc::downgrade(&listener);
        let rx = broadcast_rx;
        thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                if let Some(listener) = subscribers_ref.upgrade() {
                    let subs = listener.subscribers.lock().unwrap();
                    for sub in subs.iter() {
                        let _ = sub.try_send(event.clone());
                    }
                } else {
                    break;
                }
            }
        });

        // Spawn the event tap thread
        let tx = broadcast_tx;
        thread::spawn(move || {
            info!("Global macOS event listener thread starting");
            if let Err(e) = run_event_tap(tx) {
                error!("Event tap error: {}", e);
            }
            info!("Global macOS event listener thread exiting");
        });

        listener
    }

    fn subscribe(&self) -> Receiver<MacOSEvent> {
        let (tx, rx) = bounded::<MacOSEvent>(1024);
        let mut subs = self.subscribers.lock().unwrap();
        subs.push(tx);
        rx
    }

    fn cleanup_dead_subscribers(&self) {
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|s| !s.is_full());
    }
}

/// Get or create the global event listener
fn get_global_listener() -> Arc<GlobalEventListener> {
    GLOBAL_LISTENER
        .get_or_init(GlobalEventListener::new)
        .clone()
}

// Thread-local storage for the event tap callback
thread_local! {
    static EVENT_SENDER: std::cell::RefCell<Option<Sender<MacOSEvent>>> = const { std::cell::RefCell::new(None) };
}

/// Callback function for CGEventTap.
/// This runs in the CFRunLoop thread.
extern "C" fn event_tap_callback(
    _proxy: *mut c_void,
    event_type: CGEventType,
    cg_event: CGEventRef,
    _user_info: *mut c_void,
) -> CGEventRef {
    // Use raw FFI to read event data - no ownership, no Drop
    let macos_event = convert_event_raw(event_type, cg_event);

    if let Some(evt) = macos_event {
        EVENT_SENDER.with(|sender| {
            if let Some(ref tx) = *sender.borrow() {
                let _ = tx.try_send(evt);
            }
        });
    }

    // Return the event unchanged (we're just listening, not modifying)
    cg_event
}

/// Convert CGEvent to our event type using raw FFI (no Drop issues).
fn convert_event_raw(event_type: CGEventType, event: CGEventRef) -> Option<MacOSEvent> {
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let evt_type = match event_type {
        CGEventType::MouseMoved
        | CGEventType::LeftMouseDragged
        | CGEventType::RightMouseDragged => {
            let loc = unsafe { CGEventGetLocation(event) };
            Some(MacOSEventType::MouseMove { x: loc.x, y: loc.y })
        }
        CGEventType::LeftMouseDown => {
            let loc = unsafe { CGEventGetLocation(event) };
            Some(MacOSEventType::MouseDown {
                x: loc.x,
                y: loc.y,
                button: 0,
            })
        }
        CGEventType::LeftMouseUp => {
            let loc = unsafe { CGEventGetLocation(event) };
            Some(MacOSEventType::MouseUp {
                x: loc.x,
                y: loc.y,
                button: 0,
            })
        }
        CGEventType::RightMouseDown => {
            let loc = unsafe { CGEventGetLocation(event) };
            Some(MacOSEventType::MouseDown {
                x: loc.x,
                y: loc.y,
                button: 1,
            })
        }
        CGEventType::RightMouseUp => {
            let loc = unsafe { CGEventGetLocation(event) };
            Some(MacOSEventType::MouseUp {
                x: loc.x,
                y: loc.y,
                button: 1,
            })
        }
        CGEventType::OtherMouseDown => {
            let loc = unsafe { CGEventGetLocation(event) };
            Some(MacOSEventType::MouseDown {
                x: loc.x,
                y: loc.y,
                button: 2,
            })
        }
        CGEventType::OtherMouseUp => {
            let loc = unsafe { CGEventGetLocation(event) };
            Some(MacOSEventType::MouseUp {
                x: loc.x,
                y: loc.y,
                button: 2,
            })
        }
        CGEventType::ScrollWheel => {
            let delta_y =
                unsafe { CGEventGetIntegerValueField(event, SCROLL_WHEEL_EVENT_DELTA_AXIS_1) };
            let delta_x =
                unsafe { CGEventGetIntegerValueField(event, SCROLL_WHEEL_EVENT_DELTA_AXIS_2) };
            Some(MacOSEventType::Scroll { delta_x, delta_y })
        }
        CGEventType::KeyDown => {
            let keycode =
                unsafe { CGEventGetIntegerValueField(event, KEYBOARD_EVENT_KEYCODE) } as u16;
            Some(MacOSEventType::KeyDown { keycode })
        }
        CGEventType::KeyUp => {
            let keycode =
                unsafe { CGEventGetIntegerValueField(event, KEYBOARD_EVENT_KEYCODE) } as u16;
            Some(MacOSEventType::KeyUp { keycode })
        }
        CGEventType::FlagsChanged => {
            let keycode =
                unsafe { CGEventGetIntegerValueField(event, KEYBOARD_EVENT_KEYCODE) } as u16;
            let flags = unsafe { CGEventGetFlags(event) };
            Some(MacOSEventType::FlagsChanged { keycode, flags })
        }
        _ => None,
    };

    evt_type.map(|et| MacOSEvent {
        event_type: et,
        timestamp_ms,
    })
}

/// Run the CGEventTap (internal function).
fn run_event_tap(sender: Sender<MacOSEvent>) -> Result<(), String> {
    // Store sender in thread-local storage
    EVENT_SENDER.with(|s| {
        *s.borrow_mut() = Some(sender);
    });

    // Create event mask for all events we care about
    let event_mask: u64 = (1 << CGEventType::LeftMouseDown as u64)
        | (1 << CGEventType::LeftMouseUp as u64)
        | (1 << CGEventType::RightMouseDown as u64)
        | (1 << CGEventType::RightMouseUp as u64)
        | (1 << CGEventType::OtherMouseDown as u64)
        | (1 << CGEventType::OtherMouseUp as u64)
        | (1 << CGEventType::MouseMoved as u64)
        | (1 << CGEventType::LeftMouseDragged as u64)
        | (1 << CGEventType::RightMouseDragged as u64)
        | (1 << CGEventType::KeyDown as u64)
        | (1 << CGEventType::KeyUp as u64)
        | (1 << CGEventType::FlagsChanged as u64)
        | (1 << CGEventType::ScrollWheel as u64);

    // Create the event tap
    let tap = unsafe {
        CGEventTapCreate(
            CGEventTapLocation::HID as u32,
            CGEventTapPlacement::HeadInsertEventTap as u32,
            CGEventTapOptions::ListenOnly as u32,
            event_mask,
            event_tap_callback,
            ptr::null_mut(),
        )
    };

    if tap.is_null() {
        error!("Failed to create event tap - accessibility permission may not be granted");
        return Err("Failed to create event tap".to_string());
    }

    debug!("Event tap created successfully");

    // Create a run loop source from the event tap
    let run_loop_source = unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap, 0) };

    if run_loop_source.is_null() {
        error!("Failed to create run loop source");
        return Err("Failed to create run loop source".to_string());
    }

    // Wrap as CFRunLoopSource
    let cf_source = unsafe { CFRunLoopSource::wrap_under_create_rule(run_loop_source as *mut _) };

    // Add the source to the current run loop
    let run_loop = CFRunLoop::get_current();
    run_loop.add_source(&cf_source, unsafe { kCFRunLoopCommonModes });

    // Enable the event tap
    unsafe {
        CGEventTapEnable(tap, true);
    }

    info!("macOS event listener started, running CFRunLoop");

    // Run the loop (this blocks forever - singleton pattern means we never stop)
    CFRunLoop::run_current();

    info!("macOS event listener stopped");
    Ok(())
}

/// A handle to receive global macOS events.
/// When dropped, it unsubscribes from the global listener.
pub struct MacOSEventSubscription {
    receiver: Receiver<MacOSEvent>,
}

impl MacOSEventSubscription {
    /// Receive with timeout.
    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<MacOSEvent, crossbeam_channel::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
}

impl Drop for MacOSEventSubscription {
    fn drop(&mut self) {
        get_global_listener().cleanup_dead_subscribers();
    }
}

/// Subscribe to global macOS events.
/// Returns a subscription handle that receives events.
/// The subscription is automatically cleaned up when dropped.
pub fn subscribe_events() -> MacOSEventSubscription {
    let listener = get_global_listener();
    let receiver = listener.subscribe();
    MacOSEventSubscription { receiver }
}
