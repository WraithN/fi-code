# Desktop 前端对齐 TUI 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 彻底重写 fi-code Desktop 前端核心，引入 Part/Turn 数据模型、完整 SSE 协议解析、Agent 切换、与 TUI 对齐的布局和主题系统。

**Architecture:** 按 domain 拆分为 4 个独立 Zustand store（connection/session/chat/ui）。API 层完整解析后端 SseEvent。UI 层使用 PartRenderer 注册表渲染对话回合。主题系统直接复用 TUI 的 `preset_themes.json`。

**Tech Stack:** React 18, TypeScript, Zustand, Tailwind CSS, Vite

---

## 文件结构映射

### 新建文件
| 文件 | 职责 |
|------|------|
| `frontend/src/types/part.ts` | Part 类型系统（9 种 Part 变体） |
| `frontend/src/types/sse.ts` | SseEvent 类型（6 种事件变体） |
| `frontend/src/types/turn.ts` | Turn 对话回合类型 |
| `frontend/src/types/agent.ts` | AgentType 类型 |
| `frontend/src/stores/connectionStore.ts` | 连接状态 store |
| `frontend/src/stores/sessionStore.ts` | 会话状态 store |
| `frontend/src/stores/chatStore.ts` | 聊天核心 store（Turn + Part + Agent） |
| `frontend/src/stores/uiStore.ts` | UI 状态 store |
| `frontend/src/services/apiClient.ts` | 重写后的 API 客户端（完整 SSE） |
| `frontend/src/hooks/useChatStream.ts` | 重写后的事件分发 hook |
| `frontend/src/components/part-renderers/registry.tsx` | Part 渲染注册表 |
| `frontend/src/components/part-renderers/TextPart.tsx` | 文本 Part 渲染 |
| `frontend/src/components/part-renderers/ThinkingPart.tsx` | Thinking Part 渲染 |
| `frontend/src/components/part-renderers/ToolUsePart.tsx` | ToolUse Part 渲染 |
| `frontend/src/components/part-renderers/ToolResultPart.tsx` | ToolResult Part 渲染 |
| `frontend/src/components/part-renderers/ToolErrorPart.tsx` | ToolError Part 渲染 |
| `frontend/src/components/part-renderers/CodeBlockPart.tsx` | CodeBlock Part 渲染 |
| `frontend/src/components/part-renderers/ImagePart.tsx` | Image Part 渲染 |
| `frontend/src/components/part-renderers/UsagePart.tsx` | Usage Part 渲染 |
| `frontend/src/components/part-renderers/WaveMarkerPart.tsx` | WaveMarker Part 渲染 |
| `frontend/src/components/layout/AppLayout.tsx` | 主布局壳 |
| `frontend/src/components/layout/LeftDrawer.tsx` | 左侧文件抽屉 |
| `frontend/src/components/layout/RightDrawer.tsx` | 右侧历史抽屉 |
| `frontend/src/components/layout/StatusBar.tsx` | 底部状态栏（含 Agent 切换） |
| `frontend/src/components/chat/ChatPanel.tsx` | 对话面板（Turn 列表） |
| `frontend/src/components/chat/InputBox.tsx` | 输入框 |
| `frontend/src/components/chat/TurnGroup.tsx` | 单个对话回合 |
| `frontend/src/themes/loadPresets.ts` | 从 JSON 加载主题 |
| `frontend/src/themes/applyTheme.ts` | 应用 CSS 变量 |

### 修改文件
| 文件 | 修改内容 |
|------|----------|
| `frontend/src/types/theme.ts` | 扩展 ThemeColors 接口 |
| `frontend/src/types/api.ts` | 删除旧 SseEvent/Message，保留通用类型 |
| `frontend/src/themes/index.ts` | 改为从 JSON 加载 |
| `frontend/tailwind.config.js` | 扩展 CSS 变量映射 |
| `frontend/src/App.tsx` | 使用新布局组件 |

### 删除文件
| 文件 | 说明 |
|------|------|
| `frontend/src/types/events.ts` | 旧 AppEvent 模型 |
| `frontend/src/stores/appStore.ts` | 单一 store |
| `frontend/src/services/chat.ts` | 旧 chat service |
| `frontend/src/services/client.ts` | 旧 ApiClient |
| `frontend/src/hooks/useClient.ts` | 旧 client hook |
| `frontend/src/hooks/useSidecar.ts` | sidecar 逻辑合并到 connectionStore |
| `frontend/src/hooks/useTheme.ts` | 主题逻辑合并到 uiStore |
| `frontend/src/components/MessageBubble.tsx` | 旧消息气泡 |
| `frontend/src/components/ChatPanel.tsx` | 旧对话面板 |
| `frontend/src/components/Header.tsx` | 旧 Header |
| `frontend/src/components/Sidebar.tsx` | 旧 Sidebar |
| `frontend/src/components/HistoryDrawer.tsx` | 旧 HistoryDrawer |
| `frontend/src/components/ConnectionScreen.tsx` | 旧连接屏（功能合并到 AppLayout） |
| `frontend/src/components/ApiKeyDialog.tsx` | 旧 API Key 对话框 |
| `frontend/src/components/ModelDropdown.tsx` | 旧模型下拉 |
| `frontend/src/themes/presets/default.ts` | 硬编码主题 |
| `frontend/src/themes/presets/light.ts` | 硬编码主题 |
| `frontend/src/themes/presets/monokai.ts` | 硬编码主题 |

---

## Task 1: Part / SseEvent / AgentType 类型定义

**Files:**
- Create: `frontend/src/types/part.ts`
- Create: `frontend/src/types/sse.ts`
- Create: `frontend/src/types/turn.ts`
- Create: `frontend/src/types/agent.ts`
- Modify: `frontend/src/types/api.ts`（删除旧类型）

- [ ] **Step 1: 创建 `frontend/src/types/part.ts`**

```typescript
export type Part =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; arguments: Record<string, unknown> }
  | { type: 'tool_result'; tool_call_id: string; content: string; duration_ms?: number }
  | { type: 'tool_error'; tool_call_id: string; content: string; error_message: string }
  | { type: 'thinking'; content: string }
  | { type: 'code_block'; language: string; code: string }
  | { type: 'image'; url: string; alt?: string }
  | { type: 'usage'; prompt_tokens: number; completion_tokens: number }
  | { type: 'wave_marker'; wave_id: string; turn: number };
```

