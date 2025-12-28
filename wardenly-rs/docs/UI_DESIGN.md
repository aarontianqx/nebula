# Wardenly - UI 设计规范

## 1. 设计哲学

**"Professional Utility" (专业工具)**

Wardenly 作为一个自动化与监控工具，UI 设计应遵循以下核心原则：

1.  **内容优先**: 游戏画面（Canvas）是核心，UI 应作为框架存在，尽量降低视觉干扰（低饱和度背景）。
2.  **状态显性**: 运行、停止、Screencast 等关键状态必须一目了然，但不要通过大面积高亮造成视觉疲劳。
3.  **语义化色彩**: 颜色仅用于表达状态（绿=运行，红=停止/危险，蓝=选中/激活），避免装饰性用色。
4.  **高密度与呼吸感平衡**: 作为桌面工具，需要保持较高的信息密度，但通过微调间距（Spacing）和边框（Borders）来维持界面的条理性。

---

## 2. 主题系统架构 (Theme Architecture)

为了实现**用户自主换肤且无需重新编译**，我们采用 **"配置驱动的动态 CSS 变量注入"** 方案。

### 2.1 核心原理
1.  **配置分离**: 主题色值不硬编码在代码中，而是存储在外部配置文件（如 `themes.json`）中。
2.  **运行时注入**: 应用启动时，前端通过 Tauri 接口读取配置，将颜色值动态写入 DOM 的 `:root` 样式。
3.  **编译无关**: Tailwind CSS 仅引用变量名（如 `var(--bg-app)`），不关心具体色值，因此换肤无需重编译。

### 2.2 配置文件结构 (示例)
用户可编辑此文件来自定义主题：
```json
{
  "activeTheme": "ocean-dark",
  "themes": {
    "ocean-dark": {
      "colors": {
        "bg-app": "#0f172a",
        "bg-panel": "#1e293b",
        "accent": "#38bdf8",
        "text-primary": "#f1f5f9"
      }
    },
    "forest-light": {
      "colors": {
        "bg-app": "#f0fdf4",
        "bg-panel": "#ffffff",
        "accent": "#16a34a",
        "text-primary": "#14532d"
      }
    }
  }
}
```

### 2.3 语义化 Token 定义

| Token 变量 | 说明 | Dark Mode (Default) | Light Mode |
|:---|:---|:---|:---|
| **基础层级** | | | |
| `--bg-app` | 应用背景色 | `#0f172a` (Slate 950) | `#f8fafc` (Slate 50) |
| `--bg-panel` | 侧边栏/面板背景 | `#1e293b` (Slate 800) | `#ffffff` (White) |
| `--bg-surface` | 卡片/输入框背景 | `#334155` (Slate 700) | `#f1f5f9` (Slate 100) |
| `--border` | 边框颜色 | `#334155` (Slate 700) | `#e2e8f0` (Slate 200) |
| **文本层级** | | | |
| `--text-primary` | 主要文字 | `#f1f5f9` (Slate 100) | `#0f172a` (Slate 900) |
| `--text-secondary` | 次要文字 | `#94a3b8` (Slate 400) | `#64748b` (Slate 500) |
| `--text-muted` | 禁用/提示文字 | `#475569` (Slate 600) | `#94a3b8` (Slate 400) |
| **品牌与状态** | | | |
| `--accent` | 品牌色/选中态 | `#3b82f6` (Blue 500) | `#2563eb` (Blue 600) |
| `--accent-fg` | 品牌色上的文字 | `#ffffff` | `#ffffff` |
| `--success` | 运行/成功 | `#10b981` (Emerald 500) | `#059669` (Emerald 600) |
| `--danger` | 停止/危险 | `#ef4444` (Red 500) | `#dc2626` (Red 600) |
| `--warning` | 警告/等待 | `#f59e0b` (Amber 500) | `#d97706` (Amber 600) |

---

## 3. 组件设计规范

### 3.1 布局框架
采用经典的 **Sidebar + Header + Main** 布局。

