import React, { useEffect } from 'react';
import { LeftDrawer } from './LeftDrawer';
import { RightDrawer } from './RightDrawer';
import { StatusBar } from './StatusBar';
import { LogPanel } from './LogPanel';
import { ChatPanel } from '../chat/ChatPanel';
import { InputBox } from '../chat/InputBox';
import { useUIStore } from '../../stores/uiStore';
import { getPresetByName, applyTheme } from '../../themes';

export const AppLayout: React.FC = () => {
  const { themeName } = useUIStore();

  useEffect(() => {
    const preset = getPresetByName(themeName);
    if (preset) applyTheme(preset);
  }, [themeName]);

  return (
    <div className="w-screen h-screen flex flex-col bg-bg text-text-primary overflow-hidden">
      <div className="flex-1 flex min-h-0">
        <LeftDrawer />

        <div className="flex-1 flex flex-col min-w-0">
          <ChatPanel />
          <InputBox />
        </div>

        <RightDrawer />
      </div>

      <StatusBar />
      <LogPanel />
    </div>
  );
};
