# Phase 3: 脚本执行 ✅

## 目标

实现场景匹配与脚本执行引擎，支持自动化操作游戏。

## 完成标准

- [x] 场景定义与加载
- [x] 颜色点匹配算法
- [x] 脚本引擎核心逻辑
- [x] 循环与条件控制
- [x] OCR 资源耗尽检测
- [x] 脚本控制 UI

---

## 1. Domain 层扩展

### 1.1 Scene 值对象

**`domain/model/scene.rs`**:
```rust
use serde::{Deserialize, Serialize};
use image::DynamicImage;

/// 场景定义 - 通过颜色点匹配识别游戏画面
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub points: Vec<ColorPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPoint {
    pub x: i32,
    pub y: i32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "default_tolerance")]
    pub tolerance: u8,
}

fn default_tolerance() -> u8 { 10 }

impl Scene {
    /// 检查图像是否匹配该场景
    pub fn matches(&self, image: &DynamicImage) -> bool {
        let rgb = image.to_rgb8();
        
        self.points.iter().all(|point| {
            if point.x < 0 || point.y < 0 {
                return false;
            }
            
            let x = point.x as u32;
            let y = point.y as u32;
            
            if x >= rgb.width() || y >= rgb.height() {
                return false;
            }
            
            let pixel = rgb.get_pixel(x, y);
            let dr = (pixel[0] as i32 - point.r as i32).abs();
            let dg = (pixel[1] as i32 - point.g as i32).abs();
            let db = (pixel[2] as i32 - point.b as i32).abs();
            
            let tolerance = point.tolerance as i32;
            dr <= tolerance && dg <= tolerance && db <= tolerance
        })
    }
}
```

**场景 YAML 示例** (`resources/scenes/login_success.yaml`):
```yaml
name: login_success
points:
  - x: 540
    y: 360
    r: 255
    g: 200
    b: 100
    tolerance: 15
  - x: 100
    y: 50
    r: 50
    g: 50
    b: 150
```

### 1.2 Script 值对象

**`domain/model/script.rs`**:
```rust
use serde::{Deserialize, Serialize};

/// 脚本定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<Step>,
}

/// 脚本步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub name: Option<String>,
    pub scene: String,           // 场景名称
    pub actions: Vec<Action>,    // 匹配后执行的动作
    #[serde(default)]
    pub loop_config: Option<LoopConfig>,
    #[serde(default)]
    pub ocr_check: Option<OcrCheck>,
}

/// 动作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Click { x: i32, y: i32 },
    Drag { from_x: i32, from_y: i32, to_x: i32, to_y: i32 },
    Wait { ms: u64 },
    RandomClick { x_min: i32, x_max: i32, y_min: i32, y_max: i32 },
}

/// 循环配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopConfig {
    pub max_iterations: Option<u32>,
    pub until_scene: Option<String>,  // 匹配到该场景时退出循环
    pub timeout_ms: Option<u64>,
}

/// OCR 检查 (可选功能)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrCheck {
    pub region: OcrRegion,
    pub pattern: String,          // 正则表达式
    pub exit_on_match: bool,      // 匹配时退出脚本
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
```

**脚本 YAML 示例** (`resources/scripts/daily_quest.yaml`):
```yaml
name: daily_quest
description: 自动完成日常任务

steps:
  - name: 打开任务面板
    scene: main_screen
    actions:
      - type: click
        x: 100
        y: 500
      - type: wait
        ms: 500

  - name: 领取奖励
    scene: quest_panel
    actions:
      - type: click
        x: 800
        y: 300
    loop_config:
      max_iterations: 10
      until_scene: quest_empty
      
  - name: 关闭面板
    scene: quest_panel
    actions:
      - type: click
        x: 950
        y: 50
```

---

## 2. Infrastructure 层

### 2.1 资源加载器

