import { apiClient } from './client';
import { SseEvent } from '../types/api';

export async function sendMessage(
  sessionId: string | null,
  message: string
): Promise<ReadableStream<SseEvent>> {
  return apiClient.chatStream(sessionId, message);
}
