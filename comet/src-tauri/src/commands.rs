//! 前端可调用的 Tauri 命令。

/// 由前端在命中/离开宠物本体时调用，切换整窗鼠标穿透。
#[tauri::command]
pub fn set_click_through(window: tauri::WebviewWindow, ignore: bool) {
    let _ = window.set_ignore_cursor_events(ignore);
}

#[tauri::command]
pub fn quit(app: tauri::AppHandle) {
    app.exit(0);
}
