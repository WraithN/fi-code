import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { apiClient } from '../../services/apiClient';
import { useChatStore } from '../../stores/chatStore';
import { Part } from '../../types/part';

interface Props {
  turnId: string;
  partIndex: number;
  part: Extract<Part, { type: 'interactive_permission' }>;
}

/**
 * 风险等级对应的视觉配置
 */
const RISK_STYLES: Record<string, { dot: string; text: string; bg: string; border: string }> = {
  Critical: {
    dot: 'bg-red-500',
    text: 'text-red-400',
    bg: 'bg-red-500/10',
    border: 'border-red-500/30',
  },
  High: {
    dot: 'bg-orange-500',
    text: 'text-orange-400',
    bg: 'bg-orange-500/10',
    border: 'border-orange-500/30',
  },
  Medium: {
    dot: 'bg-yellow-500',
    text: 'text-yellow-400',
    bg: 'bg-yellow-500/10',
    border: 'border-yellow-500/30',
  },
  Low: {
    dot: 'bg-green-500',
    text: 'text-green-400',
    bg: 'bg-green-500/10',
    border: 'border-green-500/30',
  },
};

export const InteractivePermissionPart: React.FC<Props> = ({ turnId, partIndex, part }) => {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const updatePart = useChatStore((s) => s.updatePart);

  const riskStyle = RISK_STYLES[part.risk] || RISK_STYLES.Low;

  const handleApprove = async () => {
    if (loading || part.status !== 'pending') return;
    setLoading(true);
    setError(null);
    try {
      await apiClient.respondPermission(part.tool_call_id, true);
      updatePart(turnId, partIndex, (p) => ({ ...(p as any), status: 'approved' }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to approve');
    } finally {
      setLoading(false);
    }
  };

  const handleReject = async () => {
    if (loading || part.status !== 'pending') return;
    setLoading(true);
    setError(null);
    try {
      await apiClient.respondPermission(part.tool_call_id, false);
      updatePart(turnId, partIndex, (p) => ({ ...(p as any), status: 'rejected' }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reject');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="my-3">
      <div className="glass border border-tauri-border rounded-2xl p-5 shadow-lg">
        {/* 头部信息 */}
        <div className="space-y-2.5 mb-4">
          <div className="flex items-center gap-2 text-sm">
            <span className="text-text-muted">{t('permission.toolLabel')}:</span>
            <span className="font-mono text-text font-medium bg-tauri-dark px-2 py-0.5 rounded-md border border-tauri-border/50">
              {part.tool_name}
            </span>
          </div>

          <div className="flex items-center gap-2 text-sm">
            <span className="text-text-muted">{t('permission.riskLabel')}:</span>
            <span
              className={`
                inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md
                text-xs font-semibold border
                ${riskStyle.text} ${riskStyle.bg} ${riskStyle.border}
              `}
            >
              <span className={`w-1.5 h-1.5 rounded-full ${riskStyle.dot}`} />
              {part.risk}
            </span>
          </div>
        </div>

        {/* 原因说明 */}
        <div className="bg-tauri-dark/50 border border-tauri-border/30 rounded-xl px-4 py-3 mb-4">
          <p className="text-sm text-text-secondary leading-relaxed">
            {part.reason}
          </p>
        </div>

        {/* 操作按钮 */}
        {part.status === 'pending' && (
          <div className="flex gap-3">
            <button
              onClick={handleReject}
              disabled={loading}
              className="
                flex-1 px-4 py-3 rounded-xl
                bg-tauri-card border border-tauri-border
                text-text text-sm font-medium
                hover:bg-tauri-border/50 hover:border-tauri-border
                transition-all
                disabled:opacity-50 disabled:cursor-not-allowed
              "
            >
              {t('permission.reject')}
            </button>

            <button
              onClick={handleApprove}
              disabled={loading}
              className="
                flex-1 px-4 py-3 rounded-xl
                gradient-bg text-white text-sm font-medium
                hover:shadow-lg hover:shadow-primary/30
                hover:-translate-y-0.5
                transition-all
                disabled:opacity-50 disabled:cursor-not-allowed
                disabled:hover:translate-y-0 disabled:hover:shadow-none
              "
            >
              {loading ? (
                <span className="flex items-center justify-center gap-2">
                  <svg className="animate-spin h-4 w-4" viewBox="0 0 24 24">
                    <circle
                      className="opacity-25"
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      strokeWidth="4"
                      fill="none"
                    />
                    <path
                      className="opacity-75"
                      fill="currentColor"
                      d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                    />
                  </svg>
                  {t('chat.thinking')}
                </span>
              ) : (
                t('permission.approve')
              )}
            </button>
          </div>
        )}

        {/* 已处理状态 */}
        {part.status === 'approved' && (
          <div className="flex items-center gap-2 bg-green-500/10 border border-green-500/20 rounded-xl px-4 py-3">
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
              {t('permission.approved')}
            </span>
          </div>
        )}

        {part.status === 'rejected' && (
          <div className="flex items-center gap-2 bg-red-500/10 border border-red-500/20 rounded-xl px-4 py-3">
            <div className="w-5 h-5 rounded-full bg-red-500/20 flex items-center justify-center">
              <svg
                className="w-3 h-3 text-red-400"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={3}
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </div>
            <span className="text-sm text-red-400 font-medium">
              {t('permission.rejected')}
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
