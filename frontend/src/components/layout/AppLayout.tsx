import React, { useEffect, useState } from 'react';
import { LeftDrawer } from './LeftDrawer';
import { LogPanel } from './LogPanel';
import { ChatPanel } from '../chat/ChatPanel';
import { InputBox } from '../chat/InputBox';

import { useUIStore } from '../../stores/uiStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { getPresetByName, applyTheme } from '../../themes';
import { getStatus } from '../../services/model';
import { apiClient } from '../../services/apiClient';
import { CommandMeta } from '../../types/command';
import { SettingsDialog } from '../settings/SettingsDialog';

export const AppLayout: React.FC = () => {
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
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
    <div className="flex flex-col h-screen overflow-hidden bg-tauri-dark bg-grid">
      {/* 背景装饰 */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden -z-10">
        <div className="absolute top-0 left-1/4 w-96 h-96 bg-tauri-primary/20 rounded-full blur-3xl"></div>
        <div className="absolute bottom-0 right-1/4 w-96 h-96 bg-tauri-secondary/20 rounded-full blur-3xl"></div>
      </div>
      
      {/* Header */}
      <header className="glass border-b border-tauri-border h-16 flex items-center px-8 justify-between shrink-0 z-10">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl gradient-bg flex items-center justify-center">
            <svg className="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"/>
            </svg>
          </div>
          <h1 className="text-2xl font-bold gradient-text">fi-code</h1>
        </div>
        
        <div className="flex items-center gap-2 bg-tauri-card/50 px-4 py-2 rounded-xl border border-tauri-border">
          <div className="w-2 h-2 rounded-full bg-green-400 animate-pulse"></div>
          <span className="text-sm text-gray-300">Ready to code</span>
        </div>
        
        <button
          onClick={() => setIsSettingsOpen(true)}
          className="p-2 hover:bg-tauri-card rounded-lg transition-colors cursor-pointer"
          title="设置"
        >
          <svg className="w-6 h-6 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"/>
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/>
          </svg>
        </button>
      </header>
      
      {/* 主体布局 */}
      <div className="flex-1 flex min-h-0">
        {/* 左边栏 */}
        <LeftDrawer />

        {/* 主内容区 */}
        <div className="flex-1 flex flex-col min-w-0 relative">
          <ChatPanel />
    
          <InputBox />
        </div>
      </div>

      {/* 日志面板 */}
      <LogPanel />

      {/* 设置弹窗 */}
      <SettingsDialog isOpen={isSettingsOpen} onClose={() => setIsSettingsOpen(false)} />
    </div>
  );
};
