import React, { useState } from 'react';
import { apiClient } from '../../services/apiClient';
import { useChatStore } from '../../stores/chatStore';
import { Part } from '../../types/part';

interface Props {
  turnId: string;
  partIndex: number;
  part: Extract<Part, { type: 'interactive_question' }>;
}

export const InteractiveQuestionPart: React.FC<Props> = ({ turnId, partIndex, part }) => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [customAnswer, setCustomAnswer] = useState('');
  const updatePart = useChatStore((s) => s.updatePart);

  const handleSelectOption = async (optionId: string, label: string) => {
    if (loading || part.status !== 'pending') return;
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
    <div className="my-2 p-4 glass border border-tauri-border rounded-2xl space-y-3">
      <div className="text-sm font-medium text-text">
        {part.question}
      </div>

      {part.status === 'pending' && (
        <div className="space-y-2">
          {part.options.map((opt: { id: string; label: string; description?: string }) => (
            <button
              key={opt.id}
              onClick={() => handleSelectOption(opt.id, opt.label)}
              disabled={loading}
              className={`w-full text-left px-4 py-3 rounded-xl text-sm transition-colors disabled:opacity-50 ${
                part.recommended === opt.id
                  ? 'bg-primary/20 text-primary border border-primary/30 hover:bg-primary/30'
                  : 'bg-bg-tertiary text-text hover:bg-bg-overlay border border-transparent'
              }`}
            >
              <div className="flex items-center gap-2">
                {part.recommended === opt.id && (
                  <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                    <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                  </svg>
                )}
                <span className="font-medium">{opt.label}</span>
              </div>
              {opt.description && (
                <div className="text-xs text-text-muted mt-1 ml-6">{opt.description}</div>
              )}
            </button>
          ))}

          {part.allow_custom && (
            <div className="flex gap-2 pt-1">
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
                placeholder="自定义回答..."
                disabled={loading}
                className="flex-1 bg-bg-tertiary text-text text-sm rounded-xl px-4 py-2.5 border border-tauri-border focus:outline-none focus:border-primary disabled:opacity-50"
              />
              <button
                onClick={handleCustomAnswer}
                disabled={!customAnswer.trim() || loading}
                className="px-4 py-2.5 rounded-xl bg-primary text-white hover:bg-primary-hover transition-colors text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
              >
                发送
              </button>
            </div>
          )}
        </div>
      )}

      {part.status === 'answered' && (
        <div className="text-sm text-green-400 font-medium">
          ✓ 已回答: {part.answer}
        </div>
      )}

      {error && (
        <div className="text-sm text-red-400">{error}</div>
      )}
    </div>
  );
};
