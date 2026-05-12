export type AppEvent =
  | { type: 'submit_message'; message: string }
  | { type: 'sse_content'; text: string }
  | { type: 'sse_done'; sessionId: string }
  | { type: 'sse_error'; message: string }
  | { type: 'toggle_sidebar' }
  | { type: 'toggle_history' }
  | { type: 'select_model'; model: string }
  | { type: 'switch_session'; id: string }
  | { type: 'stop_generation' };
