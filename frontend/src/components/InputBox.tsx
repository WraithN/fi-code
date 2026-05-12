import React, { useState, useRef, useEffect } from 'react';
import { useAppStore } from '../stores/appStore';
import { useChatStream } from '../hooks/useChatStream';
import { Button } from './ui/Button';

export const InputBox: React.FC = () => {
  const [text, setText] = useState('');
  const isGenerating = useAppStore(s => s.isGenerating);
  const { send, stop } = useChatStream();
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 120)}px`;
    }
  }, [text]);

  const handleSubmit = () => {
    if (!text.trim() || isGenerating) return;
    send(text.trim());
    setText('');
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div className="border-t border-border bg-bg-secondary p-3">
      <div className="flex items-end gap-2">
        <textarea
          ref={textareaRef}
          value={text}
          onChange={e => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a message... (Shift+Enter for new line)"
          disabled={isGenerating}
          rows={1}
          className="flex-1 resize-none bg-bg border border-border rounded-lg px-3 py-2 text-sm text-text placeholder-text-muted focus:outline-none focus:border-accent disabled:opacity-50 min-h-[40px] max-h-[120px]"
        />
        {isGenerating ? (
          <Button variant="danger" size="sm" onClick={stop} className="shrink-0">
            Stop
          </Button>
        ) : (
          <Button
            variant="primary"
            size="sm"
            onClick={handleSubmit}
            disabled={!text.trim()}
            className="shrink-0"
          >
            Send
          </Button>
        )}
      </div>
      <div className="mt-1 text-xs text-text-muted">
        Press Enter to send, Shift+Enter for new line
      </div>
    </div>
  );
};
