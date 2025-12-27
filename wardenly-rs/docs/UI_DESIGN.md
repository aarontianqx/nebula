# Wardenly - UI 设计说明

## 设计理念

基于 React + Tailwind CSS 构建现代化 UI，遵循以下原则：

1. **分组与层次 (Hierarchy)**: 将相关联的功能通过卡片、分隔线归类，减少认知负担。
2. **留白与呼吸感 (Whitespace)**: 增加组件间距，避免"拥挤"，营造"舒缓"的视觉体验。
3. **视觉引导 (Visual Cues)**: 引入图标辅助文字，降低阅读成本；利用颜色区分操作的危险等级。
4. **对齐 (Alignment)**: 确保输入框、标签、按钮在视觉上对齐，提升"精致感"。
5. **响应式 (Responsive)**: 组件适应不同窗口尺寸。

---

## 技术选型

- **React 18**: 组件化 UI 开发
- **TypeScript**: 类型安全
- **Tailwind CSS**: 实用优先的样式框架
- **Lucide React**: 图标库
- **Zustand**: 轻量状态管理

---

## 主窗口 (MainWindow)

主窗口采用左右分栏布局，左侧为会话列表，右侧为会话详情面板。

### 整体布局

```tsx
<div className="flex h-screen bg-gray-50 dark:bg-gray-900">
  {/* 左侧边栏 */}
  <aside className="w-64 border-r border-gray-200 dark:border-gray-700">
    <SessionList />
  </aside>
  
  {/* 右侧主区域 */}
  <main className="flex-1 flex flex-col">
    <Toolbar />
    <DetailPanel />
  </main>
</div>
```

### 工具栏 (Toolbar)

工具栏位于窗口顶部，采用逻辑分组布局。

```
┌────────────────────────────────────────────────────────────────┐
│ [Account ▼] [▶ Run] │ [Group ▼] [▶▶ Run] │  ...spacer...  │ [⚙ Manage] │
├────────────────────────────────────────────────────────────────┤
│ ☐ Spread to All    ☐ Auto Refresh (1s)    ☐ Keyboard Passthrough │
└────────────────────────────────────────────────────────────────┘
```

**组件实现**:
```tsx
function Toolbar() {
  return (
    <div className="border-b border-gray-200 dark:border-gray-700 p-4 space-y-3">
      {/* 第一行：操作按钮 */}
      <div className="flex items-center gap-4">
        {/* 账户区 */}
        <Select placeholder="Select Account" options={accounts} />
        <Button icon={<Play />} onClick={runAccount}>Run</Button>
        
        <Separator orientation="vertical" />
        
        {/* 分组区 */}
        <Select placeholder="Select Group" options={groups} />
        <Button icon={<FastForward />} onClick={runGroup}>Run</Button>
        
        <div className="flex-1" />
        
        {/* 管理按钮 */}
        <Button icon={<Settings />} variant="outline" onClick={openManage}>
          Manage...
        </Button>
      </div>
      
      {/* 第二行：选项 */}
      <div className="flex items-center gap-4">
        <Checkbox label="Spread to All" checked={spreadToAll} />
        <Checkbox label="Auto Refresh (1s)" checked={autoRefresh} />
        <Checkbox label="Keyboard Passthrough" checked={keyboardPassthrough} />
      </div>
    </div>
  );
}
```

**工具栏选项说明**:

| 选项 | 说明 |
|------|------|
| **Spread to All** | 画布点击事件扩散到所有活跃会话 |
| **Auto Refresh (1s)** | 每秒自动刷新画布帧 |
| **Keyboard Passthrough** | 监听系统键盘，转换为画布点击事件 |

### Keyboard Passthrough 功能

当 **Keyboard Passthrough** 选项启用时：

1. **监听系统键盘**: 应用程序开始监听系统级键盘事件（需要辅助功能权限）
2. **单击识别**: 按下并快速释放按键 → 在当前鼠标位置触发一次画布点击
3. **长按连击**: 按住按键超过 300ms → 启动连击模式，每 100ms 触发一次点击
4. **位置检测**: 仅当鼠标在画布区域内时触发点击，画布外不响应

**交互状态**:

```
┌─────────────────────────────────────────────────────────────┐
│  ☐ Keyboard Passthrough                                     │  未启用
│     灰色文字，无特殊样式                                      │
├─────────────────────────────────────────────────────────────┤
│  ☑ Keyboard Passthrough  🔴                                 │  启用中 (监听)
│     复选框后显示小红点指示器，表示正在监听                     │
└─────────────────────────────────────────────────────────────┘
```

**权限提示 (macOS)**:

首次启用时，如果未授权辅助功能权限，显示提示：

