import React, { useState } from 'react';
import { Turn } from '../../types/turn';
import { PartRenderer } from '../part-renderers/registry';
import { useUIStore } from '../../stores/uiStore';

interface TurnGroupProps {
  turn: Turn;
}

export const TurnGroup = React.memo<TurnGroupProps>(({ turn }) => {
  const [showDetails, setShowDetails] = useState(true);
  const [copied, setCopied] = useState(false);
  const { setInputText, triggerInputFocus } = useUIStore();

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(turn.userMessage);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // 静默忽略复制失败
    }
  };

  const handleEdit = () => {
    setInputText(turn.userMessage);
    triggerInputFocus();
  };

  return (
    <div className="space-y-4 pb-6 mb-6 border-b border-tauri-border/30 last:border-b-0">
      {/* 用户消息 */}
      <div className="flex justify-end">
        <div className="max-w-[65%] glass border border-tauri-primary/30 rounded-2xl px-6 py-4 card-hover">
          <div className="text-xs text-tauri-primary mb-2 font-semibold flex items-center justify-end gap-2">
            <span>You</span>
            <div className="w-6 h-6 rounded-full bg-tauri-primary/20 flex items-center justify-center">
              <svg className="w-3 h-3 text-tauri-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"/>
              </svg>
            </div>
          </div>
          <div className="text-sm text-gray-100 whitespace-pre-wrap break-words">
            {turn.userMessage}
          </div>
          {/* 操作按钮：复制 + 编辑 */}
          <div className="mt-2 flex items-center justify-end gap-3">
            <button
              onClick={handleEdit}
              aria-label="编辑消息"
              title="编辑"
              className="flex items-center gap-1 text-gray-400 hover:text-tauri-primary transition-colors cursor-pointer"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
              </svg>
              <span className="text-xs">编辑</span>
            </button>
            <button
              onClick={handleCopy}
              aria-label="复制消息"
              title="复制"
              className="flex items-center gap-1 text-gray-400 hover:text-tauri-primary transition-colors cursor-pointer"
            >
              {copied ? (
                <>
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 13l4 4L19 7"/>
                  </svg>
                  <span className="text-xs">已复制</span>
                </>
              ) : (
                <>
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"/>
                  </svg>
                  <span className="text-xs">复制</span>
                </>
              )}
            </button>
          </div>
        </div>
      </div>

      {/* AI 回复 - 时间线风格 */}
      <div className="flex gap-4">
        {/* 时间线左侧 */}
        <div className="flex flex-col items-center">
          <div className="w-8 h-8 rounded-full gradient-bg flex items-center justify-center shadow-lg shadow-tauri-primary/30 z-10">
            <svg className="w-4 h-4 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"/>
            </svg>
          </div>
          <div className="w-0.5 flex-1 bg-gradient-to-b from-tauri-primary via-tauri-secondary to-tauri-border"></div>
        </div>

        {/* 时间线右侧内容 */}
        <div className="flex-1 min-w-0">
          <div className="glass border border-tauri-border rounded-2xl p-6 shadow-xl">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center gap-2">
                <span className="text-sm font-semibold gradient-text">Assistant</span>
                {turn.isComplete && (
                  <span className="text-xs text-green-400 flex items-center gap-1">
                    <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 13l4 4L19 7"/>
                    </svg>
                    Done
                  </span>
                )}
              </div>
              <button
                onClick={() => setShowDetails(!showDetails)}
                className="text-gray-400 hover:text-gray-200 transition-colors"
              >
                {showDetails ? (
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 9l-7 7-7-7"/>
                  </svg>
                ) : (
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 5l7 7-7 7"/>
                  </svg>
                )}
              </button>
            </div>

            {showDetails && (
              <div className="space-y-3">
                {turn.parts.map((part, index) => (
                  <PartRenderer
                    key={index}
                    part={part}
                    turnId={turn.id}
                    partIndex={index}
                    isComplete={turn.isComplete}
                  />
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
});

TurnGroup.displayName = 'TurnGroup';
