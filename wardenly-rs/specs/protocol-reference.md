# WLY 协议参考

> 2026-09-03 整理自游戏 bundle（`mobile_v614_1334`）静态分析与真实抓包。协议名/id 全表见 `src-tauri/resources/protocols/registry.json`（2642 条）；本文档讲清协议长什么样、怎么读、以及武魁高塔家族的全部细节。协议随版本可能漂移，以实际抓包为准。

## 1. 协议层 30 秒版

- 游戏与服务器间是**单条 WebSocket 长连接**，上行协议名以 `C_2_S_` 开头（1234 个），下行以 `S_2_C_` 开头（1408 个）。
- 每条消息有一个**语义化名字**和一个**数值 id**，以及一套**字段结构**（`PROTOCOL_STRUCTS`）：字段名 + 类型（`DT_INT/DT_UINT/DT_SHORT/DT_CHAR/DT_LONGLONG/DT_STRING/DT_OBJECT` 等），可嵌套对象数组。
- 加解密、二进制编解码全部由游戏自己的 `Connection` 完成。wardenly 拿到的永远是**游戏解码后的结构化 JSON**（见下例），发协议时也只需给名字 + 字段对象。
- 三种消息形态：
  1. **请求-响应**：`C_2_S_MAIL_INFO` → `S_2_C_MAILLIST_ID`（脚本用 `request` 原语）；
  2. **服务端主动推送**：心跳、资源变化（`S_2_C_UPDATE_BENEFIT`）、聊天、踢下线等，随时可能到达；
  3. **登录数据洪流**：入城后服务端一次性推几百条全量状态（角色、城市、任务、配置……），以 `S_2_C_CHAR_LOAD_END` 结束。

## 2. 数据长什么样（真实抓包）

`S_2_C_UPDATE_BENEFIT`（资源全量快照，每次变化都会推）：

```json
{
  "money": 243015873,        // 银币（2.43亿）
  "goldenCoins": 17033,      // 金币
  "geste": 841450662,        // 军功/战功（8.41亿）
  "prestige": 0,             // 声望
  "militoryOrder": 1070,     // 军令（界面上 1070/220，字段名是官方拼写错误）
  "food": 1036983863,        // 粮草
  "soldier_num": 51847928,   // 兵力
  "tower_order": 0,
  "soul_num": 83965418,      // 将魂
  "insignia_num": 9695192,
  "silver_cross": 0,
  "athletics_integral": 634459
}
```

`S_2_C_MAILLIST_ID`（邮件列表）：`{"mailNums": 50, "MailIdTypes": [{"mail_id": ..., "mail_type": ...}]}`，随后每封邮件一条 `S_2_C_MAIL_INFO {mail_id, mail_sendtime, mail_type, mail_status, get_status}`。

## 3. 三条数据通道（什么时候用哪条）

| 通道 | 形态 | 适用 |
|---|---|---|
| **GameState 推送** | 桥 patch `_parsePacket`，每条下行进 `ProtocolMessage` 事件 + 按协议名存最新一份（`state.*` 条件） | 等响应、监听变化（`request`/`wait_protocol`/`wait_state`） |
| **role 模型直读** | `role.*` 条件 → `queryRole(path)` 读游戏客户端自有模型（`__require('Account').default.get().role`） | 随时可查的当前值（军令、军功、塔次数……），不用等推送；约 100 个子模型 |
| **主动协议请求** | `send_protocol`/`request` 发 `C_2_S_*` | 触发动作或拉取不常推的数据（如 `C_2_S_LOAD_TRIALS_TOWER_INFO`） |

注意：客户端模型字段可能要先进入对应界面才由服务端下发（例：`_teamNumInfo` 首次为 `null`，进塔后才有值）。

## 4. 武魁高塔（Knight Tower）协议家族

### 4.1 玩法状态机与协议映射

```
无队 ──C_2_S_TEAM_CREATE / C_2_S_TEAM_JOIN──▶ 有队（待命）
      ◀── S_2_C_TEAM_INFO（队伍列表）/ S_2_C_PLAYER_INFO（成员）
有队 ──队长 C_2_S_TEAM_START──▶ 开战 ──C_2_S_TEAM_ATTACK──▶ 攻击 boss
      ◀── S_2_C_TEAM_START        ◀── S_2_C_PLAYER_ATTACK（每次攻击结果）
                                     ◀── S_2_C_RESULT（boss 结算+奖励）
```

### 4.2 Boss 表与按 boss 独立计数

