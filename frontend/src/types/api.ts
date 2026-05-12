export interface SessionInfo {
  id: string;
  name: string;
  message_count: number;
}

export interface SessionListResult {
  sessions: SessionInfo[];
  current_session_id: string | null;
}

export interface ApiResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

export interface FileEntry {
  path: string;
  name: string;
  is_dir: boolean;
  depth: number;
}

export interface FileTreeResult {
  root: string;
  entries: FileEntry[];
}

export interface ModelItem {
  key: string;
  name: string;
  context: number;
  output: number;
}

export interface ProviderItem {
  key: string;
  name: string;
  provider_type: string;
  models: ModelItem[];
}

export interface CommandMeta {
  name: string;
  description: string;
  args_hint: string | null;
}

export interface SseContentEvent {
  type: 'content';
  text: string;
}

export interface SseDoneEvent {
  type: 'done';
  session_id: string;
}

export interface SseErrorEvent {
  type: 'error';
  message: string;
}

export type SseEvent = SseContentEvent | SseDoneEvent | SseErrorEvent;

export interface Message {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: number;
}
