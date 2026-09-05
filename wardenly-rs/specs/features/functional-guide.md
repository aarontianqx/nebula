# Wardenly - 功能说明

## 概述

Wardenly 是一款用于 WLY 网页游戏自动化的桌面控制工具。通过 headless 浏览器运行游戏，提供实时画面显示、手动操作和自动化脚本执行功能。

## 账户与分组

### 账户管理

账户字段：
- **RoleName**: 游戏内角色名
- **UserName**: 登录用户名
- **Password**: 登录密码
- **ServerID**: 游戏服务器 ID
- **Ranking**: 排序优先级（数值越小越靠前）

账户在界面中显示为 `ServerID - RoleName` 格式（如 `126 - 追风`）。

账户下拉框按 `(Ranking ASC, ID ASC)` 排序，低 Ranking 的账户优先显示。

### 分组管理

分组字段：
- **Name**: 分组名称
- **Description**: 可选描述
- **Ranking**: 排序优先级（数值越小越靠前）
- **Accounts**: 分组内的账户列表

分组用于批量启动多个账户：
- 选择分组后点击 "Run Group" 依次启动所有成员账户（间隔 3 秒）
- 已运行的账户自动跳过
- 分组下拉框按 `(Ranking ASC, Name ASC)` 排序

### 管理操作

点击工具栏 **Manage** 按钮打开管理对话框，进行账户和分组的增删改查。

## 设置

点击工具栏 **Settings** 按钮打开设置对话框。

### 主题

选择应用界面配色主题。可选主题由应用内置，包括：
- ocean-dark (默认)
- slate-light
- midnight
- forest-dark

### 存储

选择数据存储后端：
- **SQLite (Local)**: 默认选项，本地文件存储
- **MongoDB (Remote)**: 远程数据库，支持多设备同步

如果选择 MongoDB，需填写连接 URI 和数据库名称：

- **Test Connection**: 点击测试连接，3 秒内返回结果
- **保存前验证**: 保存时自动验证连接，失败则无法保存
- **启动时回退**: 应用启动时若 MongoDB 不可达，自动回退到 SQLite 并显示警告

### 浏览器缓存

显示所有账户浏览器配置文件的总缓存大小，提供清除选项：

- **Total Cache Size**: 显示缓存占用磁盘空间
- **Clear All Cache**: 清除所有账户的浏览器缓存

> 浏览器缓存（Cookies、LocalStorage、图片缓存等）会在每次启动会话时复用，加速页面加载。如遇显示异常或需释放磁盘空间，可手动清除。

> **注意**：设置更改需要重启应用生效。

## 会话管理

### 启动会话

**单账户启动**:
1. 从下拉框选择账户
2. 点击 "Run" 按钮
3. 系统启动浏览器并自动登录

**Run 按钮下拉菜单** (点击 Run 右侧箭头):
- **Force Clean Start**: 清除该账户缓存后启动（适用于游戏更新后卡加载）
- **Clear Account Cache**: 仅清除缓存，不启动

**分组批量启动**:
1. 从下拉框选择分组
2. 点击 "Run Group"
3. 依次启动分组内所有账户

### 会话列表

左侧边栏显示所有运行中的会话，点击可切换当前操作的会话。

状态指示器：
- 🔴 脚本运行中
- (无) 待机状态

## 画布窗口

画布窗口显示当前选中会话的浏览器画面。

### 显示模式

| 模式 | 说明 |
|------|------|
| Screencast | 勾选后以 ~3 FPS 流式传输画面（仅当前选中会话） |
| 停止（默认） | 取消勾选后，停止流式传输以节省性能 |

**设计原则**：
- Screencast 是**全局配置**，同时最多只有一个会话在传输画面
- 切换会话时，自动停止旧会话的传输并启动新会话（画面无缝衔接）
- **默认关闭** - 启动会话时不会自动开始传输，需手动勾选 Screencast
- **自动选中** - 新启动的会话自动成为当前选中会话；关闭会话时自动选中下一个

### 交互操作

| 操作 | Screencast 开启 | Screencast 关闭 |
|------|----------------|----------------|
| 点击画布 | 更新 Inspector + 发送点击到浏览器 | 更新 Inspector + 截取一帧刷新画布 |
| 拖拽画布 | 发送拖拽到浏览器 | 截取一帧刷新画布 |

