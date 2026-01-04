# RFC: Browser Profile Persistence

## 1. 背景与现状

### 现状
目前，Wardenly 的浏览器会话采用 **"Ephemeral Sandbox" (临时沙盒)** 模式：
1.  **启动时**：生成一个随机的 Session ID，在 `Temp` 目录下创建一个全新的、空的 User Data Directory。
2.  **运行时**：下载所有图片、JS、CSS 资源。
3.  **停止时**：强制删除该 User Data Directory。

### 问题
1.  **带宽浪费**：每次启动游戏都需要重新下载所有静态资源（图片、背景、音频），特别是对于重资源的页游，这消耗大量带宽。
2.  **启动缓慢**：无缓存导致每次加载游戏都需要几十秒甚至更久。
3.  **体验割裂**：用户无法保存一些非 Cookie 的本地设置（如游戏的音量设置、界面布局等，通常存在 LocalStorage 或 IndexedDB 中）。

## 2. 方案目标

实现 **"Persistent Profile" (持久化配置)** 模式，使得：
1.  **缓存复用**：再次启动同一账户时，复用之前的 Cache，秒开游戏。
2.  **资源隔离**：不同账户之间的数据（Cookies, Storage）依然严格隔离，互不干扰。
3.  **磁盘管理**：避免磁盘空间无限膨胀。

## 3. 技术方案

### 3.1 目录结构变更

**当前路径**：
`{TEMP_DIR}/wardenly-browsers/{RANDOM_SESSION_ID}`

**目标路径**：
Linux: `~/.local/share/wardenly/profiles/{ACCOUNT_ID}`
Windows: `%APPDATA%\wardenly\profiles\{ACCOUNT_ID}`
macOS: `~/Library/Application Support/wardenly/profiles/{ACCOUNT_ID}`

使用 `dirs::data_dir()` 获取标准的用户数据目录，而不是临时目录。

### 3.2 生命周期变更

*   **启动 (Start)**：
    *   检查 `{DATA_DIR}/wardenly/profiles/{ACCOUNT_ID}` 是否存在。
    *   如果存在，检查是否有 `Lockfile`（意外崩溃导致）。如果有，尝试删除 Lockfile (需谨慎) 或提示用户。
    *   使用该目录启动 Chromium。
*   **停止 (Stop)**：
    *   发送关闭信号给 Chromium。
    *   等待进程退出。
    *   **不再删除目录**。
*   **账户删除 (Delete Account)**：
    *   当用户在 UI 中删除账户时，同步删除对应的 Profile 目录。

### 3.3 并发控制 (Locking)

Chromium 严禁多个进程同时使用同一个 Profile。
*   **单开限制**：需要确保同一个 Account 在同一时间只能被启动一次。目前的 UI 和后端逻辑似乎没有显式限制，需要增加 **Session Manager** 逻辑：如果该 Account 已有运行中的 Session，禁止再次启动，或自动聚焦到已有 Session。
*   **多开支持 (未来)**：如果未来支持同一个号多开（虽然少见），可能需要 `profiles/{ACCOUNT_ID}_{SLOT_ID}`。

## 4. 产品交互与逻辑变更

### 4.1 风险点
缓存可能导致页面更新不及时（虽然 HTTP 304 机制通常有效，但偶尔会失效）。

### 4.2 UI 变更建议

1.  **"清除缓存" (Clear Cache)** 功能：
    *   在账户右键菜单或设置页中，增加 "清除缓存" 按钮。
    *   功能：手动删除对应的 Profile 目录。
    *   场景：游戏更新后卡加载、页面显示异常。

2.  **"强制清除启动" (Clean Start)** (可选)：
    *   按住 `Shift` 点击启动，或者右键菜单 "无缓存启动"。
    *   功能：删除旧目录 -> 启动 -> 退出后删除（一次性模式）。

3.  **磁盘管理**：
    *   在 "设置 -> 存储" 中显示占用空间大小。
    *   提供 "一键清理所有缓存" 按钮。

## 5. 实施步骤

1.  **Infrastructure 层**：
    *   修改 `ChromiumDriver` 构造函数，接受 `account_id`。
    *   修改 User Data Dir 路径生成逻辑，指向 `data_dir`。
    *   移除 `stop()` 中的 `cleanup_user_data_dir()` 调用。
    *   在 `start()` 中增加 Lockfile 检测与清理逻辑 (防御性编程)。

2.  **Application 层**：
    *   `SessionActor` 传递 `account_id` 给 Driver。
    *   实现 "Delete Account" 时清理 Profile 目录的逻辑。

3.  **Interface 层 (UI)**：
    *   (按需) 添加清除缓存入口。
