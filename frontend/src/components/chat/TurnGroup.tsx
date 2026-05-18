import React from 'react';
import { Turn } from '../../types/turn';
import { PartRenderer } from '../part-renderers/registry';

export const TurnGroup: React.FC<{ turn: Turn }> = ({ turn }) => {
  return (
    <div className="mb-6">
      {/* 用户消息 */}
      <div className="flex justify-end mb-2">
        <div className="max-w-[80%] px-4 py-2 rounded-lg bg-user text-bg">
          <div className="text-xs text-bg opacity-70 mb-1">You</div>
          <div className="text-sm whitespace-pre-wrap break-words">{turn.userMessage}</div>
        </div>
      </div>

      {/* AI 回复 Parts */}
      <div className="flex justify-start">
        <div className="max-w-[80%] px-4 py-2 rounded-lg bg-bg-ai-area text-text-primary border border-border">
          <div className="text-xs text-text-muted mb-1">Assistant</div>
          {turn.parts.length === 0 && !turn.isComplete ? (
            <div className="text-sm text-text-muted animate-pulse">▋</div>
          ) : (
            turn.parts.map((part, i) => <PartRenderer key={i} part={part} />)
          )}
        </div>
      </div>
    </div>
  );
};
