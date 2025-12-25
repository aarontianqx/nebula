//! Windows native implementation for input hooking.
//!
//! Uses Raw Input API for keyboard capture (works regardless of window focus,
//! including when Tauri/WebView2 window is active) and WH_MOUSE_LL for mouse.

use super::{InputEventType, MouseButtonType, RawInputEvent};
use crossbeam_channel::{Receiver, Sender};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;
use tracing::{debug, error, info, trace, warn};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1,
    VK_F10, VK_F11, VK_F12, VK_F2, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7, VK_F8, VK_F9, VK_HOME,
    VK_INSERT, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NEXT, VK_NUMLOCK,
    VK_PAUSE, VK_PRIOR, VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SCROLL,
    VK_SHIFT, VK_SNAPSHOT, VK_SPACE, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
    RAWINPUTHEADER, RIDEV_INPUTSINK, RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, PostThreadMessageW, RegisterClassW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, MSLLHOOKSTRUCT, MSG,
    WH_MOUSE_LL, WM_DESTROY, WM_INPUT, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

// Thread ID for posting quit message
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

// Thread-local storage for event sender and window handle
thread_local! {
    static EVENT_SENDER: RefCell<Option<Sender<RawInputEvent>>> = const { RefCell::new(None) };
    static START_TIME: RefCell<Option<Instant>> = const { RefCell::new(None) };
    static MESSAGE_WINDOW: RefCell<HWND> = const { RefCell::new(std::ptr::null_mut()) };
}

// HID usage page and usage for keyboard
const HID_USAGE_PAGE_GENERIC: u16 = 0x01;
const HID_USAGE_GENERIC_KEYBOARD: u16 = 0x06;

/// Start the input hook using Windows native API.
pub fn start_hook(event_tx: Sender<RawInputEvent>, stop_rx: Receiver<()>) {
    info!("Input hook thread started (Windows native with Raw Input)");

    // Store thread ID for clean shutdown
    let thread_id = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
    HOOK_THREAD_ID.store(thread_id, Ordering::SeqCst);

    // Initialize thread-local state
    EVENT_SENDER.with(|sender| {
        *sender.borrow_mut() = Some(event_tx);
    });
    START_TIME.with(|time| {
        *time.borrow_mut() = Some(Instant::now());
    });

    // Create a message-only window for Raw Input
    let hwnd = create_message_window();
    if hwnd.is_null() {
        error!("Failed to create message window for Raw Input");
        return;
    }
    MESSAGE_WINDOW.with(|w| {
        *w.borrow_mut() = hwnd;
    });
    debug!("Message window created: {:?}", hwnd);

    // Register for Raw Input (keyboard)
    let raw_input_device = RAWINPUTDEVICE {
        usUsagePage: HID_USAGE_PAGE_GENERIC,
        usUsage: HID_USAGE_GENERIC_KEYBOARD,
        dwFlags: RIDEV_INPUTSINK, // Receive input even when not in foreground
        hwndTarget: hwnd,
    };

    let result = unsafe {
        RegisterRawInputDevices(
            &raw_input_device,
            1,
            std::mem::size_of::<RAWINPUTDEVICE>() as u32,
        )
    };

    if result == 0 {
        error!("Failed to register Raw Input devices");
        unsafe { DestroyWindow(hwnd) };
        return;
    }
    info!("Raw Input registered for keyboard (RIDEV_INPUTSINK)");

    // Install mouse hook (still use low-level hook for mouse, it works fine)
    let mouse_hook = unsafe {
        SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_hook_proc),
            GetModuleHandleW(std::ptr::null()),
            0,
        )
    };
    if mouse_hook.is_null() {
        error!("Failed to install mouse hook");
        unsafe { DestroyWindow(hwnd) };
        return;
    }
    debug!("Mouse hook installed");

    // Spawn a thread to monitor stop signal
    let stop_thread = std::thread::spawn(move || {
        loop {
            if stop_rx
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_ok()
            {
                info!("Stop signal received, posting WM_QUIT");
                let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
                if tid != 0 {
                    unsafe { PostThreadMessageW(tid, WM_QUIT, 0, 0) };
                }
                break;
            }
            // Also check if hooks are still valid (thread might have exited)
            if HOOK_THREAD_ID.load(Ordering::SeqCst) == 0 {
                break;
            }
        }
    });

    // Message loop
    info!("Starting Windows message loop (Raw Input + Mouse Hook)");
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        let ret = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        if ret <= 0 {
            // WM_QUIT or error
            break;
        }
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Cleanup
    info!("Unhooking and cleaning up");
    unsafe {
        UnhookWindowsHookEx(mouse_hook);
        DestroyWindow(hwnd);
    }
    HOOK_THREAD_ID.store(0, Ordering::SeqCst);

    // Wait for stop thread
    let _ = stop_thread.join();

    info!("Input hook thread exiting");
}

