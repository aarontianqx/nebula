# Pulsar — 愿景与范围

> 状态：提案 (Proposal) · 起草于 2026-06
> 本文档为点时点设计 (point-in-time)，落地后以 `AGENTS.md` 与 `specs/features/` 为准。

## 1. 是什么

**Pulsar** 是面向开发者的**本地工具工作台 (local developer workbench)** —— 把开发者每天要用的几十个小工具（JSON 格式化、Base64、时间戳转换、JWT 解析、正则测试……）收进一个**轻量、离线、跨平台**的桌面 App。

- 技术栈：Tauri v2 + React + Rust（与 `nebula/tap`、`nebula/wardenly-rs` 一致）。
- 命名：取自"脉冲星"——宇宙中最精密的天然时钟，规律地发出信号。隐喻"精确、快速、可靠的工具脉冲"。

## 2. 为什么要做（动机）

这个赛道高度饱和：DevToys（31k★，C#/WinUI）、IT-Tools、BrowserUtils（299 工具）、tool.lu 等。**单纯再做一个"工具集合"没有意义**。Pulsar 的价值不在"有哪些工具"，而在补齐现有方案的四个结构性缺陷：

| 现有方案的痛点 | Pulsar 的机会 |
|----------------|---------------|
| DevToys 以 C#/WinUI 为主，macOS/Linux 体验割裂；网页工具要么不安全要么离线不可用 | Tauri + Rust 天然跨平台、单文件 <10MB、冷启动快、纯离线、内存占用约为 Electron 的 1/10 |
| 工具之间是**孤岛**：格式化完想转格式得换工具，反复复制粘贴 | **工具可串联 (Pipeline)**：上一个工具的输出直接喂给下一个 |
| 几乎都**没有批处理与自动化**，只能一次处理一段文本 | 接续 `tap` 的自动化基因：**可保存的工作流 + 剪贴板监听自动处理 + CLI** |
| 网页工具受浏览器内存限制，**大文件处理弱** | Rust 后端可**流式处理** GB 级 JSON / 日志 / CSV |

## 3. 定位与卖点

**一句话定位**：把每天要用的几十个小工具，收进一个轻量、离线、跨平台的桌面工作台。

**Slogan（候选）**：
- Every dev tool, one pulse away.
- Tiny tools. Zero latency. Total privacy.
- 中文："趁手的工具，离线即用"

**三个核心卖点**：

1. **Local & Private** — 纯本地处理，零网络请求，数据不出本机。
2. **Fast & Light** — Rust 内核，单文件 <10MB，冷启动 <1s。
3. **Smart & Connected** — 粘贴即识别工具 (Smart Detection) + 工具可串联 (Pipeline)，区别于"一堆孤立小工具"。

**目标用户**：开发者本人及同类开发者的日常使用（通用，程序员全员适用）。

## 4. 差异化武器（核心，优先级高于堆工具数量）

以下能力才是让用户从 DevToys 切换过来的理由，也最契合本仓库技术栈与 `tap` 的自动化基因：

1. **Smart Detection（粘贴即识别）** — 顶部全局粘贴框，自动判断内容是 JSON / JWT / Base64 / 时间戳 / URL / 颜色，给出候选工具并一键跳转。
2. **Pipeline（工具串联）** — 例：`Base64 解码 → 识别为 gzip → 解压 → 识别为 JSON → 格式化`，一条链路自动跑完。竞品几乎没有。
3. **工作流保存 + 剪贴板监听自动化** — 复用 `tap` 思路。例：开启监听后，每次复制 JSON 自动格式化回剪贴板；保存固定清洗流程一键复用。
4. **CLI 双形态** — GUI 与 CLI 共享同一 `pulsar-core`，零成本得到 `pulsar json fmt < in.json`，可进 CI / 脚本。
5. **大文件 / 流式处理** — 突破浏览器内存上限，处理数百 MB 的 JSON / 日志 / CSV。
6. **隐私 + 离线 + 轻量** — 对"把数据粘到不可信网站"的最佳替代。

## 5. 工具目录与信息架构 (IA)

收敛为 **8 个一级分类**。优先级：**P0**=首发必做（高频刚需），**P1**=第二批，**P2**=加分/长尾。

> ✅ = 已实现（截至 Phase 1：全部 P0 + 纯文本/逻辑类 P1，共 30 个工具）。图片类（需二进制 IPC 链路 + 图像库）与 HTML/CSS/JS 等需重依赖的格式化工具仍待办。