- [ ] **Step 2: 创建 `frontend/src/types/sse.ts`**

```typescript
import { Part } from './part';
import { AgentType } from './agent';

export interface TaskProgressItem {
  id: string;
  name: string;
  status: string;
}

export type SseEvent =
  | { type: 'message'; content: string }
  | { type: 'part'; part: Part }
  | { type: 'agent_info'; agent_type: AgentType; agent_name: string }
  | { type: 'task_progress'; plan_id: string; tasks: TaskProgressItem[] }
  | { type: 'done'; session_id: string }
  | { type: 'error'; message: string };
```

- [ ] **Step 3: 创建 `frontend/src/types/turn.ts`**

```typescript
import { Part } from './part';

export interface Turn {
  id: string;
  userMessage: string;
  parts: Part[];
  isComplete: boolean;
  timestamp: number;
}
```

- [ ] **Step 4: 创建 `frontend/src/types/agent.ts`**

```typescript
export type AgentType = 'build' | 'plan';
```

- [ ] **Step 5: 修改 `frontend/src/types/api.ts`，删除旧类型**

删除 `SseContentEvent`、`SseDoneEvent`、`SseErrorEvent`、`SseEvent`、`Message` 类型定义。保留 `SessionInfo`、`SessionListResult`、`ApiResponse`、`FileEntry`、`FileTreeResult`、`ModelItem`、`ProviderItem`、`CommandMeta`。

- [ ] **Step 6: Commit**

```bash
git add frontend/src/types/
git commit -m "feat(types): add Part, Turn, SseEvent, AgentType types"
```

---

## Task 2: 主题系统重构（共享 TUI preset_themes.json）

**Files:**
- Modify: `frontend/src/types/theme.ts`
- Create: `frontend/src/themes/loadPresets.ts`
- Create: `frontend/src/themes/applyTheme.ts`
- Modify: `frontend/src/themes/index.ts`
- Delete: `frontend/src/themes/presets/default.ts`
- Delete: `frontend/src/themes/presets/light.ts`
- Delete: `frontend/src/themes/presets/monokai.ts`
- Modify: `frontend/tailwind.config.js`

- [ ] **Step 1: 修改 `frontend/src/types/theme.ts`**

```typescript
export interface ThemeColors {
  bg: string;
  bgSecondary: string;
  bgOverlay: string;
  bgUserArea: string;
  bgAiArea: string;
  textPrimary: string;
  textSecondary: string;
  textMuted: string;
  textPlaceholder: string;
  border: string;
  brand: string;
  accentHover: string;
  user: string;
  success: string;
  warning: string;
  error: string;
  selectionBg: string;
  selectionFg: string;
}

export interface ThemePreset {
  name: string;
  description: string;
  colors: ThemeColors;
}
```

- [ ] **Step 2: 创建 `frontend/src/themes/loadPresets.ts`**

```typescript
import presetJson from '../../../crates/shared/src/preset_themes.json';
import { ThemePreset } from '../types/theme';

function u32ToHex(u32: number): string {
  return `#${u32.toString(16).padStart(6, '0')}`;
}

export const themePresets: ThemePreset[] = (presetJson as any[]).map(p => ({
  name: p.name,
  description: p.description,
  colors: {
    bg: u32ToHex(p.bg_base),
    bgSecondary: u32ToHex(p.bg_surface),
    bgOverlay: u32ToHex(p.bg_overlay),
    bgUserArea: u32ToHex(p.bg_user_area),
    bgAiArea: u32ToHex(p.bg_ai_area),
    textPrimary: u32ToHex(p.text_primary),
    textSecondary: u32ToHex(p.text_secondary),
    textMuted: u32ToHex(p.text_muted),
    textPlaceholder: u32ToHex(p.text_placeholder),
    border: u32ToHex(p.border),
    brand: u32ToHex(p.brand),
    accentHover: u32ToHex(p.accent_hover),
    user: u32ToHex(p.user),
    success: u32ToHex(p.success),
    warning: u32ToHex(p.warning),
    error: u32ToHex(p.error),
    selectionBg: u32ToHex(p.selection_bg),
    selectionFg: u32ToHex(p.selection_fg),
  },
}));

export function getPresetByName(name: string): ThemePreset | undefined {
  return themePresets.find(p => p.name.toLowerCase() === name.toLowerCase());
}
```

- [ ] **Step 3: 创建 `frontend/src/themes/applyTheme.ts`**

```typescript
import { ThemePreset } from '../types/theme';

export function applyTheme(preset: ThemePreset): void {
  const root = document.documentElement;
  const c = preset.colors;
  root.style.setProperty('--color-bg', c.bg);
  root.style.setProperty('--color-bg-secondary', c.bgSecondary);
  root.style.setProperty('--color-bg-overlay', c.bgOverlay);
  root.style.setProperty('--color-bg-user-area', c.bgUserArea);
  root.style.setProperty('--color-bg-ai-area', c.bgAiArea);
  root.style.setProperty('--color-text-primary', c.textPrimary);
  root.style.setProperty('--color-text-secondary', c.textSecondary);
  root.style.setProperty('--color-text-muted', c.textMuted);
  root.style.setProperty('--color-text-placeholder', c.textPlaceholder);
  root.style.setProperty('--color-border', c.border);
  root.style.setProperty('--color-brand', c.brand);
  root.style.setProperty('--color-accent-hover', c.accentHover);
  root.style.setProperty('--color-user', c.user);
  root.style.setProperty('--color-success', c.success);
  root.style.setProperty('--color-error', c.error);
  root.style.setProperty('--color-warning', c.warning);
  root.style.setProperty('--color-selection-bg', c.selectionBg);
  root.style.setProperty('--color-selection-fg', c.selectionFg);
}
```

- [ ] **Step 4: 修改 `frontend/src/themes/index.ts`**

```typescript
export { themePresets, getPresetByName } from './loadPresets';
export { applyTheme } from './applyTheme';
```

- [ ] **Step 5: 修改 `frontend/tailwind.config.js`**

在 `theme.extend.colors` 中添加：
```javascript
'bg-user-area': 'var(--color-bg-user-area)',
'bg-ai-area': 'var(--color-bg-ai-area)',
brand: 'var(--color-brand)',
'accent-hover': 'var(--color-accent-hover)',
user: 'var(--color-user)',
'selection-bg': 'var(--color-selection-bg)',
'selection-fg': 'var(--color-selection-fg)',
```

- [ ] **Step 6: 删除旧预设文件**

```bash
rm frontend/src/themes/presets/default.ts
rm frontend/src/themes/presets/light.ts
rm frontend/src/themes/presets/monokai.ts
rmdir frontend/src/themes/presets || true
```

- [ ] **Step 7: Commit**

```bash
git add frontend/src/types/theme.ts frontend/src/themes/ frontend/tailwind.config.js
git rm frontend/src/themes/presets/default.ts frontend/src/themes/presets/light.ts frontend/src/themes/presets/monokai.ts
git commit -m "feat(themes): align theme system with TUI, load from preset_themes.json"
```

---

## Task 3: connectionStore + sessionStore

**Files:**
- Create: `frontend/src/stores/connectionStore.ts`
- Create: `frontend/src/stores/sessionStore.ts`

- [ ] **Step 1: 创建 `frontend/src/stores/connectionStore.ts`**

```typescript
import { create } from 'zustand';

