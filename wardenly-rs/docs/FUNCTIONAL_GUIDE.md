# Wardenly - 功能说明

## 概述

Wardenly 是一款用于 WLY 网页游戏自动化的桌面控制工具。它通过 headless 浏览器运行游戏，提供实时画面显示、手动操作和自动化脚本执行功能。

## 核心功能

### 1. 账户与分组管理

#### 账户存储
账户信息支持两种存储后端，通过 `resources/configs/app.yaml` 配置：

**SQLite（默认）**：
- 本地存储，无需外部服务
- 默认数据文件位于:
  - macOS: `~/Library/Application Support/wardenly/data.db`
  - Linux: `~/.config/wardenly/data.db`
  - Windows: `%APPDATA%\wardenly\data.db`
- 可通过配置自定义路径

**MongoDB（可选）**：
- 远程存储，支持多设备同步
- 需在配置文件中设置 `storage.type: mongodb` 并提供连接 URI

**配置示例**:
```yaml
# resources/configs/app.yaml
storage:
  type: sqlite  # 或 mongodb
  sqlite:
    path: ""    # 留空使用默认路径
  mongodb:
    uri: "mongodb://localhost:27017"
    database: "wardenly"
```

账户字段：
- **ID**: 唯一标识符
- **RoleName**: 游戏内角色名
- **UserName**: 登录用户名
- **Password**: 登录密码
- **ServerID**: 游戏服务器 ID
- **Cookies**: 保存的登录 Cookie（用于快速登录）

#### 分组存储
分组信息存储在同一存储后端：
- **ID**: 唯一标识符
- **Name**: 分组名称
- **AccountIDs**: 成员账户 ID 列表
- **Ranking**: 排序优先级

#### 账户显示
账户在 UI 中显示为 `ServerID - RoleName` 格式，例如 `126 - 追风`。

#### 管理操作
点击工具栏 **Manage...** 按钮打开管理对话框，可进行账户和分组的增删改查。

#### 分组运行
选择分组后点击 "Run Group" 会依次启动该分组内所有有效账户（无效账户自动跳过）。

### 2. 会话管理

#### 启动会话

**单账户启动**:
1. 从下拉框选择账户
2. 点击 "Run Account"
3. 系统创建 Session，启动浏览器，自动登录

**分组批量启动**:
1. 从下拉框选择分组
2. 点击 "Run Group"
3. 系统依次启动分组内所有账户（间隔 3 秒）
4. 已运行的账户会自动跳过

#### 会话列表
左侧边栏显示所有运行中的会话：
- 点击会话可切换当前查看/操作的会话
- 会话名称后的状态指示器：
  - 🔴 脚本运行中
  - (无) 待机状态

#### 会话生命周期

```
Idle ─► Starting ─► LoggingIn ─► Ready ◄─► ScriptRunning
                                   │
                                   ▼
                                Stopped
```

| 状态 | 说明 | 允许的操作 |
|------|------|-----------|
| Idle | 初始状态 | - |
| Starting | 浏览器正在启动 | - |
| LoggingIn | 正在登录游戏 | 可查看画面，可点击 |
| Ready | 登录成功，待机中 | 所有操作 |
| ScriptRunning | 脚本执行中 | Stop Script |
| Stopped | 会话已结束 | - |

### 3. 画布窗口 (Browser View)

独立的窗口显示当前选中会话的浏览器画面。

#### 显示模式

**手动刷新模式**（默认）:
- 点击画布时截取一帧并显示
- 适合低资源消耗场景

**自动刷新模式**（Auto Refresh）:
- 勾选 "Auto Refresh (1s)" 复选框启用
- 以 5 FPS 实时流式传输画面
- 适合需要实时观察的场景

#### 交互操作

**点击操作**:
- 在画布上点击会将点击事件发送到浏览器
- 坐标显示在控制面板的 X/Y 输入框中
- 如果勾选 "Spread to All"，点击会发送到所有活跃会话

**拖拽操作**:
- 在画布上拖拽会将拖拽事件发送到浏览器
- 支持模拟游戏内的滑动操作

**键盘透传 (Keyboard Passthrough)**:
- 勾选工具栏 "Keyboard Passthrough" 复选框启用
- 启用后监听系统键盘事件（需辅助功能权限）
- 按键事件转换为当前鼠标位置的画布点击
- 仅当鼠标在画布区域内时触发点击

