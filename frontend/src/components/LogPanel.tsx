import React from 'react';
import { useUIStore } from '../stores/uiStore';

export const LogPanel: React.FC = () => {
  const logOpen = useUIStore((s) => s.logOpen);
  const toggleLog = useUIStore((s) => s.toggleLog);

  if (!logOpen) return null;

  return (
    <div className="fixed bottom-4 right-4 w-96 h-64 bg-bg-secondary border border-border rounded-lg shadow-lg z-50 flex flex-col">
      <div className="flex items-center justify-between px-3 py-2 border-b border-border">
        <span className="text-sm font-medium text-text">Logs</span>
        <button onClick={toggleLog} className="text-text-muted hover:text-text">✕</button>
      </div>
      <div className="flex-1 overflow-y-auto p-2 text-xs text-text-muted font-mono">
        Log output will appear here...
      </div>
    </div>
  );
};
