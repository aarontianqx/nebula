# Script DSL V2 - 脚本语言重构设计

## 概述

本文档描述对 wardenly-rs 脚本系统的重构计划，主要解决两个核心问题：

1. **循环机制易碎**：基于索引的 `startIndex/endIndex` 方式难以维护
2. **OCR 规则硬编码**：无法通过配置扩展新的 OCR 检测逻辑

重构目标是提升**可维护性**和**可扩展性**，同时保持向后兼容性和执行器的清晰逻辑。

---

## 一、循环机制重构

### 1.1 现状问题

**当前格式**（以 `join_tower.yaml` 为例）：

```yaml
actions:
  - type: click
    points: [{x: 546, y: 679}]
  - type: wait
    duration: 1s
  - type: click
    points: [{x: 726, y: 194}]
  # ... 共 11 个 action (index 0-10)
  - type: check_scene
loop:
  startIndex: 0    # 必须手动数
  endIndex: 10     # 中间增删 action 会导致索引失效
  count: -1
  interval: 800ms
```

**问题**：
- 新增/删除 action 需同步更新 `startIndex`/`endIndex`
- 肉眼难以快速验证循环边界正确性
- 代码审查时无法直观看出哪些 action 在循环内

### 1.2 新格式设计

**设计原则**：将 `loop` 作为一种容器类型的 Action，通过结构嵌套表达循环范围。

```yaml
actions:
  # 循环前的初始化动作（可选）
  - type: click
    points: [{x: 100, y: 100}]
  
  # Loop 作为 Action 类型，内部嵌套子 actions
  - type: loop
    count: -1           # -1 表示无限循环
    interval: 800ms     # 循环间隔
    until: "success"    # 可选：匹配到此场景时退出循环
    actions:            # 嵌套的 actions 列表
      - type: click
        points: [{x: 546, y: 679}]
      - type: wait
        duration: 1s
      - type: click
        points: [{x: 726, y: 194}]
      - type: check_scene
  
  # 循环后的收尾动作（可选）
  - type: click
    points: [{x: 78, y: 27}]
```

**优势**：
- 增删 action 时无需调整任何索引
- 循环范围一目了然（缩进即边界）
- 支持多层嵌套（未来扩展）

### 1.3 Rust 结构体变更

**当前结构**（`script.rs`）：

```rust
pub struct Step {
    pub expected_scene: String,
    pub actions: Vec<Action>,
    pub loop_config: Option<LoopConfig>,  // 与 actions 平级
    // ...
}

pub struct LoopConfig {
    pub start_index: usize,
    pub end_index: usize,
    pub count: i32,
    pub until: Option<String>,
    pub interval: Option<Duration>,
}
```

**新结构**：

```rust
/// Action 现在支持嵌套（联合类型）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Click {
        points: Vec<Point>,
    },
    Wait {
        #[serde(with = "humantime_serde")]
        duration: Duration,
    },
    Drag {
        points: Vec<Point>,
    },
    Quit {
        condition: Option<Condition>,
    },
    Incr {
        key: String,
    },
    Decr {
        key: String,
    },
    CheckScene,
    
    // 新增：Loop 作为容器 Action
    Loop {
        /// 循环次数 (-1 = 无限)
        #[serde(default = "default_infinite")]
        count: i32,
        
        /// 循环间隔
        #[serde(default, with = "humantime_serde")]
        interval: Option<Duration>,
        
        /// 退出条件：匹配到此场景时退出
        until: Option<String>,
        
        /// 嵌套的 actions
        actions: Vec<Action>,
    },
}

fn default_infinite() -> i32 { -1 }
```

**Step 结构简化**：

```rust
pub struct Step {
    pub expected_scene: String,
    pub timeout: Option<Duration>,
    pub actions: Vec<Action>,  // 可包含 Loop Action
    pub ocr_rule: Option<OcrRule>,
    // 移除: pub loop_config: Option<LoopConfig>
}
```

### 1.4 执行逻辑变更

**当前逻辑**（`script_runner.rs`）：