| 操作 | 触发条件 | 效果 |
|------|----------|------|
| 单击 | 按键后快速释放（< 300ms） | 触发一次画布点击 |
| 长按连击 | 按住超过 300ms | 每 100ms 触发一次点击 |

> **权限说明**: macOS 用户首次启用时需授予"辅助功能"权限（系统设置 → 隐私与安全性 → 辅助功能）。

#### 画布状态管理

- 新会话创建后 0.5 秒内禁止截图（避免浏览器未完全启动时崩溃）
- 浏览器驱动启动后 1 秒开始帧同步
- 切换会话时自动切换画布关联的会话
- 关闭最后一个会话时画布窗口自动隐藏
- 重新打开会话时画布窗口自动显示

### 4. 浏览器控制

位于会话详情面板的第一行按钮组。

| 按钮 | 功能 | 说明 |
|------|------|------|
| Stop | 停止会话 | 关闭浏览器，结束会话 |
| Refresh Page | 刷新页面 | 重新加载当前页面 |
| Save Cookies | 保存 Cookie | 手动保存当前 Cookie |

> **注意**: 登录成功后会自动保存 Cookie，一般无需手动保存。

### 5. 脚本控制

#### 脚本选择
从下拉框选择要执行的脚本。可用脚本从 `resources/scripts/` 目录加载。

#### 脚本操作

| 按钮 | 功能 | 说明 |
|------|------|------|
| Start Script | 启动脚本 | 开始执行选中的脚本 |
| Stop Script | 停止脚本 | 中止正在执行的脚本 |
| Sync Script | 同步脚本 | 将当前脚本选择同步到所有会话 |
| Run All | 全部执行 | 启动所有会话的脚本 |
| Stop All | 全部停止 | 停止所有会话的脚本 |

#### 脚本执行逻辑

脚本由多个步骤组成，每个步骤包含：
- **场景匹配**: 通过颜色点检测当前画面是否匹配预期场景
- **动作执行**: 匹配成功后执行 click、wait、drag 等动作
- **循环控制**: 支持循环执行直到条件满足
- **OCR 检测**: 可选的 OCR 资源检测（如体力耗尽退出）

**执行流程**:
1. 截取当前画面
2. 遍历脚本步骤，尝试匹配场景
3. 找到匹配场景后执行该步骤的动作
4. 等待 500ms 后重复

**停止条件**:
- 用户手动停止
- 脚本中的 `quit` 动作触发
- OCR 检测到资源耗尽
- 浏览器关闭

### 6. 坐标与颜色查看

控制面板的 Inspector 区域用于坐标输入和颜色查看：

- **X / Y**: 坐标输入框，支持两种输入方式
  - **鼠标点击画布**: 自动填充坐标并更新颜色
  - **键盘输入**: 手动输入数字，按 Enter 或点击 Fetch 更新颜色
- **Fetch 按钮**: 获取指定坐标的颜色
- **色块**: 可视化显示颜色
- **Color 值**: 显示 RGBA 颜色值

这些信息用于调试场景配置和脚本开发。

### 7. 多会话选项

工具栏的复选框控制多会话行为：

| 选项 | 功能 |
|------|------|
| Spread to All | 启用后，画布上的点击/拖拽会发送到所有活跃会话 |
| Auto Refresh | 启用实时画面流式传输 |

### 8. 登录机制

#### Cookie 登录（优先）
如果账户存储了有效 Cookie：
1. 设置 Cookie 到浏览器
2. 访问游戏 URL
3. 等待游戏加载

#### 用户名密码登录
如果没有 Cookie 或 Cookie 失效：
1. 访问游戏登录页
2. 输入用户名和密码
3. 点击登录按钮
4. 等待游戏加载
5. 保存新的 Cookie

#### 登录等待
登录后等待游戏加载完成：
- 最多等待 20 秒（10 次尝试，每次 2 秒）
- 通过场景识别检测 `user_agreement` 或 `main_city` 场景
- 如果检测到用户协议，自动点击同意

## 场景识别系统

### 场景定义
场景在 `resources/scenes/*.yaml` 中定义：

```yaml
name: main_city
category: city
points:
  - {x: 100, y: 200, color: {r: 255, g: 128, b: 64, a: 255}}
  - {x: 300, y: 400, color: {r: 32, g: 64, b: 128, a: 255}}
actions:
  SomeButton:
    type: click
    point: {x: 500, y: 600}
```

