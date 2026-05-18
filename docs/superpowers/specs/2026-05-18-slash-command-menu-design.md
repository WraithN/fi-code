# Slash Command 自动补全菜单设计文档

## 需求概述

Web 前端输入框支持 slash (`/`) 指令自动补全：输入 `/` 后弹出指令菜单，支持键盘/鼠标选择，选中后填充到输入框。

## 核心决策

- 指令前缀：`/`
- 选择后行为：填充到输入框（如 `/models `），光标放末尾
- 指令列表来源：`GET /api/commands`，后端已有该 API

## 架构

```
AppLayout (mount)
  └── GET /api/commands ──▶ uiStore.commands

InputBox
  ├── 监听 textarea onChange/onKeyDown
  ├── 首字符为 '/' 时显示 SlashMenu
  ├── ↑↓ 导航高亮，Enter/Tab 确认，Esc 关闭
  └── 选中后 setInput('/command_name ')
```

## 组件改动

### 1. `uiStore.ts` — 新增 commands 状态
```typescript
commands: CommandMeta[];
setCommands: (commands: CommandMeta[]) => void;
```

### 2. `AppLayout.tsx` — 挂载时拉取指令
```typescript
useEffect(() => {
  apiClient.get<CommandMeta[]>('/api/commands')
    .then((cmds) => useUIStore.getState().setCommands(cmds))
    .catch(console.warn);
}, []);
```

### 3. `InputBox.tsx` — 核心交互逻辑
- `showMenu: boolean` — 是否显示菜单
- `highlightIndex: number` — 当前高亮项
- `filterText: string` — `/` 后的过滤文本
- 过滤逻辑：`commands.filter(c => c.name.startsWith(filterText))`
- 键盘事件：↑↓ Enter Tab Esc
- 菜单定位：绝对定位在 textarea 上方

### 4. 新增 `frontend/src/types/command.ts`
```typescript
export interface CommandMeta {
  name: string;
  description: string;
  args_hint: string | null;
}
```

## 交互细节

| 触发条件 | 行为 |
|----------|------|
| 输入框首字符为 `/` | 弹出菜单，展示全部指令 |
| 继续输入 `/mo` | 菜单过滤为 `models`、`init` 等匹配项 |
| ↑ / ↓ | 切换高亮项（循环） |
| Enter / Tab | 填充 `/command_name `，关闭菜单，聚焦输入框 |
| Esc | 关闭菜单 |
| Backspace 删除 `/` | 关闭菜单 |
| 鼠标点击菜单项 | 同 Enter |
| 输入框失焦 | 关闭菜单 |

## 样式

- 菜单宽度与输入框一致
- 背景 `bg-bg-secondary`，边框 `border-border`
- 高亮项 `bg-bg-overlay text-brand`
- 每项显示：指令名（加粗）+ 描述（灰色小字）+ 参数提示（可选）

## 安全/边界

- 若 `/api/commands` 失败，菜单为空，不影响正常输入
- 多行 textarea 中 `/` 不在首行时，不触发菜单（仅检测整个输入框首字符）
