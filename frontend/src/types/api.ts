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
  children?: FileEntry[];
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

export interface LogEntry {
  timestamp: string;
  level: string;
  module: string;
  message: string;
}

export interface CommandMeta {
  name: string;
  description: string;
  args_hint: string | null;
}
