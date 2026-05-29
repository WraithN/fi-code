import React, { useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useChatStore } from '../../stores/chatStore';
import { TurnGroup } from './TurnGroup';

export const ChatPanel: React.FC = () => {
  const { t } = useTranslation();
  const turns = useChatStore((s) => s.turns);
  const isGenerating = useChatStore((s) => s.isGenerating);
  const parentRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: turns.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 200,
    overscan: 3,
    measureElement: (el) => el.getBoundingClientRect().height,
  });

  // 自动滚动到底部（仅在生成中时）
  useEffect(() => {
    if (isGenerating && parentRef.current) {
      const items = virtualizer.getVirtualItems();
      if (items.length > 0) {
        virtualizer.scrollToIndex(turns.length - 1, { align: 'end' });
      }
    }
  }, [turns.length, isGenerating, virtualizer]);

  const virtualItems = virtualizer.getVirtualItems();

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div
        ref={parentRef}
        className="flex-1 overflow-y-auto p-6 scrollbar-tauri"
        style={{ contain: 'strict' }}
      >
        {turns.length === 0 ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center max-w-lg">
              <div className="w-20 h-20 mx-auto mb-6 rounded-2xl gradient-bg flex items-center justify-center">
                <svg className="w-10 h-10 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"/>
                </svg>
              </div>
              <h2 className="text-2xl font-bold gradient-text mb-3">{t('chat.emptyTitle')}</h2>
              <p className="text-gray-400 mb-2">{t('chat.emptySubtitle')}</p>
              <p className="text-sm text-gray-500">{t('chat.emptyHint')}</p>
            </div>
          </div>
        ) : (
          <div
            style={{
              height: `${virtualizer.getTotalSize()}px`,
              width: '100%',
              position: 'relative',
            }}
          >
            {virtualItems.map((virtualItem) => (
              <div
                key={virtualItem.key}
                data-index={virtualItem.index}
                ref={virtualizer.measureElement}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  transform: `translateY(${virtualItem.start}px)`,
                }}
              >
                <TurnGroup turn={turns[virtualItem.index]} />
              </div>
            ))}
          </div>
        )}
      </div>

      {isGenerating && (
        <div className="h-1 w-full bg-tauri-border overflow-hidden relative">
          <div className="absolute h-full w-1/4 gradient-bg animate-progress-slide rounded-full" />
        </div>
      )}
    </div>
  );
};
