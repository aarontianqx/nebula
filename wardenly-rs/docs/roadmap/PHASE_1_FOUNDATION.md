# Phase 1: 核心框架

## 目标

建立 Tauri + React 项目骨架，实现账户与分组的 CRUD 功能，打通前后端数据流。

## 完成标准

- [x] 设计文档完成
- [ ] Tauri v2 项目可运行
- [ ] 主窗口 UI 渲染正常
- [ ] 账户/分组增删改查功能完整
- [ ] SQLite 持久化正常工作
- [ ] 配置系统可加载 YAML

---

## 1. 项目初始化

### 1.1 创建 Tauri 项目

```bash
npm create tauri-app@latest wardenly-rs -- --template react-ts
cd wardenly-rs
npm install
```

### 1.2 安装前端依赖

```bash
npm install tailwindcss postcss autoprefixer
npm install lucide-react zustand
npx tailwindcss init -p
```

### 1.3 Rust 依赖 (Cargo.toml)

```toml
[dependencies]
tauri = { version = "2", features = [] }
tokio = { version = "1", features = ["full"] }
sea-orm = { version = "1.0", features = ["sqlx-sqlite", "runtime-tokio-rustls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "2"
anyhow = "1"
async-trait = "0.1"
dirs = "5"
uuid = { version = "1", features = ["v4", "serde"] }
```

---

## 2. 后端实现

### 2.1 目录结构

```
src-tauri/src/
├── main.rs
├── lib.rs
├── domain/
│   ├── mod.rs
│   ├── model/
│   │   ├── mod.rs
│   │   ├── account.rs
│   │   └── group.rs
│   ├── repository.rs
│   └── error.rs
├── application/
│   ├── mod.rs
│   └── service/
│       ├── mod.rs
│       └── account_service.rs
├── infrastructure/
│   ├── mod.rs
│   ├── persistence/
│   │   ├── mod.rs
│   │   └── sqlite/
│   │       ├── mod.rs
│   │       ├── connection.rs
│   │       ├── account_repo.rs
│   │       └── group_repo.rs
│   ├── config/
│   │   ├── mod.rs
│   │   ├── loader.rs
│   │   ├── paths.rs
│   │   └── app_config.rs
│   └── logging/
│       └── setup.rs
└── adapter/
    ├── mod.rs
    └── tauri/
        ├── mod.rs
        ├── commands.rs
        ├── state.rs
        └── error.rs
```

### 2.2 Domain 层

**`domain/model/account.rs`**:
```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub role_name: String,
    pub user_name: String,
    pub password: String,
    pub server_id: i32,
    pub ranking: i32,
    pub cookies: Option<String>,
}

impl Account {
    pub fn new(role_name: String, user_name: String, password: String, server_id: i32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role_name,
            user_name,
            password,
            server_id,
            ranking: 0,
            cookies: None,
        }
    }
    
    pub fn display_name(&self) -> String {
        format!("{} - {}", self.server_id, self.role_name)
    }
}
```

**`domain/model/group.rs`**:
```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub account_ids: Vec<String>,
    pub ranking: i32,
}

impl Group {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            description: None,
            account_ids: Vec::new(),
            ranking: 0,
        }
    }
}
```

**`domain/repository.rs`**:
```rust
use async_trait::async_trait;
use crate::domain::model::{Account, Group};
use crate::domain::error::DomainError;

pub type Result<T> = std::result::Result<T, DomainError>;

#[async_trait]
pub trait AccountRepository: Send + Sync {
    async fn find_by_id(&self, id: &str) -> Result<Option<Account>>;
    async fn find_all(&self) -> Result<Vec<Account>>;
    async fn save(&self, account: &Account) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}

#[async_trait]
pub trait GroupRepository: Send + Sync {
    async fn find_by_id(&self, id: &str) -> Result<Option<Group>>;
    async fn find_all(&self) -> Result<Vec<Group>>;
    async fn save(&self, group: &Group) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}
```

