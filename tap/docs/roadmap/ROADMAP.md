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
| Phase 5 | **Architecture** | 架构优化 | 跨平台成熟度 + 代码组织 | ✅ 评估完成（无需重构） |
| Phase 6 | **Plugins** | 插件系统 | Wasm 插件 + 自定义动作 | 📋 计划中 |

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
| 相对坐标 | API 就绪，Action 集成待做 | Phase 3 |
| 子宏调用 | 框架就绪，Engine 集成待做 | Phase 4 |
| 变量运行时替换 | DSL 解析完成，执行时替换待完善 | Phase 4 |
| macOS 窗口 API | 需实现 `get_foreground_window` 等 | Phase 5 |

## 文档索引

- [Phase 1 - MVP](./PHASE_1_MVP.md)
- [Phase 2 - Record & Replay](./PHASE_2_RECORD_REPLAY.md)
- [Phase 3 - Conditions](./PHASE_3_CONDITIONS.md)
- [Phase 4 - Extensibility](./PHASE_4_EXTENSIBILITY.md)
- [Phase 5 - Architecture](./PHASE_5_ARCHITECTURE.md)
