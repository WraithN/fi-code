import React, { useState, useCallback, useRef, useEffect } from 'react';
import { useChatStream } from '../../hooks/useChatStream';
import { useUIStore } from '../../stores/uiStore';
import { apiClient } from '../../services/apiClient';
import { CommandMeta } from '../../types/command';
import { ProviderItem, FileEntry } from '../../types/api';
import { themePresets, getPresetByName, applyTheme } from '../../themes';

type SubmenuKind = 'theme' | 'skill' | 'model_provider' | 'model_list' | null;

interface SubmenuItem {
  key: string;
  display: string;
  desc: string;
}

export const InputBox: React.FC = () => {
  const [input, setInput] = useState('');
  const { send } = useChatStream();
  const { commands, themeName, setThemeName } = useUIStore();
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // 一级菜单状态
  const [showMenu, setShowMenu] = useState(false);
  const [highlightIndex, setHighlightIndex] = useState(0);

  // 二级菜单状态
  const [submenuKind, setSubmenuKind] = useState<SubmenuKind>(null);
  const [submenuItems, setSubmenuItems] = useState<SubmenuItem[]>([]);
  const [submenuIndex, setSubmenuIndex] = useState(0);
  const [providers, setProviders] = useState<ProviderItem[]>([]);
  const [previewThemeBackup, setPreviewThemeBackup] = useState<string | null>(null);
  const [submenuLoading, setSubmenuLoading] = useState(false);

  // @ 文件选择器状态
  const [showFilePicker, setShowFilePicker] = useState(false);
  const [pickerPath, setPickerPath] = useState('');
  const [pickerItems, setPickerItems] = useState<FileEntry[]>([]);
  const [pickerIndex, setPickerIndex] = useState(0);
  const [pickerLoading, setPickerLoading] = useState(false);

  // 计算一级菜单过滤列表
  const filterText = showMenu ? input.slice(1) : '';
  const filteredCommands = filterText
    ? commands.filter((c) => c.name.startsWith(filterText))
    : commands;

  useEffect(() => {
    if (highlightIndex >= filteredCommands.length) {
      setHighlightIndex(0);
    }
  }, [filteredCommands.length, highlightIndex]);

  // 主题实时预览
  const previewTheme = useCallback(
    (index: number) => {
      if (submenuKind !== 'theme') return;
      const preset = themePresets[index];
      if (preset) applyTheme(preset);
    },
    [submenuKind]
  );

  const restoreTheme = useCallback(() => {
    if (previewThemeBackup) {
      const preset = getPresetByName(previewThemeBackup);
      if (preset) applyTheme(preset);
      setPreviewThemeBackup(null);
    }
  }, [previewThemeBackup]);

  // 加载 Skill 二级菜单
  const loadSkillsSubmenu = useCallback(async () => {
    setSubmenuLoading(true);
    setSubmenuKind('skill');
    setSubmenuIndex(0);
    try {
      const skills = await apiClient.getSkills();
      setSubmenuItems(
        skills.map((s) => ({ key: s.id, display: s.name, desc: s.description }))
      );
    } catch (err) {
      console.warn('[InputBox] Failed to load skills:', err);
      setSubmenuItems([]);
    } finally {
      setSubmenuLoading(false);
    }
  }, []);

  // 加载 Model Provider 二级菜单
  const loadModelProvidersSubmenu = useCallback(async () => {
    setSubmenuLoading(true);
    setSubmenuKind('model_provider');
    setSubmenuIndex(0);
    try {
      const data = (await apiClient.get<ProviderItem[]>('/api/models')) as ProviderItem[];
      setProviders(data);
      setSubmenuItems(
        data.map((p) => ({ key: p.key, display: p.name, desc: p.provider_type }))
      );
    } catch (err) {
      console.warn('[InputBox] Failed to load providers:', err);
      setSubmenuItems([]);
    } finally {
      setSubmenuLoading(false);
    }
  }, []);

  // 加载 Model List 二级菜单
  const loadModelListSubmenu = useCallback(
    (providerKey: string) => {
      const provider = providers.find((p) => p.key === providerKey);
      if (!provider) return;
      setSubmenuKind('model_list');
      setSubmenuIndex(0);
      setSubmenuItems(
        provider.models.map((m) => ({
          key: m.key,
          display: m.name,
          desc: `ctx: ${m.context}, out: ${m.output}`,
        }))
      );
    },
    [providers]
  );

  // @ 文件选择器：加载文件树
  const loadFileTree = useCallback(async (path: string = '') => {
    setPickerLoading(true);
    try {
      const res = await apiClient.getFileTree(path);
      setPickerItems(res.entries);
      setPickerIndex(0);
    } catch (err) {
      console.warn('[InputBox] Failed to load file tree:', err);
      setPickerItems([]);
    } finally {
      setPickerLoading(false);
    }
  }, []);

  // @ 文件选择器：选择文件
  const selectFile = useCallback(
    (entry: FileEntry) => {
      const filePath = entry.path;
      // 移除输入框末尾的触发式 @，替换为 @filePath
      const newInput = input.replace(/@\s*$/, '') + `@${filePath} `;
      setInput(newInput);
      setShowFilePicker(false);
      setPickerPath('');
      setTimeout(() => {
        const el = textareaRef.current;
        if (el) {
          el.focus();
          el.selectionStart = el.selectionEnd = newInput.length;
        }
      }, 0);
    },
    [input]
  );

  // @ 文件选择器：进入目录
  const enterDirectory = useCallback(
    (entry: FileEntry) => {
      const newPath = entry.path;
      setPickerPath(newPath);
      loadFileTree(newPath);
    },
    [loadFileTree]
  );

  // @ 文件选择器：返回上级
  const goUp = useCallback(() => {
    if (!pickerPath) {
      setShowFilePicker(false);
      return;
    }
    const parts = pickerPath.split('/').filter(Boolean);
    parts.pop();
    const newPath = parts.join('/');
    setPickerPath(newPath);
    loadFileTree(newPath);
  }, [pickerPath, loadFileTree]);

  const handleSubmit = useCallback(() => {
    if (!input.trim()) return;
    send(input);
    setInput('');
    setShowMenu(false);
    closeSubmenu();
    setShowFilePicker(false);
  }, [input, send]);

  const closeSubmenu = useCallback(() => {
    if (submenuKind === 'theme') {
      restoreTheme();
    }
    setSubmenuKind(null);
    setSubmenuItems([]);
    setSubmenuIndex(0);
  }, [submenuKind, restoreTheme]);

  // 确认一级菜单命令
  const confirmCommand = useCallback(
    (cmd: CommandMeta) => {
      if (cmd.name === 'themes') {
        setPreviewThemeBackup(themeName);
        setSubmenuKind('theme');
        setSubmenuItems(
          themePresets.map((p) => ({ key: p.name, display: p.name, desc: p.description }))
        );
        setSubmenuIndex(0);
        setShowMenu(false);
        setInput('');
        return;
      }
      if (cmd.name === 'skills') {
        setShowMenu(false);
        setInput('');
        loadSkillsSubmenu();
        return;
      }
      if (cmd.name === 'models') {
        setShowMenu(false);
        setInput('');
        loadModelProvidersSubmenu();
        return;
      }
      const filled = `/${cmd.name} `;
      setInput(filled);
      setShowMenu(false);
      setTimeout(() => {
        const el = textareaRef.current;
        if (el) {
          el.focus();
          el.selectionStart = el.selectionEnd = filled.length;
        }
      }, 0);
    },
    [themeName, loadSkillsSubmenu, loadModelProvidersSubmenu]
  );

  // 确认二级菜单项
  const confirmSubmenuItem = useCallback(
    async (item: SubmenuItem) => {
      if (submenuKind === 'theme') {
        setThemeName(item.key);
        try {
          await apiClient.executeCommand('themes', item.key);
        } catch (err) {
          console.warn('[InputBox] Failed to switch theme:', err);
        }
        setPreviewThemeBackup(null);
        setSubmenuKind(null);
        return;
      }
      if (submenuKind === 'skill') {
        try {
          await apiClient.executeCommand('skills', item.key);
        } catch (err) {
          console.warn('[InputBox] Failed to load skill:', err);
        }
        setSubmenuKind(null);
        return;
      }
      if (submenuKind === 'model_provider') {
        loadModelListSubmenu(item.key);
        return;
      }
      if (submenuKind === 'model_list') {
        const providerKey = providers.find((p) =>
          p.models.some((m) => m.key === item.key)
        )?.key;
        if (providerKey) {
          try {
            await apiClient.post('/api/model/switch', {
              provider: providerKey,
              model: item.key,
            });
          } catch (err) {
            console.warn('[InputBox] Failed to switch model:', err);
          }
        }
        setSubmenuKind(null);
        return;
      }
    },
    [submenuKind, providers, loadModelListSubmenu, setThemeName]
  );

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // 文件选择器打开时的键盘导航
    if (showFilePicker) {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setPickerIndex((prev) => (prev + 1) % pickerItems.length);
          break;
        case 'ArrowUp':
          e.preventDefault();
          setPickerIndex(
            (prev) => (prev - 1 + pickerItems.length) % pickerItems.length
          );
          break;
        case 'ArrowRight':
          e.preventDefault();
          if (pickerItems.length > 0 && pickerItems[pickerIndex]?.is_dir) {
            enterDirectory(pickerItems[pickerIndex]);
          }
          break;
        case 'ArrowLeft':
          e.preventDefault();
          goUp();
          break;
        case 'Enter':
          e.preventDefault();
          if (pickerItems.length > 0) {
            const entry = pickerItems[pickerIndex];
            if (entry.is_dir) {
              enterDirectory(entry);
            } else {
              selectFile(entry);
            }
          }
          break;
        case 'Escape':
          e.preventDefault();
          setShowFilePicker(false);
          setPickerPath('');
          break;
        default:
          break;
      }
      return;
    }

    // 二级菜单打开时的键盘导航
    if (submenuKind) {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setSubmenuIndex((prev) => {
            const next = (prev + 1) % submenuItems.length;
            if (submenuKind === 'theme') previewTheme(next);
            return next;
          });
          break;
        case 'ArrowUp':
          e.preventDefault();
          setSubmenuIndex((prev) => {
            const next = (prev - 1 + submenuItems.length) % submenuItems.length;
            if (submenuKind === 'theme') previewTheme(next);
            return next;
          });
          break;
        case 'Enter':
          e.preventDefault();
          if (submenuItems.length > 0) {
            confirmSubmenuItem(submenuItems[submenuIndex]);
          }
          break;
        case 'Escape':
          e.preventDefault();
          closeSubmenu();
          break;
        default:
          break;
      }
      return;
    }

    // 一级菜单打开时的键盘导航
    if (showMenu) {
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
      return;
    }

    // 普通输入
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    setInput(val);

    // 自动调整输入框高度
    const el = textareaRef.current;
    if (el) {
      el.style.height = 'auto';
      el.style.height = Math.min(el.scrollHeight, 200) + 'px';
    }

    // 检测 @ 触发文件选择器：独立 @ token（前面是空格或开头），后面无其他内容
    const atTrigger = /(?:^|\s)@\s*$/.test(val);
    if (atTrigger) {
      setShowFilePicker(true);
      setShowMenu(false);
      closeSubmenu();
      setPickerPath('');
      loadFileTree('');
      return;
    }

    if (val.startsWith('/')) {
      setShowMenu(true);
      setHighlightIndex(0);
      setShowFilePicker(false);
    } else {
      setShowMenu(false);
    }
  };

  // 菜单标题
  const submenuTitle =
    submenuKind === 'theme'
      ? 'Select Theme'
      : submenuKind === 'skill'
      ? 'Select Skill'
      : submenuKind === 'model_provider'
      ? 'Select Provider'
      : submenuKind === 'model_list'
      ? 'Select Model'
      : '';

  return (
    <div className="p-6 glass border-t border-tauri-border relative">
      {/* @ 文件选择器 */}
      {showFilePicker && (
        <div className="absolute bottom-full left-6 right-6 mb-3 max-h-60 overflow-y-auto glass border border-tauri-border rounded-2xl shadow-2xl z-50">
          <div className="px-4 py-3 text-sm font-semibold text-gray-300 border-b border-tauri-border bg-tauri-card/50 flex items-center justify-between">
            <div className="flex items-center gap-2">
              <svg className="w-4 h-4 text-tauri-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
              </svg>
              <span>Select File</span>
            </div>
            {pickerPath && (
              <span className="text-gray-500 font-mono truncate max-w-[200px] text-xs">{pickerPath}</span>
            )}
          </div>
          {pickerLoading ? (
            <div className="px-4 py-6 text-sm text-gray-500 flex items-center justify-center gap-2">
              <div className="w-4 h-4 border-2 border-tauri-primary border-t-transparent rounded-full animate-spin"></div>
              Loading...
            </div>
          ) : pickerItems.length === 0 ? (
            <div className="px-4 py-6 text-sm text-gray-500 text-center">No files</div>
          ) : (
            <div className="scrollbar-tauri">
              {pickerPath && (
                <div
                  className="px-4 py-3 cursor-pointer text-sm text-gray-400 hover:bg-tauri-card/50 flex items-center gap-3 transition-colors"
                  onClick={goUp}
                >
                  <svg className="w-4 h-4 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M11 19l-7-7 7-7m8 14l-7-7 7-7"/>
                  </svg>
                  <span>..</span>
                </div>
              )}
              {pickerItems.map((entry, idx) => (
                <div
                  key={entry.path}
                  className={`px-4 py-3 cursor-pointer text-sm flex items-center gap-3 transition-all ${
                    idx === pickerIndex
                      ? 'bg-tauri-card/70 text-tauri-primary border-l-2 border-tauri-primary'
                      : 'text-gray-200 hover:bg-tauri-card/30'
                  }`}
                  onMouseEnter={() => setPickerIndex(idx)}
                  onClick={() => {
                    if (entry.is_dir) {
                      enterDirectory(entry);
                    } else {
                      selectFile(entry);
                    }
                  }}
                >
                  {entry.is_dir ? (
                    <svg className="w-4 h-4 text-tauri-primary flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
                    </svg>
                  ) : (
                    <svg className="w-4 h-4 text-gray-500 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
                    </svg>
                  )}
                  <span className="truncate">{entry.name}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* 一级 Slash 指令菜单 */}
      {showMenu && filteredCommands.length > 0 && !submenuKind && (
        <div className="absolute bottom-full left-6 right-6 mb-3 max-h-48 overflow-y-auto glass border border-tauri-border rounded-2xl shadow-2xl z-50">
          <div className="scrollbar-tauri">
            {filteredCommands.map((cmd, idx) => (
              <div
                key={cmd.name}
                className={`px-4 py-3 cursor-pointer text-sm flex items-center justify-between transition-all ${
                  idx === highlightIndex
                    ? 'bg-tauri-card/70 gradient-text border-l-2 border-tauri-primary'
                    : 'text-gray-200 hover:bg-tauri-card/30'
                }`}
                onMouseEnter={() => setHighlightIndex(idx)}
                onClick={() => confirmCommand(cmd)}
              >
                <div className="flex items-center gap-3">
                  <span className="font-bold">/{cmd.name}</span>
                  <span className="text-gray-500 text-xs">{cmd.description}</span>
                </div>
                {cmd.args_hint && (
                  <span className="text-gray-500 text-xs font-mono bg-tauri-dark/50 px-2 py-1 rounded">
                    {cmd.args_hint}
                  </span>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 二级菜单 */}
      {submenuKind && (
        <div className="absolute bottom-full left-6 right-6 mb-3 max-h-60 overflow-y-auto glass border border-tauri-border rounded-2xl shadow-2xl z-50">
          <div className="px-4 py-3 text-sm font-semibold gradient-text border-b border-tauri-border bg-tauri-card/50">
            {submenuTitle}
          </div>
          <div className="scrollbar-tauri">
            {submenuLoading ? (
              <div className="px-4 py-6 text-sm text-gray-500 flex items-center justify-center gap-2">
                <div className="w-4 h-4 border-2 border-tauri-primary border-t-transparent rounded-full animate-spin"></div>
                Loading...
              </div>
            ) : submenuItems.length === 0 ? (
              <div className="px-4 py-6 text-sm text-gray-500 text-center">No items</div>
            ) : (
              submenuItems.map((item, idx) => (
                <div
                  key={item.key}
                  className={`px-4 py-3 cursor-pointer text-sm flex items-center justify-between transition-all ${
                    idx === submenuIndex
                      ? 'bg-tauri-card/70 gradient-text border-l-2 border-tauri-primary'
                      : 'text-gray-200 hover:bg-tauri-card/30'
                  }`}
                  onMouseEnter={() => {
                    setSubmenuIndex(idx);
                    if (submenuKind === 'theme') previewTheme(idx);
                  }}
                  onClick={() => confirmSubmenuItem(item)}
                >
                  <div className="flex items-center gap-3">
                    <span className="font-bold">{item.display}</span>
                    <span className="text-gray-500 text-xs">{item.desc}</span>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      )}

      <div className="flex gap-3">
        <div className="flex-1 relative">
          <textarea
            ref={textareaRef}
            value={input}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            placeholder="Type a message... or / for commands, @ for files"
            rows={2}
            className="w-full bg-tauri-dark/50 text-gray-100 border border-tauri-border rounded-2xl px-5 py-4 text-sm resize-none focus:outline-none focus:border-tauri-primary focus:ring-1 focus:ring-tauri-primary/30 scrollbar-tauri placeholder-gray-600"
          />
          <div className="absolute right-4 bottom-4 text-xs text-gray-600 font-mono">
            ⇧⏎ for newline
          </div>
        </div>
        <button
          onClick={handleSubmit}
          disabled={!input.trim()}
          className="px-5 py-3 gradient-bg text-white rounded-xl text-sm font-medium flex items-end space-x-2 shadow-lg hover:shadow-tauri-primary/40 hover:-translate-y-0.5 transition-all disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:shadow-none disabled:hover:translate-y-0"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z"/>
          </svg>
          <span>Send</span>
        </button>
      </div>
    </div>
  );
};
