# tap - Backlog（待办与未来工作）

> 这是 tap 唯一的前瞻性提案文档，用于“后续捡起继续做”。历史分阶段提案（phase-1..6、roadmap 等）已删除，需要时可在 git 历史中查阅。已交付能力的产品说明见 `specs/features/`。

## 已交付（背景速览）

repeat 即时工具 + 全局紧急停止；录制 / 回放 + 降噪合成；条件（窗口绑定 / 像素 / 分支 / 计数器 / wait_until）；YAML DSL + 参数化变量 + Rhai 表达式 + `call_macro`；macOS / Windows 双平台（窗口枚举 / 像素取色 / DPI）；统一 `MacroDocument` + YAML 落盘；前端 Zustand + Timeline 编辑器（List/Rail/Code + Inspector）+ 编辑即落库；模板 / 最近使用 / 元数据 / 运行前变量表单 / 原生导入导出；权限检测 + 首次引导；Key→Click 工具模式；Dry-run；按键名归一化；CI（clippy/fmt/test/tsc/eslint）。

## 待办

### 平台与稳定性

- **窗口相对坐标**：`Action` 增加 `relative_to_window`，录制 / 回放以“窗口相对坐标”存储，提升跨分辨率与窗口移动时的稳定性。底层 `WindowRect::to_absolute()` 已就绪，缺 Action 字段与录制/解析接入。
- **macOS 像素取色迁移 ScreenCaptureKit**：当前用 `CGWindowListCreateImage`（macOS 14+ 已弃用）。迁移到 ScreenCaptureKit 以面向未来；保留 `CGWindowListCreateImage` 作为回退。
- **Windows 提权检测**：当前仅 `PermissionBanner` 的静态文案提示。可加入真实检测（tap 是否提权 / 目标进程是否提权），据此更精确地引导“以管理员运行”。

### 测试与质量

- **前端 store 单测**：`documentStore`（同步契约：去抖回推 + 启动/保存前强制刷盘）、`engineStore`（运行态/倒计时）、`toolStore` 目前无单测，补齐以守护回归。
- **（可选）E2E 冒烟**：导入带变量宏 → 运行前表单填值 → 倒计时 → 按变量执行 → 随时停止。

### 分发

- **打包签名与公证**：macOS 代码签名 + notarization、Windows 代码签名，便于对外分发安装。

### 体验增强（Could）

- **颜色采样工具**：在 UI 中点击屏幕取色，便于配置 `pixel_color` 条件（复用全屏覆盖拾取器）。
- **随机抖动 / 速率限制**：对点击坐标 / 延时做可控小范围随机，避免过快输入导致目标程序卡死或误触。

### 远期：Phase 7 - 插件系统

- **Wasm 插件**（`wasmtime` / `extism`）：扩展动作类型与条件类型；定义插件 ABI；安全隔离（插件不可访问未授权资源）；提供插件 API 文档。选 Wasm 的理由：ABI 稳定、隔离强、跨平台、生态成熟。

## 明确非目标（短期不做）

企业级 RPA（复杂流程编排 / 监控 / 集中管理）、云同步、远程模板市场、OCR、多轨时间线高级编排、移动端自动化、多窗口 / 多 Profile 并行执行、Linux 支持（如确需，另行评估）。
