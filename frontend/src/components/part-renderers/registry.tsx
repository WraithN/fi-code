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

const partRenderers: Record<string, React.FC<{ part: Part }>> = {
  text: TextPart as React.FC<{ part: Part }>,
  thinking: ThinkingPart as React.FC<{ part: Part }>,
  tool_use: ToolUsePart as React.FC<{ part: Part }>,
  tool_result: ToolResultPart as React.FC<{ part: Part }>,
  tool_error: ToolErrorPart as React.FC<{ part: Part }>,
  code_block: CodeBlockPart as React.FC<{ part: Part }>,
  image: ImagePart as React.FC<{ part: Part }>,
  usage: UsagePart as React.FC<{ part: Part }>,
  wave_marker: WaveMarkerPart as React.FC<{ part: Part }>,
};

export function PartRenderer({ part }: { part: Part }) {
  const Renderer = partRenderers[part.type];
  if (!Renderer) {
    return <div className="text-xs text-error">Unknown part type: {part.type}</div>;
  }
  return <Renderer part={part} />;
}
