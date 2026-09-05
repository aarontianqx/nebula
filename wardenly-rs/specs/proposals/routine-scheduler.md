# Proposal: 任务调度器（Scheduler）——任务之上的状态循环

> 2026-09-05 v2 ｜ 状态：**提案，待评审**（v2：由"串行 routine"改为"调度循环"，覆盖周期/条件触发/每日一次/时间窗四类形态）｜ 前置：schema v2 任务（TaskRunner）、jslib 均已落地

## 1. 动机与三类真实形态

任务数量增长后，"什么时候跑任务"本身需要一层机制。真实需求不是串行流水线，而是四种调度形态：

| 形态 | 例子 | 需要的能力 |
|---|---|---|
| **周期观察** | 领邮件：每隔一段时间看一次 | `every`（距上次运行至少 N） |
| **每日一次（带完成记录）** | 群雄逐鹿：打满 30 场+领完奖，第二天才再做 | `daily` + `done_when`（完成判定读游戏真值，不依赖本地记录） |
| **条件触发 + 每日上限** | 武魁高塔：没队伍不空转，有队伍就进，满 7/10 次当天不再尝试 | `eligible_when` + `done_when` + `max_run` + `defer` |
| **时间窗** | 集市：12:30 开启，过了就没意义 | `window`（窗口内才参与调度） |

关键设计结论（与 TaskRunner 同构）：**调度器自己也是一个状态匹配循环**——条目带"什么时候该跑/什么时候算完/没轮上时多久再看"，判定语言与模板谓词完全相同（state./role. 条件、选择器、default）。模板层零改动。

## 2. 执行模型

```
loop {
  按条目顺序（=优先级）遍历：
    done_when 成立？        → 今日已完成，跳过
    window 不在窗口内？      → 跳过
    eligible_when 不成立？   → 本轮跳过
    距上次启动 < every？     → 跳过
    否则 → 启动该任务；任务自然结束、或 max_run 到点强收
  sleep(poll_interval)      # 默认 30~60s
}
```

- 一次只跑一个任务（执行权独占）；任务结束/被强收后进入下一轮；
- `defer`：本轮不可用的条目，按各自间隔退避，不空转；
- 手动停止随时生效（任务内、睡眠中立即停）。

## 3. Schema 草案（`resources/schedules/*.yaml`）

```yaml
name: 24h挂机例程
description: 邮件周期领取 + 逐鹿/高塔每日完成 + 集市窗口抢购
poll_interval: 45s                 # 调度循环间隔（默认 60s）
entries:
  - name: 领邮件
    task: 一键领邮件
    every: 30m

  - name: 群雄逐鹿
    task: 群雄逐鹿
    daily: true
    done_when:
      - { field: state.S_2_C_TOURNAMENT_LOAD.battle_num, op: gte, value: 30 }

  - name: 武魁高塔·天狼
    task: 武魁高塔·天狼
    daily: true
    done_when:
      - { field: role._knightTower._teamNumInfo.num, op: gte, value: 7 }
    eligible_when:
      - { field: "state.S_2_C_KNIGHT_TOWER_TEAM_INFO.battle_team_info_ary.@where(server_id, ends_with, [\"-888\"])", op: exists }
    max_run: 10m
    defer: 5m

  - name: 集市·打折商城
    task: 集市·打折商城
    daily: true
    window: { after: "12:25", before: "23:59" }   # 服务器时间
    # 完成判定可读（个人限购计数）则 done_when；读不到则靠完成记录
```

字段语义：

- `task`：引用现有任务名（构建期校验存在）；
- `daily: true`：每天最多"完成"一次——`done_when` 成立后记为当日完成，服务器日重置后重新评估；
- `done_when`（可选）：完成判定，**优先读游戏真值**（次数/领取列表都在协议数据里，重启、换机都不错乱）；读不到的任务用调度器的按天完成记录兜底（任务自然结束即记为完成）；
- `eligible_when`（可选）：不满足则本轮不启动（不空转、不阻塞其它条目）；
- `every`（可选）：距上次启动的最小间隔（周期形态）；
- `window`（可选）：服务器时间的每日窗口（时刻取自 `S_2_C_KEEP_ALIVE.cur_time`）；
- `max_run`（可选）：单次运行上限，到点强收并 defer（防止"任务内长等"饿死其它条目）；
- `defer`（可选）：本轮不可用后的退避时长（默认 = poll_interval）。

## 4. 三个案例的落法复核

- **领邮件**：`every: 30m`，无 done_when——纯周期，永远不会"完成"；
- **群雄逐鹿**：`daily + done_when(battle_num>=30)`。完成判定全部来自服务器数据（场次、draw_index），重启应用也不会重复做；跨天时服务器计数复位，done_when 自然失效，第二天重新触发；
- **武魁高塔**：`eligible_when`（无白名单队伍不启动）+ `max_run`（启动了不开战到点强收）+ `done_when(num>=7)`（满次数当天永久跳过）。模板里的 `on_no_match: wait 24h` 与调度兼容：调度器决定"什么时候值得启动"，任务内等待随时可被 max_run 打断；
- **集市**：`window` 卡活动时段；完成判定用个人限购计数（可读），买满即当日完成。

## 5. 实现要点

- 新增 `SchedulerRunner`（application 层）：与 TaskRunner 并列，复用 session 的脚本启动/停止/事件链路；条目的 done/eligible 判定复用 `condition_eval::conditions_met`；
- 时间基准：服务器时间（`S_2_C_KEEP_ALIVE.cur_time + time_diff`，GameState 已有）；
- 完成记录兜底：`logs/schedules/<date>.json`（只记"读不到真值"的条目完成态）；
- UI：schedule 与 task 在同一脚本列表展示、同一路由启动（schedule 优先匹配）；
- 构建期校验：引用的任务名存在、done/eligible 条件可解析。

## 6. 明确不做（第一版）

- 不做并发任务（一次一个，执行权独占）；
- 不做事件驱动唤醒（轮询足够；后续可作为同一循环上的优化）；
- 不改成有任务模板（`on_no_match` 等任务内语义原样保留）。

## 7. 验收标准

1. 四类形态的条目在同一 schedule 里共存并按预期轮转（真实账号观察至少一个完整调度日）；
2. 高塔条目：无队不启动、开战即进入、满 7 后当天不再启动；
3. 应用重启后，已完成条目不会被重做（真值判定）；
4. 手动 Stop 在任务内/睡眠中立即生效。