### 匹配算法
- 检查所有定义的颜色点
- 计算实际颜色与预期颜色的差异
- 平均差异 ≤ 5.0 视为匹配成功

### 场景分类
| 分类 | 说明 |
|------|------|
| city | 主城界面 |
| battle | 战斗界面 |
| building | 建筑界面 |
| loading | 加载/登录界面 |
| tower | 塔防界面 |
| world | 世界地图 |
| tasks | 任务界面 |

## 自动化脚本系统

### 脚本定义
脚本在 `resources/scripts/*.yaml` 中定义：

```yaml
name: Join Battle
description: Automatically join group battles
version: "1.0"
author: System
steps:
  - scene: battle_group_entrance
    timeout: 5s
    actions:
      - type: click
        points: [{x: 538, y: 544}]
      - type: wait
        duration: 1s
```

### 支持的动作类型

| 类型 | 说明 | 参数 |
|------|------|------|
| click | 点击指定坐标 | points: [{x, y}] |
| wait | 等待指定时间 | duration: 1s |
| drag | 拖拽操作 | points: [{x1, y1}, {x2, y2}] |
| incr | 计数器加 1 | key: counter_name |
| decr | 计数器减 1 | key: counter_name |
| quit | 退出脚本 | condition: {op, key, value} |
| check_scene | 检查场景并执行 OCR | (与 ocr_rule 配合) |

### 循环控制

```yaml
actions:
  - type: click
    points: [{x: 100, y: 200}]
  - type: wait
    duration: 1s
loop:
  startIndex: 0
  endIndex: 1
  count: -1        # -1 表示无限循环
  until: target_scene  # 匹配到此场景时退出
  interval: 800ms
```

### OCR 资源检测

```yaml
ocr_rule:
  name: quit_when_exhausted
  roi: {x: 100, y: 200, width: 50, height: 20}
  threshold: 5
```

当 OCR 识别到资源低于阈值时自动退出脚本。

## 内置脚本

| 脚本名称 | 功能 |
|----------|------|
| Join Battle | 自动加入群战 |
| Join Tower | 自动加入塔防 |
| Lead Battle | 带队群战 |
| Military Dispatch | 军政派遣 |
| Rivalry Reigns | 争霸赛 |

## 常见问题与注意事项

### 1. 画布窗口不显示
**可能原因**:
- 会话尚未完成启动
- 浏览器驱动启动失败
- 所有会话已关闭

**解决方法**:
- 等待会话状态变为 Ready
- 检查日志中的错误信息
- 重新启动账户

### 2. 登录失败
**可能原因**:
- Cookie 过期
- 网络问题
- 游戏服务器维护

**解决方法**:
- 清空账户的 Cookie，使用用户名密码重新登录
- 检查网络连接
- 等待服务器恢复

### 3. 脚本执行卡住
**可能原因**:
- 未匹配到任何预期场景
- 游戏界面发生变化
- 场景定义不准确

**解决方法**:
- 使用 scene-analyzer 工具检查当前场景
- 更新场景定义的颜色点
- 手动操作后重新启动脚本

### 4. 点击位置偏移
**可能原因**:
- 浏览器视口大小与预期不符
- 游戏缩放设置不正确

**解决方法**:
- 确保浏览器视口为 1080x720
- 检查 browser 模块中的配置

### 5. 帧同步延迟
**现象**: 启动会话后需要切换 tab 才能看到画面

**原因**: 这是已知行为 - screencast 会在 driver 启动 1 秒后自动开始

**解决方法**: 等待 1-2 秒或手动点击画布触发截图

### 6. 多开时资源消耗高
**建议**:
- 关闭 "Auto Refresh" 减少帧同步开销
- 只在需要观察时切换到对应会话
- 合理使用 "Run Group" 控制并发数量

## 日志

### 开发环境
日志输出到控制台，包含 DEBUG 级别信息。

### 生产环境
日志写入滚动文件：
- **位置**: `~/.config/wardenly/logs/` (Windows: `%APPDATA%\wardenly\logs\`)
- **滚动**: 单文件最大 50MB，保留 10 个备份
- **保留**: 最多保留 14 天
- **压缩**: 旧日志自动 gzip 压缩

常见日志信息：
- `Session started`: 会话启动
- `State changed`: 状态转换
- `Login with cookies succeeded`: Cookie 登录成功
- `Script started/stopped`: 脚本启动/停止
- `Screencast started/stopped`: 帧流启动/停止

