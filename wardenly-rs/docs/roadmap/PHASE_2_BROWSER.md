# Phase 2: 浏览器与画布

## 目标

集成 chromiumoxide，实现浏览器自动化、账户登录、画布实时同步。

## 完成标准

- [ ] Session 生命周期完整 (创建→登录→就绪→停止)
- [ ] 浏览器 headless 启动正常
- [ ] 自动登录功能可用
- [ ] 画布 Screencast 同步
- [ ] 画布点击/拖拽交互

---

## 1. 新增依赖

```toml
# Cargo.toml 新增
chromiumoxide = { version = "0.7", features = ["tokio-runtime"] }
image = "0.25"
base64 = "0.22"
tokio-stream = "0.1"
```

---

## 2. Domain 层扩展

### 2.1 Session 状态机

**`domain/model/session.rs`**:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Idle,
    Starting,
    LoggingIn,
    Ready,
    ScriptRunning,
    Stopped,
}

impl SessionState {
    pub fn can_transition_to(&self, target: SessionState) -> bool {
        matches!(
            (self, target),
            (Self::Idle, Self::Starting)
                | (Self::Starting, Self::LoggingIn | Self::Stopped)
                | (Self::LoggingIn, Self::Ready | Self::Stopped)
                | (Self::Ready, Self::ScriptRunning | Self::Stopped)
                | (Self::ScriptRunning, Self::Ready | Self::Stopped)
        )
    }
    
    pub fn can_accept_click(&self) -> bool {
        matches!(self, Self::LoggingIn | Self::Ready | Self::ScriptRunning)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub account_id: String,
    pub display_name: String,
    pub state: SessionState,
}
```

### 2.2 领域事件

**`domain/event.rs`**:
```rust
use serde::{Deserialize, Serialize};
use super::model::session::SessionState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    SessionCreated {
        session_id: String,
        account_id: String,
        display_name: String,
    },
    SessionStateChanged {
        session_id: String,
        old_state: SessionState,
        new_state: SessionState,
    },
    ScreencastFrame {
        session_id: String,
        image_base64: String,
        timestamp: u64,
    },
    SessionStopped {
        session_id: String,
    },
    LoginSucceeded {
        session_id: String,
    },
    LoginFailed {
        session_id: String,
        reason: String,
    },
}
```

---

## 3. Infrastructure 层

### 3.1 BrowserDriver trait

**`infrastructure/browser/driver.rs`**:
```rust
use async_trait::async_trait;
use image::DynamicImage;

#[async_trait]
pub trait BrowserDriver: Send + Sync {
    /// 启动浏览器
    async fn start(&mut self) -> anyhow::Result<()>;
    
    /// 停止浏览器
    async fn stop(&mut self) -> anyhow::Result<()>;
    
    /// 导航到 URL
    async fn navigate(&self, url: &str) -> anyhow::Result<()>;
    
    /// 点击
    async fn click(&self, x: f64, y: f64) -> anyhow::Result<()>;
    
    /// 拖拽
    async fn drag(&self, from: (f64, f64), to: (f64, f64)) -> anyhow::Result<()>;
    
    /// 截图
    async fn capture_screen(&self) -> anyhow::Result<DynamicImage>;
    
    /// 启动 Screencast
    async fn start_screencast(&self, quality: u8, max_fps: u8) -> anyhow::Result<()>;
    
    /// 停止 Screencast
    async fn stop_screencast(&self) -> anyhow::Result<()>;
    
    /// 注入 Cookies
    async fn set_cookies(&self, cookies: &str) -> anyhow::Result<()>;
    
    /// 获取 Cookies
    async fn get_cookies(&self) -> anyhow::Result<String>;
    
    /// 执行 JavaScript
    async fn evaluate(&self, script: &str) -> anyhow::Result<String>;
}
```

### 3.2 Chromium 实现

**`infrastructure/browser/chromium.rs`**:
```rust
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use chromiumoxide::cdp::browser_protocol::page::ScreencastFrameEvent;
use tokio::sync::mpsc;

pub struct ChromiumDriver {
    browser: Option<Browser>,
    page: Option<Page>,
    frame_tx: mpsc::UnboundedSender<Vec<u8>>,
}

