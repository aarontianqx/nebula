# Pulsar

`Pulsar` 是面向开发者的**本地工具工作台**（桌面 GUI + CLI）：把每天要用的几十个小工具——JSON 格式化、Base64 编解码、时间戳转换、JWT 解析、正则测试、哈希/UUID 生成……——收进一个**轻量、离线、跨平台**的应用。

和既有工具箱相比，Pulsar 的重点不是"工具更多"，而是：

- **离线 & 隐私** — 纯本地处理，零网络请求，数据不出本机。
- **快 & 轻** — Rust 内核，单文件小、冷启动快。
- **智能 & 串联** — 粘贴即识别工具 (Smart Detection) + 工具可串联 (Pipeline)，并提供 CLI 进 CI/脚本。

技术栈：Tauri v2 + React + Rust

> 项目状态：设计提案已完成（见 `specs/proposals/`），实现待开始。

## 快速开始

### 开发

```bash
cd pulsar
yarn install
yarn tauri:dev
```

### CLI（实现后）

```bash
# 与 GUI 共享同一份工具逻辑
pulsar json fmt < input.json
echo "aGVsbG8=" | pulsar base64 -d
pulsar uuid --count 5
```

### 代码检查（提交前 / CI）

```bash
# 前端：类型检查 + Lint
yarn typecheck
yarn lint

# Rust：格式化 + Clippy + 测试
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### 构建（发布 / 打包）

```bash
cd pulsar
yarn install
yarn tauri:build
```

构建产物位于 `target/release/`（workspace 根在 `pulsar/`）。

## 工具分类

| 分类 | 示例 |
|------|------|
| Converters | JSON↔YAML↔TOML、时间戳、进制、Cron、颜色 |
| Encoders / Decoders | Base64、URL、Hex、JWT、HTML 实体 |
| Formatters | JSON、SQL、XML、HTML/CSS/JS |
| Generators | UUID/ULID、哈希、密码、QR 码 |
| Testers | 正则、JSONPath、Diff |
| Text | 大小写转换、统计、去重排序、Slug |
| Graphic | 图片压缩/转换、取色器、对比度检查 |
| Reference | HTTP 状态码、MIME、Chmod、CIDR |

完整清单与优先级见 [specs/proposals/vision-and-scope.md](specs/proposals/vision-and-scope.md)。

## 文档

- `AGENTS.md` — 架构设计与开发规范
- `specs/proposals/vision-and-scope.md` — 愿景、定位、工具目录、信息架构
- `specs/proposals/architecture.md` — 分层架构、工具注册表、GUI/CLI/Pipeline 共享内核
- `specs/proposals/roadmap.md` — 分阶段开发路线图

## License

This project is licensed under the GNU Affero General Public License v3.0 - see the [LICENSE](../LICENSE) file for details.
