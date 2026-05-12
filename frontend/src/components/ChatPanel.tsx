import React, { useRef, useEffect } from 'react';
import { useAppStore } from '../stores/appStore';
import { MessageBubble } from './MessageBubble';

export const ChatPanel: React.FC = () => {
  const messages = useAppStore(s => s.messages);
  const isGenerating = useAppStore(s => s.isGenerating);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  return (
    <div className="flex-1 flex flex-col min-h-0 bg-bg">
      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto p-4 space-y-1"
      >
        {messages.length === 0 ? (
          <div className="flex items-center justify-center h-full text-text-muted">
            <div className="text-center">
              <p className="text-lg mb-2">Welcome to fi-code</p>
              <p className="text-sm">Start a conversation or use /commands</p>
            </div>
          </div>
        ) : (
          messages.map(msg => <MessageBubble key={msg.id} message={msg} />)
        )}
      </div>

      {isGenerating && (
        <div className="px-4 py-2 text-xs text-text-muted animate-pulse">
          Generating...
        </div>
      )}
    </div>
  );
};
