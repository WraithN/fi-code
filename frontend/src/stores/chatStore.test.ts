import { describe, it, expect, beforeEach } from 'vitest';
import { useChatStore } from './chatStore';

describe('chatStore', () => {
  beforeEach(() => {
    useChatStore.getState().clearTurns();
    useChatStore.getState().setAgent('build');
  });

  it('should start a new turn', () => {
    const turnId = useChatStore.getState().startTurn('hello');
    expect(turnId).toBeDefined();

    const state = useChatStore.getState();
    expect(state.turns).toHaveLength(1);
    expect(state.turns[0].userMessage).toBe('hello');
    expect(state.turns[0].isComplete).toBe(false);
    expect(state.isGenerating).toBe(true);
  });

  it('should append part to current turn', () => {
    const turnId = useChatStore.getState().startTurn('hello');
    useChatStore.getState().appendPart(turnId, { type: 'text', text: 'world' });

    const state = useChatStore.getState();
    expect(state.turns[0].parts).toHaveLength(1);
    expect(state.turns[0].parts[0]).toEqual({ type: 'text', text: 'world' });
  });

  it('should complete turn', () => {
    const turnId = useChatStore.getState().startTurn('hello');
    useChatStore.getState().completeTurn(turnId);

    const state = useChatStore.getState();
    expect(state.turns[0].isComplete).toBe(true);
    expect(state.isGenerating).toBe(false);
  });

  it('should switch agent', () => {
    useChatStore.getState().setAgent('plan');
    expect(useChatStore.getState().currentAgent).toBe('plan');
  });
});
