import React from 'react';
import { Message } from '../types/api';

interface MessageBubbleProps {
  message: Message;
}

export const MessageBubble: React.FC<MessageBubbleProps> = ({ message }) => {
  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';

  if (isSystem) {
    return (
      <div className="flex justify-center my-2">
        <span className="text-xs text-text-muted bg-bg-secondary px-3 py-1 rounded-full">
          {message.content}
        </span>
      </div>
    );
  }

  return (
    <div className={`flex ${isUser ? 'justify-end' : 'justify-start'} mb-4`}>
      <div
        className={`max-w-[80%] px-4 py-2 rounded-lg ${
          isUser
            ? 'bg-accent text-bg'
            : 'bg-bg-secondary text-text border border-border'
        }`}
      >
        <div className="text-xs text-opacity-70 mb-1">
          {isUser ? 'You' : 'Assistant'}
        </div>
        <div className="text-sm whitespace-pre-wrap break-words">
          {message.content || (isUser ? '' : '▋')}
        </div>
      </div>
    </div>
  );
};