- **Header (Toolbar)**: 高度 `56px` (h-14)，底部边框分隔。
- **Sidebar**: 宽度 `260px` (w-64)，右侧边框分隔。
- **Canvas Area**: 占据剩余空间，居中显示内容。
- **Inspector Bar**: 位于 Canvas 下方，作为独立的工具条悬浮或嵌入。

### 3.2 交互组件

#### 侧边栏列表项 (Session List Item)
摒弃全背景高亮，改用左侧指示条，保持视觉清爽。

*   **Default**: `text-secondary hover:bg-surface/50 hover:text-primary`
*   **Active**: `bg-accent/10 text-accent border-l-4 border-accent` (左侧亮条)
*   **Running**: 右侧显示微型呼吸灯 (Success color dot)。

#### 按钮系统 (Button System)
按钮高度统一为 `32px` (sm) 或 `36px` (default)，圆角 `6px` (rounded-md)。

*   **Primary (Run)**: `bg-accent text-accent-fg hover:opacity-90`
*   **Secondary (Manage)**: `bg-surface border border-border text-primary hover:bg-surface/80`
*   **Destructive (Stop)**: `text-danger hover:bg-danger/10` (图标按钮) 或 `bg-danger text-white` (实心按钮)
*   **Ghost (Icon Button)**: `text-secondary hover:text-primary hover:bg-surface`

#### 输入框与下拉框 (Inputs)
*   背景色: `var(--bg-surface)`
*   边框: `1px solid var(--border)`
*   Focus: `ring-2 ring-accent/20 border-accent`
*   样式: 去除浏览器默认样式，高度与按钮对齐。

### 3.3 Inspector 面板 (HUD 风格)
设计为紧凑的工具条，位于 Canvas 下方 16px 处。

*   **容器**: 圆角 `rounded-lg`，背景 `var(--bg-panel)`，边框 `var(--border)`，阴影 `shadow-sm`。
*   **坐标显示**: 使用等宽字体 (Monospace)，如 `JetBrains Mono` 或 `Consolas`，增加专业感。
    *   样式: `font-mono text-xs text-secondary`
*   **颜色预览**: `w-6 h-6 rounded border border-border`，点击可复制 Hex 值。
*   **操作按钮**: 集成 Fetch/Click 按钮，使用 Icon + Text 的小号按钮。

---

## 4. 技术栈升级与依赖

为实现上述设计，建议更新 `tailwind.config.js` 配置：

```javascript
module.exports = {
  theme: {
    extend: {
      colors: {
        bg: {
          app: 'var(--bg-app)',
          panel: 'var(--bg-panel)',
          surface: 'var(--bg-surface)',
        },
        text: {
          primary: 'var(--text-primary)',
          secondary: 'var(--text-secondary)',
          muted: 'var(--text-muted)',
        },
        border: 'var(--border)',
        accent: {
          DEFAULT: 'var(--accent)',
          fg: 'var(--accent-fg)',
          hover: 'var(--accent-hover)',
        },
        // ... 其他语义变量
      }
    }
  }
}
```

## 5. 响应式与可访问性

1.  **对比度**: 确保 `--text-secondary` 在 `--bg-panel` 上有足够的对比度。
2.  **Focus 状态**: 所有可交互元素必须保留 Focus Ring，支持键盘 Tab 切换。
3.  **Tooltip**: 只有图标的按钮必须包含 Tooltip 说明。

---

## 6. UI 改进清单 (Checklist for Next Dev Phase)

- [ ] **重构 Theme Provider**: 实现读取外部 JSON 配置并注入 CSS 变量的逻辑。
- [ ] **优化 Sidebar**: 实现新的选中态样式（左侧边框高亮）。
- [ ] **重构 Toolbar**: 对齐所有控件高度，增加竖线分隔符。
- [ ] **美化 Inspector**: 实现 HUD 风格，使用等宽字体。
- [ ] **字体统一**: 引入 Inter 或确保系统字体栈统一，解决 Windows/Mac 字体渲染差异。
