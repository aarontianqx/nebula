# 系统状态联动

核心理念：**宠物是系统状态的具象化**——机器累了，狗也累了。

## 行为

- CPU 高负载或电池电量低（< 20% 且未充电）时，宠物切换 `tired` 姿势（瘫倒吐舌）常驻。
- 恢复后回到正常待机轮换。
- 优先级低于饮水提醒和番茄钟：`drink` > `focus`/`rest` > `tired` > 待机。
- 仅在宠物处于待机类姿势时切换，不打断交互、走动或提醒。

## 防抖

- CPU 阈值带迟滞：> 85% 进入疲惫，< 65% 退出，避免在阈值附近来回抖动。
- 采样周期 10s（Rust 侧线程），前端仅在跨阈值时更新姿势。

## 实现

- Rust：`system::spawn_watcher`（`src-tauri/src/system.rs`），`sysinfo` 采样全局 CPU、`starship-battery` 读电池，emit `system-status` 事件。
- 前端：`onSystemStatus`（`platform/ipc.ts`）订阅，`hooks/useWellness.ts` 的 `useSystemStress` 做迟滞判定并驱动 `PetController`。
