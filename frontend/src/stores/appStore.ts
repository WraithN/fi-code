import { create } from 'zustand';
import { SessionInfo, ProviderItem, Message } from '../types/api';

interface AppState {
  mode: 'standalone' | 'remote';
  connectionStatus: 'connecting' | 'connected' | 'error';
  serverUrl: string;
  connectionError: string | null;

  currentSessionId: string | null;
  sessions: SessionInfo[];

  currentModel: string;
  providers: ProviderItem[];

  sidebarCollapsed: boolean;
  sidebarWidth: number;
  historyOpen: boolean;
  logOpen: boolean;
  themeName: string;
  isGenerating: boolean;

  messages: Message[];

  setMode: (mode: 'standalone' | 'remote') => void;
  setConnectionStatus: (status: 'connecting' | 'connected' | 'error', error?: string) => void;
  setServerUrl: (url: string) => void;
  setCurrentSessionId: (id: string | null) => void;
  setSessions: (sessions: SessionInfo[]) => void;
  setCurrentModel: (model: string) => void;
  setProviders: (providers: ProviderItem[]) => void;
  toggleSidebar: () => void;
  setSidebarWidth: (width: number) => void;
  toggleHistory: () => void;
  toggleLog: () => void;
  setThemeName: (name: string) => void;
  setIsGenerating: (generating: boolean) => void;
  addMessage: (message: Message) => void;
  appendToLastMessage: (text: string) => void;
  clearMessages: () => void;
  addSystemMessage: (content: string) => void;
}

export const useAppStore = create<AppState>((set) => ({
  mode: 'standalone',
  connectionStatus: 'connecting',
  serverUrl: 'http://localhost:4040',
  connectionError: null,

  currentSessionId: null,
  sessions: [],

  currentModel: 'unknown',
  providers: [],

  sidebarCollapsed: false,
  sidebarWidth: 240,
  historyOpen: false,
  logOpen: false,
  themeName: 'Default',
  isGenerating: false,

  messages: [],

  setMode: (mode) => set({ mode }),
  setConnectionStatus: (status, error) => set({ connectionStatus: status, connectionError: error || null }),
  setServerUrl: (url) => set({ serverUrl: url }),
  setCurrentSessionId: (id) => set({ currentSessionId: id }),
  setSessions: (sessions) => set({ sessions }),
  setCurrentModel: (model) => set({ currentModel: model }),
  setProviders: (providers) => set({ providers }),
  toggleSidebar: () => set(s => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  setSidebarWidth: (width) => set({ sidebarWidth: Math.max(180, Math.min(360, width)) }),
  toggleHistory: () => set(s => ({ historyOpen: !s.historyOpen })),
  toggleLog: () => set(s => ({ logOpen: !s.logOpen })),
  setThemeName: (name) => set({ themeName: name }),
  setIsGenerating: (generating) => set({ isGenerating: generating }),

  addMessage: (message) => set(s => ({ messages: [...s.messages, message] })),

  appendToLastMessage: (text) =>
    set(s => {
      const messages = [...s.messages];
      const last = messages[messages.length - 1];
      if (last && last.role === 'assistant') {
        last.content += text;
      }
      return { messages };
    }),

  clearMessages: () => set({ messages: [] }),

  addSystemMessage: (content) =>
    set(s => ({
      messages: [
        ...s.messages,
        {
          id: `sys-${Date.now()}`,
          role: 'system',
          content,
          timestamp: Date.now(),
        },
      ],
    })),
}));
