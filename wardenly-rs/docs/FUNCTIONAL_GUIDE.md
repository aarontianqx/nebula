# Wardenly - 功能说明

## 概述

Wardenly 是一款用于 WLY 网页游戏自动化的桌面控制工具。通过 headless 浏览器运行游戏，提供实时画面显示、手动操作和自动化脚本执行功能。

## 账户与分组

### 账户管理

账户字段：
- **RoleName**: 游戏内角色名
- **UserName**: 登录用户名
- **Password**: 登录密码
- **ServerID**: 游戏服务器 ID
- **Ranking**: 排序优先级（数值越小越靠前）
- **Cookies**: 保存的登录 Cookie

账户在界面中显示为 `ServerID - RoleName` 格式（如 `126 - 追风`）。

账户下拉框按 `(Ranking ASC, ID ASC)` 排序，低 Ranking 的账户优先显示。

### 分组管理

分组字段：
- **Name**: 分组名称
- **Description**: 可选描述
- **Ranking**: 排序优先级（数值越小越靠前）
- **Accounts**: 分组内的账户列表

分组用于批量启动多个账户：
- 选择分组后点击 "Run Group" 依次启动所有成员账户（间隔 3 秒）
- 已运行的账户自动跳过
- 分组下拉框按 `(Ranking ASC, Name ASC)` 排序

### 管理操作

点击工具栏 **Manage** 按钮打开管理对话框，进行账户和分组的增删改查。

## 设置

点击工具栏 **Settings** 按钮打开设置对话框。

### 主题

选择应用界面配色主题。可选主题由应用内置，包括：
- ocean-dark (默认)
- slate-light
- midnight
- forest-dark

### 存储

选择数据存储后端：
- **SQLite (Local)**: 默认选项，本地文件存储
- **MongoDB (Remote)**: 远程数据库，支持多设备同步

如果选择 MongoDB，需填写连接 URI 和数据库名称：

- **Test Connection**: 点击测试连接，3 秒内返回结果
- **保存前验证**: 保存时自动验证连接，失败则无法保存
- **启动时回退**: 应用启动时若 MongoDB 不可达，自动回退到 SQLite 并显示警告

### 浏览器缓存

显示所有账户浏览器配置文件的总缓存大小，提供清除选项：

- **Total Cache Size**: 显示缓存占用磁盘空间
- **Clear All Cache**: 清除所有账户的浏览器缓存

> 浏览器缓存（Cookies、LocalStorage、图片缓存等）会在每次启动会话时复用，加速页面加载。如遇显示异常或需释放磁盘空间，可手动清除。

> **注意**：设置更改需要重启应用生效。

## 会话管理

### 启动会话

**单账户启动**:
1. 从下拉框选择账户
2. 点击 "Run" 按钮
3. 系统启动浏览器并自动登录

**Run 按钮下拉菜单** (点击 Run 右侧箭头):
- **Force Clean Start**: 清除该账户缓存后启动（适用于游戏更新后卡加载）
- **Clear Account Cache**: 仅清除缓存，不启动

**分组批量启动**:
1. 从下拉框选择分组
2. 点击 "Run Group"
3. 依次启动分组内所有账户

### 会话列表

左侧边栏显示所有运行中的会话，点击可切换当前操作的会话。

状态指示器：
- 🔴 脚本运行中
- (无) 待机状态

## 画布窗口

画布窗口显示当前选中会话的浏览器画面。

### 显示模式

| 模式 | 说明 |
|------|------|
| Screencast | 勾选后以 ~3 FPS 流式传输画面（仅当前选中会话） |
| 停止（默认） | 取消勾选后，停止流式传输以节省性能 |

**设计原则**：
- Screencast 是**全局配置**，同时最多只有一个会话在传输画面
- 切换会话时，自动停止旧会话的传输并启动新会话（画面无缝衔接）
- **默认关闭** - 启动会话时不会自动开始传输，需手动勾选 Screencast
- **自动选中** - 新启动的会话自动成为当前选中会话；关闭会话时自动选中下一个

### 交互操作

| 操作 | Screencast 开启 | Screencast 关闭 |
|------|----------------|----------------|
| 点击画布 | 更新 Inspector + 发送点击到浏览器 | 更新 Inspector + 截取一帧刷新画布 |
| 拖拽画布 | 发送拖拽到浏览器 | 截取一帧刷新画布 |

| 配置 | 说明 |
|------|------|
| Spread to All | 启用后点击/拖拽事件并发发送到所有活跃会话 |

**注意**：Screencast 关闭时，点击画布不会执行浏览器点击，而是截取当前画面。如需执行点击，请使用 Inspector 面板的 Click 按钮。

### 键盘透传

勾选工具栏 "Keyboard Passthrough" 启用，按键事件转换为画布点击。

**触发规则**：
- 仅 A-Z 共 26 个字母键生效
- 仅当鼠标在画布区域内时触发
- 按键后快速释放 (<300ms)：触发一次点击
- 按住超过 300ms：每 100ms 触发一次点击

> **注意**：键盘监听在前端实现，无需系统级权限。

