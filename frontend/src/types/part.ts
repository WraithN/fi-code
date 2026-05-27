export type ToolResultMetadata = {
  tool_name?: string;
  tool_call_id?: string;
  is_error?: boolean;
  compressed?: boolean;
  truncated?: boolean;
  content_type?: string;
  line_count?: number;
  byte_count?: number;
  [key: string]: unknown;
};

export type Part =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; arguments: Record<string, unknown> }
  | { 
      type: 'tool_result'; 
      tool_call_id: string; 
      content: string; 
      duration_ms?: number;
      metadata?: ToolResultMetadata;
    }
  | { type: 'tool_error'; tool_call_id: string; content: string; error_message: string }
  | { type: 'thinking'; content: string }
  | { type: 'code_block'; language: string; code: string }
  | { type: 'image'; url: string; alt?: string }
  | { type: 'usage'; prompt_tokens: number; completion_tokens: number }
  | { type: 'wave_marker'; wave_id: string; turn: number }
  | { type: 'system_notice'; kind: string; content: string }
  | { 
      type: 'interactive_permission'; 
      tool_call_id: string; 
      tool_name: string; 
      risk: string; 
      reason: string; 
      status: 'pending' | 'approved' | 'rejected';
    }
  | { 
      type: 'interactive_question'; 
      tool_call_id: string; 
      question: string; 
      options: { id: string; label: string; description?: string }[]; 
      recommended?: string;
      allow_custom: boolean;
      status: 'pending' | 'answered';
      answer?: string;
    };
