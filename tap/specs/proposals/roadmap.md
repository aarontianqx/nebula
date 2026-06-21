# tap - Roadmap 总览

## 版本演进策略

tap 采用"渐进增强"的演进策略：从最简单的"重复点击"开始，逐步增加"录制/编辑/条件/脚本/插件"能力。每个阶段都必须是**可用、可靠、可停止**的状态。

## 阶段概览

| 阶段 | 代号 | 目标 | 预计范围 | 状态 |
|------|------|------|----------|------|
| Phase 1 | **MVP** | 最小可用产品 | 重复执行 + 安全停止 + 基础 UI | ✅ 完成 |
| Phase 2 | **Record & Replay** | 录制与回放 | 全局 Hook + 时间线生成 + 回放引擎 | ✅ 完成 |
| Phase 3 | **Conditions** | 条件与识别 | 窗口绑定 + 像素检测 + 简单分支 | ✅ 完成 |
| Phase 4 | **Extensibility** | 可编程与插件 | DSL + 参数化变量 + 表达式引擎 | ✅ 核心完成 |
| Phase 5 | **Architecture** | 架构优化 | 跨平台成熟度 + 代码组织 | ✅ 评估完成 |
| Phase 6 | **Consolidation** | 打通并跑通已建能力 | 统一文档模型 + 执行期变量/子宏 + Application 层 + 双平台 | 🚧 进行中 |
| Phase 7 | **Plugins** | 插件系统 | Wasm 插件 + 自定义动作 | 📋 计划中 |

## 优先级框架（MoSCoW）

每个阶段的功能按以下优先级分类：

- **Must**：阶段交付的必备功能，缺一不可
- **Should**：强烈建议做，显著提升体验或稳定性
- **Could**：锦上添花，时间允许可做
- **Won't**：明确不在本阶段范围

## 核心原则（贯穿所有阶段）

1. **安全停止是一等公民**：任何阶段都必须保证"全局热键立即停止"能力
2. **可观测**：用户始终知道"正在做什么 / 下一步是什么 / 为什么停了"
3. **可恢复**：崩溃或异常后，给出明确提示与恢复路径
4. **平台差异显式化**：Win 与 mac 的权限、热键、注入限制都要在 UI 中显式提示

## 验收标准（通用）

每个阶段交付前必须满足：

- [ ] 所有 Must 功能已实现并可演示
- [ ] 安全停止（全局热键）在 Win 和 mac 上都能正常工作
- [ ] 无已知的"失控"场景（执行中无法停止）
- [ ] 关键路径有日志可追溯
- [ ] README / docs 已同步更新

## 待完成项（跨阶段）

以下功能已有基础代码/API，但尚未完全集成：

| 功能 | 说明 | 原属阶段 |
|------|------|----------|
| 相对坐标 | 窗口矩形 API 就绪，录制/动作以窗口相对坐标存储待做 | Phase 3 |

> 已在 Phase 6 打通：**变量运行时替换**（M3，Resolve 阶段 + 运行前覆盖）、**子宏调用 `call_macro`**（M4，引擎就地展开 + 环/深度保护 + 子作用域）、**macOS 平台对齐**（M5，`CGWindowList` 窗口枚举 / `CGWindowListCreateImage` 像素取色 / 真实 `backingScaleFactor`，并统一坐标系：mac 用 point、Win 用物理像素，拾取器经 `browser_to_injection_scale` 换算）、**录制降噪**（M6，合成 Click/DoubleClick/Drag/KeyTap + 合并共线移动）、**拖拽插值与安全硬化**（M7，按 `duration_ms` 可中断插值且异常必抬起、注入看门狗超时、连续失败自动停、子秒级倒计时）、**前端重构与同步契约**（M8，Zustand store + 组件化；Timeline 编辑器 List/Rail/Code 三视图 + Inspector 参数编辑、增删/移动/复制/拖拽调时/批量调延时/note；编辑去抖回推 `update_profile` 且启动/保存前强制刷盘——彻底修复“前端编辑不落库”）、**Profile / 模板 / 运行前表单**（M9，编译期内嵌模板浏览 + 一键应用、最近使用列表、无损元数据编辑（描述/作者/标签）、运行前变量表单接入开始流程、原生文件对话框导入导出）。

### Quick Tools（跨阶段体验增强）

这些功能不一定进入 DSL/Timeline，但能显著提升“日常效率”和“舒适度”，并且需要保持可停止与可观测：

| 工具 | 说明 | 状态 |
|------|------|------|
| Key→Click（A–Z 按住连点） | 开启后按住 `A`–`Z` 任意键即可持续模拟鼠标点击；按下 `Space` 立即终止 | 📋 规划中（见 `specs/features/key-to-click-mode.md`） |

## 文档索引

- [Phase 1 - MVP](./phase-1-mvp.md)
- [Phase 2 - Record & Replay](./phase-2-record-replay.md)
- [Phase 3 - Conditions](./phase-3-conditions.md)
- [Phase 4 - Extensibility](./phase-4-extensibility.md)
- [Phase 5 - Architecture](./phase-5-architecture.md)
- [Phase 6 - Consolidation](./phase-6-consolidation.md)
- [Feature - Key→Click（A–Z 按住连点）](../features/key-to-click-mode.md)