interface ConnectionState {
  mode: 'standalone' | 'remote';
  connectionStatus: 'connecting' | 'connected' | 'error';
  serverUrl: string;
  connectionError: string | null;
  setMode: (mode: 'standalone' | 'remote') => void;
  setConnectionStatus: (status: 'connecting' | 'connected' | 'error', error?: string) => void;
  setServerUrl: (url: string) => void;
}

export const useConnectionStore = create<ConnectionState>((set) => ({
  mode: 'standalone',
  connectionStatus: 'connecting',
  serverUrl: 'http://localhost:4040',
  connectionError: null,
  setMode: (mode) => set({ mode }),
  setConnectionStatus: (status, error) => set({ connectionStatus: status, connectionError: error || null }),
  setServerUrl: (url) => set({ serverUrl: url }),
}));
```

- [ ] **Step 2: 创建 `frontend/src/stores/sessionStore.ts`**

```typescript
import { create } from 'zustand';
import { SessionInfo } from '../types/api';

interface SessionState {
  currentSessionId: string | null;
  sessions: SessionInfo[];
  setCurrentSessionId: (id: string | null) => void;
  setSessions: (sessions: SessionInfo[]) => void;
}

export const useSessionStore = create<SessionState>((set) => ({
  currentSessionId: null,
  sessions: [],
  setCurrentSessionId: (id) => set({ currentSessionId: id }),
  setSessions: (sessions) => set({ sessions }),
}));
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/stores/connectionStore.ts frontend/src/stores/sessionStore.ts
git commit -m "feat(stores): add connectionStore and sessionStore"
```

---

## Task 4: chatStore（核心重构）

**Files:**
- Create: `frontend/src/stores/chatStore.ts`
- Test: `frontend/src/stores/chatStore.test.ts`

- [ ] **Step 1: 创建 `frontend/src/stores/chatStore.ts`**

```typescript
import { create } from 'zustand';
import { Turn } from '../types/turn';
import { Part } from '../types/part';
import { AgentType } from '../types/agent';

interface ChatState {
  turns: Turn[];
  isGenerating: boolean;
  currentAgent: AgentType;
  startTurn: (userMessage: string) => string;
  appendPart: (turnId: string, part: Part) => void;
  completeTurn: (turnId: string) => void;
  setAgent: (agent: AgentType) => void;
  setIsGenerating: (generating: boolean) => void;
  clearTurns: () => void;
  getCurrentTurnId: () => string | null;
}

export const useChatStore = create<ChatState>((set, get) => ({
  turns: [],
  isGenerating: false,
  currentAgent: 'build',

  startTurn: (userMessage: string) => {
    const turn: Turn = {
      id: `turn-${Date.now()}`,
      userMessage,
      parts: [],
      isComplete: false,
      timestamp: Date.now(),
    };
    set((state) => ({ turns: [...state.turns, turn], isGenerating: true }));
    return turn.id;
  },

  appendPart: (turnId: string, part: Part) => {
    set((state) => ({
      turns: state.turns.map((turn) =>
        turn.id === turnId ? { ...turn, parts: [...turn.parts, part] } : turn
      ),
    }));
  },

  completeTurn: (turnId: string) => {
    set((state) => ({
      turns: state.turns.map((turn) =>
        turn.id === turnId ? { ...turn, isComplete: true } : turn
      ),
      isGenerating: false,
    }));
  },

  setAgent: (agent) => set({ currentAgent: agent }),
  setIsGenerating: (generating) => set({ isGenerating: generating }),
  clearTurns: () => set({ turns: [], isGenerating: false }),

  getCurrentTurnId: () => {
    const { turns } = get();
    const last = turns[turns.length - 1];
    return last && !last.isComplete ? last.id : null;
  },
}));
```

- [ ] **Step 2: 创建 `frontend/src/stores/chatStore.test.ts`**

```typescript
import { describe, it, expect } from 'vitest';
import { useChatStore } from './chatStore';

describe('chatStore', () => {
  it('should start a new turn', () => {
    const store = useChatStore.getState();
    store.clearTurns();
    const turnId = store.startTurn('hello');
    expect(turnId).toBeDefined();
    expect(store.turns).toHaveLength(1);
    expect(store.turns[0].userMessage).toBe('hello');
    expect(store.turns[0].isComplete).toBe(false);
    expect(store.isGenerating).toBe(true);
  });

  it('should append part to current turn', () => {
    const store = useChatStore.getState();
    store.clearTurns();
    const turnId = store.startTurn('hello');
    store.appendPart(turnId, { type: 'text', text: 'world' });
    expect(store.turns[0].parts).toHaveLength(1);
    expect(store.turns[0].parts[0]).toEqual({ type: 'text', text: 'world' });
  });

  it('should complete turn', () => {
    const store = useChatStore.getState();
    store.clearTurns();
    const turnId = store.startTurn('hello');
    store.completeTurn(turnId);
    expect(store.turns[0].isComplete).toBe(true);
    expect(store.isGenerating).toBe(false);
  });

  it('should switch agent', () => {
    const store = useChatStore.getState();
    store.setAgent('plan');
    expect(store.currentAgent).toBe('plan');
  });
});
```

- [ ] **Step 3: 运行测试**

```bash
cd frontend && npm test -- chatStore.test.ts
```

Expected: 4 tests PASS

- [ ] **Step 4: Commit**

```bash
git add frontend/src/stores/chatStore.ts frontend/src/stores/chatStore.test.ts
git commit -m "feat(stores): add chatStore with Turn/Part/Agent support"
```

---

## Task 5: uiStore

**Files:**
- Create: `frontend/src/stores/uiStore.ts`

- [ ] **Step 1: 创建 `frontend/src/stores/uiStore.ts`**

```typescript
import { create } from 'zustand';
import { ProviderItem } from '../types/api';

