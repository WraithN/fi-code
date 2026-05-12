import { apiClient } from './client';
import { FileTreeResult } from '../types/api';

export async function getFileTree(path: string = '.'): Promise<FileTreeResult> {
  return apiClient.get<FileTreeResult>(`/api/files?path=${encodeURIComponent(path)}`);
}
