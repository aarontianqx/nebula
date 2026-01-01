# Phase 4: 扩展功能 ✅

## 目标

实现 Keyboard Passthrough、批量操作、MongoDB 支持等高级功能，优化用户体验。

## 完成标准

- [x] Keyboard Passthrough 功能完整
- [x] 单击/长按/连击识别准确
- [x] 批量操作 (Spread to All) 正常
- [x] Auto Refresh 功能正常
- [x] MongoDB 支持 (可选)
- [ ] 性能优化 (持续进行)

---

## 1. Keyboard Passthrough

### 1.1 前端键盘事件监听

键盘透传在前端 React 组件中实现，通过 Canvas 元素直接监听键盘事件：

```tsx
// components/canvas/CanvasWindow.tsx
const handleKeyDown = (e: React.KeyboardEvent<HTMLCanvasElement>) => {
  if (!keyboardPassthrough) return;
  
  // 仅处理 A-Z 键
  const key = e.key.toUpperCase();
  if (!/^[A-Z]$/.test(key)) return;
  
  // 记录按下时间，用于判断长按
  keyPressTime.current[key] = Date.now();
  triggerClick(); // 立即触发一次点击
  
  // 启动长按定时器
  longPressTimer.current[key] = setInterval(() => {
    triggerClick();
  }, repeatIntervalMs);
};

const handleKeyUp = (e: React.KeyboardEvent<HTMLCanvasElement>) => {
  const key = e.key.toUpperCase();
  if (longPressTimer.current[key]) {
    clearInterval(longPressTimer.current[key]);
    delete longPressTimer.current[key];
  }
};
```

**优势**:
- 无需系统级权限（如 macOS 辅助功能权限）
- 事件与画布焦点绑定，避免误触发
- 配置从后端加载 (`gesture.yaml`)
```

### 1.2 Application 层

**`application/input/gesture.rs`**:
```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::infrastructure::input::{KeyCode, KeyEventType, RawKeyEvent};
use crate::infrastructure::config;

#[derive(Debug, Clone)]
pub enum Gesture {
    Tap { key: KeyCode },
    LongPressStart { key: KeyCode },
    LongPressRepeat { key: KeyCode },
    LongPressEnd { key: KeyCode },
}

enum KeyState {
    Idle,
    Pressed { since: Instant, timer_active: bool },
    LongPressing,
}

pub struct GestureRecognizer {
    key_states: HashMap<KeyCode, KeyState>,
    gesture_tx: mpsc::UnboundedSender<Gesture>,
    config: config::KeyboardPassthroughConfig,
}

impl GestureRecognizer {
    pub fn new(gesture_tx: mpsc::UnboundedSender<Gesture>) -> Self {
        Self {
            key_states: HashMap::new(),
            gesture_tx,
            config: config::gesture().keyboard_passthrough.clone(),
        }
    }
    
    pub fn process(&mut self, event: RawKeyEvent) {
        match event.event_type {
            KeyEventType::Press => self.on_press(event.key, event.timestamp),
            KeyEventType::Release => self.on_release(event.key, event.timestamp),
        }
    }
    
    fn on_press(&mut self, key: KeyCode, now: Instant) {
        // 防抖检查
        if let Some(KeyState::Pressed { since, .. }) = self.key_states.get(&key) {
            if now.duration_since(*since) < self.config.debounce_window() {
                return;
            }
        }
        
        self.key_states.insert(key, KeyState::Pressed {
            since: now,
            timer_active: true,
        });
        
        // 启动长按检测定时器
        let threshold = self.config.long_press_threshold();
        let repeat_interval = self.config.repeat_interval();
        let tx = self.gesture_tx.clone();
        
        tokio::spawn(async move {
            sleep(threshold).await;
            
            // 发送长按开始
            if tx.send(Gesture::LongPressStart { key }).is_err() {
                return;
            }
            
            // 周期性发送重复
            loop {
                sleep(repeat_interval).await;
                if tx.send(Gesture::LongPressRepeat { key }).is_err() {
                    break;
                }
            }
        });
    }
    
