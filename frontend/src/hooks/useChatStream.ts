import { useCallback } from 'react';
import { useAppStore } from '../stores/appStore';
import { sendMessage } from '../services/chat';
import { SseEvent } from '../types/api';

export function useChatStream() {
  const currentSessionId = useAppStore(s => s.currentSessionId);
  const setCurrentSessionId = useAppStore(s => s.setCurrentSessionId);
  const setIsGenerating = useAppStore(s => s.setIsGenerating);
  const addMessage = useAppStore(s => s.addMessage);
  const appendToLastMessage = useAppStore(s => s.appendToLastMessage);
  const addSystemMessage = useAppStore(s => s.addSystemMessage);

  const send = useCallback(
    async (message: string) => {
      if (!message.trim()) return;

      addMessage({
        id: `user-${Date.now()}`,
        role: 'user',
        content: message,
        timestamp: Date.now(),
      });

      addMessage({
        id: `assistant-${Date.now()}`,
        role: 'assistant',
        content: '',
        timestamp: Date.now(),
      });

      setIsGenerating(true);

      try {
        const stream = await sendMessage(currentSessionId, message);
        const reader = stream.getReader();

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          const event = value as SseEvent;

          switch (event.type) {
            case 'content':
              appendToLastMessage(event.text);
              break;
            case 'done':
              setCurrentSessionId(event.session_id);
              setIsGenerating(false);
              break;
            case 'error':
              addSystemMessage(`Error: ${event.message}`);
              setIsGenerating(false);
              break;
          }
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Unknown error';
        addSystemMessage(`Stream error: ${message}`);
        setIsGenerating(false);
      }
    },
    [
      currentSessionId,
      setCurrentSessionId,
      setIsGenerating,
      addMessage,
      appendToLastMessage,
      addSystemMessage,
    ]
  );

  const stop = useCallback(() => {
    setIsGenerating(false);
  }, [setIsGenerating]);

  return { send, stop };
}