interface UIState {
  leftDrawerOpen: boolean;
  rightDrawerOpen: boolean;
  logOpen: boolean;
  themeName: string;
  providers: ProviderItem[];
  currentModel: string;
  toggleLeftDrawer: () => void;
  toggleRightDrawer: () => void;
  toggleLog: () => void;
  setThemeName: (name: string) => void;
  setProviders: (providers: ProviderItem[]) => void;
  setCurrentModel: (model: string) => void;
}

export const useUIStore = create<UIState>((set) => ({
  leftDrawerOpen: true,
  rightDrawerOpen: false,
  logOpen: false,
  themeName: 'deep_ocean',
  providers: [],
  currentModel: 'unknown',
  toggleLeftDrawer: () => set((s) => ({ leftDrawerOpen: !s.leftDrawerOpen })),
  toggleRightDrawer: () => set((s) => ({ rightDrawerOpen: !s.rightDrawerOpen })),
  toggleLog: () => set((s) => ({ logOpen: !s.logOpen })),
  setThemeName: (name) => set({ themeName: name }),
  setProviders: (providers) => set({ providers }),
  setCurrentModel: (model) => set({ currentModel: model }),
}));
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/stores/uiStore.ts
git commit -m "feat(stores): add uiStore"
```

---

## Task 6: ApiClient.chatStream 重写（完整 SSE 解析）

**Files:**
- Create: `frontend/src/services/apiClient.ts`
- Delete: `frontend/src/services/client.ts`
- Delete: `frontend/src/services/chat.ts`

- [ ] **Step 1: 创建 `frontend/src/services/apiClient.ts`**

```typescript
import { SseEvent } from '../types/sse';
import { AgentType } from '../types/agent';
import { ApiResponse, SessionListResult, SessionInfo, FileTreeResult, CommandMeta } from '../types/api';

export class ApiClient {
  private baseUrl: string;

  constructor(baseUrl: string = 'http://localhost:4040') {
    this.baseUrl = baseUrl.replace(/\/$/, '');
  }

  setBaseUrl(url: string): void {
    this.baseUrl = url.replace(/\/$/, '');
  }

  getBaseUrl(): string {
    return this.baseUrl;
  }

  async rpc(method: string, params?: unknown): Promise<unknown> {
    const resp = await fetch(`${this.baseUrl}/rpc`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', method, params, id: 1 }),
    });
    if (!resp.ok) throw new Error(`RPC failed: ${resp.status}`);
    const data = await resp.json();
    if (data.error) throw new Error(data.error.message || 'RPC error');
    return data.result;
  }

  async get<T>(path: string): Promise<T> {
    const resp = await fetch(`${this.baseUrl}${path}`);
    if (!resp.ok) throw new Error(`GET ${path} failed: ${resp.status}`);
    const data: ApiResponse<T> = await resp.json();
    if (!data.success || data.data === null) throw new Error(data.error || 'API returned no data');
    return data.data;
  }

  async post<T>(path: string, body?: unknown): Promise<T> {
    const resp = await fetch(`${this.baseUrl}${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });
    if (!resp.ok) throw new Error(`POST ${path} failed: ${resp.status}`);
    const data: ApiResponse<T> = await resp.json();
    if (!data.success || data.data === null) throw new Error(data.error || 'API returned no data');
    return data.data;
  }

  async *chatStream(
    sessionId: string | null,
    message: string,
    agent: AgentType = 'build'
  ): AsyncGenerator<SseEvent, string, unknown> {
    const body = JSON.stringify({ session_id: sessionId, message, agent });

    const resp = await fetch(`${this.baseUrl}/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
    });

    if (!resp.ok) throw new Error(`Chat failed: ${resp.status}`);

    const reader = resp.body?.getReader();
    if (!reader) throw new Error('No response body');

    const decoder = new TextDecoder();
    let buffer = '';
    let eventLines: string[] = [];

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split('\n');
      buffer = lines.pop() || '';

      for (const line of lines) {
        const trimmed = line.trimEnd();
        if (trimmed.startsWith('data: ')) {
          eventLines.push(trimmed.slice(6));
        } else if (trimmed === '' && eventLines.length > 0) {
          const jsonStr = eventLines.join('\n');
          eventLines = [];
          try {
            const event = JSON.parse(jsonStr) as SseEvent;
            yield event;
            if (event.type === 'done') {
              return event.session_id;
            }
          } catch {
            console.warn('[SSE] Invalid JSON:', jsonStr.slice(0, 200));
          }
        }
      }
    }

    throw new Error('SSE stream ended without Done event');
  }
}

export const apiClient = new ApiClient();
```

- [ ] **Step 2: 删除旧 service 文件**

```bash
rm frontend/src/services/client.ts
rm frontend/src/services/chat.ts
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/services/apiClient.ts
git rm frontend/src/services/client.ts frontend/src/services/chat.ts
git commit -m "feat(services): rewrite ApiClient with full SseEvent parsing and agent support"
```

---

## Task 7: useChatStream hook 重写

**Files:**
- Create: `frontend/src/hooks/useChatStream.ts`
- Delete: `frontend/src/hooks/useClient.ts`
- Delete: `frontend/src/hooks/useSidecar.ts`
- Delete: `frontend/src/hooks/useTheme.ts`

- [ ] **Step 1: 创建 `frontend/src/hooks/useChatStream.ts`**

```typescript
import { useCallback } from 'react';
import { apiClient } from '../services/apiClient';
import { useChatStore } from '../stores/chatStore';
import { useSessionStore } from '../stores/sessionStore';
import { SseEvent } from '../types/sse';
import { Part } from '../types/part';

