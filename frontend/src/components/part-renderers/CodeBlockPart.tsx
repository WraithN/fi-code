import React from 'react';
import { Part } from '../../types/part';

export const CodeBlockPart: React.FC<{ part: Extract<Part, { type: 'code_block' }> }> = ({ part }) => (
  <div className="my-2 rounded overflow-hidden">
    <div className="text-xs text-text-muted bg-bg-secondary px-3 py-1 border-b border-border">
      {part.language || 'code'}
    </div>
    <pre className="text-sm text-text-primary bg-bg p-3 overflow-x-auto">
      <code>{part.code}</code>
    </pre>
  </div>
);
