import { create } from 'zustand';

interface UIState {
  leftDrawerOpen: boolean;
  rightDrawerOpen: boolean;
  logOpen: boolean;
  currentModel: string;
  themeName: string;
  toggleLeftDrawer: () => void;
  toggleRightDrawer: () => void;
  toggleLog: () => void;
  setThemeName: (name: string) => void;
}

export const useUIStore = create<UIState>((set) => ({
  leftDrawerOpen: true,
  rightDrawerOpen: false,
  logOpen: false,
  currentModel: 'unknown',
  themeName: 'default',
  toggleLeftDrawer: () => set((state) => ({ leftDrawerOpen: !state.leftDrawerOpen })),
  toggleRightDrawer: () => set((state) => ({ rightDrawerOpen: !state.rightDrawerOpen })),
  toggleLog: () => set((state) => ({ logOpen: !state.logOpen })),
  setThemeName: (name) => set({ themeName: name }),
}));