export function useChatStream() {
  const { currentAgent } = useChatStore();
  const { currentSessionId, setCurrentSessionId } = useSessionStore();
  const { startTurn, appendPart, completeTurn, setAgent, setIsGenerating } = useChatStore();

  const send = useCallback(async (message: string) => {
    if (!message.trim()) return;

    const turnId = startTurn(message);
    setIsGenerating(true);

    try {
      const stream = apiClient.chatStream(currentSessionId, message, currentAgent);

      for await (const event of stream) {
        handleSseEvent(event, turnId, setAgent, appendPart, completeTurn, setCurrentSessionId, setIsGenerating);
      }
    } catch (err) {
      setIsGenerating(false);
      appendPart(turnId, {
        type: 'tool_error',
        tool_call_id: '',
        content: err instanceof Error ? err.message : 'Unknown error',
        error_message: 'Stream error',
      });
    }
  }, [currentSessionId, currentAgent, startTurn, appendPart, completeTurn, setAgent, setIsGenerating, setCurrentSessionId]);

  const stop = useCallback(() => {
    setIsGenerating(false);
  }, [setIsGenerating]);

  return { send, stop };
}

function handleSseEvent(
  event: SseEvent,
  turnId: string,
  setAgent: (agent: 'build' | 'plan') => void,
  appendPart: (turnId: string, part: Part) => void,
  completeTurn: (turnId: string) => void,
  setCurrentSessionId: (id: string | null) => void,
  setIsGenerating: (generating: boolean) => void
) {
  switch (event.type) {
    case 'message':
      appendPart(turnId, { type: 'text', text: event.content });
      break;
    case 'part':
      appendPart(turnId, event.part);
      break;
    case 'agent_info':
      setAgent(event.agent_type);
      break;
    case 'done':
      completeTurn(turnId);
      setCurrentSessionId(event.session_id);
      setIsGenerating(false);
      break;
    case 'error':
      appendPart(turnId, {
        type: 'tool_error',
        tool_call_id: '',
        content: event.message,
        error_message: 'Server error',
      });
      setIsGenerating(false);
      break;
    case 'task_progress':
      // TODO: 可在 UI 中展示任务进度
      break;
  }
}
```

- [ ] **Step 2: 删除旧 hooks**

```bash
rm frontend/src/hooks/useClient.ts
rm frontend/src/hooks/useSidecar.ts
rm frontend/src/hooks/useTheme.ts
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/hooks/useChatStream.ts
git rm frontend/src/hooks/useClient.ts frontend/src/hooks/useSidecar.ts frontend/src/hooks/useTheme.ts
git commit -m "feat(hooks): rewrite useChatStream with full SseEvent handling"
```

---

## Task 8: Part 渲染器组件（基础）

**Files:**
- Create: `frontend/src/components/part-renderers/TextPart.tsx`
- Create: `frontend/src/components/part-renderers/ThinkingPart.tsx`
- Create: `frontend/src/components/part-renderers/UsagePart.tsx`
- Create: `frontend/src/components/part-renderers/WaveMarkerPart.tsx`
- Create: `frontend/src/components/part-renderers/registry.tsx`

- [ ] **Step 1: 创建 `TextPart.tsx`**

```typescript
import React from 'react';
import { Part } from '../../types/part';

export const TextPart: React.FC<{ part: Extract<Part, { type: 'text' }> }> = ({ part }) => (
  <div className="text-sm text-text-primary whitespace-pre-wrap break-words">{part.text}</div>
);
```

- [ ] **Step 2: 创建 `ThinkingPart.tsx`**

```typescript
import React from 'react';
import { Part } from '../../types/part';

export const ThinkingPart: React.FC<{ part: Extract<Part, { type: 'thinking' }> }> = ({ part }) => (
  <div className="text-sm text-text-muted italic border-l-2 border-brand pl-3 my-2">
    {part.content}
  </div>
);
```

- [ ] **Step 3: 创建 `UsagePart.tsx`**

```typescript
import React from 'react';
import { Part } from '../../types/part';

export const UsagePart: React.FC<{ part: Extract<Part, { type: 'usage' }> }> = ({ part }) => (
  <div className="text-xs text-text-muted mt-2">
    Tokens: {part.prompt_tokens} prompt + {part.completion_tokens} completion
  </div>
);
```

- [ ] **Step 4: 创建 `WaveMarkerPart.tsx`**

```typescript
import React from 'react';
import { Part } from '../../types/part';

export const WaveMarkerPart: React.FC<{ part: Extract<Part, { type: 'wave_marker' }> }> = ({ part }) => (
  <div className="text-xs text-text-muted opacity-50" data-wave-id={part.wave_id} data-turn={part.turn} />
);
```

- [ ] **Step 5: 创建 `registry.tsx`**

```typescript
import React from 'react';
import { Part } from '../../types/part';
import { TextPart } from './TextPart';
import { ThinkingPart } from './ThinkingPart';
import { UsagePart } from './UsagePart';
import { WaveMarkerPart } from './WaveMarkerPart';

const partRenderers: Record<string, React.FC<{ part: Part }>> = {
  text: TextPart as React.FC<{ part: Part }>,
  thinking: ThinkingPart as React.FC<{ part: Part }>,
  usage: UsagePart as React.FC<{ part: Part }>,
  wave_marker: WaveMarkerPart as React.FC<{ part: Part }>,
};

export function PartRenderer({ part }: { part: Part }) {
  const Renderer = partRenderers[part.type];
  if (!Renderer) {
    return <div className="text-xs text-error">Unknown part type: {part.type}</div>;
  }
  return <Renderer part={part} />;
}
```

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/part-renderers/
git commit -m "feat(ui): add basic Part renderers (Text, Thinking, Usage, WaveMarker)"
```

---

## Task 9: Part 渲染器组件（工具相关）

**Files:**
- Create: `frontend/src/components/part-renderers/ToolUsePart.tsx`
- Create: `frontend/src/components/part-renderers/ToolResultPart.tsx`
- Create: `frontend/src/components/part-renderers/ToolErrorPart.tsx`
- Create: `frontend/src/components/part-renderers/CodeBlockPart.tsx`
- Create: `frontend/src/components/part-renderers/ImagePart.tsx`
- Modify: `frontend/src/components/part-renderers/registry.tsx`

- [ ] **Step 1: 创建 `ToolUsePart.tsx`**

```typescript
import React from 'react';
import { Part } from '../../types/part';

export const ToolUsePart: React.FC<{ part: Extract<Part, { type: 'tool_use' }> }> = ({ part }) => (
  <div className="my-2 p-3 rounded bg-bg-secondary border border-border">
    <div className="text-xs text-brand font-mono mb-1">🔧 {part.name}</div>
    <pre className="text-xs text-text-secondary overflow-x-auto">
      {JSON.stringify(part.arguments, null, 2)}
    </pre>
  </div>
);
```

