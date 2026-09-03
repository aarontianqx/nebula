# Proposal: 统一任务执行架构（TaskRunner + 统一模板）

> 2026-09-03 ｜ 状态：**已实现并验证**（TaskRunner + `resources/tasks/knight_tower.yaml`，live 验证见 §8）｜ 目标：**执行器逻辑统一，新增一类任务只是新增一个模板文件**。

## 1. 现状问题

当前有两套执行引擎、两种模板 schema：

| | ScriptRunner | ProtocolRunner |
|---|---|---|
| 模板 | `resources/scripts/*.yaml` | `resources/protocols/*.yaml` |
| 执行模型 | 状态匹配循环（哪个 scene 匹配就执行哪个 step） | 线性顺序执行一遍 |
| 判定 | 截图场景识别 + ocrRule/stateRule | state./role. 条件 |
| 动作 | click/drag/wait/loop/counters | request/send_protocol/wait_*/click 兜底 |

问题：

- 武魁高塔这类**混合任务**（协议组队 + 多轮循环 + 可能的画面兜底）任何一套单独都表达不了：ProtocolRunner 没有循环和场景分支，ScriptRunner 没有协议原语；
- 新增任务要先选引擎，模板能力不对等；
- 长期必然收敛为一套，越早统一成本越低。

## 2. 架构分层：固定逻辑 vs 模板可配置

**执行器是固定程序逻辑（Rust 代码，与任何任务无关）：**

- 状态匹配循环（截图谓词 + 条件谓词的求值）；
- 动作原语全集的实现（click/drag/wait/loop/counters、send_protocol/request/wait_protocol/wait_state/eval_js、quit）；
- 规则评估（stateRule / ocrRule）；
- 条件求值（state./role./$引用，condition_eval）；
- 拟人化噪声、事件发布、run 生命周期（run_id/Start/Stop/StopReason）；
- 协议注册表校验。

**模板是可配置项（YAML，任务的全部知识都在这）：**

- steps 列表：每个 step 的**匹配谓词**（scene 和/或条件）+ **动作序列** + 可选规则；
- 阈值（如 num>=7）、协议名、payload（含 `$` 引用）、场景名、坐标；
- 循环/退出策略（on_no_match、quit reason）。

判断标准一句话：**凡是"这个任务怎么做"的知识都在模板里；凡是"怎么执行"的机制都在执行器里。** 新增任务 = 新增一个 YAML，不改 Rust。

## 3. 统一执行模型：状态匹配循环 + once 语义

不新造模型，而是把现有两个模型统一为**状态机式的匹配循环**，线性只是它的特例：

```
loop {
    若收到停止 → Manual
    step = 模板顺序中第一个「匹配谓词成立」且「once 未消耗」的 step
    if 无匹配 {
        按模板 on_no_match 策略：quit（默认，视为 Completed）或 wait（继续等待）
    }
    若 step 带规则（stateRule/ocrRule）且触发 → 按 action 退出/跳过
    顺序执行 step 的 actions
}
```

- **谓词**（每 step 可组合）：`scene: <场景名>`（截图识别）+ `conditions: [...]`（state./role. 条件，AND）。两类谓词可以单独用也可以同用（"画面在塔入口 且 num<7"）。
- **`once: true`**：该 step 每次脚本运行最多执行一次。线性任务（如邮件领取）= 全部 step 标 once，按模板顺序自然流过，执行完即无匹配 → Completed。
- **循环任务**（如刷塔）= step 不标 once，谓词持续成立就反复执行（战斗中 fightNum<3 → 攻击 step 每轮匹配），谓词失效自然流转到别的 step。天然耐受乱序实况（加入瞬间已开战、队伍秒解散、重连），这是线性模型给不了的。
- **`on_no_match`**：`quit`（默认）或 `wait { timeout }`——刷塔等队伍出现用 wait，线性任务用 quit。
- 兜底语义完整保留：scene 谓词 + click/drag 动作 = 任何协议覆盖不了的操作都能写在同一模板里。`ocrRule` 作为最后的判定兜底保留。

## 4. 模板 schema v2（示意）

