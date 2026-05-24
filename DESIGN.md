# fi-code UI Design System

## 一、设计理念

基于Tauri生态的现代、优雅、专业的AI代码助手界面。以深蓝色紫色渐变为主色调，配合玻璃拟态效果，打造既专业又现代的开发体验。

### 核心原则
1. **现代感** - 玻璃拟态 + 渐变 + 柔和阴影
2. **层次感** - 鲜明的视觉层级和空间分割
3. **专业性** - 代码优先，信息清晰展示
4. **交互动效** - 流畅的状态切换和实时反馈

---

## 二、色彩系统

### 主色调
```css
colors: {
  tauri: {
    primary: '#24C8DB',      /* 青蓝色 */
    secondary: '#C084FC',    /* 紫粉色 */
    dark: '#020617',         /* 深暗色背景 */
    darker: '#01030C',       /* 更深色背景 */
    card: '#0F172A',         /* 卡片背景 */
    border: '#1E293B'        /* 边框色 */
  }
}
```

### 渐变定义
```css
/* 主渐变 - 从主色到次色，135度角 */
.gradient-bg {
  background: linear-gradient(135deg, #24C8DB, #C084FC);
}

/* 文字渐变 */
.gradient-text {
  background: linear-gradient(90deg, #24C8DB, #C084FC);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}
```

### 语义色彩
- **成功**：绿色系 (text-green-400, bg-green-900/30)
- **警告**：黄色系 (text-yellow-400, bg-yellow-500/10)
- **错误**：红色系 (text-red-400, bg-red-500/10)
- **信息**：主色调系

---

## 三、样式组件库

### 1. 玻璃拟态 (Glass)
```css
.glass {
  background: rgba(15, 23, 42, 0.7);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
}
```

### 2. 网格背景
```css
.bg-grid {
  background-image: 
    linear-gradient(rgba(36, 200, 219, 0.05) 1px, transparent 1px),
    linear-gradient(90deg, rgba(36, 200, 219, 0.05) 1px, transparent 1px);
  background-size: 40px 40px;
}
```

### 3. 卡片悬停效果
```css
.card-hover {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.card-hover:hover {
  transform: translateY(-2px);
  box-shadow: 
    0 20px 25px -5px rgba(0, 0, 0, 0.3),
    0 10px 10px -5px rgba(0, 0, 0, 0.2);
}
```

### 4. 滚动条样式
```css
.scrollbar-tauri {
  scrollbar-width: thin;
  scrollbar-color: #334155 transparent;
}

.scrollbar-tauri::-webkit-scrollbar {
  width: 6px;
}

.scrollbar-tauri::-webkit-scrollbar-track {
  background: transparent;
}

.scrollbar-tauri::-webkit-scrollbar-thumb {
  background-color: #334155;
  border-radius: 3px;
}
```

### 5. 可折叠区域
```css
.collapsible-content {
  max-height: 0;
  overflow: hidden;
  transition: max-height 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.collapsible-content.open {
  max-height: 600px;
}
```

---

## 四、布局约束

### 整体布局
```
┌─────────────────────────────────────────────────────────┐
│  Header (固定高度64px) - 渐变标题 + 上下文指示器 + 设置    │
├──────────────┬──────────────────────────────────────────┤
│              │                                          │
│  边栏 (256px)│          主内容区 (flex-1)               │
│  - 文件树    │         (聊天消息 + 输入区)              │
│              │                                          │
└──────────────┴──────────────────────────────────────────┘
```

### 容器约束
- **Body**：flex flex-col h-screen overflow-hidden bg-grid
- **Header**：glass border-b border-tauri-border h-16 flex items-center px-8 justify-between shrink-0 z-10
- **边栏**：w-64 glass border-r border-tauri-border flex flex-col shrink-0
- **聊天区**：flex-1 flex flex-col overflow-hidden

---

## 五、组件设计规范

### 1. 消息气泡
- **用户消息**：靠右，glass边框，border-tauri-primary/30，max-width-[65%]
- **AI消息**：靠左，时间线布局，带工具调用/结果展示

### 2. 时间线组件
```
[左圆点]  [内容]
    │
    │
[左圆点]  [内容]
```

样式要求：
- 时间线竖线：bg-gradient-to-b from-tauri-primary via-tauri-secondary to-tauri-border
- 圆点：绝对定位 -left-8，w-6 h-6，rounded-full
- 内容容器：glass边框，p-6，shadow-xl

### 3. 任务列表
- checkbox样式：w-5 h-5 rounded，选中时主色调
- 任务项：p-2 rounded-lg hover:bg-tauri-card/50
- 已完成任务：line-through text-gray-500

