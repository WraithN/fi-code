import React, { useState, useCallback } from 'react';
import { useChatStream } from '../../hooks/useChatStream';

export const InputBox: React.FC = () => {
  const [input, setInput] = useState('');
  const { send } = useChatStream();

  const handleSubmit = useCallback(() => {
    if (!input.trim()) return;
    send(input);
    setInput('');
  }, [input, send]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div className="p-4 bg-bg-secondary border-t border-border">
      <div className="flex gap-2">
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a message..."
          rows={2}
          className="flex-1 bg-bg text-text-primary border border-border rounded px-3 py-2 text-sm resize-none focus:outline-none focus:border-brand"
        />
        <button
          onClick={handleSubmit}
          className="px-4 py-2 bg-brand text-bg rounded text-sm font-medium hover:bg-accent-hover transition-colors"
        >
          Send
        </button>
      </div>
    </div>
  );
};