| 配置 | 说明 |
|------|------|
| Spread to All | 启用后点击/拖拽事件并发发送到所有活跃会话 |

**注意**：Screencast 关闭时，点击画布不会执行浏览器点击，而是截取当前画面。如需执行点击，请使用 Inspector 面板的 Click 按钮。

### 键盘透传

勾选工具栏 "Keyboard Passthrough" 启用，按键事件转换为画布点击。

**触发规则**：
- 仅 A-Z 共 26 个字母键生效
- 仅当鼠标在画布区域内时触发
- 按键后快速释放 (<300ms)：触发一次点击
- 按住超过 300ms：每 100ms 触发一次点击

> **注意**：键盘监听在前端实现，无需系统级权限。

## 浏览器控制

会话详情面板提供以下控制按钮：

| 按钮 | 功能 |
|------|------|
| Stop | 停止当前会话，关闭浏览器 |
| Stop All | 停止所有会话 |
| Refresh | 刷新当前页面 |

## 脚本控制

### 脚本操作

| 按钮 | 功能 |
|------|------|
| Start | 启动选中脚本（当前会话） |
| Stop | 停止当前脚本 |
| Sync | 同步脚本选择到所有会话 |
| Start All | 启动所有会话的脚本（同时） |
| Start All ▾ → Staggered Start All | 逐个启动脚本，间隔 1 秒，跳过已在运行脚本的会话 |
| Stop All | 停止所有会话的脚本 |

### 执行逻辑

脚本由多个步骤组成：
1. **场景匹配**：通过颜色点检测当前画面
2. **动作执行**：匹配成功后执行 click、wait、drag 等动作
3. **循环控制**：支持循环直到条件满足

**执行流程**：
1. 截取当前画面
2. 遍历步骤尝试匹配场景
3. 匹配成功后执行动作
4. 等待 500ms 后重复

**停止条件**：
- 用户手动停止
- 脚本 `quit` 动作触发
- OCR 检测到资源耗尽
- 浏览器关闭

## Inspector

工具栏第二行包含 Inspector 区域，用于坐标和颜色查看：

| 组件 | 功能 |
|------|------|
| **X / Y** | 坐标输入框。鼠标点击画布自动填充，也可手动输入 |
| **Fetch** | 获取指定坐标的颜色（也可按 Enter 触发） |
| **Click** | 在指定坐标执行点击（不依赖 Screencast，适用 Spread to All） |
| **色块** | 可视化显示颜色 |
| **Color 值** | RGB 颜色值，格式 `RGB(r, g, b)` |
| **Type...** | 文本输入框，输入文字后按 Enter 或点 Send 注入到会话 |
| **Send** | 将文本发送到当前会话的焦点元素（适用 Spread to All） |

### 使用场景

- **调试脚本**：点击画布查看坐标和颜色，用于配置场景颜色点
- **精确点击**：输入坐标后点击 Click 按钮，无需鼠标精确定位
- **Screencast 关闭时**：使用 Click 按钮仍可执行点击操作
- **文本输入**：在游戏内聊天、起名、输入数量等场景，通过 Type 输入框向页面注入文字

## 文本输入

### 功能说明

通过 Inspector 面板的 Type 输入框，向当前会话的浏览器页面注入文本。使用 CDP `Input.insertText` 实现，支持中文等 Unicode 字符。

### 使用方式

1. 在 Type 输入框中输入文本
2. 按 **Enter** 或点击 **Send** 按钮
3. 文本注入到当前会话中浏览器的焦点元素

### Spread to All

勾选 Spread to All 时，文本会同时发送到所有活跃会话。

> **注意**：文本注入依赖页面中存在已获取焦点的输入元素。如游戏内的输入框位于跨域 iframe 中且主页面 CDP 无法到达，可能需要后续增强 OOPIF 支持。

## 登录机制

登录通过**三层入口链的纯 DOM 导航**完成，不依赖像素识别：

### 自动登录流程

1. 导航到区服登录页（layer 1：`www.lequ.com/server/wly/s/{server}/ish5/{server}`）
2. 检测到登录表单则执行密码登录（缓存的登录态可能跳过此步）
3. 从页面 DOM 读取区服入口 iframe 地址（layer 2，URL 携带 ticket）并导航
4. 读取 `#gameIframe.src` 得到游戏页地址（layer 3，content 票据短时效，每次启动现取），直接以顶层页面打开
5. 等待游戏自身 `Connection._connected === true` 且桥观测到登录数据推送结束（`S_2_C_CHAR_LOAD_END`），确认登录成功