### 5.1 Converters（转换）
| 工具 | 优先级 |
|------|--------|
| JSON ↔ YAML 互转 | P0 ✅ |
| 时间戳 ↔ 日期（秒/毫秒、多时区） | P0 ✅ |
| 进制转换 (2/8/10/16) | P0 ✅ |
| JSON ↔ CSV | P1 ✅ |
| Cron 表达式解析（人类可读 + 下次执行） | P1 ✅ |
| 颜色格式 (HEX/RGB/HSL) | P1 ✅ |
| XML ↔ JSON | P1 ✅ |
| TOML ↔ JSON / YAML | P1 ✅ |
| 单位换算（字节/温度/角度/时长…） | P2 |
| 罗马数字 / NATO 字母 | P2 |

### 5.2 Encoders / Decoders（编解码）
| 工具 | 优先级 |
|------|--------|
| Base64（文本） | P0 ✅ |
| URL 编解码 | P0 ✅ |
| Hex ↔ 文本 | P0 ✅ |
| JWT 解析（header/payload） | P0 ✅ |
| HTML 实体 转义/反转义 | P1 ✅ |
| Unicode 转义 | P1 ✅ |
| Base32 / Base58 | P2 |
| ROT13 / 摩斯电码 | P2 |

### 5.3 Formatters（格式化）
| 工具 | 优先级 |
|------|--------|
| JSON 格式化 / 压缩 / 校验 | P0 ✅ |
| SQL 格式化 | P1 ✅ |
| XML 格式化 | P1 ✅ |
| HTML / CSS / JS 格式化 | P1 |
| YAML / TOML 格式化 | P1 |

### 5.4 Generators（生成器）
| 工具 | 优先级 |
|------|--------|
| UUID / NanoID / ULID | P0 ✅ |
| 哈希 (MD5 / SHA-1/256/512 / CRC32) | P0 ✅ |
| 强密码生成 + 强度/熵分析 | P1 ✅ |
| HMAC | P1 ✅ |
| Bcrypt 生成 / 校验 | P1 ✅ |
| QR 码生成 (文本，SVG / ASCII) | P1 ✅ |
| Lorem Ipsum / Mock 测试数据 | P2 |
| .gitignore / Dockerfile / docker-compose 生成 | P2 |

### 5.5 Testers（测试 / 校验）
| 工具 | 优先级 |
|------|--------|
| 正则测试器（分组 + 匹配预览） | P0 ✅ |
| JSONPath 查询 | P0 ✅ |
| 文本 / JSON Diff | P0 ✅ |

### 5.6 Text（文本处理）
| 工具 | 优先级 |
|------|--------|
| 大小写 / 命名转换 (camel/snake/kebab/CONSTANT) | P0 ✅ |
| 字符 / 字数统计 | P1 ✅ |
| 去重 / 排序 / 去空白 | P1 ✅ |
| Slug 生成 | P1 ✅ |
| 不可见字符检测 | P2 |
| Unicode 字符查询 | P2 |

### 5.7 Graphic（图形 / 图片）
| 工具 | 优先级 |
|------|--------|
| 图片压缩 (PNG / JPEG / WebP) | P1 |
| 图片格式转换 / 尺寸调整 | P1 |
| 取色器 + 对比度检查 (WCAG) + 调色板 | P1 |
| QR 码识别（截图 / 上传） | P2 |

### 5.8 Reference（速查表）
| 工具 | 优先级 |
|------|--------|
| HTTP 状态码 | P2 |
| MIME 类型 | P2 |
| Chmod 计算器 | P2 |
| CIDR 子网计算器 | P2 |
| Git / Cron / Regex 速查 | P2 |

### 5.9 跨分类的全局能力（产品骨架）
- **Smart Detection**：全局粘贴框 → 内容类型识别 → 工具候选。
- **Pipeline**：工具串联（V2 引入）。
- **Command Palette**：`Ctrl/Cmd+K` 模糊搜索全部工具。
- **收藏 / 最近使用**：常用工具置顶。
- **Compact 浮窗模式**：小窗常驻置顶（借鉴 DevToys）。

## 6. 非目标 (Non-goals)

- 不做需要联网的工具（DNS 实时查询、WHOIS、在线 API 调试）——与"纯离线"定位冲突，最多提供"生成命令/参考"的离线版本。
- 不做账号体系、云同步、协作。
- 首版不做插件市场（架构预留，见架构文档）。

## 7. 竞品参考

| 产品 | 形态 | 借鉴点 |
|------|------|--------|
| DevToys | 桌面 (C#/WinUI) | Smart Detection、Compact 模式、CLI、分类体系 |
| IT-Tools | 网页 (Vue) | 工具广度与命名 |
| BrowserUtils | 网页 | 299 工具的分类收敛方式 |
| tool.lu | 网页 (中文) | 中文用户高频工具 |

## 8. 相关文档

- 架构设计：[`architecture.md`](./architecture.md)
- 路线图：[`roadmap.md`](./roadmap.md)