    fn on_release(&mut self, key: KeyCode, now: Instant) {
        let Some(state) = self.key_states.remove(&key) else {
            return;
        };
        
        match state {
            KeyState::Pressed { since, .. } => {
                let duration = now.duration_since(since);
                if duration < self.config.long_press_threshold() {
                    // 短按 → Tap
                    let _ = self.gesture_tx.send(Gesture::Tap { key });
                }
            }
            KeyState::LongPressing => {
                let _ = self.gesture_tx.send(Gesture::LongPressEnd { key });
            }
            KeyState::Idle => {}
        }
    }
}
```

**`application/input/processor.rs`**:
```rust
use tokio::sync::mpsc;

use crate::infrastructure::input::{KeyboardListener, RawKeyEvent};
use crate::application::input::gesture::{Gesture, GestureRecognizer};
use crate::application::coordinator::CoordinatorCommand;

pub struct InputEventProcessor {
    keyboard: Box<dyn KeyboardListener>,
    gesture_recognizer: GestureRecognizer,
    coordinator_tx: mpsc::Sender<CoordinatorCommand>,
    
    active_session: Option<String>,
    cursor_position: Option<(i32, i32)>,
    cursor_in_bounds: bool,
    enabled: bool,
}

impl InputEventProcessor {
    pub fn new(
        keyboard: Box<dyn KeyboardListener>,
        coordinator_tx: mpsc::Sender<CoordinatorCommand>,
    ) -> Self {
        let (gesture_tx, gesture_rx) = mpsc::unbounded_channel();
        
        Self {
            keyboard,
            gesture_recognizer: GestureRecognizer::new(gesture_tx),
            coordinator_tx,
            active_session: None,
            cursor_position: None,
            cursor_in_bounds: false,
            enabled: false,
        }
    }
    
    pub async fn run(mut self, mut gesture_rx: mpsc::UnboundedReceiver<Gesture>) {
        let mut keyboard_rx = self.keyboard.take_receiver().unwrap();
        
        loop {
            tokio::select! {
                Some(raw_event) = keyboard_rx.recv() => {
                    if self.enabled {
                        self.gesture_recognizer.process(raw_event);
                    }
                }
                
                Some(gesture) = gesture_rx.recv() => {
                    self.handle_gesture(gesture).await;
                }
            }
        }
    }
    
    async fn handle_gesture(&mut self, gesture: Gesture) {
        // 检查前置条件
        let Some(session_id) = &self.active_session else { return };
        let Some((x, y)) = self.cursor_position else { return };
        
        if !self.cursor_in_bounds {
            return;
        }
        
        // 生成点击命令
        let should_click = matches!(
            gesture,
            Gesture::Tap { .. } | Gesture::LongPressStart { .. } | Gesture::LongPressRepeat { .. }
        );
        
        if should_click {
            let cmd = CoordinatorCommand::Click {
                session_id: session_id.clone(),
                x: x as f64,
                y: y as f64,
            };
            let _ = self.coordinator_tx.send(cmd).await;
        }
    }
    
    pub fn update_cursor(&mut self, x: i32, y: i32, in_bounds: bool) {
        self.cursor_position = Some((x, y));
        self.cursor_in_bounds = in_bounds;
    }
    
    pub fn set_active_session(&mut self, session_id: Option<String>) {
        self.active_session = session_id;
    }
    
