# Wardenly - UI 设计

## 设计原则

1. **分组与层次**: 相关功能通过卡片归类，减少认知负担
2. **留白与呼吸感**: 增加组件间距，避免拥挤
3. **视觉引导**: 图标辅助文字，颜色区分操作危险等级
4. **对齐**: 输入框、标签、按钮视觉对齐
5. **响应式**: 组件适应不同窗口尺寸

## 技术选型

- **React 18**: 组件化开发
- **TypeScript**: 类型安全
- **Tailwind CSS v4**: 实用优先样式
- **Lucide React**: 图标库
- **Zustand**: 状态管理

## 布局结构

### 主窗口

左右分栏布局：

```
┌──────────────┬────────────────────────────────────────────┐
│              │                  Toolbar                    │
│   Session    ├────────────────────────────────────────────┤
│    List      │                                            │
│              │              Detail Panel                   │
│              │                                            │
└──────────────┴────────────────────────────────────────────┘
```

```tsx
<div className="flex h-screen bg-gray-50 dark:bg-gray-900">
  <aside className="w-64 border-r">
    <SessionList />
  </aside>
  <main className="flex-1 flex flex-col">
    <Toolbar />
    <DetailPanel />
  </main>
</div>
```

### 工具栏

```
┌────────────────────────────────────────────────────────────────┐
│ [Account ▼] [▶ Run] │ [Group ▼] [▶▶ Run] │  spacer  │ [⚙ Manage] │
├────────────────────────────────────────────────────────────────┤
│ ☐ Spread to All    ☐ Auto Refresh    ☐ Keyboard Passthrough    │
└────────────────────────────────────────────────────────────────┘
```

### 会话列表

```tsx
<SessionListItem>
  {/* 状态指示器 */}
  <span className={cn(
    "w-2 h-2 rounded-full",
    session.isScriptRunning ? "bg-red-500" : "bg-gray-300"
  )} />
  {/* 账户名 */}
  <span className="truncate">{session.accountName}</span>
</SessionListItem>
```

### 详情面板

右侧详情区域使用 Card 组件划分为三个板块：

1. **Browser Control**: 浏览器操作 (Stop, Refresh, Cookies)
2. **Script Engine**: 脚本控制
3. **Inspector**: 坐标与颜色查看

## 组件规范

### 图标使用

| 位置 | 按钮 | 图标 |
|------|------|------|
| Toolbar | Run Account | `Play` |
| Toolbar | Run Group | `FastForward` |
| Toolbar | Manage | `Settings` |
| SessionTab | Stop | `Square` |
| SessionTab | Refresh | `RefreshCw` |
| SessionTab | Start Script | `Play` |
| SessionTab | Stop Script | `Square` |
| SessionTab | Click | `MousePointer` |
| SessionTab | Fetch | `Pipette` |
| Management | New | `Plus` |
| Management | Delete | `Trash2` |
| Management | Save | `Save` |
| Tabs | Accounts | `User` |
| Tabs | Groups | `Folder` |

### 按钮样式

| 变体 | 样式 | 使用场景 |
|------|------|----------|
| Primary | `bg-blue-600 text-white` | Save, Run, Start |
| Destructive | `bg-red-600 text-white` | Delete, Stop |
| Outline | `border border-gray-300` | Refresh, Sync |
| Ghost | `bg-transparent hover:bg-gray-100` | Select All |

```tsx
const buttonVariants = cva(
  "inline-flex items-center justify-center rounded-md text-sm font-medium",
  {
    variants: {
      variant: {
        default: "bg-blue-600 text-white hover:bg-blue-700",
        destructive: "bg-red-600 text-white hover:bg-red-700",
        outline: "border border-gray-300 hover:bg-gray-100",
        ghost: "bg-transparent hover:bg-gray-100",
      },
      size: {
        default: "h-9 px-4",
        sm: "h-8 px-3 text-xs",
        lg: "h-10 px-6",
      },
    },
  }
);
```

## 颜色主题

支持亮色和暗色主题：

```css
:root {
  --background: 0 0% 100%;
  --foreground: 222.2 84% 4.9%;
  --primary: 221.2 83.2% 53.3%;
  --destructive: 0 84.2% 60.2%;
  --muted: 210 40% 96.1%;
  --border: 214.3 31.8% 91.4%;
}

.dark {
  --background: 222.2 84% 4.9%;
  --foreground: 210 40% 98%;
  --primary: 217.2 91.2% 59.8%;
  --destructive: 0 62.8% 30.6%;
  --muted: 217.2 32.6% 17.5%;
  --border: 217.2 32.6% 17.5%;
}
```

## 响应式断点

| 断点 | 像素 | 设备 |
|------|------|------|
| sm | 640px | 小屏 |
| md | 768px | 平板 |
| lg | 1024px | 桌面 |
| xl | 1280px | 大屏 |

侧边栏可折叠：

```tsx
<aside className={cn(
  "border-r transition-all duration-200",
  sidebarOpen ? "w-64" : "w-0 overflow-hidden"
)}>
```

## 动画

```tsx
// 列表项 hover
<div className="transition-colors hover:bg-gray-100" />

// 侧边栏折叠
<aside className="transition-all duration-200" />

// 按钮点击
<button className="transition-transform active:scale-95" />

// 对话框淡入
<DialogContent className="animate-in fade-in-0 zoom-in-95" />
```

## 无障碍

- 所有交互元素使用语义化标签
- 图标按钮包含 `aria-label`
- 表单字段关联 `<label>`
- 键盘导航支持 (Tab, Enter, Escape)
- 颜色对比度符合 WCAG AA 标准
