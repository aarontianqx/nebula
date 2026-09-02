# WLY 协议层调研：从 UI 自动化到协议驱动

> 调研时间：2026-09-02（当日复核通过）｜ 状态：侦察完成，结论已验证 ｜ 性质：point-in-time 调研记录，后续以代码为准
>
> 落地决策与实施计划见 [protocol-driven-automation.md](protocol-driven-automation.md)（已采纳形态 2，本文档 §7.2 的待定事项已关闭）。

## 摘要

本次调研回答了一个问题：**能否不经过 UI（点击 / OCR / 截图识别），直接通过游戏自身的协议层驱动《卧龙吟》H5？**

结论：**完全可以，且成本极低。** 游戏的全部网络通信收敛在一个暴露在页面全局的 `Connection` 类上，协议注册表包含 1233 个语义化协议名，所谓"加密"只是硬编码密钥的混淆。已在真实账号上完成端到端验证：注入页面后直接调用游戏协议层请求邮件列表，收到 50 封邮件的完整结构化数据。

这意味着 wardenly-rs 的"场景识别 + 模拟点击"路线之外存在一条**协议驱动**路线：更稳定（不依赖图像识别）、更快（无 UI 等待）、可 headless。落地决策（形态 2：集成进受管浏览器）与实施计划见 [protocol-driven-automation.md](protocol-driven-automation.md)。

---

## 1. 背景与动机

### 1.1 wardenly-rs 现状

wardenly-rs 是 WLY 页游自动化桌面应用（Tauri v2 + React + Rust），当前技术路线为：

- chromiumoxide 驱动 headless 浏览器，加载游戏页面
- 周期性截图同步画布，OCR + 场景定义（YAML）做状态识别
- 模拟鼠标点击 / 键盘输入执行脚本

这条路线通用性最强（不碰游戏内部），但天然脆弱：识别依赖图像与坐标，UI 改版、分辨率变化、动画时序都会导致脚本失效；且每个动作都要走完"截图→识别→点击→等待"回路，速度慢。

### 1.2 调研起点

一个前置认知（从对其他系统的前端脚本分析中得出的通用结论）：**运行在页面上下文里的脚本拥有页面的全部能力，客户端加密只是混淆**。如果游戏把协议层暴露在 JS 侧，就可以"在客户端内部驾驶客户端"——不逆向协议、不破解加密，直接调用游戏自己的函数收发消息。

本次调研就是验证这个假设在 WLY 上是否成立。结果：成立，且暴露程度远超预期。

### 1.3 调研工具与环境

- **agent-browser**（CDP 驱动的 Chrome 自动化 CLI），session 名 `wly-game-recon`（已关闭）
- 真实账号：区服 888（双线），账号与凭据不记录于本文档
- 静态分析：直接下载游戏 JS bundle 到本地 grep

---

## 2. 入口链（三层嵌套）

```
[1] http://www.lequ.com/server/wly/s/888/ish5/888        账号密码登录页
      ↓ 登录成功，生成 ticket
[2] http://s888.wly.h5.lequ.com/index.php?...&ticket=...  区服入口页（平台 PHP 后端）
      ↓ 服务端生成一次性 content 票据，写入 iframe src
[3] http://s1res.lequ.com/wlyh5/mobile_v614_1334_cn.html?t=<ts>&content=<加密票据>
      游戏本体（Cocos Creator 2.4.0，纯 canvas，无 DOM 交互元素）
```

要点：

- 第 2 层 URL 携带 `account_unique_id`、`accname`、`serverid`、`tstamp`、`ticket` 及 base64 编码的实名信息。**ticket 与 content 票据均为短期有效**，实测约 5 分钟后复用会导致游戏卡死（见 §6.3）。
- 第 2 层页面本身又会把第 3 层游戏页塞进 `#gameIframe`。三层页面分属三个不同子域（`www.` / `s888.wly.h5.` / `s1res.`），**跨域 iframe 之间 JS 不可互访**（未做 `document.domain` 对齐，实测确认）。
- 对自动化的含义：要拿到可编程的游戏页面上下文，最简单的办法是从第 2 层 DOM 中读出 `#gameIframe.src`，然后**让浏览器直接以顶层页面身份打开第 3 层 URL**（http，见 §6.1）。

---

## 3. 游戏技术栈

