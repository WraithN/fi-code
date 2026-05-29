import React from 'react';
import { Part } from '../../types/part';

// Token 数用 K/M 做单位
function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return `${n}`;
}

// 耗时：小于 1000ms 显示 ms，否则显示秒
function formatLatency(ms: number): string {
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
  return `${ms}ms`;
}

export const UsagePart: React.FC<{ part: Extract<Part, { type: 'usage' }> }> = ({ part }) => (
  <div className="text-xs text-text-muted mt-2 font-mono">
    ⬆️ {formatTokens(part.prompt_tokens)}  ⬇️ {formatTokens(part.completion_tokens)}  ·  ⏱ {formatLatency(part.latency_ms)}
  </div>
);