```tsx
<AlertDialog>
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle>Accessibility Permission Required</AlertDialogTitle>
      <AlertDialogDescription>
        Keyboard Passthrough requires accessibility permission to listen to system keyboard events.
        Please grant permission in System Settings → Privacy & Security → Accessibility.
      </AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <Button onClick={openSystemSettings}>Open System Settings</Button>
      <Button variant="outline" onClick={dismiss}>Cancel</Button>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
```

### 会话列表 (SessionList)

左侧边栏显示所有运行中的会话：

```tsx
function SessionList() {
  return (
    <div className="flex flex-col h-full">
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <h2 className="text-sm font-semibold text-gray-600 dark:text-gray-400">
          Sessions
        </h2>
      </div>
      
      <div className="flex-1 overflow-y-auto">
        {sessions.map(session => (
          <SessionListItem
            key={session.id}
            session={session}
            isActive={session.id === activeSessionId}
            onClick={() => selectSession(session.id)}
          />
        ))}
      </div>
    </div>
  );
}

function SessionListItem({ session, isActive, onClick }) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "w-full px-4 py-3 flex items-center gap-3 text-left",
        "hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors",
        isActive && "bg-blue-50 dark:bg-blue-900/20 border-r-2 border-blue-500"
      )}
    >
      {/* 状态指示器 */}
      <span className={cn(
        "w-2 h-2 rounded-full",
        session.isScriptRunning ? "bg-red-500" : "bg-gray-300"
      )} />
      
      {/* 账户名 */}
      <span className="text-sm font-medium truncate">
        {session.accountName}
      </span>
    </button>
  );
}
```

### 会话详情面板 (SessionTab)

右侧详情区域使用 Card 组件划分为三个板块：

```tsx
function SessionTab({ session }) {
  return (
    <div className="p-6 space-y-6 overflow-y-auto">
      <BrowserControlCard session={session} />
      <ScriptEngineCard session={session} />
      <InspectorCard session={session} />
    </div>
  );
}
```

#### Browser Control 卡片

```tsx
function BrowserControlCard({ session }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Browser Control</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex gap-2">
          <Button 
            icon={<Square />} 
            variant="destructive"
            onClick={stopSession}
          >
            Stop
          </Button>
          <Button 
            icon={<RefreshCw />}
            variant="outline"
            onClick={refreshPage}
            disabled={!session.isReady}
          >
            Refresh
          </Button>
          <Button 
            icon={<Save />}
            variant="outline"
            onClick={saveCookies}
            disabled={!session.isReady}
          >
            Cookies
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
```

#### Script Engine 卡片

```tsx
function ScriptEngineCard({ session }) {
  const isRunning = session.isScriptRunning;
  
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Script Engine</CardTitle>
      </CardHeader>
      <CardContent>
        {/* 单行布局：脚本选择 + Start/Stop + Sync + Run All/Stop All */}
        <div className="flex gap-2">
          <Select 
            className="flex-1"
            options={scriptNames}
            value={selectedScript}
            onChange={setSelectedScript}
            disabled={!session.isReady}
          />
          <Button
            icon={isRunning ? <Square /> : <Play />}
            variant={isRunning ? "destructive" : "default"}
            onClick={isRunning ? stopScript : startScript}
            disabled={!session.isReady}
          >
            {isRunning ? "Stop" : "Start"}
          </Button>
          <Button
            icon={<RefreshCw />}
            variant="outline"
            onClick={syncScript}
            disabled={!session.isReady}
          >
            Sync
          </Button>
          <Button
            icon={isRunning ? <Square /> : <FastForward />}
            variant="outline"
            onClick={isRunning ? stopAllScripts : runAllScripts}
            disabled={!session.isReady}
          >
            {isRunning ? "Stop All" : "Run All"}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
```

#### Inspector 卡片

Inspector 卡片用于坐标输入和颜色查看，支持两种输入方式：
- **鼠标点击画布**：自动填充坐标并更新颜色
- **键盘输入坐标**：手动输入 X/Y 值，按 Enter 或点击 Fetch 按钮更新颜色