> 首次登录出现的"用户协议"弹窗是 canvas 绘制、无 DOM 元素，是唯一仍走场景识别 + 坐标点击的环节（同意后不再出现）。

### 浏览器配置文件持久化

每个账户拥有独立的浏览器配置文件目录，自动保存：
- Cookies
- LocalStorage  
- 图片缓存

这意味着**第二次启动同一账户时通常无需重新登录**，layer 1 会直接生成区服入口 iframe，跳过密码输入步骤。

> 如遇登录问题，可在 Settings → Browser Cache 中清除该账户的缓存后重试。

## 协议桥

登录完成后，会话的游戏页面内会注入一个 JS 桥（`resources/page_bridge.js`，经 CDP init script 在页面脚本之前注入），提供两条协议通道：

### 下行观测

桥会 patch 游戏自己的 `Connection._parsePacket`，把**全部下行协议包**（已由游戏解码成结构化数据）经 CDP binding 推回宿主，以 `protocol_message` 事件广播：

```json
{
  "session_id": "...",
  "protocol_id": 4,
  "name": "S_2_C_KEEP_ALIVE",
  "data": { "cur_time": 1788339938, "time_diff": 0 }
}
```

`name` 取自游戏自身的协议注册表（如 `S_2_C_MAILLIST_ID`）；未知协议的 `name` 为 `null`。

### 上行发送

通过 Tauri command 向游戏发送协议消息（组包、加密、编码全部由游戏自己的 `Connection` 完成）：

```
send_protocol(session_id, name = "C_2_S_MAIL_INFO", payload = {})
```

`name` 必须是游戏协议注册表中的协议名（`C_2_S_` 开头的上行协议）。对应的下行响应会以 `protocol_message` 事件到达。

> 协议能力仅依赖游戏 bundle 暴露的协议层，不触碰二进制与加解密；游戏版本更新后协议名/id 可能漂移。

## 协议脚本

协议脚本（`resources/protocols/*.yaml`）是用协议原语编写的自动化任务，由 `ProtocolRunner` 线性执行一遍，全程无截图识别、无模拟点击（click/drag 仅作兜底原语）。与场景脚本（scene loop）并存，`start_script` 按名字自动选择执行引擎，两者在脚本列表中并列展示。

### 结构

```yaml
name: claim_all_mail
description: 领取全部邮件附件（协议驱动）
steps:
  - name: fetch_mail_list
    actions:
      - type: request
        protocol: C_2_S_MAIL_INFO
        expect: S_2_C_MAILLIST_ID
        timeout: 10s
        retries: 3

  - name: draw_all_rewards
    conditions:
      - { field: state.S_2_C_MAILLIST_ID.mailNums, op: gt, value: 0 }
    actions:
      - type: request
        protocol: C_2_S_MAIL_DRAW_ALL_REWARD
        expect_any: [S_2_C_MAIL_DRAW_ALL_REWARD, S_2_C_UPDATE_BENEFIT]
        timeout: 15s
```

### 动作原语

| 原语 | 说明 |
|---|---|
| `request` | 发送 + 等待响应 + 超时重发（**首选**）。`expect` 单一应答 / `expect_any` 多应答；`retries` 超时重发次数；`conditions` 校验响应字段 |
| `send_protocol` | 只发不等 |
| `wait_protocol` | 等待某下行协议（可带字段 `conditions`） |
| `wait_state` | 等待游戏状态满足 `conditions`（就绪门） |
| `wait` | 等待时长（`duration: 1s`） |
| `click` / `drag` | 画面兜底 |

### 条件

- 位置：step 级 `conditions`（不满足则跳过该 step）或 `wait_*` 的 `conditions`。
- 形式：`{ field, op, value }`；`field` 为点路径。
- 路径前缀：
  - `state.<协议名>.<字段>` — 引用结构化游戏状态（GameState 按协议名保存最新下行负载）；
  - `role.<字段>` — 直读游戏客户端 role 模型（如 `role._militaryOrder`、`role._knightTower._teamNumInfo.num`），随时可读、无需等待推送；注意部分字段（如 `_teamNumInfo`）要先进入对应界面才由服务端下发。
