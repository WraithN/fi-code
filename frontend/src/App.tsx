import React from 'react';
import { useAppStore } from './stores/appStore';
import { useClient } from './hooks/useClient';
import { useTheme } from './hooks/useTheme';
import { Header } from './components/Header';
import { Sidebar } from './components/Sidebar';
import { ChatPanel } from './components/ChatPanel';
import { InputBox } from './components/InputBox';
import { HistoryDrawer } from './components/HistoryDrawer';
import { LogPanel } from './components/LogPanel';
import { ConnectionScreen } from './components/ConnectionScreen';

const App: React.FC = () => {
  const { connectionStatus } = useAppStore();
  useClient();
  useTheme();

  if (connectionStatus !== 'connected') {
    return <ConnectionScreen />;
  }

  return (
    <div className="w-screen h-screen flex flex-col bg-bg text-text overflow-hidden">
      <Header />

      <div className="flex-1 flex min-h-0">
        <Sidebar />

        <div className="flex-1 flex flex-col min-w-0">
          <ChatPanel />
          <InputBox />
        </div>
      </div>

      <HistoryDrawer />
      <LogPanel />
    </div>
  );
};

export default App;
