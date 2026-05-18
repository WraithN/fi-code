import React, { useEffect, useState } from 'react';
import { useUIStore } from '../../stores/uiStore';
import { apiClient } from '../../services/apiClient';
import { FileEntry } from '../../types/api';

interface FileTreeNodeProps {
  entry: FileEntry;
  depth: number;
}

const FileTreeNode: React.FC<FileTreeNodeProps> = ({ entry, depth }) => {
  const [expanded, setExpanded] = useState(false);
  const indent = depth * 12;

  if (!entry.is_dir) {
    return (
      <div
        className="flex items-center py-0.5 px-1 rounded hover:bg-bg-overlay cursor-pointer text-text-secondary text-xs"
        style={{ paddingLeft: `${indent + 4}px` }}
      >
        <span className="mr-1 text-text-muted">📄</span>
        <span className="truncate">{entry.name}</span>
      </div>
    );
  }

  return (
    <div>
      <div
        className="flex items-center py-0.5 px-1 rounded hover:bg-bg-overlay cursor-pointer text-text-primary text-xs font-medium"
        style={{ paddingLeft: `${indent + 4}px` }}
        onClick={() => setExpanded(!expanded)}
      >
        <span className="mr-1 text-text-muted">{expanded ? '📂' : '📁'}</span>
        <span className="truncate">{entry.name}</span>
      </div>
      {expanded && entry.children && (
        <div>
          {entry.children.map((child, idx) => (
            <FileTreeNode key={`${child.path}-${idx}`} entry={child} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
};

export const LeftDrawer: React.FC = () => {
  const { leftDrawerOpen, toggleLeftDrawer } = useUIStore();
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!leftDrawerOpen) return;
    apiClient
      .getFileTree()
      .then((res) => setEntries(res.entries))
      .catch((err) => setError(err.message));
  }, [leftDrawerOpen]);

  if (!leftDrawerOpen) {
    return (
      <button
        onClick={toggleLeftDrawer}
        className="w-8 h-full bg-bg-secondary border-r border-border flex items-center justify-center hover:bg-bg-overlay"
      >
        <span className="text-text-muted text-xs">›</span>
      </button>
    );
  }

  return (
    <div className="w-64 bg-bg-secondary border-r border-border flex flex-col">
      <div className="h-10 flex items-center justify-between px-3 border-b border-border">
        <span className="text-sm font-medium text-text-primary">Files</span>
        <button onClick={toggleLeftDrawer} className="text-text-muted hover:text-text-primary text-xs">
          ‹
        </button>
      </div>
      <div className="flex-1 p-2 overflow-y-auto">
        {error ? (
          <p className="text-xs text-error">{error}</p>
        ) : entries.length === 0 ? (
          <p className="text-xs text-text-muted">Loading...</p>
        ) : (
          entries.map((entry, idx) => <FileTreeNode key={`${entry.path}-${idx}`} entry={entry} depth={0} />)
        )}
      </div>
    </div>
  );
};