**`infrastructure/config/resources.rs`**:
```rust
use std::fs;
use std::path::Path;
use crate::domain::model::scene::Scene;
use crate::domain::model::script::Script;

/// 加载所有场景定义
pub fn load_scenes() -> anyhow::Result<Vec<Scene>> {
    let scenes_dir = Path::new("resources/scenes");
    let mut scenes = Vec::new();
    
    if scenes_dir.exists() {
        for entry in fs::read_dir(scenes_dir)? {
            let path = entry?.path();
            if path.extension().map_or(false, |e| e == "yaml" || e == "yml") {
                let content = fs::read_to_string(&path)?;
                let scene: Scene = serde_yaml::from_str(&content)?;
                scenes.push(scene);
            }
        }
    }
    
    Ok(scenes)
}

/// 加载所有脚本定义
pub fn load_scripts() -> anyhow::Result<Vec<Script>> {
    let scripts_dir = Path::new("resources/scripts");
    let mut scripts = Vec::new();
    
    if scripts_dir.exists() {
        for entry in fs::read_dir(scripts_dir)? {
            let path = entry?.path();
            if path.extension().map_or(false, |e| e == "yaml" || e == "yml") {
                let content = fs::read_to_string(&path)?;
                let script: Script = serde_yaml::from_str(&content)?;
                scripts.push(script);
            }
        }
    }
    
    Ok(scripts)
}

/// 通过名称查找场景
pub fn find_scene(scenes: &[Scene], name: &str) -> Option<&Scene> {
    scenes.iter().find(|s| s.name == name)
}
```

### 2.2 OCR 客户端 (可选)

**`infrastructure/ocr/client.rs`**:
```rust
use image::DynamicImage;

pub struct OcrClient {
    api_url: String,
}

impl OcrClient {
    pub fn new(api_url: String) -> Self {
        Self { api_url }
    }
    
    /// 识别图像区域中的文字
    pub async fn recognize(&self, image: &DynamicImage, region: &OcrRegion) -> anyhow::Result<String> {
        // 裁剪区域
        let cropped = image.crop_imm(
            region.x as u32,
            region.y as u32,
            region.width as u32,
            region.height as u32,
        );
        
        // 编码为 base64
        let mut buf = Vec::new();
        cropped.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)?;
        let base64 = base64::encode(&buf);
        
        // 调用 OCR API
        let client = reqwest::Client::new();
        let response = client
            .post(&self.api_url)
            .json(&serde_json::json!({ "image": base64 }))
            .send()
            .await?
            .json::<OcrResponse>()
            .await?;
        
        Ok(response.text)
    }
}

#[derive(Deserialize)]
struct OcrResponse {
    text: String,
}
```

---

## 3. Application 层

### 3.1 ScriptRunner

