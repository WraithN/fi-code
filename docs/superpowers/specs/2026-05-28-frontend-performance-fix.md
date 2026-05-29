# Frontend Performance Fix: Content Collapse + Virtual Scrolling

## Problem

When chat sessions produce long outputs (e.g., grep returning hundreds of lines), the browser becomes unresponsive due to:
1. **No virtualization** — all messages stay in DOM forever
2. **No memoization** — any SSE update re-renders the entire list
3. **No content truncation** — long text/code blocks render fully
4. **Synchronous auto-scroll** — layout thrashing on every chunk

## Solution

### A. Content Collapse

Collapse long content parts with "Show All" buttons:

| Component | Threshold | Behavior |
|-----------|-----------|----------|
| `TextPart` | >30 lines or >2000 chars | Show first 15 lines + expand button |
| `CodeBlockPart` | >30 lines | Show first 30 lines + expand button |
| `ToolResultPart` | >30 lines | Collapsed by default, show summary + expand |

### B. Virtual Scrolling

Replace plain `overflow-y-auto` list with `@tanstack/react-virtual`:
- Dynamic height measurement per `TurnGroup`
- Render only viewport ± buffer items
- Keep auto-scroll to bottom on new messages

## Files Changed

- `frontend/src/components/chat/ChatPanel.tsx` — virtual list integration
- `frontend/src/components/part-renderers/TextPart.tsx` — collapse logic
- `frontend/src/components/part-renderers/CodeBlockPart.tsx` — collapse logic
- `frontend/src/components/part-renderers/ToolResultPart.tsx` — collapse + summary
- `frontend/package.json` — add `@tanstack/react-virtual`

## Non-Goals

- No backend changes (50KB truncation already exists)
- No changes to SSE streaming logic
- No changes to tool schemas or API
