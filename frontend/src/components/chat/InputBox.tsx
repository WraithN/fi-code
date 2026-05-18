import React, { useState, useCallback, useRef, useEffect } from 'react';
import { useChatStream } from '../../hooks/useChatStream';
import { useUIStore } from '../../stores/uiStore';
import { CommandMeta } from '../../types/command';

export const InputBox: React.FC = () => {
  const [input, setInput] = useState('');
  const { send } = useChatStream();
  const { commands } = useUIStore();
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Slash 菜单状态
  const [showMenu, setShowMenu] = useState(false);
  const [highlightIndex, setHighlightIndex] = useState(0);

  // 计算过滤后的指令列表
  const filterText = showMenu ? input.slice(1) : '';
  const filteredCommands = filterText
    ? commands.filter((c) => c.name.startsWith(filterText))
    : commands;

  // 高亮索引越界时重置
  useEffect(() => {
    if (highlightIndex >= filteredCommands.length) {
      setHighlightIndex(0);
    }
  }, [filteredCommands.length, highlightIndex]);

  const handleSubmit = useCallback(() => {
    if (!input.trim()) return;
    send(input);
    setInput('');
    setShowMenu(false);
  }, [input, send]);

  const confirmCommand = useCallback(
    (cmd: CommandMeta) => {
      const filled = `/${cmd.name} `;
      setInput(filled);
      setShowMenu(false);
      // 光标移到末尾
      setTimeout(() => {
        const el = textareaRef.current;
        if (el) {
          el.focus();
          el.selectionStart = el.selectionEnd = filled.length;
        }
      }, 0);
    },
    []
  );

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (!showMenu) {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
      }
      return;
    }

    // 菜单打开时的键盘导航
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setHighlightIndex((prev) => (prev + 1) % filteredCommands.length);
        break;
      case 'ArrowUp':
        e.preventDefault();
        setHighlightIndex(
          (prev) => (prev - 1 + filteredCommands.length) % filteredCommands.length
        );
        break;
      case 'Enter':
      case 'Tab':
        e.preventDefault();
        if (filteredCommands.length > 0) {
          confirmCommand(filteredCommands[highlightIndex]);
        }
        break;
      case 'Escape':
        e.preventDefault();
        setShowMenu(false);
        break;
      default:
        break;
    }
  };

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    setInput(val);

    if (val.startsWith('/')) {
      setShowMenu(true);
      setHighlightIndex(0);
    } else {
      setShowMenu(false);
    }
  };

  return (
    <div className="p-4 bg-bg-secondary border-t border-border relative">
      {/* Slash 指令菜单 */}
      {showMenu && filteredCommands.length > 0 && (
        <div className="absolute bottom-full left-4 right-4 mb-2 max-h-48 overflow-y-auto bg-bg-secondary border border-border rounded shadow-lg z-50">
          {filteredCommands.map((cmd, idx) => (
            <div
              key={cmd.name}
              className={`px-3 py-2 cursor-pointer text-sm flex items-center justify-between ${
                idx === highlightIndex
                  ? 'bg-bg-overlay text-brand'
                  : 'text-text-primary hover:bg-bg-overlay'
              }`}
              onMouseEnter={() => setHighlightIndex(idx)}
              onClick={() => confirmCommand(cmd)}
            >
              <div className="flex items-center gap-2">
                <span className="font-bold">/{cmd.name}</span>
                <span className="text-text-muted text-xs">{cmd.description}</span>
              </div>
              {cmd.args_hint && (
                <span className="text-text-muted text-xs font-mono">{cmd.args_hint}</span>
              )}
            </div>
          ))}
        </div>
      )}

      <div className="flex gap-2">
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleChange}
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
