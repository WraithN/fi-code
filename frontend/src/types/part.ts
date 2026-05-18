export type Part =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; arguments: Record<string, unknown> }
  | { type: 'tool_result'; tool_call_id: string; content: string; duration_ms?: number }
  | { type: 'tool_error'; tool_call_id: string; content: string; error_message: string }
  | { type: 'thinking'; content: string }
  | { type: 'code_block'; language: string; code: string }
  | { type: 'image'; url: string; alt?: string }
  | { type: 'usage'; prompt_tokens: number; completion_tokens: number }
  | { type: 'wave_marker'; wave_id: string; turn: number };
