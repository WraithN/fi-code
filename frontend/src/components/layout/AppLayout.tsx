import React, { useEffect } from 'react';
import { LeftDrawer } from './LeftDrawer';
import { RightDrawer } from './RightDrawer';
import { StatusBar } from './StatusBar';
import { LogPanel } from './LogPanel';
import { ChatPanel } from '../chat/ChatPanel';
import { InputBox } from '../chat/InputBox';
import { useUIStore } from '../../stores/uiStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { getPresetByName, applyTheme } from '../../themes';
import { getStatus } from '../../services/model';
import { apiClient } from '../../services/apiClient';
import { CommandMeta } from '../../types/command';

export const AppLayout: React.FC = () => {
  const { themeName, setCurrentModel, setCommands } = useUIStore();
  const { setConnectionStatus } = useConnectionStore();

  useEffect(() => {
    let preset = getPresetByName(themeName);
    if (!preset) {
      preset = getPresetByName('deep_ocean');
    }
    if (preset) applyTheme(preset);
  }, [themeName]);

  // 初始化：获取当前模型名和连接状态
  useEffect(() => {
    getStatus()
      .then((model) => {
        setCurrentModel(model);
        setConnectionStatus('connected');
      })
      .catch((err) => {
        console.warn('[AppLayout] Failed to get status:', err);
        setConnectionStatus('error', err.message);
      });
  }, [setCurrentModel, setConnectionStatus]);

  // 拉取可用指令列表
  useEffect(() => {
    apiClient
      .get<CommandMeta[]>('/api/commands')
      .then((cmds) => setCommands(cmds))
      .catch((err) => console.warn('[AppLayout] Failed to load commands:', err));
  }, [setCommands]);

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