客户端 `StaticData.knightTower._data.Boss`（2026-09-03 实测提取）：

| ident | boss | lvLimit | 难度档（boss 等级） | 每日次数上限（实测/暂定） |
|---|---|---|---|---|
| 1 | 天狼 | 100 | 100~220 | 7（实测） |
| 2 | 混沌 | 240 | 240~300 | 10（暂定） |
| 3 | 穷奇 | 310 | 310~350 | 10（暂定） |
| 4 | 饕餮 | 360 | 360~400 | 10（暂定） |

- `C_2_S_TEAM_NUM {ident}` 按 boss 分别返回**各自独立的今日次数** `num`（客户端 `teamNumInfo` 一次只装一个 boss，含 `ident` 字段，可作为"当前目标 boss"状态）；
- 客户端配置里 boss 只有中文名（天狼/混沌/穷奇/饕餮），ident 为纯数字；wardenly 的 per-boss 模板（`resources/tasks/knight_tower_{tianlang,hundun}.yaml`）按此表配置。

### 4.3 关键问题 → 协议答案

**今日刷了几次 / 军令剩多少（现有 OCR 替代已用）**

- `S_2_C_KNIGHT_TOWER_TEAM_NUM`（进塔界面时下发）：

  ```json
  {"ident": 1, "count": 0, "num": 7, "is_re_connect": 0, "chaos_level": 220}
  ```

  `num` = 今日已刷次数（界面显示 `军令/(num+1)`，如 `1066/8`）；`chaos_level` = 当前 boss 等级。对应模型：`role._knightTower._teamNumInfo.num`、`role._militaryOrder`。

**可以加入谁的队伍？**

- `S_2_C_KNIGHT_TOWER_TEAM_INFO {count, battle_team_info_ary[]}`，每个元素（`NpcMutiBattleTeamInfo`）：

  | 字段 | 含义 |
  |---|---|
  | `name` | 队长名 |
  | `server_id` | 队长所在服 |
  | `create_id` | 队伍 id（加入时用） |
  | `limit_level` | 等级限制 |
  | `country_limit` / `server_limit` | 国家/服限制（1=不限 2=同国 3=同军团） |
  | `crops_name` | 军团名 |
  | `player_count` | 当前人数 |
  | `player_level` | 队伍等级 |
  | `is_first` | 是否首杀队 |

  拉取方式：进塔界面时服务端下发；客户端也可发 `C_2_S_KNIGHT_TOWER_TEAM_NUM_INFO {ident}` 请求指定难度/区域的队伍信息。

- 成员列表：`S_2_C_KNIGHT_TOWER_PLAYER_INFO {server_id, create_id, count, player_data_ary[]}`，元素 `{name, player_level, server_id}`。

**是否自动加入？**

- `C_2_S_KNIGHT_TOWER_TEAM_JOIN {ident, create_id, server_id}` — 按队伍列表里的 `create_id`/`server_id` 加入；
- 自建：`C_2_S_KNIGHT_TOWER_TEAM_CREATE {ident, limit_level, server_limit, chaos_level}`。

**是否已经开战？**

- 队长发起：`C_2_S_KNIGHT_TOWER_TEAM_START {}`；开战广播：`S_2_C_KNIGHT_TOWER_TEAM_START`；
- 客户端状态：`role._knightTower._teamState`（1=NONE 2=TEAM_LEADER …）、`role._knightTower._isBattle`；
- 断线重连：`S_2_C_KNIGHT_TOWER_TEAM_RECONNECT {fail_time, over_time, cur_boss_soldier_num, num}`。

**是否攻击 boss / 打了几次？**

- 攻击：`C_2_S_KNIGHT_TOWER_TEAM_ATTACK {}`；
- 每次攻击结果：`S_2_C_KNIGHT_TOWER_PLAYER_ATTACK {failed_time, name, server_id, boss_soldier_num, report_id}`——`boss_soldier_num` 是 boss 剩余兵力，`report_id` 是战报（`DT_REPORT` 二进制，不可直接读）；
- 结算：`S_2_C_KNIGHT_TOWER_RESULT {ident, state, count, reward_ary[]}`——`state` 胜负，`reward_ary` 奖励列表。结算后客户端把 `fightNum`（本场已用攻击次数，0~3，超过要花金币复活）清零。

### 4.3 结论：组队/开战/攻击全流程都能纯协议化

