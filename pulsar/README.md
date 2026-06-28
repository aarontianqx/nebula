# Pulsar

`Pulsar` 是面向开发者的**本地工具工作台**（桌面 GUI + CLI）：把每天要用的几十个小工具——JSON 格式化、Base64 编解码、时间戳转换、JWT 解析、正则测试、哈希/UUID 生成……——收进一个**轻量、离线、跨平台**的应用。

和既有工具箱相比，Pulsar 的重点不是"工具更多"，而是：

- **离线 & 隐私** — 纯本地处理，零网络请求，数据不出本机。
- **快 & 轻** — Rust 内核，单文件小、冷启动快。
- **智能 & 串联** — 粘贴即识别工具 (Smart Detection) + 工具可串联 (Pipeline)，并提供 CLI 进 CI/脚本。

技术栈：Tauri v2 + React + Rust

> 项目状态：Phase 1（内核 + MVP）实质完成，Phase 2/3 进行中。已实现 **30 个工具** + Smart Detection + Command Palette + **CLI**；Pipeline、工作流/剪贴板自动化、大文件流式仍在路线图上（见 `specs/proposals/roadmap.md`）。

## 快速开始

### 开发

```bash
cd pulsar
yarn install
yarn tauri:dev
```

### CLI

`pulsar-cli` 与 GUI 共享同一份 `pulsar-core` 工具逻辑（零重复）；子命令由工具注册表**动态派生**，新增工具即自动出现在 CLI 中。二进制名为 `pulsar`（workspace 根在 `pulsar/`）。

#### 编译 / 安装（任选其一）

```bash
cd pulsar

# 方式 A：开发编译，产物在 target/debug/pulsar（运行需 ./ 前缀）
cargo build -p pulsar-cli
./target/debug/pulsar list

# 方式 B：cargo run（自动编译再跑；程序参数放在 -- 之后）
cargo run -p pulsar-cli -- uuid --count 5

# 方式 C：安装到 PATH，之后任意目录直接敲 pulsar（推荐日常用）
#   cargo install 默认即 release 优化构建（无需也不接受 --release；
#   想装 debug 版加 --debug）。装到 ~/.cargo/bin/pulsar。
cargo install --path crates/pulsar-cli
pulsar list

# 发布用、压体积的 release 二进制：target/release/pulsar
cargo build -p pulsar-cli --release
```

#### 使用

```bash
# 主输入走 stdin（管道）或位置参数；结果到 stdout，错误到 stderr + 非零退出码
echo aGVsbG8= | pulsar base64 --mode decode
cat data.json  | pulsar json
pulsar uuid --count 5
echo "255"     | pulsar number_base

pulsar list                 # 列出全部工具（--json 供脚本消费）
echo '{"a":1}' | pulsar detect      # 智能识别并推荐工具（--json 同上）
pulsar base64 --help        # 每个工具的参数 = 其 descriptor，自动生成
```

约定：布尔参数为 `--flag` / `--no-flag`；其余为 `--key <值>`；短命令名（`base64`）与完整 id（`encoders.base64`）均可用。

#### Shell 自动补全

`completions` 子命令生成补全脚本（含全部工具与 flag）；新增工具后重新生成一次即可。

```bash
pulsar completions zsh  > ~/.zfunc/_pulsar          # zsh（确保该目录在 fpath 中）
pulsar completions bash > /usr/local/etc/bash_completion.d/pulsar
pulsar completions fish > ~/.config/fish/completions/pulsar.fish
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

已实现 **30 个工具**（截至当前）：

| 分类 | 已实现工具 |
|------|------|
| Converters | JSON↔YAML、TOML↔JSON/YAML、XML↔JSON、JSON↔CSV、时间戳、进制、Cron、颜色 |
| Encoders / Decoders | Base64、URL、Hex、JWT（仅解码）、HTML 实体、Unicode |
| Formatters | JSON、SQL、XML |
| Generators | UUID/ULID/NanoID、哈希、密码、HMAC、Bcrypt、QR 码 |
| Testers | 正则、JSONPath、文本 Diff |
| Text | 大小写转换、统计、去重/排序/去空白、Slug |

**规划中（尚未实现）**：Formatters 的 HTML/CSS/JS、Graphic（图片压缩/转换、取色器、对比度）、Reference（HTTP 状态码、MIME、Chmod、CIDR）、代码生成器等 —— 见路线图。

完整清单与优先级见 [specs/proposals/vision-and-scope.md](specs/proposals/vision-and-scope.md)；进度见 [specs/proposals/roadmap.md](specs/proposals/roadmap.md)。

## 文档

- `AGENTS.md` — 架构设计与开发规范
- `specs/proposals/vision-and-scope.md` — 愿景、定位、工具目录、信息架构
- `specs/proposals/architecture.md` — 分层架构、工具注册表、GUI/CLI/Pipeline 共享内核
- `specs/proposals/roadmap.md` — 分阶段开发路线图

## License

This project is licensed under the GNU Affero General Public License v3.0 - see the [LICENSE](../LICENSE) file for details.
