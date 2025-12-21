# Wardenly - UI 设计说明

## 设计理念

�?Fyne 框架的约束下，通过以下原则提升用户体验�?

1. **分组与层�?(Hierarchy)**: 将相关联的功能通过容器、卡片或分隔线归类，减少认知负担�?
2. **留白与呼吸感 (Whitespace)**: 增加组件间距，避�?拥挤�?，营�?舒缓"的视觉体验�?
3. **视觉引导 (Visual Cues)**: 引入图标辅助文字，降低阅读成本；利用颜色区分操作的危险等级�?
4. **对齐 (Alignment)**: 确保输入框、标签、按钮在视觉上对齐，提升"精致�?�?

---

## 主窗�?(Main Window)

主窗口采用左右分栏布局，左侧为会话列表，右侧为会话详情面板�?

### 工具�?(Toolbar)

工具栏位于窗口顶部，采用逻辑分组布局�?

```
[账户下拉框] [�?Run] | [分组下拉框] [▶▶ Run] |  ...spacer...  | [�?Manage...]
[�?Spread to All] [�?Auto Refresh (1s)]
```

**设计要点**:
- 使用图标按钮：`Run` 使用 `MediaPlayIcon`，`Run Group` 使用 `MediaFastForwardIcon`，`Manage...` 使用 `SettingsIcon`
- 使用分隔符区分账户区和分组区
- 使用 Spacer 将管理按钮推至右�?
- 选项行位于按钮行下方

### 会话列表 (Session List)

左侧边栏显示所有运行中的会话：
- 每个列表项包含状态指示器（圆形）和账户名�?
- 状态指示器颜色：绿色表示脚本运行中，灰色表示待�?
- 列表项带有内边距，提升触摸友好度

### 会话详情面板 (Session Tab)

右侧详情区域使用 Card 组件划分为三个板块：

#### Browser Control
浏览器控制卡片，包含�?
- `[�?Stop]` - 停止会话
- `[�?Refresh]` - 刷新页面
- `[💾 Cookies]` - 保存 Cookie

#### Script Engine
脚本控制卡片，包含：
- 第一行：脚本下拉框、`[�?Start]`、`[�?Sync]`
- 第二行：`[▶▶ Run All]`
- 按钮图标和文本会根据运行状态动态切换（Start �?Stop�?

#### Inspector
检查器卡片，包含：
- 坐标显示：X、Y 输入�?
- 颜色显示：颜色值输入框 + 颜色预览�?
- 操作按钮：`[Click]`、`[�?Save Screenshot]`
- 临近点日志区

---

## 管理对话�?(Management Dialog)

使用独立窗口，采用原�?`AppTabs` 组件实现标签页切换�?

### 标签�?

- **Accounts** (👤 图标): 账户管理
- **Groups** (📁 图标): 分组管理

Tabs 直接填充整个窗口，无需额外�?Close 按钮（窗�?X 按钮已足够）�?

### 账户表单 (Account Form)

使用 `widget.Form` 布局，自动对齐标签和输入框：

| 字段 | 说明 |
|------|------|
| Role Name | 游戏内角色名 |
| User Name | 登录用户�?|
| Password | 登录密码（密码输入框�?|
| Server ID | 服务�?ID |
| Ranking | 排序优先�?|

**按钮布局**:
- 左侧：`[🗑 Delete]` (红色危险样式)
- 右侧：`[💾 Save]` (蓝色主要样式)
- 使用 Spacer 分隔两侧

### 分组表单 (Group Form)

采用 BorderLayout 实现成员列表的自适应高度�?

**顶部区域**:
- Name、Description、Ranking 输入框（使用 `widget.Form`�?
- Members 标题和工具栏：`[Select All]` `[Deselect All]`

**中心区域**:
- 成员 Checkbox 列表（VScroll�?
- 自动填充剩余垂直空间，窗口越大显示越�?

**底部区域**:
- 分隔�?
- `[🗑 Delete]` ... Spacer ... `[💾 Save]`

---

## 图标使用规范

| 位置 | 按钮 | 图标 |
|------|------|------|
| Toolbar | Run Account | `theme.MediaPlayIcon` |
| Toolbar | Run Group | `theme.MediaFastForwardIcon` |
| Toolbar | Manage | `theme.SettingsIcon` |
| SessionTab | Stop | `theme.MediaStopIcon` |
| SessionTab | Refresh | `theme.ViewRefreshIcon` |
| SessionTab | Cookies | `theme.DocumentSaveIcon` |
| SessionTab | Start Script | `theme.MediaPlayIcon` |
| SessionTab | Stop Script | `theme.MediaStopIcon` |
| SessionTab | Sync | `theme.MediaReplayIcon` |
| SessionTab | Run All | `theme.MediaFastForwardIcon` |
| SessionTab | Click | `theme.MailSendIcon` |
| Management | New Account/Group | `theme.ContentAddIcon` |
| Management | Delete | `theme.DeleteIcon` |
| Management | Save | `theme.DocumentSaveIcon` |
| Tabs | Accounts | `theme.AccountIcon` |
| Tabs | Groups | `theme.FolderIcon` |

---

## 按钮样式规范

| 重要�?| 样式 | 使用场景 |
|--------|------|----------|
| High (蓝色) | `widget.HighImportance` | 主要操作：Save, Run, New |
| Danger (红色) | `widget.DangerImportance` | 危险操作：Delete |
| Medium (默认) | `widget.MediumImportance` | 次要操作：其他按�?|

---

## 布局技�?

### BorderLayout 实现自适应高度
当需要某个区域填充剩余空间时，使�?`container.NewBorder`�?
```go
container.NewBorder(
    topContent,    // 固定高度
    bottomContent, // 固定高度
    nil, nil,
    centerContent, // 填充剩余空间
)
```

### Spacer 实现左右分离
将按钮分隔到两端�?
```go
container.NewHBox(
    leftButton,
    layout.NewSpacer(),
    rightButton,
)
```

### Padded 容器增加内边�?
为紧凑的组件增加呼吸空间�?
```go
container.NewPadded(content)
```

