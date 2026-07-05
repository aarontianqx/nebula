//! Comet 桌宠 Tauri 后端：应用装配。
//!
//! - `commands`：前端可调用命令（穿透切换 / 退出）
//! - `cursor`：全局光标轮询（动态穿透支撑）
//! - `system`：CPU / 电池采样（疲惫姿势联动）

mod commands;
mod cursor;
mod system;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("main window must exist");
            cursor::spawn_watcher(window.clone());
            system::spawn_watcher(window);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::set_click_through,
            commands::quit
        ])
        .run(tauri::generate_context!())
        .expect("error while running comet");
}
