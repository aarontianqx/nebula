# 番茄钟

核心理念：**计时状态通过宠物姿态表达，零 UI、零弹窗**。

## 行为

| 事件 | 宠物表现 |
|------|----------|
| 双击宠物（off 时） | `greet` 打招呼 0.8s → 进入专注期，`focus` 姿势（戴眼镜盯屏幕）常驻 |
| 专注期结束（默认 25 min） | `cheer` 欢呼 2s → 进入休息期，`rest` 姿势（趴卧）常驻 |
| 休息期结束（默认 5 min） | 回到正常待机（轮换 + 走动恢复） |
| 双击宠物（进行中） | 取消当前会话，回到待机 |

- 专注/休息期间暂停待机轮换与随机走动；拖拽/抚摸等临时交互结束后回落到当前阶段姿势。
- 常驻姿势优先级：饮水提醒 `drink` > 番茄钟阶段 > 待机。

## 持久化

- `localStorage["comet.pomodoro"]`：进行中会话 `{phase, endsAt}`，重启后恢复剩余计时；已过期的会话直接丢弃。
- `localStorage["comet.pomodoroFocusMin"]` / `comet.pomodoroBreakMin`：时长（分钟），暂无设置界面。

## 实现

`src/lib/pomodoro.ts`：`Pomodoro` 类（1s tick，phase 状态机 off → focus → break → off）；姿势编排在 `App.tsx`（`basePose()` 统一计算常驻姿势）。