### 2.3 Infrastructure 层

**`infrastructure/config/paths.rs`**:
```rust
use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    { dirs::config_dir().unwrap_or_default().join("wardenly") }
    
    #[cfg(target_os = "macos")]
    { dirs::home_dir().unwrap_or_default().join("Library/Application Support/wardenly") }
    
    #[cfg(target_os = "linux")]
    { dirs::config_dir().unwrap_or_default().join("wardenly") }
}

pub fn default_sqlite_path() -> PathBuf {
    config_dir().join("data.db")
}

pub fn log_dir() -> PathBuf {
    config_dir().join("logs")
}
```

**`infrastructure/persistence/sqlite/connection.rs`**:
```rust
use sea_orm::{Database, DatabaseConnection};
use std::path::Path;

pub async fn create_connection(db_path: &Path) -> anyhow::Result<DatabaseConnection> {
    // 确保目录存在
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let db = Database::connect(&url).await?;
    
    // 执行迁移 (使用 sea-orm-migration 或手动 SQL)
    // ...
    
    Ok(db)
}
```

### 2.4 Adapter 层

**`adapter/tauri/commands.rs`**:
```rust
use tauri::State;
use crate::adapter::tauri::state::AppState;
use crate::domain::model::{Account, Group};

#[tauri::command]
pub async fn get_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    state.account_service
        .list_all()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_account(
    role_name: String,
    user_name: String,
    password: String,
    server_id: i32,
    state: State<'_, AppState>,
) -> Result<Account, String> {
    state.account_service
        .create(role_name, user_name, password, server_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_account(account: Account, state: State<'_, AppState>) -> Result<(), String> {
    state.account_service
        .update(&account)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_account(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.account_service
        .delete(&id)
        .await
        .map_err(|e| e.to_string())
}

// 类似的 Group CRUD 命令...
```

**`main.rs`**:
```rust
use tauri::Manager;

mod domain;
mod application;
mod infrastructure;
mod adapter;

use infrastructure::config;
use infrastructure::persistence::sqlite;
use infrastructure::logging;
use adapter::tauri::{commands, state::AppState};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // 1. 初始化日志
            logging::setup(false);
            
            // 2. 加载配置
            config::init();
            
            // 3. 初始化数据库
            let db_path = config::app().storage.sqlite.effective_path();
            let db = tauri::async_runtime::block_on(sqlite::create_connection(&db_path))?;
            
            // 4. 创建应用状态
            let state = AppState::new(db);
            app.manage(state);
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_accounts,
            commands::create_account,
            commands::update_account,
            commands::delete_account,
            commands::get_groups,
            commands::create_group,
            commands::update_group,
            commands::delete_group,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

## 3. 前端实现

### 3.1 目录结构

```
src/
├── App.tsx
├── main.tsx
├── components/
│   ├── layout/
│   │   └── MainWindow.tsx
│   └── management/
│       ├── ManagementDialog.tsx
│       ├── AccountsPanel.tsx
│       ├── AccountForm.tsx
│       ├── GroupsPanel.tsx
│       └── GroupForm.tsx
├── hooks/
│   └── useAccounts.ts
├── stores/
│   └── accountStore.ts
├── types/
│   └── index.ts
└── styles/
    └── globals.css
```

### 3.2 类型定义

**`types/index.ts`**:
```typescript
export interface Account {
  id: string;
  role_name: string;
  user_name: string;
  password: string;
  server_id: number;
  ranking: number;
  cookies?: string;
}

export interface Group {
  id: string;
  name: string;
  description?: string;
  account_ids: string[];
  ranking: number;
}
```

### 3.3 状态管理

**`stores/accountStore.ts`**:
```typescript
import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Account, Group } from '../types';

interface AccountStore {
  accounts: Account[];
  groups: Group[];
  loading: boolean;
  error: string | null;
  