impl ChromiumDriver {
    pub fn new(frame_tx: mpsc::UnboundedSender<Vec<u8>>) -> Self {
        Self {
            browser: None,
            page: None,
            frame_tx,
        }
    }
}

#[async_trait]
impl BrowserDriver for ChromiumDriver {
    async fn start(&mut self) -> anyhow::Result<()> {
        let config = BrowserConfig::builder()
            .window_size(1080, 840)
            .viewport(chromiumoxide::handler::viewport::Viewport {
                width: 1080,
                height: 720,
                ..Default::default()
            })
            .build()?;
        
        let (browser, mut handler) = Browser::launch(config).await?;
        
        // 在后台处理浏览器事件
        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                // 处理事件
            }
        });
        
        let page = browser.new_page("about:blank").await?;
        
        self.browser = Some(browser);
        self.page = Some(page);
        
        Ok(())
    }
    
    async fn click(&self, x: f64, y: f64) -> anyhow::Result<()> {
        if let Some(page) = &self.page {
            page.move_mouse(x, y).await?;
            page.click(chromiumoxide::element::Element::new_position(x, y)).await?;
        }
        Ok(())
    }
    
    async fn start_screencast(&self, quality: u8, max_fps: u8) -> anyhow::Result<()> {
        if let Some(page) = &self.page {
            let tx = self.frame_tx.clone();
            
            let mut stream = page.start_screencast(
                chromiumoxide::cdp::browser_protocol::page::StartScreencastParams::builder()
                    .format(chromiumoxide::cdp::browser_protocol::page::StartScreencastFormat::Jpeg)
                    .quality(quality as i64)
                    .max_width(1080)
                    .max_height(720)
                    .build()
            ).await?;
            
            tokio::spawn(async move {
                while let Some(frame) = stream.next().await {
                    if let Ok(data) = base64::decode(&frame.data) {
                        let _ = tx.send(data);
                    }
                }
            });
        }
        Ok(())
    }
    
    // ... 其他方法实现
}
```

---

## 4. Application 层

### 4.1 EventBus

**`application/eventbus.rs`**:
```rust
use tokio::sync::broadcast;
use crate::domain::event::DomainEvent;

pub struct EventBus {
    sender: broadcast::Sender<DomainEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
    
    pub fn publish(&self, event: DomainEvent) {
        let _ = self.sender.send(event);
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.sender.subscribe()
    }
}
```

### 4.2 SessionActor

**`application/service/session_service.rs`**:
```rust
use tokio::sync::mpsc;
use crate::domain::model::session::{SessionState, SessionInfo};
use crate::domain::model::account::Account;
use crate::domain::event::DomainEvent;
use crate::infrastructure::browser::BrowserDriver;
use crate::application::eventbus::EventBus;

pub enum SessionCommand {
    Start,
    Stop,
    Click { x: f64, y: f64 },
    Drag { from: (f64, f64), to: (f64, f64) },
    StartScreencast,
    StopScreencast,
}

pub struct SessionHandle {
    pub id: String,
    pub info: SessionInfo,
    pub cmd_tx: mpsc::Sender<SessionCommand>,
}

pub struct SessionActor {
    id: String,
    account: Account,
    state: SessionState,
    
    cmd_rx: mpsc::Receiver<SessionCommand>,
    event_bus: EventBus,
    browser: Box<dyn BrowserDriver>,
    
