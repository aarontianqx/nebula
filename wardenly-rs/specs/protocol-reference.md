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

### 4.2 关键问题 → 协议答案

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

## 5. 自助查询任意协议的方法

1. **找名字**：`grep -i tower wardenly-rs/src-tauri/resources/protocols/registry.json`（或在游戏页 `Object.keys(__require('ProtocolBase').Protocol)`）；
2. **看字段结构**：bundle 里搜 `type=a.Protocol.<名字>`，前方几百字符就是字段定义（`this.fields=[...]` 列出顺序，`this.<字段>={type:...}` 给出类型）；
3. **看真实数据**：live session 里 patch `_parsePacket` 抓包（`tests/` 下的集成测试和调研文档 §9 都有现成做法）；或直接读客户端模型 `__require('Account').default.get().role`。

## 6. 已知注意事项

- **结构解析错位风险**：个别协议的字段宽度在注册表与客户端间可能不一致（实测 `S_2_C_KNIGHT_TOWER_TEAM_NUM.chaos_level` 经桥解析为 2173，客户端模型中为 220——2173 恰是另一协议的 id）。**以 role 模型直读为准**，它是游戏自己的解析结果；GameState 推送值用于触发/等待，数值判断优先 `role.*`。
- `DT_REPORT`/`DT_BINARY` 字段（战报、活动详情 blob）是不透明二进制，客户端另有解析器，需要时再针对性逆向。
- 协议随游戏版本漂移：名字变化会在脚本启动时被 registry 校验拦下（显式报错）；字段语义变化最危险，需要靠 §5 的方法重新核对（diff 工具待实际需要时再做）。