上面每一环都有明确的请求协议和可观测的下行推送/模型字段，不需要任何截图检测。剩余的客户端侧信息只有静态配置（难度表 `KnightTowerJson`、消耗/复活规则）——这些在 bundle 的客户端 JSON 里，属离线可读数据。

### 4.4 实测验证的完整自动化流程（2026-09-03，7 轮真实组队战斗）

以下流程在真实账号上跑通 7 轮（num 0→7），每步都有协议级确认信号：

```
1. C_2_S_TEAM_NUM {ident:1}                  → S_2_C_TEAM_NUM {num, count, chaos_level}
   （等价于客户端 enterKnightTower(1)，无需打开任何界面）
2. C_2_S_TEAM_NUM_INFO {ident:1}             → S_2_C_TEAM_INFO（队伍列表）
3. 按 player_count 选人多的队，过滤 limit_level/server_limit/server_id 白名单
4. C_2_S_TEAM_JOIN {ident, create_id, server_id_len, server_id}
                                              → S_2_C_PLAYER_INFO / PLAYER_COUNT 确认入队
   —— server_id_len 由桥按 server_id 自动计算（stringUTFLen 语义），跨服长度不同不能写死
   —— 多人环境下队伍随时满员/开战/解散：JOIN 前先重新拉列表，JOIN 失败（超时）交回状态机重选，不判任务失败
5. 等 S_2_C_TEAM_START（_isBattle=true）      —— 开战
6. C_2_S_TEAM_PLAYER_MOVE {channel:1} 先移入 channel（**必须**，否则攻击静默无效且照样扣军令）
7. C_2_S_TEAM_ATTACK {}                       → S_2_C_PLAYER_ATTACK（自己名字）确认命中
   —— 首击经常不落地，需 request/retry 语义（~5–10s 未确认就重发）
   —— **等待应取 expect_any [PLAYER_ATTACK, TEAM_RESULT]**：结算也是本轮攻击的答案，
      boss 一死立即进入下一轮；只等 PLAYER_ATTACK 会在战斗结束窗口空等重试（实测 10~30s）
   —— 每人每场最多 3 次（fightNum 0..3），第 4 次起耗金币；自动化只发到 3，金币弹窗是客户端 UI 行为，协议路径不会出现
8. S_2_C_RESULT {state, reward_ary}          —— 结算：fightNum 清零、num+1、队伍自动解散
9. 回到 1（teamNumInfo 战后会清空，必须重新请求 TEAM_NUM 获取）
```

实测要点：

- **进塔/读队/加入全部可以纯协议，无需打开任何界面**：客户端的 `enterKnightTower(ident)` 等价于 `C_2_S_KNIGHT_TOWER_TEAM_NUM {ident}`（bundle 源码确认，ident 为 boss 标识，天狼=1）。2026-09-03 在新账号上验证：不发任何 UI 事件，`TEAM_NUM → TEAM_NUM_INFO（队伍列表）→ TEAM_JOIN（入队成功）` 全链路成立；
- 注意：客户端收到 `S_2_C_TEAM_NUM` 响应后**会自己打开塔界面**（bundle 内建行为）。这对自动化无影响（数据不依赖画面），但"画面完全不动"是做不到的——也不需要；
- **RESULT 时客户端模型全量复位**（`_onKnightTowerResult` → `clear()`，纯协议处理、不依赖任何界面）：`_isBattle=false`、`_selfteamId=-1`、`_teamNumInfo=null`、`_fightNum=0`。2026-09-04 实测：RESULT 后 ~0.3s 内全部生效；
- **"退出战斗结算"按钮 = 重新发 `C_2_S_KNIGHT_TOWER_TEAM_NUM {ident}`**（结算面板 `ok_btn` → `_onClose()` → `enterKnightTower()`，源码确认）。其响应在非战斗分支会刷新 `_teamNumInfo` 并 `REPLACE_ALL` 切回塔界面——自动化无需点击任何按钮，战后重发 TEAM_NUM 即同时完成"退出结算画面"与"刷新今日次数"。客户端战后**不会**自动重拉，不发就永远停留在 null；
- **战中禁止发 TEAM_NUM**：`_onKnightTowerTeamNum` 在 `is_re_connect || _isBattle` 分支不更新 `_teamNumInfo`（仅重连时更新），还会强制把画面拉回战斗视图。战中拉次数 = 纯浪费 + 画面干扰；
- **`fightNum` 的维护**：开战时由 `S_2_C_TEAM_RECONNECT.num` 初始化（0），攻击命中后增加，RESULT 时清零；≥3 后客户端走 `_goldRelive` 金币复活路径（自动化用 `fightNum<3` 谓词规避，永不触发）；
- 队长侧链路（供将来队长模板参考）：建队 = `C_2_S_KNIGHT_TOWER_SET_CUR_BOSS_ID {boss_level}` + `C_2_S_KNIGHT_TOWER_TEAM_CREATE {ident, limit_level, server_limit, chaos_level}`（chaos_level 取 `teamNumInfo.chaos_level`）；开战 = 打开战斗视图，其 `start()` 自动发 `C_2_S_KNIGHT_TOWER_TEAM_START`（非重连且非战中时）；
- **退出**：待命时用 `C_2_S_TEAM_LEAVE {}` → `S_2_C_PLAYER_LEAVE` 确认；结算后队伍自动解散无需手动退；
- **num 语义**：今日参与场次（含未命中的 phantom 参与），界面显示 `军令/(num+1)`（如 `1066/8`）——军令只在开战参与时扣，第 N 场耗 N 令，战斗内 3 次攻击不额外耗令；7 是天狼性价比阈值，之后仍可打但性价比低；
- **teamState 不可靠**（战中/重组期间会与实际不符），以 `selfteamId + _isBattle + TEAM_RECONNECT` 为准；
- **军令会计**：未命中 channel 的"幽灵攻击"也扣军令——先 MOVE 后 ATTACK 是硬性顺序；
- 战斗中重连用 `C_2_S_TEAM_PLAYER_INFO {}` 拉 `S_2_C_TEAM_RECONNECT {cur_boss_soldier_num, over_time, num}` 恢复现场。

