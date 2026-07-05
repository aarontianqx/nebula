//! 全局光标监听：动态鼠标穿透的 Rust 侧支撑。
//!
//! 穿透开启时 WebView 收不到任何鼠标事件，因此由 Rust 侧轮询全局光标，
//! 折算成窗口内逻辑坐标发给前端做像素级命中判定。

use std::{thread, time::Duration};

use tauri::Emitter;

const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// 启动光标轮询线程。仅在光标位于窗口范围内时发 `cursor-pos` 事件
/// （离开时发一次 `cursor-left`），避免常驻唤醒 WebView。
pub fn spawn_watcher(window: tauri::WebviewWindow) {
    thread::spawn(move || {
        let mut was_inside = false;
        loop {
            match local_position(&window) {
                Some(local) => {
                    was_inside = true;
                    let _ = window.emit("cursor-pos", local);
                }
                None if was_inside => {
                    was_inside = false;
                    let _ = window.emit("cursor-left", ());
                }
                None => {}
            }
            thread::sleep(POLL_INTERVAL);
        }
    });
}

/// 光标在窗口内时返回窗口内逻辑坐标（与 CSS 像素一致），否则返回 None。
fn local_position(window: &tauri::WebviewWindow) -> Option<(f64, f64)> {
    let cursor = window.cursor_position().ok()?;
    let origin = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let x = cursor.x - origin.x as f64;
    let y = cursor.y - origin.y as f64;
    if x < 0.0 || y < 0.0 || x > size.width as f64 || y > size.height as f64 {
        return None;
    }
    let scale = window.scale_factor().unwrap_or(1.0);
    Some((x / scale, y / scale))
}
