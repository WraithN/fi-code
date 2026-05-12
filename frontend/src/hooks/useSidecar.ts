import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

export function useSidecar() {
  const [sidecarUrl, setSidecarUrl] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  const start = useCallback(async () => {
    setStarting(true);
    try {
      const url = await invoke<string>('start_sidecar');
      setSidecarUrl(url);
      return url;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      throw new Error(`Failed to start sidecar: ${message}`);
    } finally {
      setStarting(false);
    }
  }, []);

  const stop = useCallback(async () => {
    try {
      await invoke('stop_sidecar');
      setSidecarUrl(null);
    } catch (err) {
      console.error('Failed to stop sidecar:', err);
    }
  }, []);

  return { sidecarUrl, starting, start, stop };
}
