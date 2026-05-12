import { apiClient } from './client';

export async function listModels(): Promise<unknown> {
  return apiClient.get<unknown>('/api/models');
}

export async function switchModel(
  provider: string,
  model: string,
  apiKey?: string
): Promise<unknown> {
  return apiClient.post<unknown>('/api/model/switch', { provider, model, api_key: apiKey });
}

export async function getStatus(): Promise<string> {
  const result = (await apiClient.rpc('get_status')) as { current_model?: string };
  return result.current_model || 'unknown';
}