### 4. 权限确认面板
- 标题：text-yellow-400，带闪烁动画
- 选项：多选checkbox + 自定义输入
- 按钮：主色调渐变按钮 + 次要按钮 + 稍后按钮

### 5. 代码块
- 使用highlight.js进行语法高亮
- 主题：atom-one-dark
- 容器：bg-tauri-darker rounded-xl p-4 font-mono text-sm border border-tauri-border
- 支持diff展示（绿色+和红色-）

### 6. 工具结果展示
- 分两部分：元数据标题 + 内容区域
- 元数据：工具名 + 调用ID + 状态 + 耗时
- 内容区：支持多种类型展示（代码、文字、JSON等）

---

## 六、字体排版

### 字体选择
```css
font-family: {
  inter: ['Inter', 'sans-serif'],        /* 主要UI文字 */
  mono: ['JetBrains Mono', 'monospace']  /* 代码、工具输出 */
}
```

### 字号层次
- **大标题**：text-2xl font-bold gradient-text (标题)
- **中标题**：text-xl font-bold gradient-text (区域标题)
- **小标题**：text-lg font-semibold (组件标题)
- **正文**：text-base text-gray-100 (主要内容)
- **辅助**：text-sm text-gray-400 (元数据、时间)

### 字重使用
- 标题：font-bold (700)
- 副标题：font-semibold (600)
- 强调文本：font-medium (500)
- 正文：font-normal (400)

---

## 七、动画与过渡

### 过渡时间
- **快速交互**：0.15s (按钮悬停)
- **标准交互**：0.3s (卡片悬停、折叠展开)
- **进度更新**：0.5s (进度条变化)
- **页面切换**：0.3-0.5s (状态切换)

### 缓动函数
```css
cubic-bezier(0.4, 0, 0.2, 1)  /* 标准缓动 */
```

### 特殊动画
- **心跳脉冲**：animate-pulse (需要用户注意的状态)
- **背景装饰**：固定定位的模糊彩色圆圈

---

## 八、图标系统

### 图标来源
使用Heroicons v2，outline风格，保持统一的24px基础尺寸。

### 图标颜色规则
- **主图标**：gradient-text 或 text-tauri-primary
- **次要图标**：text-gray-400，hover时text-white
- **语义图标**：使用对应语义颜色（绿色成功、红色错误等）

---

## 九、响应式设计

### 断点
- **移动设备**：< 640px - 单列布局，边栏可收起
- **平板**：640px - 1024px - 两栏布局
- **桌面**：> 1024px - 完整三栏布局

### 适配策略
1. 先考虑桌面布局，然后适配小屏幕
2. 边栏在小屏幕上可收起/展开
3. 字体大小在小屏幕上相应缩放

---

## 十、可访问性

### 色彩对比度
确保文字与背景有足够对比度（WCAG AA标准）。

### 键盘导航
- 所有交互元素可通过tab键访问
- 支持enter和space触发按钮

### 焦点状态
清晰的focus状态，主要元素：focus:outline-none focus:border-tauri-primary

---

## 十一、性能优化建议

1. **使用CSS transition而非JS动画**：尽量使用CSS原生过渡
2. **虚拟滚动**：聊天记录列表使用虚拟滚动优化性能
3. **懒加载组件**：非首屏组件按需加载
4. **避免频繁重绘**：尽量减少大量元素同时动画

---

## 十二、开发规范

### 文件组织
```
frontend/src/
├── components/
│   ├── layout/           # 布局组件
│   ├── chat/             # 聊天相关组件
│   ├── common/           # 通用组件
│   └── icons/            # 图标组件
├── styles/               # 全局样式和工具
├── types/                # 类型定义
└── hooks/                # React hooks
```

### 组件命名
- 使用PascalCase命名组件文件
- 使用kebab-case命名CSS类
- 使用camelCase命名内部变量和函数

### 样式策略
1. 主要使用Tailwind CSS类
2. 复杂或复用样式使用@layer utilities
3. 尽量避免内联样式，除非动态计算

---

## 附录：完整颜色对照表

| 用途 | 颜色 | 色值 |
|------|------|------|
| 主色调 - 青蓝 | --tauri-primary | #24C8DB |
| 主色调 - 紫粉 | --tauri-secondary | #C084FC |
| 背景深色 | --tauri-dark | #020617 |
| 背景更深色 | --tauri-darker | #01030C |
| 卡片背景 | --tauri-card | #0F172A |
| 边框色 | --tauri-border | #1E293B |
