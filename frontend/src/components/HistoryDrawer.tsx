import React, { useEffect } from 'react';
import { useAppStore } from '../stores/appStore';
import { listSessions, switchSession } from '../services/session';

export const HistoryDrawer: React.FC = () => {
  const historyOpen = useAppStore(s => s.historyOpen);
  const sessions = useAppStore(s => s.sessions);
  const currentSessionId = useAppStore(s => s.currentSessionId);
  const setSessions = useAppStore(s => s.setSessions);
  const setCurrentSessionId = useAppStore(s => s.setCurrentSessionId);
  const toggleHistory = useAppStore(s => s.toggleHistory);
  const clearMessages = useAppStore(s => s.clearMessages);

  useEffect(() => {
    if (historyOpen && sessions.length === 0) {
      listSessions()
        .then(result => setSessions(result.sessions))
        .catch(console.error);
    }
  }, [historyOpen, sessions.length, setSessions]);

  const handleSwitchSession = async (id: string) => {
    try {
      await switchSession(id);
      setCurrentSessionId(id);
      clearMessages();
      toggleHistory();
    } catch (err) {
      console.error('Failed to switch session:', err);
    }
  };

  if (!historyOpen) return null;

  return (
    <>
      <div
        className="fixed inset-0 bg-bg-overlay z-40"
        onClick={toggleHistory}
      />

      <div className="fixed right-0 top-12 bottom-0 w-72 bg-bg-secondary border-l border-border z-50 flex flex-col">
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <h3 className="text-sm font-medium text-text">Session History</h3>
          <button
            onClick={toggleHistory}
            className="text-text-muted hover:text-text transition-colors"
          >
            ✕
          </button>
        </div>

        <div className="flex-1 overflow-y-auto py-2">
          {sessions.length === 0 ? (
            <div className="px-4 py-2 text-sm text-text-muted">No sessions found</div>
          ) : (
            sessions.map(session => (
              <button
                key={session.id}
                onClick={() => handleSwitchSession(session.id)}
                className={`w-full text-left px-4 py-2 text-sm hover:bg-bg transition-colors ${
                  session.id === currentSessionId ? 'text-accent bg-bg' : 'text-text-secondary'
                }`}
              >
                <div className="font-medium truncate">{session.name}</div>
                <div className="text-xs text-text-muted">{session.message_count} messages</div>
              </button>
            ))
          )}
        </div>
      </div>
    </>
  );
};