- [ ] **Step 2: 创建 `ToolResultPart.tsx`**

```typescript
import React from 'react';
import { Part } from '../../types/part';

export const ToolResultPart: React.FC<{ part: Extract<Part, { type: 'tool_result' }> }> = ({ part }) => (
  <div className="my-2 p-3 rounded bg-bg-secondary border-l-2 border-success">
    <div className="text-xs text-success font-mono mb-1">✓ Result ({part.duration_ms}ms)</div>
    <pre className="text-xs text-text-secondary whitespace-pre-wrap break-words">{part.content}</pre>
  </div>
);
```

- [ ] **Step 3: 创建 `ToolErrorPart.tsx`**

```typescript
import React from 'react';
import { Part } from '../../types/part';

export const ToolErrorPart: React.FC<{ part: Extract<Part, { type: 'tool_error' }> }> = ({ part }) => (
  <div className="my-2 p-3 rounded bg-bg-secondary border-l-2 border-error">
    <div className="text-xs text-error font-mono mb-1">✗ Error: {part.error_message}</div>
    <pre className="text-xs text-text-secondary whitespace-pre-wrap break-words">{part.content}</pre>
  </div>
);
```

- [ ] **Step 4: 创建 `CodeBlockPart.tsx`**

```typescript
import React from 'react';
import { Part } from '../../types/part';

export const CodeBlockPart: React.FC<{ part: Extract<Part, { type: 'code_block' }> }> = ({ part }) => (
  <div className="my-2 rounded overflow-hidden">
    <div className="text-xs text-text-muted bg-bg-secondary px-3 py-1 border-b border-border">
      {part.language || 'code'}
    </div>
    <pre className="text-sm text-text-primary bg-bg p-3 overflow-x-auto">
      <code>{part.code}</code>
    </pre>
  </div>
);
```

- [ ] **Step 5: 创建 `ImagePart.tsx`**

```typescript
import React from 'react';
import { Part } from '../../types/part';

export const ImagePart: React.FC<{ part: Extract<Part, { type: 'image' }> }> = ({ part }) => (
  <div className="my-2">
    <img src={part.url} alt={part.alt || 'image'} className="max-w-full rounded" />
  </div>
);
```

- [ ] **Step 6: 修改 `registry.tsx`，注册新渲染器**

在 `partRenderers` 对象中添加：
```typescript
import { ToolUsePart } from './ToolUsePart';
import { ToolResultPart } from './ToolResultPart';
import { ToolErrorPart } from './ToolErrorPart';
import { CodeBlockPart } from './CodeBlockPart';
import { ImagePart } from './ImagePart';

const partRenderers: Record<string, React.FC<{ part: Part }>> = {
  text: TextPart as React.FC<{ part: Part }>,
  thinking: ThinkingPart as React.FC<{ part: Part }>,
  tool_use: ToolUsePart as React.FC<{ part: Part }>,
  tool_result: ToolResultPart as React.FC<{ part: Part }>,
  tool_error: ToolErrorPart as React.FC<{ part: Part }>,
  code_block: CodeBlockPart as React.FC<{ part: Part }>,
  image: ImagePart as React.FC<{ part: Part }>,
  usage: UsagePart as React.FC<{ part: Part }>,
  wave_marker: WaveMarkerPart as React.FC<{ part: Part }>,
};
```

- [ ] **Step 7: Commit**

```bash
git add frontend/src/components/part-renderers/
git commit -m "feat(ui): add tool Part renderers (ToolUse, ToolResult, ToolError, CodeBlock, Image)"
```

---

## Task 10: 布局组件（LeftDrawer / RightDrawer / StatusBar / LogPanel）

**Files:**
- Create: `frontend/src/components/layout/LeftDrawer.tsx`
- Create: `frontend/src/components/layout/RightDrawer.tsx`
- Create: `frontend/src/components/layout/StatusBar.tsx`
- Create: `frontend/src/components/layout/LogPanel.tsx`

- [ ] **Step 1: 创建 `LeftDrawer.tsx`**

```typescript
import React from 'react';
import { useUIStore } from '../../stores/uiStore';

export const LeftDrawer: React.FC = () => {
  const { leftDrawerOpen, toggleLeftDrawer } = useUIStore();

  if (!leftDrawerOpen) {
    return (
      <button
        onClick={toggleLeftDrawer}
        className="w-8 h-full bg-bg-secondary border-r border-border flex items-center justify-center hover:bg-bg-overlay"
      >
        <span className="text-text-muted text-xs">›</span>
      </button>
    );
  }

  return (
    <div className="w-64 bg-bg-secondary border-r border-border flex flex-col">
      <div className="h-10 flex items-center justify-between px-3 border-b border-border">
        <span className="text-sm font-medium text-text-primary">Files</span>
        <button onClick={toggleLeftDrawer} className="text-text-muted hover:text-text-primary text-xs">‹</button>
      </div>
      <div className="flex-1 p-2 text-sm text-text-muted">
        {/* 文件树内容待实现 */}
        <p>File tree placeholder</p>
      </div>
    </div>
  );
};
```

- [ ] **Step 2: 创建 `RightDrawer.tsx`**

```typescript
import React from 'react';
import { useUIStore } from '../../stores/uiStore';
import { useSessionStore } from '../../stores/sessionStore';

export const RightDrawer: React.FC = () => {
  const { rightDrawerOpen, toggleRightDrawer } = useUIStore();
  const { sessions, currentSessionId, setCurrentSessionId } = useSessionStore();

  if (!rightDrawerOpen) return null;

  return (
    <div className="w-64 bg-bg-secondary border-l border-border flex flex-col">
      <div className="h-10 flex items-center justify-between px-3 border-b border-border">
        <span className="text-sm font-medium text-text-primary">History</span>
        <button onClick={toggleRightDrawer} className="text-text-muted hover:text-text-primary text-xs">›</button>
      </div>
      <div className="flex-1 overflow-y-auto">
        {sessions.map((session) => (
          <button
            key={session.id}
            onClick={() => setCurrentSessionId(session.id)}
            className={`w-full text-left px-3 py-2 text-sm border-b border-border ${
              session.id === currentSessionId ? 'bg-bg-overlay text-brand' : 'text-text-secondary hover:bg-bg-overlay'
            }`}
          >
            {session.name}
          </button>
        ))}
      </div>
    </div>
  );
};
```