### 4.5 金币复活路径与攻击确认（2026-09-04，bundle 源码）

- **第 4 次攻击起花金币**：客户端在 `fightNum>=3` 时再攻击 → 弹"是否花费金币×N 进行复活战斗"（`N = 5*(fightNum+1-3)`，封顶 20）。确认按钮 → 发 `C_2_S_KNIGHT_TOWER_TEAM_PLAYER_MOVE`（复活+进 channel 同一协议），**金币由服务器侧扣除**。裸发第 4 次攻击协议时服务器是否直接扣金未验证（也不应去试）——自动化必须保证第 4 击发不出去；
- **弹窗的另一个触发点**：fighter 阵亡且队伍存活人数>1 时客户端自动弹（`_deadCallback`）。我们的纯协议路径不点击、不会主动触发；
- **安全关闭**：弹窗是通用 MessageBox（确定=回调发协议，取消=纯关闭零消耗）；自动化更稳的做法是 `__require('UIManager').UIManager.instance.hide(__require('ViewEnum').ViewModule.MESSAGE_BOX_VIEW)`——直接销毁视图、零回调，不依赖坐标点击；
- **攻击确认的归属**：`S_2_C_KNIGHT_TOWER_PLAYER_ATTACK` 带 `name`/`server_id`，客户端 `isSelf` 用 `role.accName` 比对——模板据此用 `conditions: [{field: name, op: eq, value: "$role.accName"}]` 只认自己的命中；
- **`fightNum` 跨场残留**（实测）：快速连场时新场开局读数可能带上上一场的计数（观测到 0→1→2 逐场递增），来源（服务器 RECONNECT.num 回报 vs 客户端竞态）待确认。影响是保守方向（少打，不会多打）；3 次上限的判定因此叠加"自己命中确认"双保险；
- **攻击确认延迟**：服务器约 7s 一个回合处理攻击，命中广播并非即时；队员侧节奏天然同步到这个回合周期。

### 4.6 打折商城（集市）协议族（2026-09-05，live 验证）

- **加载**：打开集市视图客户端发 `C_2_S_DISCOUNT_SHOP_INFO {}`；纯协议可直发。响应两条：
  - `S_2_C_DISCOUNT_SHOP_INFO {count, gift:[{id, buyNum}]}`——`buyNum` = **个人已购**（从未购买则无该条目）；
  - `S_2_C_DISCOUNT_SHOP_GIFT_BUY_NUM {count, gift:[{id, buyNum}]}`——`buyNum` = **全服已购**；他人购买时还有 `S_2_C_DISCOUNT_SHOP_GIFT_BUY_TOTAL_NUM` 实时广播。
