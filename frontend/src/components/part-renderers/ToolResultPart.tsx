import React from 'react';
import { Part } from '../../types/part';

export const ToolResultPart: React.FC<{ part: Extract<Part, { type: 'tool_result' }> }> = ({ part }) => (
  <div className="my-2 p-3 rounded bg-bg-secondary border-l-2 border-success">
    <div className="text-xs text-success font-mono mb-1">✓ Result ({part.duration_ms}ms)</div>
    <pre className="text-xs text-text-secondary whitespace-pre-wrap break-words">{part.content}</pre>
  </div>
);