```rust
async fn execute_step_by_index(&mut self, step_idx: usize, image: &DynamicImage) -> StepResult {
    let step = self.script.steps[step_idx].clone();
    
    // OCR 检查
    if let Some(ref ocr_rule) = step.ocr_rule { ... }
    
    // 分支：有循环 vs 无循环
    if let Some(loop_config) = &step.loop_config {
        return self.execute_loop_cloned(&step, loop_config.clone()).await;
    }
    
    self.execute_actions(&step.actions).await
}
```

**新逻辑**：

```rust
async fn execute_step_by_index(&mut self, step_idx: usize, image: &DynamicImage) -> StepResult {
    let step = self.script.steps[step_idx].clone();
    
    // OCR 检查（保持不变）
    if let Some(ref ocr_rule) = step.ocr_rule { ... }
    
    // 统一执行 actions（Loop 由 execute_action 递归处理）
    self.execute_actions(&step.actions).await
}

async fn execute_action(&mut self, action: &Action) -> StepResult {
    match action {
        Action::Click { points } => { ... }
        Action::Wait { duration } => { ... }
        Action::Drag { points } => { ... }
        Action::Quit { condition } => { ... }
        Action::Incr { key } => { ... }
        Action::Decr { key } => { ... }
        Action::CheckScene => { ... }
        
        // 新增：递归执行嵌套循环
        Action::Loop { count, interval, until, actions } => {
            self.execute_loop(count, interval, until, actions).await
        }
    }
}

async fn execute_loop(
    &mut self,
    count: &i32,
    interval: &Option<Duration>,
    until: &Option<String>,
    actions: &[Action],
) -> StepResult {
    let mut iteration = 0;
    let is_infinite = *count < 0;
    
    while self.running.load(Ordering::Relaxed) {
        // 检查停止命令
        if let Ok(ScriptCommand::Stop) = self.cmd_rx.try_recv() {
            return StepResult::Quit;
        }
        
        // 执行循环体（递归调用 execute_actions）
        let result = self.execute_actions(actions).await;
        if result != StepResult::Continue {
            return result;
        }
        
        // 检查 until 场景退出条件
        if let Some(until_scene) = until {
            if let Ok(image) = self.browser.capture_screen().await {
                if let Some(scene) = resources::find_scene(&self.scenes, until_scene) {
                    if self.scene_matcher.matches(scene, &image) {
                        tracing::debug!(scene = %until_scene, "Until scene matched");
                        break;
                    }
                }
            }
        }
        
        iteration += 1;
        
        // 检查次数限制
        if !is_infinite && iteration >= *count as usize {
            break;
        }
        
        // 循环间隔
        if let Some(interval_duration) = interval {
            sleep(*interval_duration).await;
        } else {
            sleep(Duration::from_millis(300)).await;
        }
    }
    
    StepResult::Continue
}
```

---

## 二、OCR 规则泛化

### 2.1 现状问题

**当前格式**：

```yaml
ocrRule:
  name: quit_when_exhausted
  roi:
    x: 510
    y: 602
    width: 90
    height: 50
  threshold: 7
```

**当前硬编码逻辑**（`script_runner.rs`）：

```rust
async fn check_ocr_rule(&self, ocr_rule: &OcrRule, image: &DynamicImage) -> Option<StepResult> {
    // 硬编码规则名判断
    if ocr_rule.name != "quit_when_exhausted" {
        tracing::warn!(rule = %ocr_rule.name, "Unknown OCR rule, skipping");
        return None;
    }
    
    // 调用下游接口（已固定，返回 UsageRatioResult { numerator, denominator }）
    match self.ocr_client.recognize_usage_ratio(image, Some(&roi)).await {
        Ok(result) => {
            // 硬编码判断逻辑
            if result.denominator > ocr_rule.threshold || result.denominator > result.numerator {
                return Some(StepResult::ResourceExhausted);
            }
        }
        // ...
    }
}
```

**问题**：
- 规则名称和判断逻辑绑定在 Rust 代码中
- 新增 OCR 场景需要修改 Rust 代码并重新编译
- `threshold` 字段语义不明确（是阈值还是最大值？）