  fetchAccounts: () => Promise<void>;
  fetchGroups: () => Promise<void>;
  createAccount: (data: Omit<Account, 'id' | 'ranking' | 'cookies'>) => Promise<void>;
  updateAccount: (account: Account) => Promise<void>;
  deleteAccount: (id: string) => Promise<void>;
  // ... group 方法
}

export const useAccountStore = create<AccountStore>((set, get) => ({
  accounts: [],
  groups: [],
  loading: false,
  error: null,
  
  fetchAccounts: async () => {
    set({ loading: true, error: null });
    try {
      const accounts = await invoke<Account[]>('get_accounts');
      set({ accounts, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  
  // ... 其他方法实现
}));
```

### 3.4 主窗口

**`components/layout/MainWindow.tsx`**:
```tsx
import { useState, useEffect } from 'react';
import { Settings } from 'lucide-react';
import { useAccountStore } from '../../stores/accountStore';
import ManagementDialog from '../management/ManagementDialog';

export default function MainWindow() {
  const { accounts, groups, fetchAccounts, fetchGroups } = useAccountStore();
  const [manageOpen, setManageOpen] = useState(false);
  const [selectedAccount, setSelectedAccount] = useState<string>('');
  const [selectedGroup, setSelectedGroup] = useState<string>('');
  
  useEffect(() => {
    fetchAccounts();
    fetchGroups();
  }, []);
  
  return (
    <div className="flex h-screen bg-gray-50 dark:bg-gray-900">
      {/* 左侧边栏 - 会话列表 (Phase 2) */}
      <aside className="w-64 border-r border-gray-200 dark:border-gray-700 p-4">
        <p className="text-gray-500">Sessions (Phase 2)</p>
      </aside>
      
      {/* 右侧主区域 */}
      <main className="flex-1 flex flex-col">
        {/* 工具栏 */}
        <div className="border-b border-gray-200 dark:border-gray-700 p-4 space-y-3">
          <div className="flex items-center gap-4">
            {/* 账户选择 */}
            <select 
              value={selectedAccount}
              onChange={(e) => setSelectedAccount(e.target.value)}
              className="border rounded px-3 py-2"
            >
              <option value="">Select Account</option>
              {accounts.map(acc => (
                <option key={acc.id} value={acc.id}>
                  {acc.server_id} - {acc.role_name}
                </option>
              ))}
            </select>
            <button 
              disabled
              className="px-4 py-2 bg-blue-500 text-white rounded opacity-50"
            >
              Run (Phase 2)
            </button>
            
            <div className="flex-1" />
            
            {/* 管理按钮 */}
            <button
              onClick={() => setManageOpen(true)}
              className="px-4 py-2 border rounded flex items-center gap-2"
            >
              <Settings className="w-4 h-4" />
              Manage...
            </button>
          </div>
        </div>
        
        {/* 详情面板 (Phase 2) */}
        <div className="flex-1 p-4">
          <p className="text-gray-500">Session Details (Phase 2)</p>
        </div>
      </main>
      
      {/* 管理对话框 */}
      <ManagementDialog open={manageOpen} onClose={() => setManageOpen(false)} />
    </div>
  );
}
```

---

## 4. 验收检查

### 功能验收

- [ ] 应用启动无错误
- [ ] 可添加新账户
- [ ] 可编辑账户信息
- [ ] 可删除账户
- [ ] 可创建分组
- [ ] 可将账户添加到分组
- [ ] 关闭重启后数据持久化

### 代码质量

- [ ] `cargo clippy` 无警告
- [ ] `cargo fmt` 格式化
- [ ] 错误处理完善

### 文档

- [ ] README 更新
- [ ] 代码注释完整

---

## 5. 后续准备

Phase 1 完成后，为 Phase 2 准备：
- EventBus 结构 (可先空实现)
- Coordinator 结构 (可先空实现)
- Session 状态机定义 (仅类型，无逻辑)