    frame_rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl SessionActor {
    pub async fn run(mut self) {
        self.transition_to(SessionState::Starting).await;
        
        // 启动浏览器
        if let Err(e) = self.browser.start().await {
            tracing::error!("Failed to start browser: {}", e);
            self.transition_to(SessionState::Stopped).await;
            return;
        }
        
        // 导航到游戏
        let game_url = "https://game.example.com/";
        if let Err(e) = self.browser.navigate(game_url).await {
            tracing::error!("Failed to navigate: {}", e);
            self.transition_to(SessionState::Stopped).await;
            return;
        }
        
        self.transition_to(SessionState::LoggingIn).await;
        
        // 执行登录
        if let Err(e) = self.perform_login().await {
            tracing::error!("Login failed: {}", e);
            self.event_bus.publish(DomainEvent::LoginFailed {
                session_id: self.id.clone(),
                reason: e.to_string(),
            });
        } else {
            self.transition_to(SessionState::Ready).await;
            self.event_bus.publish(DomainEvent::LoginSucceeded {
                session_id: self.id.clone(),
            });
        }
        
        // 命令处理循环
        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    if !self.handle_command(cmd).await {
                        break;
                    }
                }
                Some(frame) = self.frame_rx.recv() => {
                    self.handle_frame(frame).await;
                }
            }
        }
        
        self.cleanup().await;
    }
    
    async fn handle_command(&mut self, cmd: SessionCommand) -> bool {
        match cmd {
            SessionCommand::Stop => {
                self.transition_to(SessionState::Stopped).await;
                return false;
            }
            SessionCommand::Click { x, y } => {
                if self.state.can_accept_click() {
                    let _ = self.browser.click(x, y).await;
                }
            }
            SessionCommand::Drag { from, to } => {
                if self.state.can_accept_click() {
                    let _ = self.browser.drag(from, to).await;
                }
            }
            SessionCommand::StartScreencast => {
                let _ = self.browser.start_screencast(80, 5).await;
            }
            SessionCommand::StopScreencast => {
                let _ = self.browser.stop_screencast().await;
            }
            _ => {}
        }
        true
    }
    
    async fn handle_frame(&self, frame: Vec<u8>) {
        let base64 = base64::encode(&frame);
        self.event_bus.publish(DomainEvent::ScreencastFrame {
            session_id: self.id.clone(),
            image_base64: base64,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        });
    }
    
    async fn perform_login(&mut self) -> anyhow::Result<()> {
        // 优先使用 Cookies
        if let Some(cookies) = &self.account.cookies {
            self.browser.set_cookies(cookies).await?;
            self.browser.navigate("https://game.example.com/").await?;
            // 检查是否登录成功
            // ...
            return Ok(());
        }
        
        // 使用用户名密码登录
        // 填写表单，点击登录按钮
        // ...
        
        // 保存 Cookies
        let cookies = self.browser.get_cookies().await?;
        self.account.cookies = Some(cookies);
        
        Ok(())
    }
    
    async fn transition_to(&mut self, new_state: SessionState) {
        let old_state = self.state;
        self.state = new_state;
        
        self.event_bus.publish(DomainEvent::SessionStateChanged {
            session_id: self.id.clone(),
            old_state,
            new_state,
        });
    }
    
    async fn cleanup(&mut self) {
        let _ = self.browser.stop().await;
        self.event_bus.publish(DomainEvent::SessionStopped {
            session_id: self.id.clone(),
        });
    }
}
```

### 4.3 Coordinator

**`application/coordinator.rs`**:
```rust
use std::collections::HashMap;
use tokio::sync::mpsc;
use crate::application::service::session_service::{SessionHandle, SessionActor, SessionCommand};
use crate::domain::model::account::Account;

pub enum CoordinatorCommand {
    CreateSession { account: Account },
    StopSession { session_id: String },
    Click { session_id: String, x: f64, y: f64 },
    ClickAll { x: f64, y: f64 },
}

pub struct Coordinator {
    sessions: HashMap<String, SessionHandle>,
    event_bus: EventBus,
    cmd_rx: mpsc::Receiver<CoordinatorCommand>,
}

impl Coordinator {
    pub async fn run(mut self) {
        while let Some(cmd) = self.cmd_rx.recv().await {
            self.handle_command(cmd).await;
        }
    }
    
    async fn handle_command(&mut self, cmd: CoordinatorCommand) {
        match cmd {
            CoordinatorCommand::CreateSession { account } => {
                self.create_session(account).await;
            }
            CoordinatorCommand::StopSession { session_id } => {
                if let Some(handle) = self.sessions.get(&session_id) {
                    let _ = handle.cmd_tx.send(SessionCommand::Stop).await;
                }
                self.sessions.remove(&session_id);
            }
            CoordinatorCommand::Click { session_id, x, y } => {
                if let Some(handle) = self.sessions.get(&session_id) {
                    let _ = handle.cmd_tx.send(SessionCommand::Click { x, y }).await;
                }
            }
            CoordinatorCommand::ClickAll { x, y } => {
                for handle in self.sessions.values() {
                    let _ = handle.cmd_tx.send(SessionCommand::Click { x, y }).await;
                }
            }
        }
    }
    
