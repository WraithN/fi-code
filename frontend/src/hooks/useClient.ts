import { useEffect, useCallback } from 'react';
import { useAppStore } from '../stores/appStore';
import { apiClient } from '../services/client';
import { getStatus } from '../services/model';

export function useClient() {
  const serverUrl = useAppStore(s => s.serverUrl);
  const setConnectionStatus = useAppStore(s => s.setConnectionStatus);
  const setCurrentModel = useAppStore(s => s.setCurrentModel);

  const checkConnection = useCallback(async () => {
    try {
      apiClient.setBaseUrl(serverUrl);
      const model = await getStatus();
      setConnectionStatus('connected');
      setCurrentModel(model);
      return true;
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Connection failed';
      setConnectionStatus('error', message);
      return false;
    }
  }, [serverUrl, setConnectionStatus, setCurrentModel]);

  useEffect(() => {
    checkConnection();
  }, [checkConnection]);

  return { checkConnection };
}