**`application/service/script_service.rs`**:
```rust
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;
use rand::Rng;

use crate::domain::model::scene::Scene;
use crate::domain::model::script::{Script, Step, Action, LoopConfig};
use crate::infrastructure::browser::BrowserDriver;
use crate::infrastructure::config::resources;

pub enum ScriptCommand {
    Stop,
}

pub struct ScriptRunner {
    script: Script,
    scenes: Vec<Scene>,
    browser: Arc<dyn BrowserDriver>,
    cmd_rx: mpsc::Receiver<ScriptCommand>,
    running: bool,
}

impl ScriptRunner {
    pub fn new(
        script: Script,
        scenes: Vec<Scene>,
        browser: Arc<dyn BrowserDriver>,
        cmd_rx: mpsc::Receiver<ScriptCommand>,
    ) -> Self {
        Self {
            script,
            scenes,
            browser,
            cmd_rx,
            running: true,
        }
    }
    
    pub async fn run(&mut self) {
        tracing::info!(script = %self.script.name, "Script started");
        
        while self.running {
            // 检查停止命令
            if let Ok(ScriptCommand::Stop) = self.cmd_rx.try_recv() {
                break;
            }
            
            // 截取屏幕
            let image = match self.browser.capture_screen().await {
                Ok(img) => img,
                Err(e) => {
                    tracing::error!("Failed to capture screen: {}", e);
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };
            
            // 遍历步骤，查找匹配场景
            let mut matched = false;
            for step in &self.script.steps {
                if let Some(scene) = resources::find_scene(&self.scenes, &step.scene) {
                    if scene.matches(&image) {
                        matched = true;
                        tracing::debug!(step = ?step.name, scene = %step.scene, "Scene matched");
                        
                        // 执行动作
                        self.execute_step(step).await;
                        break;
                    }
                }
            }
            
            if !matched {
                tracing::trace!("No scene matched, waiting...");
            }
            
            // 等待间隔
            sleep(Duration::from_millis(500)).await;
        }
        
        tracing::info!(script = %self.script.name, "Script stopped");
    }
    
    async fn execute_step(&mut self, step: &Step) {
        // 处理循环
        if let Some(loop_config) = &step.loop_config {
            self.execute_loop(step, loop_config).await;
            return;
        }
        
        // 执行动作列表
        for action in &step.actions {
            self.execute_action(action).await;
        }
    }
    
    async fn execute_loop(&mut self, step: &Step, config: &LoopConfig) {
        let start = Instant::now();
        let mut iterations = 0u32;
        
        loop {
            // 检查最大迭代次数
            if let Some(max) = config.max_iterations {
                if iterations >= max {
                    tracing::debug!("Loop max iterations reached");
                    break;
                }
            }
            
            // 检查超时
            if let Some(timeout) = config.timeout_ms {
                if start.elapsed() > Duration::from_millis(timeout) {
                    tracing::debug!("Loop timeout");
                    break;
                }
            }
            
            // 检查退出场景
            if let Some(until_scene) = &config.until_scene {
                let image = self.browser.capture_screen().await.ok();
                if let Some(img) = image {
                    if let Some(scene) = resources::find_scene(&self.scenes, until_scene) {
                        if scene.matches(&img) {
                            tracing::debug!(scene = %until_scene, "Until scene matched, exiting loop");
                            break;
                        }
                    }
                }
            }
            
            // 执行动作
            for action in &step.actions {
                self.execute_action(action).await;
            }
            
            iterations += 1;
            sleep(Duration::from_millis(300)).await;
        }
    }
    
    async fn execute_action(&self, action: &Action) {
        match action {
            Action::Click { x, y } => {
                let _ = self.browser.click(*x as f64, *y as f64).await;
            }
            Action::Drag { from_x, from_y, to_x, to_y } => {
                let _ = self.browser.drag(
                    (*from_x as f64, *from_y as f64),
                    (*to_x as f64, *to_y as f64),
                ).await;
            }
            Action::Wait { ms } => {
                sleep(Duration::from_millis(*ms)).await;
            }
            Action::RandomClick { x_min, x_max, y_min, y_max } => {
                let mut rng = rand::thread_rng();
                let x = rng.gen_range(*x_min..=*x_max) as f64;
                let y = rng.gen_range(*y_min..=*y_max) as f64;
                let _ = self.browser.click(x, y).await;
            }
        }
    }
}
```

### 3.2 Session 集成

扩展 `SessionActor` 以支持脚本：

```rust
// application/service/session_service.rs 扩展

pub enum SessionCommand {
    // ... 现有命令
    StartScript { script_name: String },
    StopScript,
}

impl SessionActor {
    async fn handle_command(&mut self, cmd: SessionCommand) -> bool {
        match cmd {
            // ... 现有处理
            SessionCommand::StartScript { script_name } => {
                self.start_script(&script_name).await;
            }
            SessionCommand::StopScript => {
                self.stop_script().await;
            }
        }
        true
    }
    
    async fn start_script(&mut self, script_name: &str) {
        if self.state != SessionState::Ready {
            return;
        }
        
        // 加载脚本
        let scripts = resources::load_scripts().unwrap_or_default();
        let script = scripts.into_iter().find(|s| s.name == script_name);
        
        let Some(script) = script else {
            tracing::error!(script = %script_name, "Script not found");
            return;
        };
        
        // 加载场景
        let scenes = resources::load_scenes().unwrap_or_default();
        
        // 创建 ScriptRunner
        let (cmd_tx, cmd_rx) = mpsc::channel(8);
        let runner = ScriptRunner::new(
            script,
            scenes,
            self.browser.clone(),
            cmd_rx,
        );
        
        self.script_cmd_tx = Some(cmd_tx);
        self.transition_to(SessionState::ScriptRunning).await;
        
        // 启动脚本
        tokio::spawn(async move {
            runner.run().await;
        });
    }
    
    async fn stop_script(&mut self) {
        if let Some(tx) = self.script_cmd_tx.take() {
            let _ = tx.send(ScriptCommand::Stop).await;
        }
        self.transition_to(SessionState::Ready).await;
    }
}
```

