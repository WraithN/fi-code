import { apiClient } from './apiClient';
import { SessionListResult, SessionInfo } from '../types/api';

export async function listSessions(): Promise<SessionListResult> {
  return apiClient.get<SessionListResult>('/api/sessions');
}

export async function createSession(name: string): Promise<SessionInfo> {
  return apiClient.post<SessionInfo>('/api/sessions', { name });
}

export async function switchSession(id: string): Promise<SessionInfo> {
  return apiClient.post<SessionInfo>(`/api/sessions/${id}/switch`);
}
