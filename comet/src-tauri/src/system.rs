//! 系统状态采样：CPU 与电池，供前端做"疲惫"姿势联动。

use std::{thread, time::Duration};

use serde::Serialize;
use tauri::Emitter;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Serialize, Clone, Copy)]
pub struct SystemStatus {
    /// 全局 CPU 使用率 0~100。
    cpu: f32,
    /// 电池电量 0~100；台式机/无电池为 None。
    battery: Option<f32>,
    /// 是否在充电/接电源；无电池为 None。
    charging: Option<bool>,
}

/// 启动周期采样线程。10s 一次，开销可忽略；
/// WebView 侧只在数值跨过阈值时才会变更姿势。
pub fn spawn_watcher(window: tauri::WebviewWindow) {
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
            thread::sleep(SAMPLE_INTERVAL);
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
