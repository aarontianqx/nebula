# Wardenly-RS 开发路线图

## 概述

本路线图将 Wardenly 从 Go 重构到 Rust 的开发过程分为 4 个阶段。每个阶段完成后，应用处于**可用、可靠、可停止**的状态。

## 阶段概览

| 阶段 | 名称 | 目标 | 状态 |
|------|------|------|------|
| **Phase 1** | 核心框架 | Tauri 骨架 + 账户管理 + 存储 | ✅ 完成 |
| **Phase 2** | 浏览器与画布 | 浏览器自动化 + 登录 + 画布同步 | ✅ 完成 |
| **Phase 3** | 脚本执行 | 场景匹配 + 脚本引擎 + OCR | ✅ 完成 |
| **Phase 4** | 扩展功能 | 键盘透传 + 批量操作 + 优化 | ✅ 完成 |

## 里程碑定义

### ✅ 可用 (Usable)
- 核心功能可正常使用
- UI 可交互，无致命错误

### ✅ 可靠 (Reliable)
- 错误处理完善
- 资源正确释放
- 状态一致性保证

### ✅ 可停止 (Stoppable)
- 可随时暂停开发
- 代码可维护
- 文档完整

---

## Phase 1: 核心框架

**目标**: 建立项目骨架，实现账户/分组管理，可在 UI 中增删改查账户。

**交付物**:
- Tauri v2 项目结构
- React + Tailwind 前端骨架
- SQLite 持久化
- 账户/分组 CRUD

**详细规划**: [PHASE_1_FOUNDATION.md](./PHASE_1_FOUNDATION.md)

---

## Phase 2: 浏览器与画布

**目标**: 实现浏览器自动化，可启动账户、自动登录、显示游戏画面。

**交付物**:
- chromiumoxide 集成
- Session 生命周期管理
- Screencast 画布同步
- 画布点击/拖拽交互

**详细规划**: [PHASE_2_BROWSER.md](./PHASE_2_BROWSER.md)

---

## Phase 3: 脚本执行

**目标**: 实现自动化脚本执行，可根据场景自动操作游戏。

**交付物**:
- 场景定义与匹配
- 脚本引擎
- 循环与条件控制
- OCR 集成 (可选)

**详细规划**: [PHASE_3_SCRIPTING.md](./PHASE_3_SCRIPTING.md)

---

## Phase 4: 扩展功能

**目标**: 实现高级功能，提升使用体验。

**交付物**:
- Keyboard Passthrough
- 批量操作 (Spread to All, Run All)
- MongoDB 支持 (可选)
- 性能优化

**详细规划**: [PHASE_4_EXTENSIBILITY.md](./PHASE_4_EXTENSIBILITY.md)

---

## 技术债务与持续改进

每个阶段结束时评估：
- [ ] 代码质量 (clippy, fmt)
- [ ] 测试覆盖
- [ ] 文档同步
- [ ] 性能基准

## 风险与依赖

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| chromiumoxide API 变更 | 浏览器功能 | 锁定版本，抽象 Driver trait |
| Tauri v2 不稳定 | 全局 | 关注更新，及时适配 |