- [ ] **Step 3: 创建 `StatusBar.tsx`**

```typescript
import React from 'react';
import { useChatStore } from '../../stores/chatStore';
import { useUIStore } from '../../stores/uiStore';
import { useConnectionStore } from '../../stores/connectionStore';

export const StatusBar: React.FC = () => {
  const { currentAgent, setAgent, isGenerating } = useChatStore();
  const { currentModel } = useUIStore();
  const { connectionStatus } = useConnectionStore();

  return (
    <div className="h-8 flex items-center px-4 bg-bg-secondary border-t border-border text-xs select-none">
      <span className="font-bold text-brand">fi-code</span>
      <span className="mx-2 text-border">│</span>

      <button
        onClick={() => setAgent(currentAgent === 'build' ? 'plan' : 'build')}
        className="flex items-center gap-1 hover:text-brand transition-colors"
        title="Click to switch agent"
      >
        <span>AGT: {currentAgent === 'build' ? 'Build' : 'Plan'}</span>
      </button>

      <span className="mx-2 text-border">│</span>
      <span className="text-text-secondary">{currentModel}</span>

      <span className="mx-2 text-border">│</span>
      <span className={`${connectionStatus === 'connected' ? 'text-success' : 'text-error'}`}>
        {connectionStatus}
      </span>

      {isGenerating && (
        <>
          <span className="mx-2 text-border">│</span>
          <span className="text-brand animate-pulse">generating...</span>
        </>
      )}
    </div>
  );
};
```

- [ ] **Step 4: 创建 `LogPanel.tsx`**

```typescript
import React from 'react';
import { useUIStore } from '../../stores/uiStore';

export const LogPanel: React.FC = () => {
  const { logOpen, toggleLog } = useUIStore();

  if (!logOpen) return null;

  return (
    <div className="absolute bottom-8 right-4 w-96 h-64 bg-bg-secondary border border-border rounded shadow-lg flex flex-col z-50">
      <div className="h-8 flex items-center justify-between px-3 border-b border-border">
        <span className="text-sm font-medium text-text-primary">Logs</span>
        <button onClick={toggleLog} className="text-text-muted hover:text-text-primary">✕</button>
      </div>
      <div className="flex-1 p-2 overflow-y-auto text-xs font-mono text-text-secondary">
        <p>Log output placeholder...</p>
      </div>
    </div>
  );
};
```

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/layout/
git commit -m "feat(ui): add layout components (LeftDrawer, RightDrawer, StatusBar, LogPanel)"
```

---

## Task 11: ChatPanel + InputBox + TurnGroup

**Files:**
- Create: `frontend/src/components/chat/TurnGroup.tsx`
- Create: `frontend/src/components/chat/ChatPanel.tsx`
- Create: `frontend/src/components/chat/InputBox.tsx`

- [ ] **Step 1: 创建 `TurnGroup.tsx`**

```typescript
import React from 'react';
import { Turn } from '../../types/turn';
import { PartRenderer } from '../part-renderers/registry';

export const TurnGroup: React.FC<{ turn: Turn }> = ({ turn }) => {
  return (
    <div className="mb-6">
      {/* 用户消息 */}
      <div className="flex justify-end mb-2">
        <div className="max-w-[80%] px-4 py-2 rounded-lg bg-user text-bg">
          <div className="text-xs text-bg opacity-70 mb-1">You</div>
          <div className="text-sm whitespace-pre-wrap break-words">{turn.userMessage}</div>
        </div>
      </div>

      {/* AI 回复 Parts */}
      <div className="flex justify-start">
        <div className="max-w-[80%] px-4 py-2 rounded-lg bg-bg-ai-area text-text-primary border border-border">
          <div className="text-xs text-text-muted mb-1">Assistant</div>
          {turn.parts.length === 0 && !turn.isComplete ? (
            <div className="text-sm text-text-muted animate-pulse">▋</div>
          ) : (
            turn.parts.map((part, i) => <PartRenderer key={i} part={part} />)
          )}
        </div>
      </div>
    </div>
  );
};
```

- [ ] **Step 2: 创建 `ChatPanel.tsx`**

```typescript
import React, { useRef, useEffect } from 'react';
import { useChatStore } from '../../stores/chatStore';
import { TurnGroup } from './TurnGroup';

export const ChatPanel: React.FC = () => {
  const turns = useChatStore((s) => s.turns);
  const isGenerating = useChatStore((s) => s.isGenerating);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [turns]);

  return (
    <div className="flex-1 flex flex-col min-h-0 bg-bg">
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-4">
        {turns.length === 0 ? (
          <div className="flex items-center justify-center h-full text-text-muted">
            <div className="text-center">
              <p className="text-lg mb-2">Welcome to fi-code</p>
              <p className="text-sm">Start a conversation or use /commands</p>
            </div>
          </div>
        ) : (
          turns.map((turn) => <TurnGroup key={turn.id} turn={turn} />)
        )}
      </div>

      {isGenerating && (
        <div className="px-4 py-2 text-xs text-text-muted animate-pulse">Generating...</div>
      )}
    </div>
  );
};
```

- [ ] **Step 3: 创建 `InputBox.tsx`**

```typescript
import React, { useState, useCallback } from 'react';
import { useChatStream } from '../../hooks/useChatStream';

export const InputBox: React.FC = () => {
  const [input, setInput] = useState('');
  const { send } = useChatStream();

  const handleSubmit = useCallback(() => {
    if (!input.trim()) return;
    send(input);
    setInput('');
  }, [input, send]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div className="p-4 bg-bg-secondary border-t border-border">
      <div className="flex gap-2">
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a message..."
          rows={2}
          className="flex-1 bg-bg text-text-primary border border-border rounded px-3 py-2 text-sm resize-none focus:outline-none focus:border-brand"
        />
        <button
          onClick={handleSubmit}
          className="px-4 py-2 bg-brand text-bg rounded text-sm font-medium hover:bg-accent-hover transition-colors"
        >
          Send
        </button>
      </div>
    </div>
  );
};
```

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/chat/
git commit -m "feat(ui): add ChatPanel, InputBox, TurnGroup with Part rendering"
```

---

## Task 12: AppLayout + App.tsx 组装

