# Proposal: 任务调度器（Routine）——把任务当积木的执行层

> 2026-09-05 ｜ 状态：**提案，待评审** ｜ 前置：schema v2 任务（TaskRunner）、jslib 均已落地

## 1. 背景与动机

现状：任务（`resources/tasks/*.yaml`）是最小执行单元，一次运行一个，跑完即止。随着任务数量增加（高塔×2、集市、逐鹿、邮件、未来的更多日常），出现三类模板层解决不了的需求：

1. **每日一条龙**：用户希望"点一次，把一批日常按顺序跑完"（邮件 → 逐鹿 → 高塔 → 集市），而不是逐个 Start；
2. **周期性重跑**："每 N 分钟跑一次某任务"（如定时看集市是否开启）——这是调度语义，不属于任务模板；
3. **24 小时挂机**：多个任务 + 活动窗口等待 + 失败续跑 + 跨天重置。

设计原则与 TaskRunner 相同：**机制在执行器，知识在模板**。Routine 不新增任何"任务内"能力，只编排任务之间的顺序与重跑策略。

## 2. 定位：Routine 与 Task 的关系

```
RoutineRunner（新增，轻量）
  └─ 按 routine 定义的顺序调度
       └─ TaskRunner.run()（现有，原样复用）
            └─ 模板 YAML（现有，无需改动）
```

Routine 是一个 YAML（`resources/routines/*.yaml`），UI 的脚本下拉框中与任务并列展示（路由：routines 先于 tasks 匹配或加前缀区分）。

## 3. Schema 草案

```yaml
name: 日常例程
description: 每日日常：邮件 → 逐鹿 → 高塔 → 集市（开着就买）
loop: { policy: once }              # once | interval: 1h | daily_reset
on_task_fail: continue              # continue（默认）| stop
steps:
  - { task: 一键领邮件 }

  - { task: 群雄逐鹿 }

  - { task: 武魁高塔·天狼, retry: { count: 2, interval: 5m } }

  # 等待条件（比如活动开启）：复用现有条件语法，带超时与超时策略
  - wait_until:
      conditions:
        - { field: "state.S_2_C_ACTIVITY_INFO", op: exists }   # 示意：以活动数据到达为准
      timeout: 3h
      on_timeout: skip            # skip（跳过下一步）| fail（例程失败）

  - { task: 集市·打折商城 }
```

语义：

- **顺序执行**：上一个 TaskRunner 结束（Completed/ResourceExhausted）才启动下一个；
- **`on_task_fail: continue`**：某任务因 infra/配置失败时跳过它继续后续；`stop` 则整个例程失败；
- **`retry`**：任务失败时按间隔重试 N 次；
- **`wait_until`**：阻塞直到条件成立（复用 state./role. 条件）或超时；`on_timeout: skip` 跳过**紧随的下一个任务步骤**，`fail` 终止例程；
- **loop**：
  - `once`：跑完一遍结束；
  - `interval`：跑完一遍睡 N 后再跑（周期任务）；
  - `daily_reset`：跑完一遍睡到游戏日重置（服务器 0 点）再跑——24 小时挂机形态；
- **手动停止**：与现有 Stop 一致（随时可停，运行中的 TaskRunner 收到 Manual）。

## 4. 三个用例对照

| 用例 | routine 写法 |
|---|---|
| 每日一条龙 | 上面的例子，`loop: once` |
| 周期任务 | `steps: [{task: 集市·打折商城}], loop: { policy: interval, interval: 20m }` |
| 24h 挂机 | 日常例程 + `loop: daily_reset` + 集市前的 `wait_until`（活动窗口） |

## 5. 实现要点（小）

- 新增 `RoutineRunner`：不碰 TaskRunner；每个 routine step 复用 `session_actor.start_script` 的现有启动/停止/监听链路（ScriptStopped 事件驱动流转）；
- `wait_until` 复用 `condition_eval::conditions_met` 轮询（200ms）；
- `daily_reset` 的重置时刻：读服务器时间（`S_2_C_KEEP_ALIVE.cur_time` + time_diff 已有），睡到次日 00:00（时区按游戏服）；
- 校验：routine 引用的任务名构建期检查存在（同 jslib 校验）；
- UI：routine 与 task 同一入口（脚本列表合并展示，名字区分即可）。

## 6. 明确不做

- 不做任务内并发（一次只跑一个任务；并发的复杂度远高于收益）；
- 不做跨账号编排（分组批量由现有 Run Group / Start All 承担）；
- 不替代 `on_no_match: wait`（任务内等待依旧归模板）。

## 7. 验收标准

1. "每日一条龙"routine 在真实账号顺序跑通 3+ 任务，中间任务的失败被正确跳过/重试；
2. `interval` 周期任务按间隔重跑；
3. `daily_reset` 在服务器 0 点后自动开始新一轮；
4. 手动 Stop 在任意环节（任务内 / wait_until / 睡眠中）立即停止。