- `value` 可以是字面量，也可以是 `"$<路径>"` 引用另一个字段做字段间比较（如 `"$role._militaryOrder"`）。
- `request` / `wait_protocol` 的响应 `conditions` 同样支持 `"$<路径>"`（如 `{ field: name, op: eq, value: "$role.accName" }` 只认自己名字的命中广播）；`$` 引用在动作开始时解析一次，不会每条广播都查一次。
- `op`：`eq / neq / gt / gte / lt / lte / exists / missing`（`missing` 只要求路径解析不出**非 null 值**，忽略 value）。
- **null 语义**：游戏客户端复位模型时把字段置 null（如战后 `role._knightTower._teamNumInfo = null`）——null 一律视为"无值"：`missing` 命中、`exists` 不命中。刷新类谓词（`missing → 重新拉取`）依赖这一语义才能在每次战后重燃。

### stateRule（场景脚本的精确判定，替代 ocrRule）

场景脚本的 step 除 `ocrRule` 外还支持 `stateRule`——在同一决策点（step 动作前、每次 loop 迭代前）用上面的条件判定，不再走 OCR 服务：

```yaml
  - scene: tower_entrance_1
    stateRule:
      any: true            # true = 任一条件满足即触发（OR）；缺省 false = 全部满足（AND）
      conditions:
        - { field: role._knightTower._teamNumInfo.num, op: gte, value: 7 }
        - { field: role._knightTower._teamNumInfo.num, op: gte, value: "$role._militaryOrder" }
      action: quit_exhausted   # quit_exhausted / quit / skip，与 ocrRule 相同
```

`join_tower.yaml` 的三处判定已从 OCR 迁移为 stateRule（阈值 7/10/10 保留）：军令数读 `role._militaryOrder`，今日刷塔次数读 `role._knightTower._teamNumInfo.num`，与界面上 "1066/8" 的显示一一对应（used = 次数+1，total = 军令）。

### 协议注册表

`resources/protocols/registry.json` 是从游戏 bundle 提取的协议名 → id 映射（标注 bundle 版本），脚本启动前校验所有引用协议名，未知名字直接失败并报错。游戏版本更新后需重新提取（在游戏页执行 `Object.entries(__require('ProtocolBase').Protocol)` 导出）。

## 统一任务模板（v2，推荐）

任务模板（`resources/tasks/*.yaml`）是自动化的推荐写法：**执行器（TaskRunner）统一且与任务无关，新增一类任务只是新增一个模板文件**。执行模型是状态匹配循环——模板顺序即优先级，每轮执行第一个谓词成立的 step；`once: true` 的 step 每次运行只执行一次（线性流程是全 once 的特例，循环任务不标 once）。

```yaml
name: knight_tower
description: 武魁高塔组队刷塔（协议驱动）
on_no_match: { policy: wait, timeout: 120s }   # 无匹配时：wait 等待 / quit 结束（默认 quit）
steps:
  - name: finish                     # 顺序即优先级，终止条件放最前
    match:
      conditions:
        - { field: role._knightTower._teamNumInfo.num, op: gte, value: 7 }
    actions:
      - { type: quit, reason: exhausted }

  - name: fight                      # 战斗中 fightNum<3 就反复匹配（状态循环）
    match:
      conditions:
        - { field: role._knightTower._isBattle, op: eq, value: true }
        - { field: role._knightTower._fightNum, op: lt, value: 3 }
    actions:
      - { type: send_protocol, protocol: C_2_S_KNIGHT_TOWER_TEAM_PLAYER_MOVE, payload: { channel: 1 } }
      - { type: request, protocol: C_2_S_KNIGHT_TOWER_TEAM_ATTACK,
          expect: S_2_C_KNIGHT_TOWER_PLAYER_ATTACK, timeout: 8s, retries: 3 }

  # 画面兜底与协议在同模板混用：
  # - name: dismiss_popup
  #   match: { scene: some_popup }
  #   actions: [{ type: click, points: [{x: 540, y: 400}] }]
```

### 谓词（match）

- `scene: <场景名>`：截图场景识别（与 conditions 可同用，AND）；
- `conditions: [...]`：state./role. 条件（同 stateRule 语法）；
- `once: true`：每次运行最多执行一次。

### 动作原语