### 2.2 设计目标

1. **下游接口复用**：`OcrClient::recognize_usage_ratio()` 保持不变，返回 `UsageRatioResult { numerator, denominator }`
2. **规则可配置**：通过 YAML 配置判断条件，无需修改 Rust 代码
3. **扩展性**：未来支持其他 OCR 类型（如纯文本识别）时可低成本添加

### 2.3 新格式设计

**核心思路**：引入 `mode` 字段区分 OCR 类型，使用表达式语法配置判断条件。

```yaml
# 用法一：资源耗尽检测（使用当前的 ratio 接口）
ocrRule:
  mode: ratio              # 调用 recognize_usage_ratio 接口
  roi:
    x: 510
    y: 602
    width: 90
    height: 50
  condition: "used >= total || used > 7"  # 表达式：used=denominator, total=numerator
  action: quit_exhausted                   # 匹配时的动作

# 用法二：未来扩展 - 纯文本识别（预留，暂不实现）
# ocrRule:
#   mode: text
#   roi: ...
#   pattern: "\\d+"         # 正则提取数字
#   variable: "gold"        # 提取为变量
#   condition: "gold < 1000"
#   action: quit
```

**变量映射**（针对 `mode: ratio`）：

| 表达式变量 | 映射值 | 说明 |
|-----------|--------|------|
| `used` | `result.denominator` | 已使用量（如：1/10 中的分母 10 表示当前消耗，视业务场景而定） |
| `total` | `result.numerator` | 总量（如：1/10 中的分子 1 表示剩余） |

> **注意**：变量名的语义由具体游戏场景决定。当前 `quit_when_exhausted` 的逻辑是 `denominator > threshold || denominator > numerator`，表示"当消耗量超过阈值或超过总量时退出"。新设计保持相同语义。

**支持的操作符**：

| 操作符 | 含义 |
|--------|------|
| `>` | 大于 |
| `>=` | 大于等于 |
| `<` | 小于 |
| `<=` | 小于等于 |
| `==` | 等于 |
| `!=` | 不等于 |
| `&&` or `and` | 逻辑与 |
| `||` or `or` | 逻辑或 |

**支持的 action**：

| Action | 说明 |
|--------|------|
| `quit_exhausted` | 返回 `StopReason::ResourceExhausted`，脚本正常退出 |
| `quit` | 返回 `StopReason::Completed`，脚本完成退出 |
| `skip` | 跳过当前 Step，继续下一轮主循环 |

### 2.4 Rust 结构体变更

**新结构**：

```rust
/// OCR 规则（支持多种模式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRule {
    /// OCR 模式
    #[serde(default = "default_ratio_mode")]
    pub mode: OcrMode,
    
    /// 识别区域
    pub roi: OcrRegion,
    
    /// 条件表达式（如 "used >= total || used > 7"）
    pub condition: String,
    
    /// 条件满足时的动作
    #[serde(default = "default_quit_exhausted")]
    pub action: OcrAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrMode {
    Ratio,  // 调用 recognize_usage_ratio
    // Text, // 未来扩展
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrAction {
    QuitExhausted,  // StopReason::ResourceExhausted
    Quit,           // StopReason::Completed
    Skip,           // 跳过当前 Step
}

fn default_ratio_mode() -> OcrMode { OcrMode::Ratio }
fn default_quit_exhausted() -> OcrAction { OcrAction::QuitExhausted }
```

### 2.5 表达式求值器

引入轻量级表达式解析器（可使用 `evalexpr` crate 或自行实现简单版本）。

**接口设计**：

```rust
/// 表达式求值上下文
pub struct ExprContext {
    variables: HashMap<String, i64>,
}

impl ExprContext {
    pub fn new() -> Self {
        Self { variables: HashMap::new() }
    }
    
    pub fn set(&mut self, name: &str, value: i64) {
        self.variables.insert(name.to_string(), value);
    }
    
    /// 求值表达式，返回布尔结果
    pub fn evaluate(&self, expr: &str) -> Result<bool, ExprError> {
        // 实现方案 A: 使用 evalexpr crate
        // 实现方案 B: 手写简单解析器（仅支持有限操作符）
    }
}
```