/// Create a message-only window for receiving Raw Input messages.
fn create_message_window() -> HWND {
    unsafe {
        let class_name: Vec<u16> = "TapRawInputWindow\0".encode_utf16().collect();
        let window_name: Vec<u16> = "Tap Raw Input\0".encode_utf16().collect();

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(std::ptr::null()),
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };

        RegisterClassW(&wc);

        // Use HWND_MESSAGE (-3) for message-only window, but that requires isize cast
        // Instead, create a hidden window
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_OVERLAPPEDWINDOW, // Will be hidden anyway
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    }
}

/// Window procedure for handling Raw Input messages.
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_INPUT => {
            handle_raw_input(lparam);
            0
        }
        WM_DESTROY => {
            // Unregister Raw Input
            let raw_input_device = RAWINPUTDEVICE {
                usUsagePage: HID_USAGE_PAGE_GENERIC,
                usUsage: HID_USAGE_GENERIC_KEYBOARD,
                dwFlags: 0x00000001, // RIDEV_REMOVE
                hwndTarget: std::ptr::null_mut(),
            };
            RegisterRawInputDevices(
                &raw_input_device,
                1,
                std::mem::size_of::<RAWINPUTDEVICE>() as u32,
            );
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Handle WM_INPUT message and extract keyboard events.
unsafe fn handle_raw_input(lparam: LPARAM) {
    let mut size: u32 = 0;

    // Get required buffer size
    let result = GetRawInputData(
        lparam as HRAWINPUT,
        RID_INPUT,
        std::ptr::null_mut(),
        &mut size,
        std::mem::size_of::<RAWINPUTHEADER>() as u32,
    );

    if result != 0 {
        return;
    }

    if size == 0 {
        return;
    }

    // Allocate buffer and get data
    let mut buffer: Vec<u8> = vec![0u8; size as usize];
    let result = GetRawInputData(
        lparam as HRAWINPUT,
        RID_INPUT,
        buffer.as_mut_ptr() as *mut _,
        &mut size,
        std::mem::size_of::<RAWINPUTHEADER>() as u32,
    );

    if result == u32::MAX {
        warn!("GetRawInputData failed");
        return;
    }

    let raw_input = &*(buffer.as_ptr() as *const RAWINPUT);

    // Only handle keyboard input
    if raw_input.header.dwType == RIM_TYPEKEYBOARD {
        let keyboard = &raw_input.data.keyboard;
        let vk = keyboard.VKey;
        let flags = keyboard.Flags;

        // Flags: bit 0 = key up (RI_KEY_BREAK), bit 1 = E0 prefix, bit 2 = E1 prefix
        let is_key_up = (flags & 0x01) != 0;

        let key = vk_to_string(vk);

        let event_type = if is_key_up {
            trace!(key = %key, vk = vk, "Raw KeyUp");
            InputEventType::KeyUp { key }
        } else {
            trace!(key = %key, vk = vk, "Raw KeyDown");
            InputEventType::KeyDown { key }
        };

        send_event(event_type);
    }
}

/// Low-level mouse hook procedure.
unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let ms = &*(lparam as *const MSLLHOOKSTRUCT);
        let x = ms.pt.x;
        let y = ms.pt.y;

        let event_type = match wparam as u32 {
            WM_MOUSEMOVE => Some(InputEventType::MouseMove { x, y }),
            WM_LBUTTONDOWN => Some(InputEventType::MouseDown {
                x,
                y,
                button: MouseButtonType::Left,
            }),
            WM_LBUTTONUP => Some(InputEventType::MouseUp {
                x,
                y,
                button: MouseButtonType::Left,
            }),
            WM_RBUTTONDOWN => Some(InputEventType::MouseDown {
                x,
                y,
                button: MouseButtonType::Right,
            }),
            WM_RBUTTONUP => Some(InputEventType::MouseUp {
                x,
                y,
                button: MouseButtonType::Right,
            }),
            WM_MBUTTONDOWN => Some(InputEventType::MouseDown {
                x,
                y,
                button: MouseButtonType::Middle,
            }),
            WM_MBUTTONUP => Some(InputEventType::MouseUp {
                x,
                y,
                button: MouseButtonType::Middle,
            }),
            WM_MOUSEWHEEL => {
                let delta = ((ms.mouseData >> 16) as i16) as i32;
                Some(InputEventType::Scroll {
                    delta_x: 0,
                    delta_y: delta / 120,
                })
            }
            _ => None,
        };

        if let Some(event) = event_type {
            send_event(event);
        }
    }

    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

