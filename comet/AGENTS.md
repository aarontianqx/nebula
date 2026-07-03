# Comet -- Agent Guidelines

## Overview

Comet 是**极致轻量的桌面宠物**：以主人的真实泰迪照片为原型，透明无边框窗口常驻桌面，动态鼠标穿透保证不影响正常办公。核心差异化是"轻 + 实用"：健康饮水提醒、番茄钟、系统状态联动等实用功能通过宠物姿态自然表达，而非弹窗打扰。技术栈 Tauri v2 + React + Rust。

> 当前阶段：Phase 1。透明窗口、动态穿透、拖拽、16 格序列帧资产（皮克斯风）、待机轮换、拖拽/抚摸姿势反馈、屏幕随机走动（walk_a/b 交替 + 窗口位移 + 朝向镜像）、健康饮水提醒、番茄钟（双击切换）、系统状态联动（CPU/电量 → tired；见 `specs/features/`）已落地；快捷工具箱、AI 挂件等在路线图上。

## Key design decisions

### 动画路线：序列帧 + 待机态 Live2D（混合）

- 每个宠物状态对应一张静态姿势图（4×4 宫格切片），姿态切换 = 换图；图之上叠加程序化微动画（呼吸/浮动/拖拽倾斜）。走路 = 2 帧循环 + 窗口水平位移。
- **待机正面姿势升级 Live2D**（进行中）：分层素材已就绪（`assets-src/live2d/comet_live2d.psd`，15 层，由 AI 部件分解图切片对位合成），等待人工在 Cubism Editor 完成绑定（操作手册：`specs/features/live2d-rigging-guide.md`）。moc3 导出后由 pixi-live2d-display 驱动：眼神跟随、眨眼、歪头、吐舌、耳朵尾巴物理。
- Live2D 只覆盖待机态；走路等其它姿势仍用序列帧，切换时交叉淡入衔接。
- Live2D 素材再生管线：`scripts/slice_parts.py`（部件分解图切片）→ `scripts/compose_parts.py`（对位预览）→ `scripts/export_psd.py`（分层 PSD）。

### 动态鼠标穿透（核心机制）

穿透开启时 WebView 收不到任何鼠标事件，因此采用 Rust 侧轮询：

1. Rust 线程 50ms 轮询全局光标（`window.cursor_position()`），折算为窗口内逻辑坐标，光标在窗口内时 emit `cursor-pos` 事件（离开时 emit `cursor-left`）。
2. 前端对 canvas 做像素级 alpha 命中判定，命中宠物本体 → `set_click_through(false)`，否则 `true`。
3. IPC 带去重（状态未变不发送），光标不在窗口内时零事件，保证空闲零开销。

macOS 透明窗口依赖 `macOSPrivateApi: true`（tauri.conf.json）与 tauri 的 `macos-private-api` feature。

### 低资源消耗原则

- 窗口只包住宠物本体（260×300），不做全屏透明层。
- 动画用 CSS animation / rAF，宠物 idle 时应可降帧或暂停（规划）。
- 常驻 CPU 目标 < 1%。

## 目录结构

```
comet/
├── src/                    # React 前端
│   ├── assets/pet/         # 16 格姿势切片（透明 PNG）+ _source-grid.png 原宫格
│   ├── components/         # PetCanvas（姿势贴图渲染 + 像素命中）
│   ├── lib/                # ipc.ts（IPC 边界）/ poses.ts（姿势映射）/ walker.ts（走动）/ hydration.ts（饮水）/ pomodoro.ts（番茄钟）
│   └── styles/             # 全局样式 + 程序化微动画 keyframes
├── src-tauri/              # Tauri 后端：穿透控制 + 光标轮询线程
└── specs/features/         # pose-matrix.md：16 格状态矩阵规格
```

### 资产管线（AI 生图 → 切片）

1. 以真实泰迪照片为参考，AI 生成 4×4 宫格图（提示词规格见 `specs/features/pose-matrix.md`）。
2. `scripts/slice_grid.py`（pillow + numpy + scipy）做高质量切片：连通域按质心归属格子（肢体/道具越过格线不被裁断）、封闭白洞剔除（腿间/碗沿白底转透明，保留眼睛高光）、白底去污染反解前景色（毛发软边缘无白晕）。
3. 各切片按内容紧裁、尺寸不一，渲染时以原宫格单元 (384×256) 为统一参考系缩放，保证体型一致（见 `PetCanvas`）。

## 16 格状态矩阵（资产规划）

| 行 | 1 | 2 | 3 | 4 |
|----|---|---|---|---|
| 待机 | 站立正面 | 坐姿歪头 | 趴卧休息 | 蜷缩睡觉 |
| 移动 | 走路帧 A | 走路帧 B | 奔跑 | 起身伸懒腰 |
| 交互 | 被抚摸眯眼 | 被拖拽悬空 | 摇尾巴打招呼 | 委屈（久未互动） |
| 功能 | 喝水提醒 | 番茄钟专注 | 疲惫（CPU 高/低电量） | 任务完成欢呼 |

## Conventions

- 遵循仓库根 `AGENTS.md` 的 Rust / TypeScript 约定。
- 前端保持薄：状态机与计时逻辑优先放 Rust 侧或独立 TS 模块，组件只渲染。
- 端口 1430（避免与 pulsar 的 1420 冲突）。