- **商品表**：不在本协议族里——在客户端活动详情 `role.activity.getActivityInfo(61)`（`S_2_C_ACTIVITY_INFO` 分片下发，字节数组分块、**index 递减、index=0 为最后一片**，客户端拼好后 `bytesToString` 成 JSON）。每项：`{name, price, oldPrice, discount, limitNum(个人限购), count(全服总量), level(等级门槛), viplevel(vip 门槛), bag, reward:["type x id x num"]}`；购买用的 `ident` 是该数组的**下标**。内容物名称用 `StaticDataUtil.getCostInfo(type, id, num)` 解析。
- **购买**：`C_2_S_DISCOUNT_SHOP_GIFT_BUY {ident}` → `S_2_C_DISCOUNT_SHOP_GIFT_BUY {index, buyNum(个人已购), buyTotalNum}`。实测 888 区完整往返，扣款精确（135 金/单酒馆礼盒）。
- **拒绝是静默的**：vip/等级/限购不满足时服务器**不回任何包**（无错误提示协议），所以模板必须在客户端侧判全部门槛再发购买，超时即视为未成交交回状态机。
- 本期商品（2026-09-05 集市）名称对照：战魂血玉=血玉礼盒（ident 4）、东海鲛珠=鲛珠礼包（11）、琉璃灯盏=许愿大礼包（5）、五花马=五花马礼包（1）、酒馆礼包=酒馆礼盒（3）。**活动换货 ident 会漂移**，模板第一步强制做 ident→名称校验，对不上直接失败。

## 5. 自助查询任意协议的方法

1. **找名字**：`grep -i tower wardenly-rs/src-tauri/resources/protocols/registry.json`（或在游戏页 `Object.keys(__require('ProtocolBase').Protocol)`）；
2. **看字段结构**：bundle 里搜 `type=a.Protocol.<名字>`，前方几百字符就是字段定义（`this.fields=[...]` 列出顺序，`this.<字段>={type:...}` 给出类型）；
3. **看真实数据**：live session 里 patch `_parsePacket` 抓包（`tests/` 下的集成测试和调研文档 §9 都有现成做法）；或直接读客户端模型 `__require('Account').default.get().role`；
4. **最省事：事件流水**。每个会话的全部协议流量（下行 + 上行，含游戏自己发的）都落在 `logs/sessions/*.jsonl`（见 functional-guide「事件流水」节）。手动玩一遍功能再翻流水，就是最快的侦察方式。

### 5.1 常用参数取数表

| 参数 | 取数路径 | 来源 |
|---|---|---|
| 银币 | `state.S_2_C_UPDATE_BENEFIT.money` | 资源每次变化服务器全量推 |
| 金币 | `state.S_2_C_UPDATE_BENEFIT.goldenCoins` / `role.gold` | 同上 / 客户端模型 |
| 军令 | `state.S_2_C_UPDATE_BENEFIT.militoryOrder` / `role._militaryOrder` | 同上 / 客户端模型（官方字段拼写错误是 militoryOrder） |
| 军功/战功 | `state.S_2_C_UPDATE_BENEFIT.geste` | 同上 |
| 声望 | `state.S_2_C_UPDATE_BENEFIT.prestige` | 同上 |
| 粮草 | `state.S_2_C_UPDATE_BENEFIT.food` | 同上 |
| 兵力 | `state.S_2_C_UPDATE_BENEFIT.soldier_num` | 同上 |
| 将魂 | `state.S_2_C_UPDATE_BENEFIT.soul_num` | 同上 |
| 角色名 | `role.accName` | 客户端模型（`isSelf` 判定用它） |
| 等级 | `role.level` | 客户端模型 |
| 高塔今日次数 | `role._knightTower._teamNumInfo.num`（含 `.ident` 校验目标 boss） | 客户端模型，战后需重新拉（见 §4.5） |

> 写任务时不确定路径对不对：用内置模板「调试·资源快照」跑一次，日志里就是当前真实值。

## 6. 已知注意事项

- **结构解析错位风险**：个别协议的字段宽度在注册表与客户端间可能不一致（实测 `S_2_C_KNIGHT_TOWER_TEAM_NUM.chaos_level` 经桥解析为 2173，客户端模型中为 220——2173 恰是另一协议的 id）。**以 role 模型直读为准**，它是游戏自己的解析结果；GameState 推送值用于触发/等待，数值判断优先 `role.*`。
- `DT_REPORT`/`DT_BINARY` 字段（战报、活动详情 blob）是不透明二进制，客户端另有解析器，需要时再针对性逆向。
- 协议随游戏版本漂移：名字变化会在脚本启动时被 registry 校验拦下（显式报错）；字段语义变化最危险，需要靠 §5 的方法重新核对（diff 工具待实际需要时再做）。
