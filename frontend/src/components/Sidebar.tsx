import React, { useEffect, useState } from 'react';
import { useAppStore } from '../stores/appStore';
import { getFileTree } from '../services/file';
import { FileEntry } from '../types/api';

const FileIcon: React.FC<{ isDir: boolean }> = ({ isDir }) => (
  <span className="mr-1 text-text-muted">
    {isDir ? '📁' : '📄'}
  </span>
);

const FileTreeItem: React.FC<{ entry: FileEntry; depth: number }> = ({ entry, depth }) => (
  <div
    className="flex items-center py-1 px-2 text-sm text-text-secondary hover:text-text hover:bg-bg cursor-pointer transition-colors"
    style={{ paddingLeft: `${8 + depth * 16}px` }}
    title={entry.path}
  >
    <FileIcon isDir={entry.is_dir} />
    <span className="truncate">{entry.name}</span>
  </div>
);

export const Sidebar: React.FC = () => {
  const sidebarCollapsed = useAppStore(s => s.sidebarCollapsed);
  const sidebarWidth = useAppStore(s => s.sidebarWidth);
  const toggleSidebar = useAppStore(s => s.toggleSidebar);
  const setSidebarWidth = useAppStore(s => s.setSidebarWidth);
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [isResizing, setIsResizing] = useState(false);

  useEffect(() => {
    getFileTree('.')
      .then(result => setEntries(result.entries))
      .catch(console.error);
  }, []);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing) return;
      const newWidth = e.clientX;
      setSidebarWidth(newWidth);
    };

    const handleMouseUp = () => {
      setIsResizing(false);
    };

    if (isResizing) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isResizing, setSidebarWidth]);

  if (sidebarCollapsed) {
    return (
      <div className="flex flex-col items-center py-2 bg-bg-secondary border-r border-border w-12">
        <button
          onClick={toggleSidebar}
          className="p-2 text-text-muted hover:text-text transition-colors"
          title="Expand sidebar"
        >
          ☰
        </button>
        <div className="mt-2 p-2 text-text-muted hover:text-text cursor-pointer" title="Files">
          📁
        </div>
      </div>
    );
  }

  return (
    <div
      className="flex flex-col bg-bg-secondary border-r border-border relative"
      style={{ width: sidebarWidth }}
    >
      <div className="flex items-center justify-between px-3 py-2 border-b border-border">
        <span className="text-sm font-medium text-text-secondary">Project Files</span>
        <button
          onClick={toggleSidebar}
          className="p-1 text-text-muted hover:text-text transition-colors"
          title="Collapse sidebar"
        >
          ←
        </button>
      </div>

      <div className="flex-1 overflow-y-auto py-1">
        {entries.length === 0 ? (
          <div className="px-3 py-2 text-sm text-text-muted">Loading...</div>
        ) : (
          entries.map((entry, idx) => (
            <FileTreeItem key={idx} entry={entry} depth={entry.depth} />
          ))
        )}
      </div>

      <div
        className="absolute right-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-accent transition-colors"
        onMouseDown={() => setIsResizing(true)}
      />
    </div>
  );
};