| 项 | 值 |
|---|---|
| 引擎 | Cocos Creator **v2.4.0** |
| 渲染 | 单一 `<canvas id="GameCanvas">`，无 DOM UI，Accessibility 树为空 |
| 模块系统 | browserify 风格，`window.__require('<ModuleName>')` 可按名取模块 |
| 游戏 bundle | `https://s1res.lequ.com/wlyh5/assets/main/index.6a9bb.js`（约 11 MB，未混淆到不可读的程度，类名 / 协议名 / 中文字符串齐全） |
| 其他全局 | `Zlib`（资源解压）、`spine`（骨骼动画）、`uqeeSdk`（平台 SDK 桥） |
| 调试 | 生产构建 `debugMode: ERROR`，`cc.log` 协议收发日志默认被抑制 |

游戏源码模块路径语义清晰，关键模块：

- `network/Connection` — 唯一网络连接类（单例）
- `network/protocol/ProtocolBase` — 协议注册表（`Protocol` 枚举 + `PROTOCOL_STRUCTS` 编解码结构）
- `login/Login` — 登录流程编排
- `utils/CryptUtil` — AES 加解密
- `network/DataBuffer` — 二进制读写

---

## 4. 传输层

### 4.1 连接形态

- **单个 WebSocket 长连接**：`ws://s888.wly.h5.lequ.com:443`（**明文 WS 跑在 443 端口，不是 TLS**）
- 连接地址由 `Login.start()` 拼接：

  ```js
  // 非 SSL 分支（当前 888 区实测 isSsl=false，走这里）
  address = "ws://{server}:{port}"
  // SSL 分支（isSsl 且 channelHandler.sslHost 非空时）
  address = "wss://{sslHost}:{port}?UQEE_INTERNAL_HOST={server}"
  ```

- 账号配置中 `sslHost = cls.uqeegame.com`（厂商 SSL 中转网关），但 `isSsl=false` 所以未启用。实测手动向 `wss://cls.uqeegame.com:443?...` 与 `wss://s888.wly.h5.lequ.com:443` 发起连接均立即 1006 关闭——**443 端口只讲明文 WS**。
- 心跳：服务端定时推 `S_2_C_KEEP_ALIVE`（协议 id 4），携带服务器时间 `cur_time` 与 `time_diff`。
- **单会话顶号**：重复登录会收到 `S_2_C_KICK_OUT` 踢掉旧会话。自动化脚本与手动游戏互斥。

### 4.2 报文格式（静态分析自 `Connection` 类）

帧为二进制（`binaryType = "arraybuffer"`），大端序。两种模式，由连接建立后硬编码开启 secure 模式：

**非 secure 模式**（未实际使用，仅作对照）：

```
| int32 totalLen | int32 protocolId | payload |
```

**secure 模式**（实际使用）：

```
| int32 totalLen(=4+加密体长度) | AES-CBC 加密体 |
                                | int32 tick      |  自增计数
                                | byte[16] md5    |  MD5(protocolId + payload)，完整性校验
                                | int32 protocolId|
                                | payload         |  按 PROTOCOL_STRUCTS 定义编解码
```

**AES 密钥硬编码在客户端**，`Connection._onOpen` 中赋值：

```
sessionKey = "P%2BViyZLtO^gRT2Huxqx#5Vygbfl$8m"
```

即：你在抓包工具里看到的"加密乱码"，密钥随客户端下发，仅属混淆。

复核确认（2026-09-02）：该密钥是 bundle 中的字符串字面量（全 bundle 仅 1 处），**不随登录、不随连接变化**——每次 `_onOpen` 赋的都是同一个常量；仅游戏版本更新（bundle 变化）时才可能漂移。运行时读取 `Connection.get()._sessionKey` 与该字面量逐字符一致。且协议驱动路线**根本不需要碰这层加密**，见 §5。

### 4.3 协议注册表

`ProtocolBase` 模块导出：

- `Protocol`：协议名 → 数值 id 的枚举。**C_2_S_ 开头的协议名共 1233 个**，另有对应的 S_2_C_ 下行协议。
- `PROTOCOL_STRUCTS`：数值 id → 字段编解码结构（`Connection._parsePacket` 用它把 payload 解析成 JS 对象）。

实测取样（真实流量确认）：

| 协议名 | id | 方向 | 含义 |
|---|---|---|---|
| `S_2_C_KEEP_ALIVE` | 4 | 下行 | 心跳（服务器时间） |
| `C_2_S_MAIL_INFO` | 154 | 上行 | 请求邮件列表 |
| `S_2_C_MAIL_INFO` | 2173 | 下行 | 单封邮件记录 |
| `S_2_C_MAILLIST_ID` | 2177 | 下行 | 邮件列表（`mailNums` + `MailIdTypes[]`） |

协议名高度语义化，日常任务相关的直接可读，例如：

