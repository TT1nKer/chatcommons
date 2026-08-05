import { describe, expect, it } from 'vitest';
import {
  shouldScrollToLatest,
  shouldSubmitComposer,
} from './App';

describe('composer keyboard behavior', () => {
  it('submits a normal Enter press', () => {
    expect(shouldSubmitComposer({
      key: 'Enter',
      shiftKey: false,
      isComposing: false,
      keyCode: 13,
    })).toBe(true);
  });

  it('does not submit while an input method is confirming composition', () => {
    expect(shouldSubmitComposer({
      key: 'Enter',
      shiftKey: false,
      isComposing: true,
      keyCode: 13,
    })).toBe(false);
    expect(shouldSubmitComposer({
      key: 'Enter',
      shiftKey: false,
      isComposing: false,
      keyCode: 229,
    })).toBe(false);
  });

  it('keeps Shift Enter as a newline', () => {
    expect(shouldSubmitComposer({
      key: 'Enter',
      shiftKey: true,
      isComposing: false,
      keyCode: 13,
    })).toBe(false);
  });
});

describe('message list behavior', () => {
  it('scrolls for a room change or a newly sent own message', () => {
    const ownMessage = {
      id: 'message',
      author: 'tester',
      avatar: 'T',
      tone: 'self' as const,
      sentAt: 'now',
      body: 'hello',
      own: true,
    };
    expect(shouldScrollToLatest('room-a', 'room-b', 0, [])).toBe(true);
    expect(shouldScrollToLatest('room-a', 'room-a', 0, [ownMessage])).toBe(true);
    expect(shouldScrollToLatest('room-a', 'room-a', 1, [ownMessage])).toBe(false);
  });
});
