import {
  ApiResponse,
  SessionListResult,
  SessionInfo,
  FileTreeResult,
  CommandMeta,
  SseEvent,
} from '../types/api';

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
      body: JSON.stringify({
        jsonrpc: '2.0',
        method,
        params,
        id: 1,
      }),
    });

    if (!resp.ok) {
      throw new Error(`RPC failed: ${resp.status} ${resp.statusText}`);
    }

    const data = await resp.json();
    if (data.error) {
      throw new Error(data.error.message || 'RPC error');
    }
    return data.result;
  }

  async get<T>(path: string): Promise<T> {
    const resp = await fetch(`${this.baseUrl}${path}`);
    if (!resp.ok) {
      throw new Error(`GET ${path} failed: ${resp.status}`);
    }
    const data: ApiResponse<T> = await resp.json();
    if (!data.success || data.data === null) {
      throw new Error(data.error || 'API returned no data');
    }
    return data.data;
  }

  async post<T>(path: string, body?: unknown): Promise<T> {
    const resp = await fetch(`${this.baseUrl}${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });
    if (!resp.ok) {
      throw new Error(`POST ${path} failed: ${resp.status}`);
    }
    const data: ApiResponse<T> = await resp.json();
    if (!data.success || data.data === null) {
      throw new Error(data.error || 'API returned no data');
    }
    return data.data;
  }

  chatStream(sessionId: string | null, message: string): ReadableStream<SseEvent> {
    const body = JSON.stringify({ session_id: sessionId, message });

    return new ReadableStream({
      start: async (controller) => {
        try {
          const resp = await fetch(`${this.baseUrl}/chat`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body,
          });

          if (!resp.ok) {
            controller.error(new Error(`Chat failed: ${resp.status}`));
            return;
          }

          const reader = resp.body?.getReader();
          if (!reader) {
            controller.error(new Error('No response body'));
            return;
          }

          const decoder = new TextDecoder();
          let buffer = '';

          while (true) {
            const { done, value } = await reader.read();
            if (done) break;

            buffer += decoder.decode(value, { stream: true });
            const lines = buffer.split('\n');
            buffer = lines.pop() || '';

            for (const line of lines) {
              if (line.startsWith('data: ')) {
                const jsonStr = line.slice(6);
                try {
                  const event = JSON.parse(jsonStr);
                  if (event.content !== undefined && event.content !== null) {
                    controller.enqueue({ type: 'content', text: event.content });
                  } else if (event.reasoning_content) {
                    controller.enqueue({ type: 'content', text: event.reasoning_content });
                  } else if (event.done || event.session_id) {
                    controller.enqueue({ type: 'done', session_id: event.session_id || '' });
                    controller.close();
                    return;
                  } else if (event.error) {
                    controller.enqueue({ type: 'error', message: event.error });
                    controller.close();
                    return;
                  }
                } catch {
                  // Ignore parse errors for incomplete chunks
                }
              }
            }
          }

          controller.close();
        } catch (err) {
          controller.error(err);
        }
      },
    });
  }
}

export const apiClient = new ApiClient();
