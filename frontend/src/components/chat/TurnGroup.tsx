import React, { useState } from 'react';
import { Turn } from '../../types/turn';
import { PartRenderer } from '../part-renderers/registry';

export const TurnGroup: React.FC<{ turn: Turn }> = ({ turn }) => {
  const [showDetails, setShowDetails] = useState(true);

  return (
    <div className="space-y-4">
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
                className="p-1 hover:bg-tauri-card rounded-lg transition-colors"
              >
                {showDetails ? (
                  <svg className="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M20 12H4"/>
                  </svg>
                ) : (
                  <svg className="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 4v16m8-8H4"/>
                  </svg>
                )}
              </button>
            </div>

            <div className={showDetails ? "space-y-4" : "hidden"}>
              {turn.parts.length === 0 && !turn.isComplete ? (
                <div className="flex items-center gap-2 text-gray-400">
                  <div className="w-2 h-2 bg-tauri-primary rounded-full animate-pulse"></div>
                  <div className="w-2 h-2 bg-tauri-secondary rounded-full animate-pulse" style={{ animationDelay: '0.2s' }}></div>
                  <div className="w-2 h-2 bg-tauri-primary rounded-full animate-pulse" style={{ animationDelay: '0.4s' }}></div>
                  <span className="text-sm">Thinking...</span>
                </div>
              ) : (
                turn.parts
                  .filter(part => {
                    if ('for_context_only' in part) {
                      return !part.for_context_only;
                    }
                    return true;
                  })
                  .map((part, i) => <PartRenderer key={i} part={part} turnId={turn.id} partIndex={i} />)
              )}
            </div>

            {!showDetails && turn.parts.length > 0 && (
              <div className="text-sm text-gray-500">
                {turn.parts.length} part{turn.parts.length !== 1 ? 's' : ''} hidden
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
