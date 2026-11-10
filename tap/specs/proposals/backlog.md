# tap - Backlog（待办与未来工作）

> 这是 tap 的前瞻性待办清单，用于“后续捡起继续做”。已立项的工作见 `editor_and_engine_hardening.md`（编辑器与引擎加固）。历史分阶段提案（phase-1..6、roadmap 等）已删除，需要时可在 git 历史中查阅。已交付能力的产品说明见 `specs/features/`。

## 已交付（背景速览）

repeat 即时工具 + 全局紧急停止；录制 / 回放 + 降噪合成；条件（窗口绑定 / 像素 / 分支 / 计数器 / wait_until）；YAML DSL + 参数化变量 + Rhai 表达式 + `call_macro`；macOS / Windows 双平台（窗口枚举 / 像素取色 / DPI）；统一 `MacroDocument` + YAML 落盘；前端 Zustand + Timeline 编辑器（List/Rail/Code + Inspector）+ 编辑即落库；模板 / 最近使用 / 元数据 / 运行前变量表单 / 原生导入导出；权限检测 + 首次引导；Key→Click 工具模式；Dry-run；按键名归一化；CI（clippy/fmt/test/tsc/eslint）。

## 待办

> 已转入 `editor_and_engine_hardening.md`：窗口相对坐标、macOS ScreenCaptureKit 迁移、前端 store 单测、颜色采样工具、随机抖动。

### 平台与稳定性

- **Windows 提权检测**：当前仅 `PermissionBanner` 的静态文案提示。可加入真实检测（tap 是否提权 / 目标进程是否提权），据此更精确地引导“以管理员运行”。

### 测试与质量

- **（可选）E2E 冒烟**：导入带变量宏 → 运行前表单填值 → 倒计时 → 按变量执行 → 随时停止。

### 分发

- **打包签名与公证**：macOS 代码签名 + notarization、Windows 代码签名，便于对外分发安装。

### 远期：Phase 7 - 插件系统

- **Wasm 插件**（`wasmtime` / `extism`）：扩展动作类型与条件类型；定义插件 ABI；安全隔离（插件不可访问未授权资源）；提供插件 API 文档。选 Wasm 的理由：ABI 稳定、隔离强、跨平台、生态成熟。

## 明确非目标（短期不做）

企业级 RPA（复杂流程编排 / 监控 / 集中管理）、云同步、远程模板市场、OCR、多轨时间线高级编排、移动端自动化、多窗口 / 多 Profile 并行执行、Linux 支持（如确需，另行评估）。
