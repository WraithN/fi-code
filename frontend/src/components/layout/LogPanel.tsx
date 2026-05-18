import React, { useEffect, useRef, useState } from 'react';
import { useUIStore } from '../../stores/uiStore';
import { apiClient } from '../../services/apiClient';
import { LogEntry } from '../../types/api';

const LEVEL_COLORS: Record<string, string> = {
  INFO: 'text-success',
  DEBUG: 'text-text-muted',
  ERROR: 'text-error',
  WARN: 'text-warning',
};

export const LogPanel: React.FC = () => {
  const { logOpen, toggleLog } = useUIStore();
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<AbortController | null>(null);

  // 初始加载历史日志
  useEffect(() => {
    if (!logOpen) return;
    setError(null);
    apiClient
      .getLogs(100)
      .then((entries) => setLogs(entries))
      .catch((err) => setError(err.message));
  }, [logOpen]);

  // SSE 实时订阅日志
  useEffect(() => {
    if (!logOpen) return;

    let cancelled = false;
    const controller = new AbortController();
    abortRef.current = controller;

    async function streamLogs() {
      try {
        for await (const entry of apiClient.subscribeLogs()) {
          if (cancelled) break;
          setLogs((prev) => [...prev.slice(-199), entry]);
        }
      } catch (err) {
        if (!cancelled) {
          console.warn('[LogPanel] SSE error:', err);
        }
      }
    }

    streamLogs();
    return () => {
      cancelled = true;
      controller.abort();
    };
  }, [logOpen]);

  // 自动滚动到底部
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs]);

  if (!logOpen) return null;

  return (
    <div className="absolute bottom-8 right-4 w-96 h-64 bg-bg-secondary border border-border rounded shadow-lg flex flex-col z-50">
      <div className="h-8 flex items-center justify-between px-3 border-b border-border">
        <span className="text-sm font-medium text-text-primary">Logs</span>
        <button onClick={toggleLog} className="text-text-muted hover:text-text-primary">
          ✕
        </button>
      </div>
      <div ref={scrollRef} className="flex-1 p-2 overflow-y-auto text-xs font-mono space-y-0.5">
        {error ? (
          <p className="text-error">{error}</p>
        ) : logs.length === 0 ? (
          <p className="text-text-muted">No logs yet...</p>
        ) : (
          logs.map((log, idx) => (
            <div key={idx} className="flex gap-2">
              <span className="text-text-muted shrink-0">{log.timestamp.split('T')[1]?.replace('Z', '') || log.timestamp}</span>
              <span className={`shrink-0 font-bold ${LEVEL_COLORS[log.level] || 'text-text-secondary'}`}>
                {log.level}
              </span>
              <span className="text-text-muted shrink-0">[{log.module}]</span>
              <span className="text-text-secondary break-all">{log.message}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
