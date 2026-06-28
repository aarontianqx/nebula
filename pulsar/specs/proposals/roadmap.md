# Pulsar — 开发路线图

> 状态：提案 (Proposal) · 起草于 2026-06 · 进度刷新于 2026-06
> 配套文档：[`vision-and-scope.md`](./vision-and-scope.md)、[`architecture.md`](./architecture.md)

## 概述

将 Pulsar 的开发分为 4 个阶段。每个阶段完成后，应用都处于**可用、可靠、可停止**的状态。**优先级原则：差异化能力（Smart Detection / Pipeline / CLI）优先于堆工具数量。**

## 阶段概览

| 阶段 | 名称 | 目标 | 状态 |
|------|------|------|------|
| **Phase 1** | 内核与 MVP | 项目骨架 + 工具注册表 + P0 工具 + Smart Detection + 搜索 | ✅ 实质完成 |
| **Phase 2** | 广度与串联 | 补齐 P1 工具 + Pipeline（杀手锏） | 🟡 进行中（工具已补齐，Pipeline 未做） |
| **Phase 3** | 自动化与 CLI | CLI + 工作流保存 + 剪贴板监听 + 大文件流式 | 🟡 进行中（CLI 已落地，其余未做） |
| **Phase 4** | 长尾与生态 | P2 工具 + Compact 模式 + 插件机制 | ⏳ 未开始（仅个别 P2） |

> **实现现状速览（截至 2026-06）** —— 整体约 45%，工具广度足，差异化能力开始补齐。
>
> - ✅ **已落地**：分层骨架（缺 Infrastructure 层）+ `Tool` trait/descriptor/注册表；**30 个工具**（14 P0 + 16 P1），165 测试全绿；Smart Detection（后端规则 + 前端 DetectBar）；**CLI（`pulsar-cli`，子命令由注册表动态派生，与 GUI 同源）**；Command Palette、侧栏分类/搜索/收藏（localStorage）、布局原型重构、自动运行、I/O 工效。
> - ❌ **尚未实现的核心差异化**：Pipeline 工具串联、工作流保存、剪贴板监听、大文件流式（`ToolValue::Stream` 未引入）、SQLite 持久化、Compact 浮窗、插件机制、"最近使用"。
> - 下一个建议优先项：**Pipeline 工具串联**（当初定位的核心卖点，CLI 已先行打底）。

## 里程碑定义

- **可用 (Usable)**：核心功能可正常使用，UI 可交互，无致命错误。
- **可靠 (Reliable)**：错误处理完善，资源正确释放，工具有单测覆盖。
- **可停止 (Stoppable)**：可随时暂停开发，代码可维护，文档完整。

---

## Phase 1：内核与 MVP

**目标**：证明"比 DevToys 更快更轻 + 粘贴即识别"。建立内核架构，跑通 P0 工具。

**交付物**：
- Tauri v2 + React + Tailwind 项目骨架（`pulsar-core` / `pulsar-app` / `src-tauri`）。
- `Tool` trait + `ToolDescriptor` + `ToolRegistry`（加工具只改一处的机制）。
- **P0 工具**（约 12 个）：
  - JSON 格式化/压缩/校验、JSON↔YAML↔TOML
  - Base64（文本/图片/文件）、URL 编解码、Hex↔文本/二进制
  - 时间戳↔日期、进制转换
  - JWT 解析
  - 哈希 (MD5/SHA/CRC32)、UUID/NanoID/ULID
  - 正则测试器、JSONPath、文本/JSON Diff
  - 大小写/命名转换
- **Smart Detection** v1（全局粘贴框 + 候选跳转）。
- **Command Palette**（`Cmd/Ctrl+K` 搜索）+ 左侧分类树 + 暗色主题。

**验收**：
- [x] `cargo fmt` / `cargo clippy -- -D warnings` 通过。
- [x] 每个 P0 工具有单测（核心 137 + 应用 10，共 147 测试）。
- [x] 粘贴 JSON/JWT/时间戳能被正确识别并推荐工具。
- [ ] 冷启动 <1s、安装包 <10MB（基准尚未记录）。

---

## Phase 2：广度与串联

**目标**：补齐高频工具，引入**工具串联 Pipeline**——这是相对竞品的核心差异，优先级高于继续堆数量。