**执行逻辑**：

```rust
async fn check_ocr_rule(&self, ocr_rule: &OcrRule, image: &DynamicImage) -> Option<StepResult> {
    match ocr_rule.mode {
        OcrMode::Ratio => {
            let result = self.ocr_client.recognize_usage_ratio(image, Some(&roi)).await.ok()?;
            
            // 构建表达式上下文
            let mut ctx = ExprContext::new();
            ctx.set("used", result.denominator as i64);
            ctx.set("total", result.numerator as i64);
            
            // 求值条件表达式
            match ctx.evaluate(&ocr_rule.condition) {
                Ok(true) => {
                    // 条件满足，执行相应动作
                    match ocr_rule.action {
                        OcrAction::QuitExhausted => Some(StepResult::ResourceExhausted),
                        OcrAction::Quit => Some(StepResult::Quit),
                        OcrAction::Skip => Some(StepResult::Continue), // 特殊处理
                    }
                }
                Ok(false) => None,  // 条件不满足，继续执行
                Err(e) => {
                    tracing::warn!("OCR condition evaluation failed: {}", e);
                    None
                }
            }
        }
        // OcrMode::Text => { ... } // 未来扩展
    }
}
```

### 2.6 向后兼容

为支持旧格式（`name` + `threshold`），提供兼容性转换：

```rust
impl OcrRule {
    /// 从旧格式迁移到新格式
    pub fn migrate_legacy(&mut self) {
        // 检测旧格式（有 name 字段但无 condition 字段）
        if let Some(legacy_name) = &self.legacy_name {
            if legacy_name == "quit_when_exhausted" && self.condition.is_empty() {
                let threshold = self.legacy_threshold.unwrap_or(0);
                self.mode = OcrMode::Ratio;
                self.condition = format!("used > {} || used > total", threshold);
                self.action = OcrAction::QuitExhausted;
            }
        }
    }
}
```

> **建议**：在正式上线后的下一个版本移除旧格式支持。

---

## 三、脚本迁移示例

### 3.1 `join_tower.yaml` 迁移

**迁移前**：

```yaml
- scene: tower_entrance_1
  ocrRule:
    name: quit_when_exhausted
    roi: {x: 510, y: 602, width: 90, height: 50}
    threshold: 7
  actions:
    - type: click
      points: [{x: 546, y: 679}]
    - type: wait
      duration: 1s
    # ... 共 11 个 action
    - type: check_scene
  loop:
    startIndex: 0
    endIndex: 10
    count: -1
    interval: 800ms
```

**迁移后**：

```yaml
- scene: tower_entrance_1
  ocrRule:
    mode: ratio
    roi: {x: 510, y: 602, width: 90, height: 50}
    condition: "used > 7 || used > total"
    action: quit_exhausted
  actions:
    - type: loop
      count: -1
      interval: 800ms
      actions:
        - type: click
          points: [{x: 546, y: 679}]
        - type: wait
          duration: 1s
        - type: click
          points: [{x: 726, y: 194}]
        - type: wait
          duration: 300ms
        - type: click
          points: [{x: 726, y: 314}]
        - type: wait
          duration: 1s
        - type: click
          points: [{x: 394, y: 447}]
        - type: wait
          duration: 1s
        - type: click
          points: [{x: 538, y: 420}]
        - type: wait
          duration: 1s
        - type: check_scene
```

### 3.2 `rivalry_reigns.yaml` 迁移

**迁移前**：

```yaml
- scene: daily_tasks
  actions:
    - type: drag
      points: [ ... ]  # 多点路径
    - type: wait
      duration: 1s
    # ... 共 7 个 action
    - type: quit
  loop:
    startIndex: 0
    endIndex: 6
    count: 30
    interval: 1s
```

**迁移后**：

```yaml
- scene: daily_tasks
  actions:
    - type: loop
      count: 30
      interval: 1s
      actions:
        - type: drag
          points: [ ... ]
        - type: wait
          duration: 1s
        - type: click
          points: [{x: 488, y: 374}]
        - type: wait
          duration: 2s
        - type: click
          points: [{x: 540, y: 526}]
        - type: wait
          duration: 500ms
        - type: click
          points: [{x: 78, y: 24}]
        - type: quit
```

