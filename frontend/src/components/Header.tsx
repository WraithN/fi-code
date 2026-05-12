import React, { useState, useRef, useEffect, useCallback } from 'react';
import { useAppStore } from '../stores/appStore';
import { Button } from './ui/Button';
import { ModelDropdown } from './ModelDropdown';
import { ApiKeyDialog } from './ApiKeyDialog';
import { listModels, switchModel } from '../services/model';
import { ProviderItem } from '../types/api';

export const Header: React.FC = () => {
  const currentModel = useAppStore(s => s.currentModel);
  const isGenerating = useAppStore(s => s.isGenerating);
  const themeName = useAppStore(s => s.themeName);
  const toggleHistory = useAppStore(s => s.toggleHistory);
  const clearMessages = useAppStore(s => s.clearMessages);
  const setThemeName = useAppStore(s => s.setThemeName);
  const setCurrentModel = useAppStore(s => s.setCurrentModel);

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [modelDropdownOpen, setModelDropdownOpen] = useState(false);
  const [providers, setProviders] = useState<ProviderItem[]>([]);
  const [apiKeyDialog, setApiKeyDialog] = useState<{ provider: string; model: string } | null>(null);
  const settingsRef = useRef<HTMLDivElement>(null);
  const modelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (settingsRef.current && !settingsRef.current.contains(event.target as Node)) {
        setSettingsOpen(false);
      }
      if (modelRef.current && !modelRef.current.contains(event.target as Node)) {
        setModelDropdownOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const loadModels = useCallback(async () => {
    try {
      const data = await listModels() as { providers?: ProviderItem[] };
      if (data.providers) {
        setProviders(data.providers);
      }
    } catch (err) {
      console.error('Failed to load models:', err);
    }
  }, []);

  const handleModelSelect = async (provider: string, model: string, needsKey: boolean) => {
    setModelDropdownOpen(false);
    if (needsKey) {
      setApiKeyDialog({ provider, model });
    } else {
      try {
        await switchModel(provider, model);
        setCurrentModel(model);
      } catch (err) {
        console.error('Failed to switch model:', err);
      }
    }
  };

  const handleApiKeySubmit = async (apiKey: string) => {
    if (!apiKeyDialog) return;
    try {
      await switchModel(apiKeyDialog.provider, apiKeyDialog.model, apiKey || undefined);
      setCurrentModel(apiKeyDialog.model);
    } catch (err) {
      console.error('Failed to switch model:', err);
    }
    setApiKeyDialog(null);
  };

  const handleNewSession = () => {
    clearMessages();
    setSettingsOpen(false);
  };

  const handleThemeChange = (name: string) => {
    setThemeName(name);
    setSettingsOpen(false);
  };

  return (
    <>
      <header className="h-12 flex items-center justify-between px-4 bg-bg-secondary border-b border-border select-none">
        <div className="flex items-center gap-2">
          <span className="text-lg font-bold text-accent">fi-code</span>
          {isGenerating && (
            <span className="text-xs text-text-muted animate-pulse">generating...</span>
          )}
        </div>

        <div className="relative" ref={modelRef}>
          <button
            onClick={() => {
              setModelDropdownOpen(!modelDropdownOpen);
              if (!modelDropdownOpen && providers.length === 0) {
                loadModels();
              }
            }}
            className="flex items-center gap-1 px-3 py-1.5 rounded bg-bg border border-border text-sm text-text hover:border-accent transition-colors"
          >
            <span>{currentModel}</span>
            <span className="text-text-muted">▼</span>
          </button>

          {modelDropdownOpen && (
            <ModelDropdown
              providers={providers}
              currentModel={currentModel}
              onSelect={handleModelSelect}
              onClose={() => setModelDropdownOpen(false)}
            />
          )}
        </div>

        <div className="flex items-center gap-2">
          <Button variant="ghost" size="sm" onClick={toggleHistory} title="History">
            History
          </Button>

          <div className="relative" ref={settingsRef}>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setSettingsOpen(!settingsOpen)}
              title="Settings"
            >
              ⚙
            </Button>

            {settingsOpen && (
              <div className="absolute top-full right-0 mt-1 w-48 bg-bg-secondary border border-border rounded-lg shadow-lg z-50 py-1">
                <button
                  onClick={handleNewSession}
                  className="w-full text-left px-4 py-2 text-sm text-text hover:bg-bg transition-colors"
                >
                  New Session
                </button>
                <div className="border-t border-border my-1" />
                <div className="px-4 py-1 text-xs text-text-muted">Theme</div>
                {['Default', 'Light', 'Monokai'].map(name => (
                  <button
                    key={name}
                    onClick={() => handleThemeChange(name)}
                    className={`w-full text-left px-4 py-2 text-sm ${
                      themeName === name ? 'text-accent' : 'text-text-secondary'
                    } hover:bg-bg transition-colors`}
                  >
                    {themeName === name && '✓ '}{name}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      </header>

      <ApiKeyDialog
        isOpen={!!apiKeyDialog}
        provider={apiKeyDialog?.provider || ''}
        model={apiKeyDialog?.model || ''}
        onSubmit={handleApiKeySubmit}
        onCancel={() => setApiKeyDialog(null)}
      />
    </>
  );
};
