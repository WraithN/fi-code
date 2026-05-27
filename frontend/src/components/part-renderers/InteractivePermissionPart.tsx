import React, { useState } from 'react';
import { apiClient } from '../../services/apiClient';
import { useChatStore } from '../../stores/chatStore';
import { Part } from '../../types/part';

interface Props {
  turnId: string;
  partIndex: number;
  part: Extract<Part, { type: 'interactive_permission' }>;
}

export const InteractivePermissionPart: React.FC<Props> = ({ turnId, partIndex, part }) => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const updatePart = useChatStore((s) => s.updatePart);

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
    <div className="my-2 p-4 glass border border-tauri-border rounded-2xl space-y-3">
      <div className="flex items-center gap-2 text-sm text-text-muted">
        <span>工具:</span>
        <span className="font-mono">{part.tool_name}</span>
      </div>
      <div className="flex items-center gap-2 text-sm">
        <span className="text-text-muted">风险等级:</span>
        <span className={`font-semibold ${
          part.risk === 'Critical'
            ? 'text-red-500'
            : part.risk === 'High'
            ? 'text-orange-500'
            : 'text-yellow-500'
        }`}>
          {part.risk}
        </span>
      </div>
      <p className="text-sm text-text-secondary bg-bg-tertiary rounded-lg px-3 py-2">
        {part.reason}
      </p>

      {part.status === 'pending' && (
        <div className="flex gap-3 pt-1">
          <button
            onClick={handleReject}
            disabled={loading}
            className="flex-1 px-4 py-2.5 rounded-xl bg-bg-tertiary text-text hover:bg-bg-overlay transition-colors text-sm font-medium disabled:opacity-50"
          >
            拒绝
          </button>
          <button
            onClick={handleApprove}
            disabled={loading}
            className="flex-1 px-4 py-2.5 rounded-xl bg-primary text-white hover:bg-primary-hover transition-colors text-sm font-medium disabled:opacity-50"
          >
            {loading ? '处理中...' : '确认执行'}
          </button>
        </div>
      )}

      {part.status === 'approved' && (
        <div className="text-sm text-green-400 font-medium">✓ 已确认执行</div>
      )}
      {part.status === 'rejected' && (
        <div className="text-sm text-red-400 font-medium">✗ 已拒绝</div>
      )}

      {error && (
        <div className="text-sm text-red-400">{error}</div>
      )}
    </div>
  );
};