**交付物**：
- **P1 工具**（部分完成）：
  - [x] SQL / XML 格式化、HTML 实体 / Unicode、HMAC / Bcrypt、密码生成+强度、颜色、QR 生成、Cron、JSON↔CSV、TOML↔JSON/YAML、XML↔JSON。
  - [ ] HTML/CSS/JS 格式化（缺成熟纯 Rust 实现，待选型）、AES 加解密、图片压缩/转换、Chmod/CIDR。
- [ ] **Pipeline 执行器**：步骤编辑 UI + 相邻步骤类型兼容校验 + 自动串联执行。（`ToolValue::kind()` / `IoKind` 已预留，执行器与前端 `pipelineStore` 未实现）
- [ ] 历史记录与收藏（SQLite 持久化）。当前仅收藏，且用 localStorage，无 SQLite。

**验收**：
- [ ] 能保存并复用一条多步 Pipeline（如 Base64 解码 → gzip 解压 → JSON 格式化）。
- [ ] Pipeline 构建期对类型不兼容给出明确提示。

---

## Phase 3：自动化与 CLI

**目标**：接续 `tap` 的自动化基因，让 Pulsar 能进脚本、进 CI、自动响应剪贴板。

> 状态：**CLI 已落地**；工作流保存、剪贴板监听、大文件流式仍待实现。

**交付物**：
- [x] **`pulsar-cli`**：子命令映射工具，stdin/stdout 管道，flag → 参数，可进 CI/脚本。
  - 子命令由 `ToolRegistry` 的 descriptor **动态派生**（与 GUI 同源，新增工具自动出现，无每工具代码）。映射约定集中在 `crates/pulsar-cli/src/dispatch.rs`。
  - 内置 `list`（`--json`）与 `detect`（`--json`）；错误走 stderr + 非零退出码；bool 参数 `--flag` / `--no-flag`，短名与完整 id 均可用。
- **工作流保存**：Pipeline 可序列化为文件，GUI 复用 + CLI 执行 (`pulsar run flow.yaml`)。
- **剪贴板监听自动化**：规则触发（检测命中 → 跑工具/Pipeline → 写回/通知）。
- **大文件流式处理**：`ToolValue::Stream`，哈希/Base64/行处理类工具支持数百 MB 文件。

**验收**：
- [x] `cat in.json | pulsar json` 等 CLI 命令可用，可进 CI（成功→0+stdout，失败→非零+stderr）。
- [ ] 开启监听后，复制 JSON 自动格式化回剪贴板。
- [ ] 对 >100MB 文件做哈希/Base64 不爆内存（基准记录）。

---

## Phase 4：长尾与生态

**目标**：补齐长尾工具，完善体验，开放扩展。

> 状态：**未开始**。P2 工具尚无一落地；Compact 浮窗、插件机制、多语言均未实现。

**交付物**：
- **P2 工具**：代码生成器 (JSON→TS/Go/Rust/Swift…)、cURL 转代码、Markdown、Mock 数据、速查表、单位换算、不可见字符检测等。
- **Compact 浮窗模式**（小窗常驻置顶）。
- **插件机制**：让社区/自己扩展工具（wasm 沙箱 / 子进程 / 动态库，届时定）。
- 多语言（中/英）。

**验收**：
- [ ] 第三方可按约定新增一个工具并被注册表识别。

---

## 技术债务与持续改进

每阶段结束时评估：
- [ ] 代码质量 (`clippy` / `fmt`)
- [ ] 测试覆盖（工具单测 + Pipeline/Detector 测试）
- [ ] 文档同步（`README.md` / `AGENTS.md` / `specs/features/`）
- [ ] 性能基准（启动时间、包体积、大文件吞吐）

## 风险与依赖

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 赛道饱和，缺乏差异化 | 产品价值 | 优先做 Smart Detection / Pipeline / CLI，而非堆工具 |
| formatter 缺成熟 Rust 实现 | P1 格式化工具 | 首版只选有成熟 Rust crate 的；其余延后或嵌 wasm |
| 图片处理依赖体积大 | 包体积卖点 | 评估 `image` crate 产物体积，必要时按需特性裁剪 |
| Tauri v2 变更 | 全局 | 锁版本，适配层隔离（与 wardenly-rs 一致） |
| 工具数量膨胀后架构腐化 | 可维护性 | 严守"工具=纯函数+descriptor"约束，注册集中一处 |