`click / drag / wait / loop / incr / decr / quit(reason: completed|exhausted)`、`send_protocol / request（expect 或 expect_any，retries，on_timeout: fail|continue，abort_if）/ wait_protocol / wait_state`、`eval_js`（逃生舱：执行任意 JS，如调用客户端函数）。

> `on_timeout: continue` 用于"相关性会随时间失效"的请求（如攻击将死的 boss）：重试耗尽后不判任务失败，而是交回状态机重新评估（战斗若已结束，自然流转到其它 step）。默认 `continue`。
> `abort_if: [conditions]` 用于"前提可能中途失效"的请求：发送前、每次重试前、等待中（200ms 轮询）都会检查；条件成立立即中止并交回状态机，不重试不超时。典型用法：攻击请求配 `abort_if: isBattle==false`——RESULT 在 step 匹配与发送之间到达时（每场必现的竞态），避免向死 boss 幽灵攻击、白烧超时。

### payload 的 `$` 引用与数组选择器

- payload 字符串以 `$` 开头时发送前解析为 state./role. 路径值（如 `"$state.X.ary.@max(n).id"`）；
- 路径支持链式数组选择器：`@first / @last / @max(field) / @min(field) / @where(field, op, value)`；`@where` 返回过滤后的数组（value 给列表时任一命中），需配合 `@first`/`@max` 或索引取值；
- 例（白名单选队）：`battle_team_info_ary.@where(server_id, ends_with, ["-888","-11014","-11020"]).@max(player_count).create_id`；
- 条件 op：`eq / neq / gt / gte / lt / lte / exists / missing / contains / ends_with`（后两者为字符串 op，value 为列表时任一命中）。

## 场景识别

### 场景定义

场景在 `resources/scenes/*.yaml` 中定义：

```yaml
name: main_city
category: city
points:
  - {x: 100, y: 200, color: {r: 255, g: 128, b: 64, a: 255}}
actions:
  SomeButton:
    type: click
    point: {x: 500, y: 600}
```

### 匹配算法

- 检查所有定义的颜色点
- 计算实际颜色与预期颜色的差异
- 平均差异 ≤ 5.0 视为匹配成功

## 自动化脚本

### 脚本定义

脚本在 `resources/scripts/*.yaml` 中定义：

```yaml
name: Join Battle
description: Automatically join group battles
steps:
  - scene: battle_group_entrance
    timeout: 5s
    actions:
      - type: click
        points: [{x: 538, y: 544}]
      - type: wait
        duration: 1s
```

### 支持的动作

| 类型 | 说明 | 参数 |
|------|------|------|
| click | 点击坐标 | points: [{x, y}] |
| wait | 等待时间 | duration: 1s |
| drag | 平滑拖拽 | points: [{x, y}, ...] 支持多点路径 |
| incr/decr | 计数器操作 | key: counter_name |
| quit | 退出脚本 | condition: {op, key, value} |

### 循环控制

循环使用 `loop` 类型的 Action 实现，动作嵌套在内部：

```yaml
actions:
  - type: loop
    count: -1           # -1 表示无限循环
    interval: 800ms     # 循环间隔
    until: target_scene # 匹配到此场景时退出（可选）
    actions:            # 嵌套的动作列表
      - type: click
        points: [{x: 100, y: 200}]
      - type: wait
        duration: 1s
```

### OCR 资源检测

使用 `ocrRule` 检测屏幕上的资源数值：

```yaml
- scene: tower_entrance
  ocrRule:
    mode: ratio              # 识别 "数字/数字" 格式
    roi: {x: 510, y: 602, width: 90, height: 50}
    condition: "used > 7 || used > total"  # 表达式求值
    action: quit_exhausted   # 条件满足时退出
  actions:
    - type: loop
      ...
```

**变量映射**：
- `used`: 分母值（已使用）
- `total`: 分子值（总量）

**支持操作符**：`>`, `>=`, `<`, `<=`, `==`, `!=`, `&&`, `||`

## 常见问题

### 画布不显示

- 等待会话状态变为 Ready
- 检查日志中的错误信息

### 登录失败

- 清除账户浏览器缓存后重试（Settings → Browser Cache）
- 检查网络连接
- 确认账户密码正确

### 脚本卡住

- 使用 Inspector 检查当前场景
- 更新场景定义的颜色点

### 点击偏移

- 确保浏览器视口为 1080x720