---

## 四、影响范围与改动清单

### 4.1 需修改的文件

| 文件 | 改动类型 | 说明 |
|------|----------|------|
| `src-tauri/src/domain/model/script.rs` | **重写** | Action 改为 enum，移除 LoopConfig，新增 OcrMode/OcrAction |
| `src-tauri/src/application/service/script_runner.rs` | **重写** | 循环执行逻辑重构为递归，OCR 检查重写为表达式求值 |
| `src-tauri/Cargo.toml` | 修改 | 添加 `evalexpr` 依赖（或其他表达式库） |
| `src-tauri/resources/scripts/*.yaml` | **全部迁移** | 5 个脚本文件需更新格式 |
| `docs/FUNCTIONAL_GUIDE.md` | 更新 | 更新脚本配置格式说明 |
| `docs/PROJECT_STRUCTURE.md` | 更新 | 更新 Script/Action 结构说明 |

### 4.2 不受影响的部分

| 模块 | 说明 |
|------|------|
| `OcrClient` trait 及实现 | 下游接口不变 |
| `Scene` 及场景匹配逻辑 | 不涉及 |
| 前端 UI | 不涉及（脚本配置无 UI） |
| Tauri commands/events | 不涉及 |
| Account/Group/Session 管理 | 不涉及 |

### 4.3 实施步骤建议

1. **Phase 1**: 修改 `script.rs`，完成 Action enum 重构和新 OcrRule 结构
2. **Phase 2**: 重写 `script_runner.rs`，实现递归执行和表达式求值
3. **Phase 3**: 迁移所有脚本 YAML 文件
4. **Phase 4**: 更新文档
5. **Phase 5**: 测试验证（手动运行每个脚本确认行为一致）

---

## 五、未来扩展预留

### 5.1 脚本复用（Call Script）

当前 Action enum 设计允许低成本添加新类型：

```rust
// 未来扩展
pub enum Action {
    // ... existing variants ...
    
    CallScript {
        name: String,           // 被调用脚本名
        args: Option<HashMap<String, Value>>,  // 可选参数
    },
}
```

对应 YAML：

```yaml
actions:
  - type: call_script
    name: "common_login_flow"
```

执行时从资源中加载目标 Script 并递归执行其 steps。

### 5.2 新 OCR 模式（Text Recognition）

当需要识别纯文本（如金币数量、倒计时）时：

```rust
pub enum OcrMode {
    Ratio,
    Text {
        pattern: String,        // 正则提取
        variable: String,       // 变量名
    },
}
```

对应 YAML：

```yaml
ocrRule:
  mode: text
  roi: ...
  pattern: "\\d+"
  variable: "gold"
  condition: "gold < 1000"
  action: quit
```

需同时在 `OcrClient` trait 中添加 `recognize_text()` 方法。

---

## 六、风险与注意事项

1. **表达式注入安全**：使用受限的表达式库（如 evalexpr），禁止函数调用和副作用
2. **递归深度**：嵌套 Loop 可能导致栈溢出，考虑设置最大嵌套深度限制（如 5 层）
3. **迁移验证**：迁移后需逐一验证每个脚本的执行行为与迁移前一致
4. **向后兼容期**：建议保留旧格式解析支持 1-2 个版本，便于平滑过渡

---

## 附录：现有脚本清单

| 脚本 | 是否使用 Loop | 是否使用 OCR | 迁移复杂度 |
|------|---------------|--------------|------------|
| `join_battle.yaml` | ✅ (无限循环) | ❌ | 低 |
| `join_tower.yaml` | ✅ (无限循环 x2) | ✅ (x2) | 中 |
| `lead_battle.yaml` | ✅ (无限循环) | ❌ | 低 |
| `military_dispatch.yaml` | ❌ | ❌ | 最低 |
| `rivalry_reigns.yaml` | ✅ (30次) | ❌ | 低 |
