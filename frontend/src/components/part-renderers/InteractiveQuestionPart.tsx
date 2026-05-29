import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { apiClient } from '../../services/apiClient';
import { useChatStore } from '../../stores/chatStore';
import { Part } from '../../types/part';
import { DynamicOption } from './DynamicOption';

interface Props {
  turnId: string;
  partIndex: number;
  part: Extract<Part, { type: 'interactive_question' }>;
}

export const InteractiveQuestionPart: React.FC<Props> = ({ turnId, partIndex, part }) => {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [customAnswer, setCustomAnswer] = useState('');
  const [selectedOption, setSelectedOption] = useState<string | null>(null);
  const updatePart = useChatStore((s) => s.updatePart);

  const handleSelectOption = async (optionId: string, label: string) => {
    if (loading || part.status !== 'pending') return;
    setSelectedOption(optionId);
    setLoading(true);
    setError(null);
    try {
      await apiClient.respondQuestion(part.tool_call_id, { type: 'option', id: optionId, label });
      updatePart(turnId, partIndex, (p) => ({
        ...(p as any),
        status: 'answered',
        answer: label,
      }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to respond');
      setSelectedOption(null);
    } finally {
      setLoading(false);
    }
  };

  const handleCustomAnswer = async () => {
    if (loading || part.status !== 'pending' || !customAnswer.trim()) return;
    setLoading(true);
    setError(null);
    try {
      await apiClient.respondQuestion(part.tool_call_id, { type: 'custom', value: customAnswer.trim() });
      updatePart(turnId, partIndex, (p) => ({
        ...(p as any),
        status: 'answered',
        answer: customAnswer.trim(),
      }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to send');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="my-3">
      <div className="glass border border-tauri-border rounded-2xl p-5 shadow-lg">
        {/* 问题标题 */}
        <p className="text-sm font-medium text-text mb-4 leading-relaxed">
          {part.question}
        </p>

        {part.status === 'pending' && (
          <div className="space-y-3">
            {/* 选项列表 - 使用 DynamicOption 组件 */}
            {part.options.map((opt: { id: string; label: string; description?: string }) => (
              <DynamicOption
                key={opt.id}
                label={opt.label}
                description={opt.description}
                selected={selectedOption === opt.id}
                recommended={part.recommended === opt.id}
                disabled={loading}
                onClick={() => handleSelectOption(opt.id, opt.label)}
              />
            ))}

            {/* 自定义输入 */}
            {part.allow_custom && (
              <div className="flex items-center gap-3 mt-4 pt-2">
                <input
                  type="text"
                  value={customAnswer}
                  onChange={(e) => setCustomAnswer(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      handleCustomAnswer();
                    }
                  }}
                  placeholder={t('question.customPlaceholder')}
                  disabled={loading}
                  className="
                    flex-1 bg-tauri-dark rounded-xl px-4 py-2.5
                    text-text text-sm placeholder-text-muted
                    border border-tauri-border/50
                    focus:outline-none focus:border-primary/60
                    focus:shadow-[0_0_0_2px_rgba(36,200,219,0.2)]
                    disabled:opacity-50 transition-all
                  "
                />
                <button
                  onClick={handleCustomAnswer}
                  disabled={!customAnswer.trim() || loading}
                  className="
                    gradient-bg text-white px-4 py-2.5 rounded-xl
                    text-sm font-medium flex items-center justify-center gap-2
                    hover:shadow-lg hover:shadow-primary/30
                    transition-all
                    disabled:opacity-50 disabled:cursor-not-allowed
                    disabled:hover:shadow-none
                  "
                >
                  <svg
                    className="w-4 h-4"
                    style={{ transform: 'rotate(-45deg)' }}
                    fill="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z" />
                  </svg>
                  <span>{t('question.sendButton')}</span>
                </button>
              </div>
            )}
          </div>
        )}

        {/* 已回答状态 */}
        {part.status === 'answered' && (
          <div className="flex items-center gap-2 mt-3 pt-3 border-t border-tauri-border/30">
            <div className="w-5 h-5 rounded-full bg-green-500/20 flex items-center justify-center">
              <svg
                className="w-3 h-3 text-green-400"
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
            <span className="text-sm text-green-400 font-medium">
              {t('question.answered', { answer: part.answer })}
            </span>
          </div>
        )}

        {error && (
          <div className="mt-3 text-sm text-error bg-error/10 rounded-lg px-3 py-2">
            {error}
          </div>
        )}
      </div>
    </div>
  );
};