---

## 4. Adapter 层

### 4.1 Tauri Commands

**`adapter/tauri/commands.rs`** 扩展:
```rust
#[tauri::command]
pub async fn get_scripts(state: State<'_, AppState>) -> Result<Vec<ScriptInfo>, String> {
    let scripts = resources::load_scripts().map_err(|e| e.to_string())?;
    Ok(scripts.into_iter().map(|s| ScriptInfo {
        name: s.name,
        description: s.description,
    }).collect())
}

#[tauri::command]
pub async fn start_script(
    session_id: String,
    script_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.coordinator
        .dispatch(CoordinatorCommand::StartScript { session_id, script_name })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_script(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.coordinator
        .dispatch(CoordinatorCommand::StopScript { session_id })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_all_scripts(
    session_scripts: HashMap<String, String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.coordinator
        .dispatch(CoordinatorCommand::StartAllScripts { session_scripts })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_all_scripts(state: State<'_, AppState>) -> Result<(), String> {
    state.coordinator
        .dispatch(CoordinatorCommand::StopAllScripts)
        .await
        .map_err(|e| e.to_string())
}
```

---

## 5. 前端扩展

### 5.1 脚本控制 UI

**`components/session/ScriptControls.tsx`**:
```tsx
import { useState, useEffect } from 'react';
import { Play, Square, RefreshCw, PlayCircle } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface Props {
  sessionId: string;
  sessionState: string;
}

export default function ScriptControls({ sessionId, sessionState }: Props) {
  const [scripts, setScripts] = useState<ScriptInfo[]>([]);
  const [selectedScript, setSelectedScript] = useState('');
  
  useEffect(() => {
    invoke<ScriptInfo[]>('get_scripts').then(setScripts);
  }, []);
  
  const isRunning = sessionState === 'ScriptRunning';
  const canStart = sessionState === 'Ready' && selectedScript;
  
  const handleStart = () => invoke('start_script', { sessionId, scriptName: selectedScript });
  const handleStop = () => invoke('stop_script', { sessionId });
  const handleSync = () => invoke('sync_script_selection', { scriptName: selectedScript });
  const handleRunAll = () => invoke('start_all_scripts', { sessionScripts });
  
  return (
    <div className="flex items-center gap-2">
      <select
        value={selectedScript}
        onChange={(e) => setSelectedScript(e.target.value)}
        className="border rounded px-3 py-2"
        disabled={isRunning}
      >
        <option value="">Select Script</option>
        {scripts.map(s => (
          <option key={s.name} value={s.name}>{s.name}</option>
        ))}
      </select>
      
      {isRunning ? (
        <button onClick={handleStop} className="p-2 bg-red-500 text-white rounded">
          <Square className="w-4 h-4" />
        </button>
      ) : (
        <button
          onClick={handleStart}
          disabled={!canStart}
          className="p-2 bg-green-500 text-white rounded disabled:opacity-50"
        >
          <Play className="w-4 h-4" />
        </button>
      )}
      
      <button onClick={handleSync} className="p-2 border rounded">
        <RefreshCw className="w-4 h-4" />
      </button>
      
      <button onClick={handleRunAll} className="p-2 border rounded">
        <PlayCircle className="w-4 h-4" />
      </button>
    </div>
  );
}
```

---

## 6. 验收检查

### 功能验收

- [ ] 场景 YAML 正确加载
- [ ] 脚本 YAML 正确加载
- [ ] 场景匹配算法正确
- [ ] 脚本步骤按顺序执行
- [ ] 循环逻辑正常
- [ ] 可停止脚本
- [ ] 多会话脚本独立运行

### 稳定性

- [ ] 脚本异常不崩溃
- [ ] 无场景匹配时正常等待
- [ ] 停止脚本时资源释放

---

## 7. 后续准备

Phase 3 完成后，为 Phase 4 准备：
- KeyboardListener trait 定义
- GestureRecognizer 结构
- InputEventProcessor 结构

