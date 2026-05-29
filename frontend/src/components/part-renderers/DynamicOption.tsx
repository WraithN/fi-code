import React from 'react';

interface Props {
  selected: boolean;
  recommended?: boolean;
  disabled?: boolean;
  onClick: () => void;
  label: string;
  description?: string;
}

/**
 * 通用动态选项组件
 * 专为 LLM 动态生成的内容设计，无内容特定图标
 * 选中时显示动画化的对勾指示器
 */
export const DynamicOption: React.FC<Props> = ({
  selected,
  recommended,
  disabled,
  onClick,
  label,
  description,
}) => {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`
        w-full text-left rounded-xl p-4 cursor-pointer
        transition-all duration-200 ease-[cubic-bezier(0.4,0,0.2,1)]
        disabled:opacity-50 disabled:cursor-not-allowed
        ${selected
          ? 'bg-primary/10 border border-primary/60 shadow-[0_0_0_1px_rgba(36,200,219,0.3),0_4px_16px_rgba(0,0,0,0.25)] translate-x-[3px]'
          : 'border border-tauri-border/50 hover:bg-tauri-border/30 hover:translate-x-[3px] hover:border-primary/30'
        }
      `}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex-1 min-w-0">
          <span
            className={`font-medium text-sm ${recommended ? 'text-primary' : 'text-text'}`}
          >
            {label}
          </span>
          {description && (
            <p className="text-xs text-text-muted mt-1.5 leading-relaxed">
              {description}
            </p>
          )}
        </div>

        {/* 选中指示器 - 带动画 */}
        <div
          className={`
            flex-shrink-0 w-5 h-5 rounded-full bg-primary
            flex items-center justify-center
            transition-all duration-200 ease-[cubic-bezier(0.4,0,0.2,1)]
            ${selected
              ? 'opacity-100 scale-100 translate-x-0'
              : 'opacity-0 scale-75 -translate-x-1'
            }
          `}
        >
          <svg
            className="w-3 h-3 text-white"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={3}
              d="M5 13l4 4L19 7"
            />
          </svg>
        </div>
      </div>
    </button>
  );
};
