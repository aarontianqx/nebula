//! Comet 桌宠 Tauri 后端：鼠标穿透控制、全局光标监听与系统状态采样。

use std::{thread, time::Duration};

use serde::Serialize;
use tauri::{Emitter, Manager};

/// 由前端在命中/离开宠物本体时调用，切换整窗鼠标穿透。
#[tauri::command]
fn set_click_through(window: tauri::WebviewWindow, ignore: bool) {
    let _ = window.set_ignore_cursor_events(ignore);
}

#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("main window must exist");
            spawn_cursor_watcher(window.clone());
            spawn_system_watcher(window);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![set_click_through, quit])
        .run(tauri::generate_context!())
        .expect("error while running comet");
}

/// 穿透开启时 WebView 收不到任何鼠标事件，因此由 Rust 侧轮询全局光标，
/// 折算成窗口内逻辑坐标发给前端做像素级命中判定。
/// 仅在光标位于窗口范围内时发事件，避免常驻唤醒 WebView。
fn spawn_cursor_watcher(window: tauri::WebviewWindow) {
    thread::spawn(move || {
        let mut was_inside = false;
        loop {
            match cursor_local_position(&window) {
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
            thread::sleep(Duration::from_millis(50));
        }
    });
}

#[derive(Serialize, Clone, Copy)]
struct SystemStatus {
    /// 全局 CPU 使用率 0~100。
    cpu: f32,
    /// 电池电量 0~100；台式机/无电池为 None。
    battery: Option<f32>,
    /// 是否在充电/接电源；无电池为 None。
    charging: Option<bool>,
}

/// 周期采样 CPU 与电池，供前端做“疲惫”姿势联动。
/// 10s 一次，开销可忽略；WebView 侧只在数值跨过阈值时才会变更姿势。
fn spawn_system_watcher(window: tauri::WebviewWindow) {
    thread::spawn(move || {
        let mut sys = sysinfo::System::new();
        let battery_manager = starship_battery::Manager::new().ok();
        loop {
            // CPU 使用率需要两次采样间隔
            sys.refresh_cpu_usage();
            thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
            sys.refresh_cpu_usage();
            let cpu = sys.global_cpu_usage();

            let (battery, charging) = battery_status(battery_manager.as_ref());
            let _ = window.emit(
                "system-status",
                SystemStatus {
                    cpu,
                    battery,
                    charging,
                },
            );
            thread::sleep(Duration::from_secs(10));
        }
    });
}

fn battery_status(manager: Option<&starship_battery::Manager>) -> (Option<f32>, Option<bool>) {
    let Some(manager) = manager else {
        return (None, None);
    };
    let Some(Ok(bat)) = manager.batteries().ok().and_then(|mut it| it.next()) else {
        return (None, None);
    };
    let pct = bat.state_of_charge().value * 100.0;
    let charging = !matches!(bat.state(), starship_battery::State::Discharging);
    (Some(pct), Some(charging))
}

/// 光标在窗口内时返回窗口内逻辑坐标（与 CSS 像素一致），否则返回 None。
fn cursor_local_position(window: &tauri::WebviewWindow) -> Option<(f64, f64)> {
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