```tsx
function InspectorCard({ session }) {
  const [x, setX] = useState('');
  const [y, setY] = useState('');
  const [color, setColor] = useState('');
  const [colorValue, setColorValue] = useState('#000000');
  
  // 从画布获取指定坐标的颜色
  const fetchColor = useCallback(async () => {
    const px = parseInt(x, 10);
    const py = parseInt(y, 10);
    if (isNaN(px) || isNaN(py)) return;
    
    const result = await invoke<ColorResult>('get_color_at', { 
      sessionId: session.id, 
      x: px, 
      y: py 
    });
    setColor(result.rgba);
    setColorValue(result.hex);
  }, [x, y, session.id]);
  
  // 键盘 Enter 触发获取颜色
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      fetchColor();
    }
  };
  
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Inspector</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* 操作按钮 */}
        <div className="flex items-center gap-2">
          <Button icon={<MousePointer />} onClick={sendClick}>
            Click
          </Button>
          <Checkbox label="Save Screenshot" checked={saveScreenshot} />
        </div>
        
        {/* 坐标输入和颜色显示 */}
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1">
            <Label className="text-sm text-gray-500">X</Label>
            <Input 
              type="number"
              value={x}
              onChange={(e) => setX(e.target.value)}
              onKeyDown={handleKeyDown}
              className="w-20"
              placeholder="0"
            />
          </div>
          <div className="flex items-center gap-1">
            <Label className="text-sm text-gray-500">Y</Label>
            <Input 
              type="number"
              value={y}
              onChange={(e) => setY(e.target.value)}
              onKeyDown={handleKeyDown}
              className="w-20"
              placeholder="0"
            />
          </div>
          <Button 
            variant="outline" 
            size="sm"
            onClick={fetchColor}
            icon={<Pipette />}
          >
            Fetch
          </Button>
          <div className="flex items-center gap-2 ml-2">
            <div 
              className="w-8 h-8 rounded border border-gray-300 shadow-inner"
              style={{ backgroundColor: colorValue }}
            />
            <span className="text-sm font-mono text-gray-600 dark:text-gray-400">
              {color || 'RGBA(-, -, -, -)'}
            </span>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
```

---

## 画布窗口 (CanvasWindow)

独立窗口显示浏览器画面，支持点击和拖拽交互。

```tsx
function CanvasWindow() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  
  const handleClick = (e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    
    invoke('click_session', { sessionId: activeSession, x, y });
  };
  
  const handleDrag = (startX, startY, endX, endY) => {
    invoke('drag_session', { 
      sessionId: activeSession, 
      fromX: startX, 
      fromY: startY,
      toX: endX,
      toY: endY
    });
  };
  
  return (
    <div className="bg-black">
      <canvas
        ref={canvasRef}
        width={1080}
        height={720}
        onClick={handleClick}
        onMouseDown={startDrag}
        onMouseUp={endDrag}
        className="cursor-crosshair"
      />
    </div>
  );
}
```

---

## 管理对话框 (ManagementDialog)

使用 Tab 组件实现标签页切换。

```tsx
function ManagementDialog({ open, onClose }) {
  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="max-w-2xl max-h-[80vh]">
        <DialogHeader>
          <DialogTitle>Manage Accounts & Groups</DialogTitle>
        </DialogHeader>
        
        <Tabs defaultValue="accounts">
          <TabsList>
            <TabsTrigger value="accounts">
              <User className="w-4 h-4 mr-2" />
              Accounts
            </TabsTrigger>
            <TabsTrigger value="groups">
              <Folder className="w-4 h-4 mr-2" />
              Groups
            </TabsTrigger>
          </TabsList>
          
          <TabsContent value="accounts">
            <AccountsPanel />
          </TabsContent>
          
          <TabsContent value="groups">
            <GroupsPanel />
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}
```

### 账户表单 (AccountForm)

```tsx
function AccountForm({ account, onSave, onDelete }) {
  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div className="grid grid-cols-2 gap-4">
        <FormField label="Role Name" name="roleName" required />
        <FormField label="User Name" name="userName" required />
        <FormField label="Password" name="password" type="password" required />
        <FormField label="Server ID" name="serverId" type="number" required />
        <FormField label="Ranking" name="ranking" type="number" />
      </div>
      
      <Separator />
      
      <div className="flex justify-between">
        <Button 
          type="button"
          variant="destructive"
          icon={<Trash2 />}
          onClick={onDelete}
        >
          Delete
        </Button>
        <Button type="submit" icon={<Save />}>
          Save
        </Button>
      </div>
    </form>
  );
}
```

### 分组表单 (GroupForm)

```tsx
function GroupForm({ group, accounts, onSave, onDelete }) {
  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div className="space-y-4">
        <FormField label="Name" name="name" required />
        <FormField label="Description" name="description" />
        <FormField label="Ranking" name="ranking" type="number" />
      </div>
      
      {/* 成员选择 */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <Label>Members</Label>
          <div className="space-x-2">
            <Button type="button" variant="ghost" size="sm" onClick={selectAll}>
              Select All
            </Button>
            <Button type="button" variant="ghost" size="sm" onClick={deselectAll}>
              Deselect All
            </Button>
          </div>
        </div>
        
        <div className="max-h-48 overflow-y-auto border rounded p-2 space-y-1">
          {accounts.map(account => (
            <Checkbox
              key={account.id}
              label={account.identity}
              checked={selectedIds.includes(account.id)}
              onChange={() => toggleMember(account.id)}
            />
          ))}
        </div>
      </div>
      
      <Separator />
      
      <div className="flex justify-between">
        <Button 
          type="button"
          variant="destructive"
          icon={<Trash2 />}
          onClick={onDelete}
        >
          Delete
        </Button>
        <Button type="submit" icon={<Save />}>
          Save
        </Button>
      </div>
    </form>
  );
}
```

