import React from 'react';
import { Part } from '../../types/part';

export const ToolUsePart: React.FC<{ part: Extract<Part, { type: 'tool_use' }> }> = ({ part }) => (
  <div className="my-2 p-3 rounded bg-bg-secondary border border-border">
    <div className="text-xs text-brand font-mono mb-1">🔧 {part.name}</div>
    <pre className="text-xs text-text-secondary overflow-x-auto">
      {JSON.stringify(part.arguments, null, 2)}
    </pre>
  </div>
);
