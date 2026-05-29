import React, { useState } from 'react';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus, vs } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { Part } from '../../types/part';
import { useTheme } from '../../hooks/useTheme';

const MAX_LINES = 30;

export const CodeBlockPart: React.FC<{ part: Extract<Part, { type: 'code_block' }> }> = ({ part }) => {
  const { isLight } = useTheme();
  const language = part.language || 'text';
  const [expanded, setExpanded] = useState(false);

  const renderLine = (line: string, index: number) => {
    let className = 'block';

    if (line.startsWith('+')) {
      className += ' bg-green-900/30 text-green-400';
    } else if (line.startsWith('-')) {
      className += ' bg-red-900/30 text-red-400';
    }

    return (
      <span key={index} className={className}>
        {line}
        {'\n'}
      </span>
    );
  };

  const allLines = part.code.split('\n');
  const isDiff = language === 'diff' || allLines.some(l => l.startsWith('+') || l.startsWith('-'));
  const shouldCollapse = allLines.length > MAX_LINES;
  const visibleLines = expanded || !shouldCollapse ? allLines : allLines.slice(0, MAX_LINES);
  const visibleCode = visibleLines.join('\n');

  return (
    <div className="my-2 rounded overflow-hidden border border-border">
      <div className="text-xs text-text-muted bg-bg-secondary px-3 py-1 border-b border-border flex justify-between items-center">
        <span>{part.language || 'code'}</span>
        {shouldCollapse && (
          <button
            onClick={() => setExpanded(!expanded)}
            className="text-tauri-primary hover:text-tauri-primary-hover text-xs underline cursor-pointer"
          >
            {expanded ? '收起' : `展开全部（${allLines.length} 行）`}
          </button>
        )}
      </div>
      <div className="text-sm text-text-primary bg-bg p-3 overflow-x-auto" style={{ tabSize: 4 }}>
        {isDiff ? (
          <pre style={{ whiteSpace: 'pre', margin: 0 }}>
            <code>{visibleLines.map((line, idx) => renderLine(line, idx))}</code>
          </pre>
        ) : (
          <SyntaxHighlighter
            language={language}
            style={isLight ? vs : vscDarkPlus}
            customStyle={{ margin: 0, backgroundColor: 'transparent', padding: 0 }}
            showLineNumbers={false}
            wrapLines={false}
          >
            {visibleCode}
          </SyntaxHighlighter>
        )}
      </div>
    </div>
  );
};
