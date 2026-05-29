import React, { useMemo } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Part } from '../../types/part';
import { markdownComponents } from './markdownComponents';

interface TextPartProps {
  part: Extract<Part, { type: 'text' }>;
  isComplete?: boolean;
}

export const TextPart = React.memo<TextPartProps>(({ part, isComplete }) => {
  const text = part.text;

  // 流式输出期间禁用 Markdown 解析，避免反复重解析导致卡死
  const useMarkdown = isComplete !== false;

  const content = useMemo(() => {
    if (useMarkdown) {
      return (
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          components={markdownComponents}
        >
          {text}
        </ReactMarkdown>
      );
    }
    return <div className="whitespace-pre-wrap break-words">{text}</div>;
  }, [text, useMarkdown]);

  return (
    <div className="text-sm text-gray-200 break-words leading-relaxed">
      {content}
    </div>
  );
});

TextPart.displayName = 'TextPart';
