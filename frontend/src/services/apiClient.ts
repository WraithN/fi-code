import { SseEvent } from '../types/sse';
import { AgentType } from '../types/agent';
import { ApiResponse, FileTreeResult, LogEntry } from '../types/api';

function detectBaseUrl(): string {
  // Tauri Desktop 环境使用本地 Sidecar 地址
  if ((window as any).__TAURI_INTERNALS__) {
    return 'http://127.0.0.1:4040';
  }
  // 浏览器/Web 模式使用当前页面 origin
  return window.location.origin;
}

export class ApiClient {
  private baseUrl: string;

  constructor(baseUrl?: string) {
    this.baseUrl = (baseUrl ?? detectBaseUrl()).replace(/\/$/, '');
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

  async getFileTree(path: string = ''): Promise<FileTreeResult> {
    const query = path ? `?path=${encodeURIComponent(path)}` : '';
    return this.get<FileTreeResult>(`/api/files${query}`);
  }

  async getLogs(limit: number = 200): Promise<LogEntry[]> {
    return this.get<LogEntry[]>(`/api/logs?limit=${limit}`);
  }

  async *subscribeLogs(): AsyncGenerator<LogEntry, void, unknown> {
    const resp = await fetch(`${this.baseUrl}/api/logs/stream`);
    if (!resp.ok) throw new Error(`Log stream failed: ${resp.status}`);

    const reader = resp.body?.getReader();
    if (!reader) throw new Error('No response body');

    const decoder = new TextDecoder();
    let buffer = '';

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split('\n');
      buffer = lines.pop() || '';

      for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed.startsWith('data: ')) {
          const jsonStr = trimmed.slice(6);
          try {
            const entry = JSON.parse(jsonStr) as LogEntry;
            yield entry;
          } catch {
            console.warn('[Log SSE] Invalid JSON:', jsonStr.slice(0, 200));
          }
        }
      }
    }
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
