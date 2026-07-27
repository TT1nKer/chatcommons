import { describe, expect, it } from 'vitest';
import { DemoAdapter } from './demo';

describe('DemoAdapter', () => {
  it('returns an isolated review snapshot', async () => {
    const adapter = new DemoAdapter();
    const first = await adapter.load();
    first.communities[0].name = 'changed';

    const second = await adapter.load();
    expect(second.communities[0].name).toBe('周末游戏组');
    expect(second.communities[0].rooms[0].id).toBe('general');
    expect(second.mode).toBe('demo');
  });

  it('creates a local review message without mutating protocol state', async () => {
    const adapter = new DemoAdapter();
    const message = await adapter.sendMessage({
      communityId: 'weekend',
      roomId: 'general',
      body: 'hello',
    });

    expect(message.body).toBe('hello');
    expect(message.own).toBe(true);
    expect(adapter.kind).toBe('review');
  });

  it('implements the complete review boundary without invoking Rust', async () => {
    const adapter = new DemoAdapter();
    expect((await adapter.sync()).mode).toBe('demo');
    expect((await adapter.joinCommunity('cc1_demo')).communities).toHaveLength(3);
    expect(await adapter.feedbackStatus()).toBeNull();
    expect((await adapter.submitFeedback({
      whatHappened: 'A',
      expected: 'B',
      screen: 'home',
      screenshot: '',
      viewportWidth: 1180,
      viewportHeight: 760,
      confirmed: true,
    })).status).toBe('pending');
  });
});
