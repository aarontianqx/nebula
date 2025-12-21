# tap (Timed Action Performer)

`tap` 是一个桌面自动化应用（GUI），目标是处理大量重复性操作：简单重复点击、操作录制/重放，以及后续的可编程/可配置/插件式扩展。

技术路线：**Tauri + React（前端）** + **Rust（后端/引擎）**，面向 Win + mac 的长期自用与持续迭代。

## 快速开始

### 前置要求

- **Node.js**（建议 LTS）
- **Rust toolchain**（稳定版）
- Windows：需要 **WebView2 Runtime**（一般系统已自带；否则安装即可）

### 运行（开发）

```bash
cd tap
npm install
npm run tauri:dev
```

### 构建（发布 / 打包）

```bash
cd tap
npm install
npm run tauri:build
```

> Rust 后端入口在 `tap/src-tauri/`；前端入口在 `tap/src/`（Vite + React）。

## 文档

- `docs/FUNCTIONAL_GUIDE.md`：产品功能与阶段规划
- `docs/PROJECT_STRUCTURE.md`：技术选型与架构设计（含演进路线）
- `docs/UI_DESIGN.md`：UI/UX 设计（面向长期自用的体验优化）