/// Send event through the channel.
fn send_event(event: InputEventType) {
    let timestamp_ms = START_TIME.with(|time| {
        time.borrow()
            .as_ref()
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    });

    EVENT_SENDER.with(|sender| {
        if let Some(ref tx) = *sender.borrow() {
            let raw_event = RawInputEvent {
                timestamp_ms,
                event,
            };
            let _ = tx.try_send(raw_event);
        }
    });
}

/// Convert Windows virtual key code to key name string.
fn vk_to_string(vk: VIRTUAL_KEY) -> String {
    match vk {
        VK_BACK => "Backspace".into(),
        VK_TAB => "Tab".into(),
        VK_RETURN => "Return".into(),
        VK_SHIFT | VK_LSHIFT => "ShiftLeft".into(),
        VK_RSHIFT => "ShiftRight".into(),
        VK_CONTROL | VK_LCONTROL => "ControlLeft".into(),
        VK_RCONTROL => "ControlRight".into(),
        VK_MENU | VK_LMENU => "Alt".into(),
        VK_RMENU => "AltGr".into(),
        VK_PAUSE => "Pause".into(),
        VK_CAPITAL => "CapsLock".into(),
        VK_ESCAPE => "Escape".into(),
        VK_SPACE => "Space".into(),
        VK_PRIOR => "PageUp".into(),
        VK_NEXT => "PageDown".into(),
        VK_END => "End".into(),
        VK_HOME => "Home".into(),
        VK_LEFT => "Left".into(),
        VK_UP => "Up".into(),
        VK_RIGHT => "Right".into(),
        VK_DOWN => "Down".into(),
        VK_SNAPSHOT => "PrintScreen".into(),
        VK_INSERT => "Insert".into(),
        VK_DELETE => "Delete".into(),
        VK_LWIN | VK_RWIN => "MetaLeft".into(),
        VK_NUMLOCK => "NumLock".into(),
        VK_SCROLL => "ScrollLock".into(),
        VK_F1 => "F1".into(),
        VK_F2 => "F2".into(),
        VK_F3 => "F3".into(),
        VK_F4 => "F4".into(),
        VK_F5 => "F5".into(),
        VK_F6 => "F6".into(),
        VK_F7 => "F7".into(),
        VK_F8 => "F8".into(),
        VK_F9 => "F9".into(),
        VK_F10 => "F10".into(),
        VK_F11 => "F11".into(),
        VK_F12 => "F12".into(),
        // Number keys 0-9
        0x30 => "0".into(),
        0x31 => "1".into(),
        0x32 => "2".into(),
        0x33 => "3".into(),
        0x34 => "4".into(),
        0x35 => "5".into(),
        0x36 => "6".into(),
        0x37 => "7".into(),
        0x38 => "8".into(),
        0x39 => "9".into(),
        // Letter keys A-Z
        0x41 => "a".into(),
        0x42 => "b".into(),
        0x43 => "c".into(),
        0x44 => "d".into(),
        0x45 => "e".into(),
        0x46 => "f".into(),
        0x47 => "g".into(),
        0x48 => "h".into(),
        0x49 => "i".into(),
        0x4A => "j".into(),
        0x4B => "k".into(),
        0x4C => "l".into(),
        0x4D => "m".into(),
        0x4E => "n".into(),
        0x4F => "o".into(),
        0x50 => "p".into(),
        0x51 => "q".into(),
        0x52 => "r".into(),
        0x53 => "s".into(),
        0x54 => "t".into(),
        0x55 => "u".into(),
        0x56 => "v".into(),
        0x57 => "w".into(),
        0x58 => "x".into(),
        0x59 => "y".into(),
        0x5A => "z".into(),
        // Numpad
        0x60 => "Kp0".into(),
        0x61 => "Kp1".into(),
        0x62 => "Kp2".into(),
        0x63 => "Kp3".into(),
        0x64 => "Kp4".into(),
        0x65 => "Kp5".into(),
        0x66 => "Kp6".into(),
        0x67 => "Kp7".into(),
        0x68 => "Kp8".into(),
        0x69 => "Kp9".into(),
        0x6A => "KpMultiply".into(),
        0x6B => "KpPlus".into(),
        0x6D => "KpMinus".into(),
        0x6E => "KpDelete".into(),
        0x6F => "KpDivide".into(),
        // OEM keys
        0xBA => ";".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xC0 => "`".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "'".into(),
        _ => format!("Unknown(0x{:02X})", vk),
    }
}