```
C_2_S_ACTIVITY_SIGN_IN                  签到
C_2_S_MAIL_INFO / C_2_S_MAIL_DRAW_ALL_REWARD   邮件 / 一键领取附件
C_2_S_ONLINE_REWARD_INDEX               在线奖励
C_2_S_ACTIVITY_DAILY_REWARD_DRAW        每日抽奖
C_2_S_ANCIENT_CITY_*                    古城副本系列
C_2_S_CROSS_SERVER_WORLD_BOSS_*         跨服世界BOSS系列
```

完整清单可从 bundle 中grep：`grep -o 'C_2_S_[A-Z0-9_]\+' wly_game_main.js | sort -u`。

---

## 5. 驾驶方式（核心结论）

### 5.1 调用游戏自己的协议层

游戏模块系统未做隔离，`__require` 全局可用。发送任意协议只需：

```js
const c = __require('Connection').default.get();   // Connection 单例
const P = __require('ProtocolBase').Protocol;       // 协议枚举

c.send(P.C_2_S_MAIL_INFO, {});                      // 发送（加密/组包由游戏代劳）
c.on(P.S_2_C_MAILLIST_ID, (data) => { ... }, 'myKey');  // 注册响应回调
c.off(P.S_2_C_MAILLIST_ID, 'myKey');
```

`Connection.send()` 内部完成：查 `PROTOCOL_STRUCTS` → 编码 payload → tick 自增 → MD5 → AES → 加长度头 → `socket.send()`。**调用方完全不接触二进制与密码学。**

### 5.2 观测全部下行流量

注册回调需要预先知道响应协议名。更通用的做法是给解析函数打补丁（实例属性 patch，对动态查找生效）：

```js
const orig = c._parsePacket.bind(c);
c._parsePacket = function (struct, buf) {
  const data = orig(struct, buf);          // 游戏自己解析成对象
  // 在此记录 data（可顺带做 id→name 反查）
  return data;
};
```

### 5.3 端到端验证记录（2026-09-02）

在真实账号（888 区，等级 230）上完成：

1. 按 §2 方式直达游戏页（http + init script 注入 hook）；
2. 点过用户协议后游戏自动连接 `ws://s888.wly.h5.lequ.com:443` 并登录进主城；
3. 页面内执行 `c.send(P.C_2_S_MAIL_INFO, {})`；
4. 收到完整响应：`S_2_C_MAILLIST_ID`（`mailNums: 50`，含每封邮件的 `mail_id / mail_type / mail_status / get_status / mail_sendtime`）+ 3 条单邮件记录 + 心跳。

全程未模拟任何点击，未解密任何字节。

---

## 6. 侦察过程中踩过的坑（后续实施必读）

### 6.1 https 升级导致连接失败（最大的坑）

Chrome 的 HTTPS-First 机制会把 `http://s1res.lequ.com/...` 自动升级为 https。而游戏要连的是**明文 `ws://`**，https 页面发起 ws 会被 mixed-content 策略直接拦死（`new WebSocket` 抛 SecurityError，游戏静默卡死在开场画面，无任何流量、无报错日志）。

现象：游戏日志停在 `登陆参数` 一行，`Connection._socket` 为 null。

解法：启动 Chrome 时加 `--disable-features=HttpsUpgrades`（agent-browser 下为 `--args "--disable-features=HttpsUpgrades"`），让页面保持 http。游戏官方运行形态本来就是 http（三层 iframe 全是 http），所以这不影响正常玩法。

### 6.2 `dispatchEvent` 补丁拦不到下行帧

浏览器原生触发的事件分发**不经过** `WebSocket.prototype.dispatchEvent` 的 JS 覆写，因此 send 能拦、onmessage 收不到。可靠做法按优先级：

1. 直接包 `socket.onmessage`（对已有连接即时生效）；
2. init script 里覆写 `onmessage` 的 setter / `addEventListener`（从 boot 起生效）；
3. 最省心：不管 socket，直接 patch `Connection._parsePacket`（§5.2）。

### 6.3 content 票据短时效 + 单会话顶号

第 3 层 URL 的 `content` 票据几分钟后复用会导致游戏卡死（同样是停在 `登陆参数`，无报错）。每次启动应从第 2 层页面现取 `#gameIframe.src`。另外重复登录互踢（`S_2_C_KICK_OUT`），调试时注意别让两个页面同时在线。

### 6.4 从 boot 捕获需要 init script

游戏在页面加载早期就建立 WS。要捕获完整登录流量，hook 必须通过 `Page.addScriptToEvaluateOnNewDocument` 级机制注入（agent-browser 的 `--init-script <path>`），事后 eval 注入会错过握手。

