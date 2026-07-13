# Comet -- Agent Guidelines

## Overview

Comet 是**极致轻量的桌面宠物**：以主人的真实泰迪照片为原型，透明无边框窗口常驻桌面，动态鼠标穿透保证不影响正常办公。核心差异化是"轻 + 实用"：健康饮水提醒、番茄钟、系统状态联动等实用功能通过宠物姿态自然表达，而非弹窗打扰。技术栈 Tauri v2 + React + Rust。

> 当前阶段：Phase 2。透明窗口、动态穿透、拖拽、15 状态混合动画（标准姿势 / 语义关键帧 / 连续序列）、待机轮换、拖拽/抚摸反馈、屏幕随机走动与奔跑、健康饮水提醒、番茄钟、系统状态联动（CPU/电量 → tired；见 `specs/features/`）已落地；快捷工具箱、AI 挂件等在路线图上。

## Key design decisions

### 动画路线：一致性优先的混合动画

- 微动作状态使用一张标准姿势加程序动画；交互动作使用少量语义关键姿势和可变停留时间；只有连续肢体运动保留帧序列。运行协议见 `src/pet/assets.ts`。
- 程序层负责呼吸、眨眼、歪头、眼镜、爱心、Zzz、问号、拖拽钟摆和落地 squash & stretch，避免为小变化重绘整只宠物。
- 曾评估并放弃 Live2D 路线（需人工 Cubism Editor 绑定，工作流重）；也曾升级 25 帧（5×5 + 超分 + RIFE 插帧）后回退——AI 生成 25 帧非连续动作反而加剧跳变，连贯性应由生成质量保证而非后处理补救。
- 走路/奔跑 = 经闭环指标筛选的短帧循环 + 窗口水平位移（`walker.ts`）；素材统一朝右，向左由 canvas 镜像。
- 状态矩阵、资产规格、切片管线、重生成 checklist 详见 `specs/features/pose-matrix.md`。

### 资产管线（原狗 reference → AI 关键姿势 → 验证）

1. 角色身份以主人原狗照片及 `assets-src/live2d/parts/reference.png` 为最高优先级 reference；`template-grid-pixar.png` 与已验收帧为补充参考。
2. `scripts/imagegen_url.py` 适配返回签名 URL 的 OpenAI Images endpoint，立即下载并校验图片；凭据只从 `~/.config/codex/imagegen.env` 读取。
3. 新姿势必须逐张生成和验收。若脸型、眼距、头身比或四肢拓扑漂移，拒绝该批资产并回退到已验收姿势，不用插帧掩盖。
4. 旧 4×4 源图仍由 `scripts/slice_frames.py` 完成去背、对齐、共用裁剪和压缩，作为连续序列及迁移来源。
5. `scripts/validate_animation.py` 生成真实节奏 GIF 和结构指标；`preview_frames.py` 生成联络表。动态预览至少检查三个循环。

### 动态鼠标穿透（核心机制）

穿透开启时 WebView 收不到任何鼠标事件，因此采用 Rust 侧轮询：

1. Rust 线程 50ms 轮询全局光标（`window.cursor_position()`），折算为窗口内逻辑坐标，光标在窗口内时 emit `cursor-pos` 事件（离开时 emit `cursor-left`）。
2. 前端对 canvas 做像素级 alpha 命中判定，命中宠物本体 → `set_click_through(false)`，否则 `true`。
3. IPC 带去重（状态未变不发送），光标不在窗口内时零事件，保证空闲零开销。

macOS 透明窗口依赖 `macOSPrivateApi: true`（tauri.conf.json）与 tauri 的 `macos-private-api` feature。

### 低资源消耗原则

- 窗口只包住宠物本体（210×200），不做全屏透明层。
- 帧动画用 setInterval 按状态 fps 驱动（慢状态 1.5~3 fps），微动画用 CSS animation。
- 常驻 CPU 目标 < 1%（实测 idle ~0.9%，debug 构建）。

## 目录结构

```
comet/
├── assets-src/frames/      # AI 生成的原始宫格图（每状态一张 4×4）
├── scripts/                # slice_frames.py（切片+压缩）/ preview_frames.py（目检拼图）
├── src/                    # React 前端（分层：platform → pet/features → hooks → App）
│   ├── assets/pet/frames/  # 切片产物：{state}_{i}.png + manifest.json
│   ├── platform/           # 平台适配：env（Tauri 检测）/ ipc（命令+事件）/ storage（localStorage 解析）
│   ├── pet/                # 宠物域：types / assets（资产表+fps）/ behavior（待机轮换）/ PetCanvas
│   ├── features/           # 独立功能模块：walker / hydration / pomodoro（互不依赖）
│   ├── hooks/              # 行为挂钩：状态机（PetController）/ 待机调度 / 健康效率 / 穿透 / 手势
│   ├── App.tsx             # 纯组装层：状态机 + 挂钩 + 渲染
│   └── styles/             # 全局样式 + 程序化微动画 keyframes
├── src-tauri/src/          # Rust 后端：lib（装配）/ commands / cursor（光标轮询）/ system（CPU/电池采样）
└── specs/features/         # pose-matrix.md：状态帧序列矩阵规格
```

### 前端分层依赖方向

`platform`（最底层，封装 Tauri/localStorage）← `pet` / `features`（业务域，互不依赖）← `hooks`（行为挂钩，通过 `PetController` 驱动状态机）← `App.tsx`（组装）。上层可依赖下层，反向禁止；`features` 各模块之间不互相引用。

## Conventions

- 遵循仓库根 `AGENTS.md` 的 Rust / TypeScript 约定。
- 组件只渲染：业务逻辑放 hooks / features，新功能优先写成独立 feature 模块 + 一个 hook 挂钩。
- 共享标志（提醒中/高压/番茄钟阶段）集中在 `PetController` 的 ref 上，不散落各组件。
- 端口 1430（避免与 pulsar 的 1420 冲突）。
