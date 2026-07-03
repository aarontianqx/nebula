# 健康饮水提醒

核心理念：**提醒通过宠物姿态自然表达，而非弹窗打扰**。

## 行为

1. 距上次饮水超过间隔（默认 45 分钟）时，宠物切换到 `drink` 姿势（舔水碗）并保持。
2. 提醒期间不参与待机轮换和随机走动；拖拽/抚摸等交互结束后回到 `drink`。
3. 用户点击宠物 = 确认已喝水：宠物 `cheer` 欢呼 1.5s，重新开始计时。
4. 每个周期只提醒一次，不重复打扰。

## 持久化

- `localStorage["comet.lastDrinkAt"]`：上次饮水时间戳，应用重启不重置计时。
- `localStorage["comet.drinkIntervalMin"]`：提醒间隔（分钟），暂无设置界面，可手动改。
- 首次运行从启动时刻起算，避免装机即提醒。

## 实现

`src/lib/hydration.ts` 提供 `startHydrationScheduler`（30s 轮询检查）与 `acknowledgeDrink`；姿势编排在 `App.tsx`（`remindingRef` 标记提醒中状态）。
