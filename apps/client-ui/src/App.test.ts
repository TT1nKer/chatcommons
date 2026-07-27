import { describe, expect, it } from 'vitest';
import { shouldSubmitComposer } from './App';

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
