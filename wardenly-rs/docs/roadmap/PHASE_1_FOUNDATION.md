# Phase 1: 核心框架

## 目标

建立 Tauri + React 项目骨架，实现账户与分组的 CRUD 功能，打通前后端数据流。

## 完成标准

- [x] 设计文档完成
- [x] Tauri v2 项目可运行
- [x] 主窗口 UI 渲染正常
- [x] 账户/分组增删改查功能完整
- [x] SQLite 持久化正常工作
- [x] 配置系统可加载 YAML

---

## 1. 项目初始化

### 1.1 创建 Tauri 项目

使用 yarn 管理前端依赖：

```bash
yarn create tauri-app wardenly-rs --template react-ts --manager yarn
cd wardenly-rs
yarn install
```

### 1.2 安装前端依赖

```bash
yarn add tailwindcss @tailwindcss/postcss postcss autoprefixer lucide-react zustand
```

### 1.3 Rust 依赖 (Cargo.toml)

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
tokio = { version = "1", features = ["full"] }
rusqlite = { version = "0.32", features = ["bundled"] }
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
│       ├── account_service.rs
│       └── group_service.rs
├── infrastructure/
│   ├── mod.rs
│   ├── persistence/
│   │   ├── mod.rs
│   │   └── sqlite/
│   │       ├── mod.rs
│   │       ├── account_repo.rs
│   │       └── group_repo.rs
│   ├── config/
│   │   ├── mod.rs
│   │   ├── loader.rs
│   │   ├── paths.rs
│   │   └── app_config.rs
│   └── logging/
│       └── mod.rs
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
}
```

**`domain/repository.rs`** (同步接口):
```rust
use crate::domain::model::{Account, Group};
use crate::domain::error::DomainError;

pub type Result<T> = std::result::Result<T, DomainError>;

pub trait AccountRepository: Send + Sync {
    fn find_by_id(&self, id: &str) -> Result<Option<Account>>;
    fn find_all(&self) -> Result<Vec<Account>>;
    fn save(&self, account: &Account) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
}

pub trait GroupRepository: Send + Sync {
    fn find_by_id(&self, id: &str) -> Result<Option<Group>>;
    fn find_all(&self) -> Result<Vec<Group>>;
    fn save(&self, group: &Group) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
}
```

### 2.3 Infrastructure 层

**`infrastructure/persistence/sqlite/mod.rs`** (使用 rusqlite):
```rust
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub type DbConnection = Arc<Mutex<Connection>>;

pub fn init_database() -> anyhow::Result<DbConnection> {
    let db_path = config::app().storage.sqlite.effective_path();
    
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    let conn = Connection::open(&db_path)?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            role_name TEXT NOT NULL,
            user_name TEXT NOT NULL,
            password TEXT NOT NULL,
            server_id INTEGER NOT NULL,
            ranking INTEGER DEFAULT 0,
            cookies TEXT
        )",
        [],
    )?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS groups (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            account_ids TEXT NOT NULL,
            ranking INTEGER DEFAULT 0
        )",
        [],
    )?;
    
    Ok(Arc::new(Mutex::new(conn)))
}
```

### 2.4 Adapter 层

**`adapter/tauri/commands.rs`** (同步命令):
```rust
use tauri::State;
use crate::adapter::tauri::state::AppState;
use crate::domain::model::{Account, Group};

#[tauri::command]
pub fn get_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    state.account_service
        .get_all()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_account(
    state: State<'_, AppState>,
    request: CreateAccountRequest,
) -> Result<Account, String> {
    let account = Account::new(
        request.role_name,
        request.user_name,
        request.password,
        request.server_id,
    );
    state.account_service
        .create(account)
        .map_err(|e| e.to_string())
}

// ... 其他 CRUD 命令
```

**`lib.rs`**:
```rust
pub fn run() {
    logging::setup(false);
    config::init();
    
    let db = persistence::sqlite::init_database()
        .expect("Failed to initialize database");
    
    let state = AppState::new(db);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
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
│   ├── dialogs/
│   │   └── ManagementDialog.tsx
│   └── forms/
│       ├── AccountForm.tsx
│       └── GroupForm.tsx
├── stores/
│   └── accountStore.ts
└── styles/
    └── globals.css
```

### 3.2 状态管理

**`stores/accountStore.ts`**:
```typescript
import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

interface AccountStore {
  accounts: Account[];
  groups: Group[];
  loading: boolean;
  error: string | null;
  
  fetchAccounts: () => Promise<void>;
  createAccount: (data: CreateAccountData) => Promise<void>;
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

### 3.3 样式配置

**`postcss.config.js`**:
```javascript
export default {
  plugins: {
    "@tailwindcss/postcss": {},
  },
};
```

**`src/styles/globals.css`**:
```css
@import "tailwindcss";

:root {
  --color-bg-primary: #0f172a;
  --color-bg-secondary: #1e293b;
  --color-accent: #3b82f6;
  /* ... */
}
```

---

## 4. 资源文件

**`src-tauri/resources/configs/app.yaml`**:
```yaml
storage:
  sqlite:
    # Leave empty for platform default path
    path: ""
```

---

## 5. 验收检查

### 功能验收

- [x] 应用启动无错误
- [x] 可添加新账户
- [x] 可编辑账户信息
- [x] 可删除账户
- [x] 可创建分组
- [x] 可将账户添加到分组
- [x] 关闭重启后数据持久化

### 代码质量

- [x] `cargo check` 无警告
- [x] 错误处理完善

---

## 6. 后续准备

Phase 1 完成后，为 Phase 2 准备：
- EventBus 结构 (可先空实现)
- Coordinator 结构 (可先空实现)
- Session 状态机定义 (仅类型，无逻辑)