### 6.5 eval 的执行上下文

agent-browser 的 `eval` 只在**顶层页面**执行，无法进入跨域 iframe。这是必须"直达第 3 层 URL"的原因（§2）。

---

## 7. 对 wardenly-rs 的意义

### 7.1 两条路线对比

| 维度 | 现状：OCR + 场景识别 + 点击 | 协议驱动 |
|---|---|---|
| 稳定性 | 受 UI 改版 / 分辨率 / 动画时序影响 | 只依赖协议 id 与字段结构，版本更新时可能漂移但 diff 可审计 |
| 速度 | 每动作需截图-识别-点击-等待 | 直接收发消息，毫秒级 |
| 实现成本 | 需维护场景 YAML、OCR 规则 | 从 bundle 提取协议定义即可，1233 个协议语义化命名 |
| 可观测性 | 只能看画面 | 全量结构化下行数据（精确状态，无需识别） |
| 被检测面 | 行为像人（点击轨迹） | 无 UI 事件，纯协议流量，行为指纹明显（需自行拟人化节奏） |
| 维护风险 | 前端 UI 变更 | 游戏版本更新可能改协议 / 换密钥（bundle 可重新下载分析） |

### 7.2 可行形态（已决策）

调研时评估了三种形态：① Tampermonkey 脚本（最轻量，依赖用户自己开浏览器）；② 集成进 wardenly-rs 受管浏览器（协议编排 + 画布兜底，与现有架构兼容最好）；③ 独立脱机客户端（威力最大，工程量与维护成本最高，且需逆向登录票据链）。

**决策：采纳形态 2。** 落地理由与分阶段计划见 [protocol-driven-automation.md](protocol-driven-automation.md)，此处不再展开。

### 7.3 风险与约束

- **封号风险自负**：协议驱动绕过了全部 UI 行为，理论上服务端可做行为指纹检测（无页面事件、固定间隔、非人类速度）。实施时必须拟人化节奏（随机间隔、控制在线时长）。
- **协议漂移**：游戏版本更新（当前 `mobile_v614_1334`）可能改协议 id / 字段 / 密钥。bundle 可随时重新下载做 diff。
- **合规**：仅限本人账号自用，不做多开、不做规模化。

---

## 8. 调研产物清单

> 以下产物均在 `/tmp` 下，属临时文件，可能被系统清理；关键结论已全部固化在正文，产物仅用于复查原始材料。

| 产物 | 位置 | 说明 |
|---|---|---|
| WS/XHR hook 脚本 | `/tmp/wly_hook.js` | 可作 agent-browser `--init-script` 注入；含 send/onmessage/XHR/fetch 全量捕获 |
| 游戏主 bundle | `/tmp/wly_game_main.js` | 11 MB，`Connection` / `ProtocolBase` / `Login` 等全部协议定义在内 |
| bundle 引导文件 | `/tmp/wly_main.js`, `/tmp/wly_sdk.js`, `/tmp/wly_settings.js` | 启动流程与 jsList |
| 最近一次游戏 URL | `/tmp/wly_game_url.txt` | 含 content 票据，已过期，仅作格式参考 |
| 过程截图 | `/tmp/wly_*.png` | 登录页 / 协议弹窗 / 主城 / 直连游戏各阶段 |

关键代码位置（bundle 内检索关键词）：

- `Connection` 类：搜 `"Connection"` 模块定义（含 `connect` / `socketSend` / `_onMessage` / `_parsePacket`）
- AES 密钥：搜 `P%2BViyZLtO`（位于 `_onOpen`）
- 登录流程：搜 `\u767b\u9646\u53c2\u6570`（"登陆参数"日志，位于 `Login.start`）
- 协议枚举：搜 `C_2_S_MAIL_INFO`

## 9. 复现步骤（精简版）

```bash
# 1. 起 agent-browser，关闭 HTTPS 自动升级
export AGENT_BROWSER_SESSION="wly-recon"

# 2. 打开区服入口页，从 DOM 取最新游戏 URL（content 票据短时效，必须现取）
agent-browser open "http://s888.wly.h5.lequ.com/index.php?<登录参数，见 §2>"
agent-browser eval --stdin  # => document.getElementById('gameIframe').src

# 3. 带 hook 直达游戏页（保持 http）
agent-browser --args "--disable-features=HttpsUpgrades" \
  --init-script /tmp/wly_hook.js open "<上一步的 src>"

# 4. 点"同意"用户协议（坐标点击，canvas 无 DOM），游戏自动连接+登录

# 5. 驾驶
agent-browser eval --stdin  # => 见 §5.1 / §5.2 代码片段
```