    async fn create_session(&mut self, account: Account) {
        // 检查是否已存在
        if self.sessions.values().any(|h| h.info.account_id == account.id) {
            return;
        }
        
        let session_id = uuid::Uuid::new_v4().to_string();
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let (frame_tx, frame_rx) = mpsc::unbounded_channel();
        
        let actor = SessionActor::new(
            session_id.clone(),
            account.clone(),
            cmd_rx,
            self.event_bus.clone(),
            frame_rx,
            frame_tx,
        );
        
        let handle = SessionHandle {
            id: session_id.clone(),
            info: SessionInfo {
                id: session_id.clone(),
                account_id: account.id.clone(),
                display_name: account.display_name(),
                state: SessionState::Idle,
            },
            cmd_tx,
        };
        
        self.sessions.insert(session_id, handle);
        
        // 启动 Actor
        tokio::spawn(actor.run());
    }
}
```

---

## 5. 前端扩展

### 5.1 事件监听

**`hooks/useTauriEvents.ts`**:
```typescript
import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useSessionStore } from '../stores/sessionStore';

export function useTauriEvents() {
  const { addSession, updateSession, removeSession, setFrame } = useSessionStore();
  
  useEffect(() => {
    const unlisteners: (() => void)[] = [];
    
    listen('session_created', (event) => {
      addSession(event.payload);
    }).then(u => unlisteners.push(u));
    
    listen('session_state_changed', (event) => {
      updateSession(event.payload.session_id, { state: event.payload.new_state });
    }).then(u => unlisteners.push(u));
    
    listen('screencast_frame', (event) => {
      setFrame(event.payload.session_id, event.payload.image_base64);
    }).then(u => unlisteners.push(u));
    
    listen('session_stopped', (event) => {
      removeSession(event.payload.session_id);
    }).then(u => unlisteners.push(u));
    
    return () => {
      unlisteners.forEach(u => u());
    };
  }, []);
}
```

### 5.2 画布组件

**`components/canvas/CanvasWindow.tsx`**:
```tsx
import { useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useSessionStore } from '../../stores/sessionStore';

interface Props {
  sessionId: string;
}

export default function CanvasWindow({ sessionId }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const frame = useSessionStore(s => s.frames[sessionId]);
  
  // 绘制帧
  useEffect(() => {
    if (!frame || !canvasRef.current) return;
    
    const ctx = canvasRef.current.getContext('2d');
    const img = new Image();
    img.onload = () => {
      ctx?.drawImage(img, 0, 0);
    };
    img.src = `data:image/jpeg;base64,${frame}`;
  }, [frame]);
  
  // 点击处理
  const handleClick = async (e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    
    await invoke('click_session', { sessionId, x, y });
  };
  
  return (
    <canvas
      ref={canvasRef}
      width={1080}
      height={720}
      onClick={handleClick}
      className="border cursor-crosshair"
    />
  );
}
```

---

## 6. 验收检查

### 功能验收

- [ ] 选择账户后点击 Run 可启动会话
- [ ] 浏览器自动导航到游戏页面
- [ ] 自动登录成功 (Cookie 或 用户名密码)
- [ ] 画布显示游戏画面
- [ ] 点击画布可触发游戏内点击
- [ ] 拖拽操作正常
- [ ] 可停止会话
- [ ] 资源正确释放 (无僵尸进程)

### 稳定性

- [ ] 多会话并发正常
- [ ] 会话异常退出不影响其他会话
- [ ] 浏览器崩溃可恢复

---

## 7. 后续准备

Phase 2 完成后，为 Phase 3 准备：
- Scene 值对象定义
- Script 值对象定义
- ScriptRunner 结构 (空实现)