```yaml
name: knight_tower
description: 武魁高塔组队刷塔（协议为主，画面兜底）
on_no_match: { policy: wait, timeout: 120s }   # 无匹配时等队伍出现
steps:
  # 顺序即优先级：终止条件放最前
  - name: finish
    match:
      conditions:
        - { field: role._knightTower._teamNumInfo.num, op: gte, value: 7 }   # 各 boss 阈值模板可调
    actions:
      - { type: quit, reason: exhausted }

  - name: reload_tower_info        # 战后再进/首次进入时信息为空，先加载
    match:
      conditions:
        - { field: role._knightTower._teamNumInfo, op: missing }   # 新增 missing（exists 的反义）
    actions:
      - { type: eval_js, script: "__require('Account').default.get().role._knightTower.enterKnightTower()" }
      - { type: wait_protocol, protocol: S_2_C_KNIGHT_TOWER_TEAM_NUM, timeout: 10s }
      - { type: request, protocol: C_2_S_KNIGHT_TOWER_TEAM_NUM_INFO, payload: {ident: 1},
          expect: S_2_C_KNIGHT_TOWER_TEAM_INFO, timeout: 10s, retries: 2 }

  - name: join_team
    match:
      conditions:
        - { field: role._knightTower._selfteamId, op: eq, value: -1 }
        # 存在「本服白名单内」的队伍才执行（值列表 = 模板可配置的服号白名单）
        - { field: state.S_2_C_KNIGHT_TOWER_TEAM_INFO.battle_team_info_ary.@where(server_id, ends_with, ["-888","-11014","-11020"]), op: exists }
    actions:
      - type: request
        protocol: C_2_S_KNIGHT_TOWER_TEAM_JOIN
        payload:                        # payload 字段支持 $ 引用（发送时解析，与条件同一选择器）
          ident: 1
          create_id: "$state.S_2_C_KNIGHT_TOWER_TEAM_INFO.battle_team_info_ary.@where(server_id, ends_with, ["-888","-11014","-11020"]).@max(player_count).create_id"
          server_id_len: 21
          server_id: "$state.S_2_C_KNIGHT_TOWER_TEAM_INFO.battle_team_info_ary.@where(server_id, ends_with, ["-888","-11014","-11020"]).@max(player_count).server_id"
        expect_any: [S_2_C_KNIGHT_TOWER_PLAYER_INFO, S_2_C_KNIGHT_TOWER_PLAYER_COUNT]
        timeout: 10s
        retries: 2

  - name: fight
    match:
      conditions:
        - { field: role._knightTower._isBattle, op: eq, value: true }
        - { field: role._knightTower._fightNum, op: lt, value: 3 }   # 至多 3 次，永不触发金币路径
    actions:
      - { type: send_protocol, protocol: C_2_S_KNIGHT_TOWER_TEAM_PLAYER_MOVE, payload: {channel: 1} }
      - { type: request, protocol: C_2_S_KNIGHT_TOWER_TEAM_ATTACK,
          expect: S_2_C_KNIGHT_TOWER_PLAYER_ATTACK, timeout: 8s, retries: 3 }

  # 画面兜底示例（不是本任务必需，展示混合写法）：
  # - name: dismiss_popup
  #   match: { scene: some_popup }
  #   actions: [{ type: click, points: [{x: 540, y: 400}] }]
```

## 5. 需要新增的执行器能力（小而收敛）

1. **`once` 语义与 `on_no_match` 策略**（新）；
2. **谓词组合**：scene + conditions 同 step（scene 谓词仅在任一步声明时才截图，避免无谓开销）；
3. **payload 的 `$` 引用**（复用 condition_eval 的解析逻辑，发送时求值）；
4. **`eval_js` 逃生舱**：执行任意 JS（如 `enterKnightTower()`），文档标注"最后手段"；
5. **条件 op 增加 `missing`**（exists 反义）；
6. **通用数组选择器**（一期）：在条件和 payload `$` 引用的路径语法中支持链式选择器——
   - `@where(field, op, value)`：过滤数组元素（value 为列表时任一命中即保留）；
   - `@max(field)` / `@min(field)` / `@first` / `@last`：选取元素；
   - 用途示例（队伍过滤与择优）：`battle_team_info_ary.@where(server_id, ends_with, ["-888","-11014","-11020"]).@max(player_count).create_id`；
   - 同一语法同时服务 match 条件（选完后 `exists` 判定"存在合条件的队伍"）与 payload 取值（取出选中元素的字段），保证判定与取值不错位。

不需要改：条件求值、stateRule、拟人化、桥、登录、事件体系、registry 校验——全是现成的。

## 6. 迁移路径

1. 实现 `TaskRunner`（统一执行器）+ schema v2，落 `resources/tasks/*.yaml`；
2. 用 `knight_tower` 模板做旗舰验证（真实跑通组队-开战-攻击循环）；
3. 迁移存量模板（5 个场景脚本 + 1 个协议脚本）到 v2，验证等价；
4. 老引擎（ScriptRunner/ProtocolRunner）与旧模板目录下线，文档更新。

第 3 步完成前老引擎保留，双轨运行，互不影响。

## 7. 验收标准

- `knight_tower` 模板不改一行 Rust 跑通完整循环（组队→开战→攻击→结算→再组队），num 满阈值自动退出；
- `claim_all_mail` 迁移为 v2 模板后行为等价；
- 一个含 scene 谓词 + click 动作的混合 step 在同一模板内生效（证明画面兜底能力）。

## 8. 实现与验证记录（2026-09-03）

已实现（提交见 git 历史）：TaskRunner、schema v2（`resources/tasks/`）、数组选择器（`@where/@max/@min/@first/@last` 含链式组合与列表 any-hit）、payload `$` 引用、`missing/contains/ends_with` 条件 op、`eval_js` 逃生舱（包装为必返字符串）、tasks 优先的路由与脚本列表合并。

验证：

- **单测 22/22**：schema 解析、选择器全族（白名单+@max 链式）、once 线性流、条件终止、scene+click 混合谓词、wait 超时策略、runner 接线（mock 驱动）；
- **live 验证**（真实账号 + 外部账号组队开战，使用高阈值临时模板避免当日 num=7 立即终止，用后已删除）：日志完整记录 `reload_tower_info → join_team（白名单选择器选中并入队）→ fight ×2（移动 channel + 攻击命中）→ 战斗结算`，随后执行器按状态机自然进入下一轮——**全程零任务特定 Rust 代码**，全部行为由 YAML 模板驱动；
- 选择器白名单在真实队伍列表上正确过滤并选中人数最多的队伍。

验收标准对照：① knight_tower 循环 ✓（阈值终止路径由单测 `finish_on_state_condition` 覆盖）；②③ 单测覆盖（`linear_once_flow_completes`、`scene_predicate_with_click_fallback`）。存量模板（场景脚本 5 个 + 协议脚本 1 个）的迁移与旧引擎下线按 §6 后续进行。