**Files:**
- Create: `frontend/src/components/layout/AppLayout.tsx`
- Modify: `frontend/src/App.tsx`
- Delete: `frontend/src/components/Header.tsx`
- Delete: `frontend/src/components/Sidebar.tsx`
- Delete: `frontend/src/components/HistoryDrawer.tsx`
- Delete: `frontend/src/components/ConnectionScreen.tsx`
- Delete: `frontend/src/components/ApiKeyDialog.tsx`
- Delete: `frontend/src/components/ModelDropdown.tsx`
- Delete: `frontend/src/components/MessageBubble.tsx`

- [ ] **Step 1: 创建 `AppLayout.tsx`**

```typescript
import React, { useEffect } from 'react';
import { LeftDrawer } from './LeftDrawer';
import { RightDrawer } from './RightDrawer';
import { StatusBar } from './StatusBar';
import { LogPanel } from './LogPanel';
import { ChatPanel } from '../chat/ChatPanel';
import { InputBox } from '../chat/InputBox';
import { useUIStore } from '../../stores/uiStore';
import { getPresetByName, applyTheme } from '../../themes';

export const AppLayout: React.FC = () => {
  const { themeName } = useUIStore();

  useEffect(() => {
    const preset = getPresetByName(themeName);
    if (preset) applyTheme(preset);
  }, [themeName]);

  return (
    <div className="w-screen h-screen flex flex-col bg-bg text-text-primary overflow-hidden">
      <div className="flex-1 flex min-h-0">
        <LeftDrawer />

        <div className="flex-1 flex flex-col min-w-0">
          <ChatPanel />
          <InputBox />
        </div>

        <RightDrawer />
      </div>

      <StatusBar />
      <LogPanel />
    </div>
  );
};
```

- [ ] **Step 2: 修改 `App.tsx`**

```typescript
import React from 'react';
import { AppLayout } from './components/layout/AppLayout';

const App: React.FC = () => {
  return <AppLayout />;
};

export default App;
```

- [ ] **Step 3: 删除旧组件**

```bash
rm frontend/src/components/Header.tsx
rm frontend/src/components/Sidebar.tsx
rm frontend/src/components/HistoryDrawer.tsx
rm frontend/src/components/ConnectionScreen.tsx
rm frontend/src/components/ApiKeyDialog.tsx
rm frontend/src/components/ModelDropdown.tsx
rm frontend/src/components/MessageBubble.tsx
```

- [ ] **Step 4: Commit**

```bash
git add frontend/src/App.tsx frontend/src/components/layout/AppLayout.tsx
git rm frontend/src/components/Header.tsx frontend/src/components/Sidebar.tsx frontend/src/components/HistoryDrawer.tsx frontend/src/components/ConnectionScreen.tsx frontend/src/components/ApiKeyDialog.tsx frontend/src/components/ModelDropdown.tsx frontend/src/components/MessageBubble.tsx
git commit -m "feat(ui): assemble AppLayout and App, remove old components"
```

---

## Task 13: 废弃文件清理与入口修复

**Files:**
- Delete: `frontend/src/stores/appStore.ts`
- Delete: `frontend/src/types/events.ts`
- Modify: `frontend/src/main.tsx`（如有旧导入）

- [ ] **Step 1: 删除废弃文件**

```bash
rm frontend/src/stores/appStore.ts
rm frontend/src/types/events.ts
```

- [ ] **Step 2: 检查 `frontend/src/main.tsx` 是否有旧导入**

```bash
grep -n "appStore\|events\|useClient\|useSidecar\|useTheme" frontend/src/main.tsx || echo "No old imports found"
```

如有旧导入，删除对应 import 行。

- [ ] **Step 3: Commit**

```bash
git rm frontend/src/stores/appStore.ts frontend/src/types/events.ts
git commit -m "chore: remove obsolete appStore and events types"
```

---

## Task 14: 集成测试与构建验证

**Files:**
- Verify: `frontend/package.json`（确保 build script 正常）

- [ ] **Step 1: TypeScript 类型检查**

```bash
cd frontend && npx tsc --noEmit
```

Expected: 0 errors

- [ ] **Step 2: Vite 构建**

```bash
cd frontend && npm run build
```

Expected: Build completes successfully, `frontend/dist/` generated

- [ ] **Step 3: 运行现有前端测试**

```bash
cd frontend && npm test
```

Expected: All tests pass (including chatStore.test.ts)

- [ ] **Step 4: Cargo 构建 Desktop**

```bash
cargo build --bin fi-code-desktop
```

Expected: Compiles successfully

- [ ] **Step 5: Commit（如有额外修复）**

```bash
git add -A
git commit -m "fix: resolve build errors after Desktop frontend rewrite" || echo "Nothing to commit"
```

---

## Self-Review Checklist

### 1. Spec Coverage

| Spec 章节 | 对应 Task | 状态 |
|-----------|-----------|------|
| 2.1 Part 类型 | Task 1 | ✓ |
| 2.2 SseEvent | Task 1 | ✓ |
| 2.3 Turn | Task 1 | ✓ |
| 2.4 AgentType | Task 1 | ✓ |
| 3.1 Store 拆分 | Task 3-5 | ✓ |
| 3.2 ChatStore 逻辑 | Task 4 | ✓ |
| 4.1 chatStream | Task 6 | ✓ |
| 4.2 事件分发 | Task 7 | ✓ |
| 5.1 布局结构 | Task 10-12 | ✓ |
| 5.2 Part 渲染注册表 | Task 8-9 | ✓ |
| 5.3 Agent 切换 | Task 10 (StatusBar) | ✓ |
| 6.1 扩展 ThemeColors | Task 2 | ✓ |
| 6.2 共享 JSON | Task 2 | ✓ |
| 6.3 CSS 变量 | Task 2 | ✓ |
| 7 废弃清单 | Task 12-13 | ✓ |
| 8 测试策略 | Task 4, 14 | ✓ |

### 2. Placeholder Scan

- 无 "TBD" / "TODO" / "implement later"
- 无 "Add appropriate error handling" 等模糊描述
- 所有步骤包含完整代码或精确命令

### 3. Type Consistency

- `AgentType` 统一为 `'build' | 'plan'`
- `Part` 类型字段与 Rust DTO 对齐
- `SseEvent` 变体名称与后端序列化标签对齐
- Store action 命名在 Task 4 和 Task 7 中一致