## 浏览器控制

会话详情面板提供以下控制按钮：

| 按钮 | 功能 |
|------|------|
| Stop | 停止会话，关闭浏览器 |
| Refresh Page | 刷新当前页面 |
| Save Cookies | 手动保存 Cookie |

> 登录成功后会自动保存 Cookie，一般无需手动保存。

## 脚本控制

### 脚本操作

| 按钮 | 功能 |
|------|------|
| Start Script | 启动选中脚本 |
| Stop Script | 停止当前脚本 |
| Sync Script | 同步脚本选择到所有会话 |
| Run All | 启动所有会话的脚本 |
| Stop All | 停止所有会话的脚本 |

### 执行逻辑

脚本由多个步骤组成：
1. **场景匹配**：通过颜色点检测当前画面
2. **动作执行**：匹配成功后执行 click、wait、drag 等动作
3. **循环控制**：支持循环直到条件满足

**执行流程**：
1. 截取当前画面
2. 遍历步骤尝试匹配场景
3. 匹配成功后执行动作
4. 等待 500ms 后重复

**停止条件**：
- 用户手动停止
- 脚本 `quit` 动作触发
- OCR 检测到资源耗尽
- 浏览器关闭

## Inspector

工具栏第二行包含 Inspector 区域，用于坐标和颜色查看：

| 组件 | 功能 |
|------|------|
| **X / Y** | 坐标输入框。鼠标点击画布自动填充，也可手动输入 |
| **Fetch** | 获取指定坐标的颜色（也可按 Enter 触发） |
| **Click** | 在指定坐标执行点击（不依赖 Screencast，适用 Spread to All） |
| **色块** | 可视化显示颜色 |
| **Color 值** | RGB 颜色值，格式 `RGB(r, g, b)` |

### 使用场景

- **调试脚本**：点击画布查看坐标和颜色，用于配置场景颜色点
- **精确点击**：输入坐标后点击 Click 按钮，无需鼠标精确定位
- **Screencast 关闭时**：使用 Click 按钮仍可执行点击操作

## 登录机制

### Cookie 登录（优先）

如果账户存储了有效 Cookie：
1. 设置 Cookie 到浏览器
2. 访问游戏 URL
3. 等待游戏加载

### 用户名密码登录

如果没有 Cookie 或 Cookie 失效：
1. 访问游戏登录页
2. 输入用户名和密码
3. 点击登录按钮
4. 等待游戏加载
5. 保存新的 Cookie

## 场景识别

### 场景定义

场景在 `resources/scenes/*.yaml` 中定义：

```yaml
name: main_city
category: city
points:
  - {x: 100, y: 200, color: {r: 255, g: 128, b: 64, a: 255}}
actions:
  SomeButton:
    type: click
    point: {x: 500, y: 600}
```

### 匹配算法

- 检查所有定义的颜色点
- 计算实际颜色与预期颜色的差异
- 平均差异 ≤ 5.0 视为匹配成功

## 自动化脚本

### 脚本定义

脚本在 `resources/scripts/*.yaml` 中定义：

```yaml
name: Join Battle
description: Automatically join group battles
steps:
  - scene: battle_group_entrance
    timeout: 5s
    actions:
      - type: click
        points: [{x: 538, y: 544}]
      - type: wait
        duration: 1s
```

### 支持的动作

| 类型 | 说明 | 参数 |
|------|------|------|
| click | 点击坐标 | points: [{x, y}] |
| wait | 等待时间 | duration: 1s |
| drag | 平滑拖拽 | points: [{x, y}, ...] 支持多点路径 |
| incr/decr | 计数器操作 | key: counter_name |
| quit | 退出脚本 | condition: {op, key, value} |

### 循环控制

循环使用 `loop` 类型的 Action 实现，动作嵌套在内部：

```yaml
actions:
  - type: loop
    count: -1           # -1 表示无限循环
    interval: 800ms     # 循环间隔
    until: target_scene # 匹配到此场景时退出（可选）
    actions:            # 嵌套的动作列表
      - type: click
        points: [{x: 100, y: 200}]
      - type: wait
        duration: 1s
```

### OCR 资源检测

使用 `ocrRule` 检测屏幕上的资源数值：

```yaml
- scene: tower_entrance
  ocrRule:
    mode: ratio              # 识别 "数字/数字" 格式
    roi: {x: 510, y: 602, width: 90, height: 50}
    condition: "used > 7 || used > total"  # 表达式求值
    action: quit_exhausted   # 条件满足时退出
  actions:
    - type: loop
      ...
```

**变量映射**：
- `used`: 分母值（已使用）
- `total`: 分子值（总量）

**支持操作符**：`>`, `>=`, `<`, `<=`, `==`, `!=`, `&&`, `||`

## 常见问题

### 画布不显示

- 等待会话状态变为 Ready
- 检查日志中的错误信息

### 登录失败

- 清空 Cookie 重新登录
- 检查网络连接

### 脚本卡住

- 使用 Inspector 检查当前场景
- 更新场景定义的颜色点

### 点击偏移

- 确保浏览器视口为 1080x720
