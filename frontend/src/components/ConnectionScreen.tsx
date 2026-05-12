import React, { useState, useEffect } from 'react';
import { useAppStore } from '../stores/appStore';
import { useSidecar } from '../hooks/useSidecar';
import { Button } from './ui/Button';
import { Spinner } from './ui/Spinner';

export const ConnectionScreen: React.FC = () => {
  const { connectionStatus, connectionError, mode, serverUrl, setMode, setServerUrl, setConnectionStatus } = useAppStore();
  const { start, starting } = useSidecar();
  const [editUrl, setEditUrl] = useState(serverUrl);
  const [editMode, setEditMode] = useState<'standalone' | 'remote'>(mode);

  useEffect(() => {
    if (mode === 'standalone' && connectionStatus === 'connecting') {
      start()
        .then(url => {
          setServerUrl(url);
        })
        .catch(err => {
          setConnectionStatus('error', err.message);
        });
    }
  }, [mode, connectionStatus, start, setServerUrl, setConnectionStatus]);

  const handleConnect = () => {
    setMode(editMode);
    setServerUrl(editUrl);
    setConnectionStatus('connecting');
  };

  const isBusy = connectionStatus === 'connecting' || starting;

  return (
    <div className="w-screen h-screen flex items-center justify-center bg-bg text-text">
      <div className="w-full max-w-md p-8 bg-bg-secondary rounded-lg border border-border">
        <h1 className="text-2xl font-bold mb-2 text-center">fi-code</h1>
        <p className="text-text-muted text-center mb-6">AI Coding Agent Desktop</p>

        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-secondary mb-2">Mode</label>
            <div className="flex gap-2">
              <button
                onClick={() => setEditMode('standalone')}
                className={`flex-1 py-2 px-4 rounded border ${
                  editMode === 'standalone'
                    ? 'bg-accent text-bg border-accent'
                    : 'bg-bg border-border text-text-secondary hover:text-text'
                }`}
              >
                Standalone
              </button>
              <button
                onClick={() => setEditMode('remote')}
                className={`flex-1 py-2 px-4 rounded border ${
                  editMode === 'remote'
                    ? 'bg-accent text-bg border-accent'
                    : 'bg-bg border-border text-text-secondary hover:text-text'
                }`}
              >
                Remote
              </button>
            </div>
          </div>

          {editMode === 'remote' && (
            <div>
              <label className="block text-sm font-medium text-text-secondary mb-2">Server URL</label>
              <input
                type="text"
                value={editUrl}
                onChange={e => setEditUrl(e.target.value)}
                placeholder="http://localhost:4040"
                className="w-full px-3 py-2 bg-bg border border-border rounded text-text placeholder-text-muted focus:outline-none focus:border-accent"
              />
            </div>
          )}

          {isBusy && (
            <div className="flex items-center justify-center gap-2 py-2">
              <Spinner size="sm" />
              <span className="text-text-secondary">
                {starting ? 'Starting sidecar...' : 'Connecting...'}
              </span>
            </div>
          )}

          {connectionStatus === 'error' && (
            <div className="p-3 bg-error bg-opacity-10 border border-error rounded text-error text-sm">
              {connectionError || 'Failed to connect'}
            </div>
          )}

          <Button onClick={handleConnect} disabled={isBusy} className="w-full">
            {isBusy ? 'Please wait...' : 'Connect'}
          </Button>
        </div>
      </div>
    </div>
  );
};