    pub async fn set_enabled(&mut self, enabled: bool) -> anyhow::Result<()> {
        if enabled && !self.enabled {
            self.keyboard.start().await?;
        } else if !enabled && self.enabled {
            self.keyboard.stop();
        }
        self.enabled = enabled;
        Ok(())
    }
}
```

### 1.3 Tauri Commands

```rust
#[tauri::command]
pub async fn set_keyboard_passthrough(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.input_processor
        .lock()
        .await
        .set_enabled(enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_cursor_position(
    x: i32,
    y: i32,
    in_bounds: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.input_processor
        .lock()
        .await
        .update_cursor(x, y, in_bounds);
    Ok(())
}
```

### 1.4 前端集成

```tsx
// components/canvas/CanvasWindow.tsx
const handleMouseMove = throttle((e: React.MouseEvent) => {
  const rect = canvasRef.current?.getBoundingClientRect();
  if (!rect) return;
  
  invoke('update_cursor_position', {
    x: Math.round(e.clientX - rect.left),
    y: Math.round(e.clientY - rect.top),
    inBounds: true,
  });
}, 50);

const handleMouseLeave = () => {
  invoke('update_cursor_position', { x: 0, y: 0, inBounds: false });
};
```

---

## 2. 批量操作

### 2.1 Spread to All

当启用 "Spread to All" 时，画布点击会发送到所有活跃会话：

```rust
// application/coordinator.rs
pub enum CoordinatorCommand {
    // ...
    ClickAll { x: f64, y: f64 },
    SyncScriptSelection { script_name: String },
    StartAllScripts { script_name: String },
    StopAllScripts,
}

impl Coordinator {
    async fn handle_command(&mut self, cmd: CoordinatorCommand) {
        match cmd {
            CoordinatorCommand::ClickAll { x, y } => {
                for handle in self.sessions.values() {
                    if handle.info.state.can_accept_click() {
                        let _ = handle.cmd_tx.send(SessionCommand::Click { x, y }).await;
                    }
                }
            }
            CoordinatorCommand::StartAllScripts { script_name } => {
                for handle in self.sessions.values() {
                    if handle.info.state == SessionState::Ready {
                        let _ = handle.cmd_tx.send(SessionCommand::StartScript {
                            script_name: script_name.clone(),
                        }).await;
                    }
                }
            }
            CoordinatorCommand::StopAllScripts => {
                for handle in self.sessions.values() {
                    if handle.info.state == SessionState::ScriptRunning {
                        let _ = handle.cmd_tx.send(SessionCommand::StopScript).await;
                    }
                }
            }
            // ...
        }
    }
}
```

### 2.2 Auto Refresh

```rust
// application/service/session_service.rs
impl SessionActor {
    async fn handle_command(&mut self, cmd: SessionCommand) -> bool {
        match cmd {
            SessionCommand::SetAutoRefresh { enabled, interval_ms } => {
                if enabled {
                    self.start_auto_refresh(interval_ms).await;
                } else {
                    self.stop_auto_refresh().await;
                }
            }
            // ...
        }
        true
    }
    
    async fn start_auto_refresh(&mut self, interval_ms: u64) {
        // 启动 Screencast
        let _ = self.browser.start_screencast(80, 5).await;
    }
    
    async fn stop_auto_refresh(&mut self) {
        let _ = self.browser.stop_screencast().await;
    }
}
```

---

## 3. MongoDB 支持

通过配置文件选择存储后端，无需编译时 feature flag。

### 3.1 配置

```yaml
# resources/configs/app.yaml
storage:
  type: mongodb  # 或 sqlite (默认)
  mongodb:
    uri: "mongodb://localhost:27017"
    database: "wardenly"
```

### 3.2 主键处理

MongoDB 使用 `_id` 作为主键，SQLite 使用 `id`。通过 ULID 确保 ID 时间有序，便于数据迁移：
- ULID 字符串作为主键，兼容两种存储
- MongoDB 将 `id` 字段映射到 `_id`
- 排序按 `(ranking ASC, id ASC)` 保持一致

---

## 4. 性能优化

### 4.1 Screencast 优化

- 限制最大帧率 (5 FPS)
- 使用 JPEG 压缩 (quality 80)
- 仅当画布可见时启用

### 4.2 事件节流

- 鼠标位置更新节流 (50ms)
- 键盘事件防抖 (50ms)

### 4.3 内存管理

- 帧缓冲复用
- 及时释放浏览器资源

---

## 5. 验收检查

### 功能验收

- [ ] Keyboard Passthrough 可启用/禁用
- [ ] 单击识别准确
- [ ] 长按连击正常
- [ ] Spread to All 点击扩散
- [ ] Auto Refresh 自动刷新
- [ ] Run All 批量启动脚本
- [ ] Stop All 批量停止脚本
- [ ] MongoDB 连接正常 (可选)

### 权限处理

- [ ] macOS 辅助功能权限提示
- [ ] 权限拒绝时优雅降级

### 性能指标

- [ ] 帧率稳定 (≥5 FPS)
- [ ] CPU 占用合理 (<30%)
- [ ] 内存无泄漏

---

## 6. 发布准备

### 6.1 文档完善

- [ ] README 完整
- [ ] 用户手册
- [ ] 配置说明

### 6.2 打包测试

- [ ] macOS 签名/公证
- [ ] Windows 安装包
- [ ] Linux AppImage

### 6.3 已知问题

记录已知问题和限制，在后续版本修复。

