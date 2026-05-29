import { create } from 'zustand';
import { ProviderItem } from '../types/api';
import { CommandMeta } from '../types/command';

interface UIState {
  leftDrawerOpen: boolean;
  rightDrawerOpen: boolean;
  logOpen: boolean;
  themeName: string;
  providers: ProviderItem[];
  currentModel: string;
  commands: CommandMeta[];
  inputText: string;
  inputFocusTrigger: number;
  toggleLeftDrawer: () => void;
  toggleRightDrawer: () => void;
  toggleLog: () => void;
  setThemeName: (name: string) => void;
  setProviders: (providers: ProviderItem[]) => void;
  setCurrentModel: (model: string) => void;
  setCommands: (commands: CommandMeta[]) => void;
  setInputText: (text: string) => void;
  triggerInputFocus: () => void;
}

const savedTheme = localStorage.getItem('fi-code-theme');

export const useUIStore = create<UIState>((set) => ({
  leftDrawerOpen: true,
  rightDrawerOpen: false,
  logOpen: false,
  themeName: savedTheme || 'deep_ocean',
  providers: [],
  currentModel: 'unknown',
  commands: [],
  inputText: '',
  inputFocusTrigger: 0,
  toggleLeftDrawer: () => set((s) => ({ leftDrawerOpen: !s.leftDrawerOpen })),
  toggleRightDrawer: () => set((s) => ({ rightDrawerOpen: !s.rightDrawerOpen })),
  toggleLog: () => set((s) => ({ logOpen: !s.logOpen })),
  setThemeName: (name) => {
    localStorage.setItem('fi-code-theme', name);
    set({ themeName: name });
  },
  setProviders: (providers) => set({ providers }),
  setCurrentModel: (model) => set({ currentModel: model }),
  setCommands: (commands) => set({ commands }),
  setInputText: (text) => set({ inputText: text }),
  triggerInputFocus: () => set((s) => ({ inputFocusTrigger: s.inputFocusTrigger + 1 })),
}));
