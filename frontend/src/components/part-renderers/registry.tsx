import React from 'react';
import { Part } from '../../types/part';
import { TextPart } from './TextPart';
import { ThinkingPart } from './ThinkingPart';
import { UsagePart } from './UsagePart';
import { WaveMarkerPart } from './WaveMarkerPart';
import { ToolUsePart } from './ToolUsePart';
import { ToolResultPart } from './ToolResultPart';
import { ToolErrorPart } from './ToolErrorPart';
import { CodeBlockPart } from './CodeBlockPart';
import { ImagePart } from './ImagePart';
import { SystemNoticePart } from './SystemNoticePart';
import { InteractivePermissionPart } from './InteractivePermissionPart';
import { InteractiveQuestionPart } from './InteractiveQuestionPart';

const partRenderers: Record<string, React.FC<{ part: Part; isComplete?: boolean }>> = {
  text: TextPart as React.FC<{ part: Part; isComplete?: boolean }>,
  thinking: ThinkingPart as React.FC<{ part: Part; isComplete?: boolean }>,
  tool_use: ToolUsePart as React.FC<{ part: Part; isComplete?: boolean }>,
  tool_result: ToolResultPart as React.FC<{ part: Part; isComplete?: boolean }>,
  tool_error: ToolErrorPart as React.FC<{ part: Part; isComplete?: boolean }>,
  code_block: CodeBlockPart as React.FC<{ part: Part; isComplete?: boolean }>,
  image: ImagePart as React.FC<{ part: Part; isComplete?: boolean }>,
  usage: UsagePart as React.FC<{ part: Part; isComplete?: boolean }>,
  wave_marker: WaveMarkerPart as React.FC<{ part: Part; isComplete?: boolean }>,
  system_notice: SystemNoticePart as React.FC<{ part: Part; isComplete?: boolean }>,
};

export function PartRenderer({
  part,
  turnId,
  partIndex,
  isComplete,
}: {
  part: Part;
  turnId?: string;
  partIndex?: number;
  isComplete?: boolean;
}) {
  if (part.type === 'interactive_permission' && turnId !== undefined && partIndex !== undefined) {
    return <InteractivePermissionPart turnId={turnId} partIndex={partIndex} part={part as any} />;
  }
  if (part.type === 'interactive_question' && turnId !== undefined && partIndex !== undefined) {
    return <InteractiveQuestionPart turnId={turnId} partIndex={partIndex} part={part as any} />;
  }
  const Renderer = partRenderers[part.type];
  if (!Renderer) {
    return <div className="text-xs text-error">Unknown part type: {part.type}</div>;
  }
  return <Renderer part={part} isComplete={isComplete} />;
}
