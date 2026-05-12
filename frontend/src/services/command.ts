import { apiClient } from './client';
import { CommandMeta } from '../types/api';

export async function listCommands(): Promise<CommandMeta[]> {
  return apiClient.get<CommandMeta[]>('/api/commands');
}

export async function executeCommand(
  name: string,
  args?: string,
  sessionId?: string
): Promise<unknown> {
  return apiClient.post<unknown>(`/api/commands/${name}/execute`, {
    args,
    session_id: sessionId,
  });
}
