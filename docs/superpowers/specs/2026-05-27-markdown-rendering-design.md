# Markdown 渲染支持设计规格书

**日期：** 2026-05-27
**模块：** frontend
**范围：** AI 回复消息中的文本部分（`text` part）增加 Markdown 格式化渲染

---

## 1. 背景与动机

当前前端 `TextPart.tsx` 使用 `whitespace-pre-wrap` 直接渲染纯文本，AI 回复中的 Markdown 语法（如 `**粗体**`、`- 列表`、`# 标题` 等）以原始标记形式展示，可读性较差。用户希望在 AI 回复中看到这些 Markdown 被正确格式化为富文本。

后端已将围栏代码块（```）拆分为独立的 `code_block` part，因此 `TextPart` 中只需处理内联格式和结构元素。

---

## 2. 方案选择

采用**方案 A：前端集成 `react-markdown` + `remark-gfm`**。

- 功能完整，支持 GFM（表格、删除线、任务列表等）
- 实现直接，无需改动后端
- 与现有 `code_block` part 拆分逻辑不冲突
- 安全：`react-markdown` 默认过滤 raw HTML

---

## 3. 依赖

```bash
npm install react-markdown remark-gfm
```

| 包 | 版本 | 用途 |
|---|---|---|
| `react-markdown` | latest | Markdown → React 组件 |
| `remark-gfm` | latest | GFM 插件（表格、删除线、任务列表） |

---

## 4. 实现细节

### 4.1 TextPart.tsx 改造

将纯文本渲染替换为 `ReactMarkdown`：

```tsx
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { markdownComponents } from './markdownComponents';

// ...

<ReactMarkdown
  remarkPlugins={[remarkGfm]}
  components={markdownComponents}
  className="text-sm text-gray-200 break-words leading-relaxed"
>
  {displayText}
</ReactMarkdown>
```

### 4.2 Markdown 元素样式映射

所有样式通过 Tailwind 类名定义，与项目主题一致。

| 元素 | 样式 |
|------|------|
| `h1` | `text-xl font-bold text-gray-100 mt-4 mb-2 border-b border-tauri-border pb-1` |
| `h2` | `text-lg font-bold text-gray-100 mt-4 mb-2 border-b border-tauri-border pb-1` |
| `h3` | `text-base font-semibold text-gray-100 mt-3 mb-1.5` |
| `h4` | `text-sm font-semibold text-gray-100 mt-3 mb-1.5` |
| `p` | `my-2` |
| `ul` | `list-disc list-inside my-2 space-y-1` |
| `ol` | `list-decimal list-inside my-2 space-y-1` |
| `li` | `text-gray-200` |
| `li > p` | `inline`（消除列表项内段落的额外边距） |
| `a` | `text-tauri-primary hover:text-tauri-secondary underline transition-colors` |
| `strong` | `font-bold text-gray-100` |
| `em` | `italic text-gray-300` |
| `del` | `line-through text-gray-500` |
| `code` (inline) | `bg-tauri-dark/80 text-tauri-primary px-1.5 py-0.5 rounded text-xs font-mono` |
| `blockquote` | `border-l-4 border-tauri-primary/50 pl-4 py-1 my-2 bg-tauri-dark/30 rounded-r italic text-gray-300` |
| `table` | `w-full border-collapse my-3 text-xs` |
| `thead` | `bg-tauri-dark/60` |
| `th` | `border border-tauri-border px-3 py-2 text-left font-semibold text-gray-100` |
| `td` | `border border-tauri-border px-3 py-2 text-left` |
| `tr:nth-child(even)` | `bg-tauri-dark/20` |
| `hr` | `border-tauri-border my-4` |
| `pre` | `bg-tauri-dark/80 p-3 rounded-lg overflow-x-auto my-2 text-xs font-mono`（兜底处理，理论上 TextPart 不会遇到） |

### 4.3 折叠逻辑兼容

保留现有折叠机制，不做改动：

1. 基于**原始文本**判断是否需要折叠（行数 > `MAX_LINES` 或字符数 > `MAX_CHARS`）
2. 如需折叠，截断原始 Markdown 文本（取前 `MAX_LINES` 行）
3. 对截断后的文本调用 `ReactMarkdown` 渲染
4. 显示 "展开全部（{lines.length} 行）" 按钮，点击后渲染完整文本

> 截断 Markdown 可能导致列表、表格只显示一半，但这是可接受的——用户点击展开即可看到完整渲染结果。

---

## 5. 安全考虑

- `react-markdown` 默认**不允许 raw HTML**（`<script>`、`<iframe>` 等危险标签被自动过滤）
- 如需支持 raw HTML，需额外安装 `rehype-raw`，但本规格书**明确不推荐**
- 围栏代码块（```）已由后端拆分为独立 `code_block` part，TextPart 中理论上不会遇到。如遇到兜底情况，`pre` 组件仅做纯文本渲染，不执行高亮

---

## 6. 测试策略

1. **基础 Markdown**：验证 `# 标题`、`**粗体**`、`*斜体*`、`` `代码` ``、`[链接](url)` 是否正确渲染
2. **列表**：验证无序列表（`-` / `*`）和有序列表（`1.`）的缩进和间距
3. **表格**：GFM 表格语法是否正确渲染为表格
4. **任务列表**：`- [ ]` 和 `- [x]` 是否正确显示
5. **引用块**：`> 引用` 是否正确显示左边框
6. **折叠兼容**：验证超长 Markdown 文本的折叠/展开行为正常
7. **边界 case**：HTML 注入尝试（如 `<script>`）是否被过滤

---

## 7. 相关文件

- `frontend/src/components/part-renderers/TextPart.tsx` — 主要修改文件
- `frontend/src/components/part-renderers/markdownComponents.tsx` — 新增：Markdown 组件样式映射
- `frontend/package.json` — 添加依赖

---

## 8. 风险评估

| 风险 | 缓解措施 |
|------|---------|
| Bundle 体积增加 | `react-markdown` + `remark-gfm` 约增加 ~100KB gzipped，项目已有 `react-syntax-highlighter`，增量可接受 |
| 与现有折叠逻辑冲突 | 折叠基于原始文本行数判断，截断后再渲染，不影响 |
| 与 `code_block` part 重复渲染 | 后端已拆分围栏代码块，TextPart 中理论上不会遇到 `pre`，`pre` 组件仅做兜底纯文本渲染 |
| Markdown 截断导致不完整的标签 | 可接受，点击展开后能看到完整内容 |