---

## 图标使用规范

使用 Lucide React 图标库：

| 位置 | 按钮 | 图标 |
|------|------|------|
| Toolbar | Run Account | `Play` |
| Toolbar | Run Group | `FastForward` |
| Toolbar | Manage | `Settings` |
| SessionTab | Stop | `Square` |
| SessionTab | Refresh | `RefreshCw` |
| SessionTab | Cookies | `Save` |
| SessionTab | Start Script | `Play` |
| SessionTab | Stop Script | `Square` |
| SessionTab | Sync | `RefreshCw` |
| SessionTab | Run All | `FastForward` |
| SessionTab | Click | `MousePointer` |
| SessionTab | Fetch | `Pipette` |
| Management | New | `Plus` |
| Management | Delete | `Trash2` |
| Management | Save | `Save` |
| Tabs | Accounts | `User` |
| Tabs | Groups | `Folder` |

---

## 按钮样式规范

使用 Tailwind CSS 变体：

| 重要性 | 样式 | 使用场景 |
|--------|------|----------|
| Primary | `bg-blue-600 text-white` | 主要操作：Save, Run, Start |
| Destructive | `bg-red-600 text-white` | 危险操作：Delete, Stop |
| Outline | `border border-gray-300` | 次要操作：Refresh, Sync |
| Ghost | `bg-transparent hover:bg-gray-100` | 最小化操作：Select All |

```tsx
// Button 组件变体定义
const buttonVariants = cva(
  "inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors",
  {
    variants: {
      variant: {
        default: "bg-blue-600 text-white hover:bg-blue-700",
        destructive: "bg-red-600 text-white hover:bg-red-700",
        outline: "border border-gray-300 bg-transparent hover:bg-gray-100",
        ghost: "bg-transparent hover:bg-gray-100",
      },
      size: {
        default: "h-9 px-4",
        sm: "h-8 px-3 text-xs",
        lg: "h-10 px-6",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
);
```

---

## 颜色主题

支持亮色和暗色主题：

```css
/* globals.css */
:root {
  --background: 0 0% 100%;
  --foreground: 222.2 84% 4.9%;
  --card: 0 0% 100%;
  --card-foreground: 222.2 84% 4.9%;
  --primary: 221.2 83.2% 53.3%;
  --primary-foreground: 210 40% 98%;
  --destructive: 0 84.2% 60.2%;
  --muted: 210 40% 96.1%;
  --border: 214.3 31.8% 91.4%;
}

.dark {
  --background: 222.2 84% 4.9%;
  --foreground: 210 40% 98%;
  --card: 222.2 84% 4.9%;
  --card-foreground: 210 40% 98%;
  --primary: 217.2 91.2% 59.8%;
  --primary-foreground: 222.2 47.4% 11.2%;
  --destructive: 0 62.8% 30.6%;
  --muted: 217.2 32.6% 17.5%;
  --border: 217.2 32.6% 17.5%;
}
```

---

## 响应式设计

关键断点：

- **sm** (640px): 移动设备
- **md** (768px): 平板设备
- **lg** (1024px): 桌面设备
- **xl** (1280px): 大屏设备

侧边栏在小屏幕上可折叠：

```tsx
function MainLayout() {
  const [sidebarOpen, setSidebarOpen] = useState(true);
  
  return (
    <div className="flex h-screen">
      {/* 侧边栏 - 可折叠 */}
      <aside className={cn(
        "border-r transition-all duration-200",
        sidebarOpen ? "w-64" : "w-0 overflow-hidden"
      )}>
        <SessionList />
      </aside>
      
      {/* 主区域 */}
      <main className="flex-1 flex flex-col min-w-0">
        <Toolbar onToggleSidebar={() => setSidebarOpen(!sidebarOpen)} />
        <DetailPanel />
      </main>
    </div>
  );
}
```

---

## 动画与过渡

使用 Tailwind CSS 过渡类：

```tsx
// 列表项 hover 效果
<div className="transition-colors hover:bg-gray-100" />

// 侧边栏折叠动画
<aside className="transition-all duration-200" />

// 按钮点击反馈
<button className="transition-transform active:scale-95" />

// 对话框淡入
<DialogContent className="animate-in fade-in-0 zoom-in-95" />
```

---

## 无障碍 (Accessibility)

- 所有交互元素使用语义化标签 (`<button>`, `<input>`)
- 图标按钮包含 `aria-label`
- 表单字段关联 `<label>`
- 键盘导航支持 (Tab, Enter, Escape)
- 颜色对比度符合 WCAG AA 标准

