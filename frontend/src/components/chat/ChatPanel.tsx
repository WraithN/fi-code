import React, { useRef, useEffect } from 'react';
import { useChatStore } from '../../stores/chatStore';
import { TurnGroup } from './TurnGroup';

export const ChatPanel: React.FC = () => {
  const turns = useChatStore((s) => s.turns);
  const isGenerating = useChatStore((s) => s.isGenerating);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [turns]);

  return (
    <div className="flex-1 flex flex-col min-h-0 bg-bg">
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-4">
        {turns.length === 0 ? (
          <div className="flex items-center justify-center h-full text-text-muted">
            <div className="text-center">
              <p className="text-lg mb-2">Welcome to fi-code</p>
              <p className="text-sm">Start a conversation or use /commands</p>
            </div>
          </div>
        ) : (
          turns.map((turn) => <TurnGroup key={turn.id} turn={turn} />)
        )}
      </div>

      {isGenerating && (
        <div className="px-4 py-2 text-xs text-text-muted animate-pulse">Generating...</div>
      )}
    </div>
  );
};
