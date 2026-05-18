import React from 'react';
import { Part } from '../../types/part';

export const ToolErrorPart: React.FC<{ part: Extract<Part, { type: 'tool_error' }> }> = ({ part }) => (
  <div className="my-2 p-3 rounded bg-bg-secondary border-l-2 border-error">
    <div className="text-xs text-error font-mono mb-1">✗ Error: {part.error_message}</div>
    <pre className="text-xs text-text-secondary whitespace-pre-wrap break-words">{part.content}</pre>
  </div>
);
