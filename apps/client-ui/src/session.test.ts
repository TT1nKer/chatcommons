import { describe, expect, it } from 'vitest';
import type { ClientSnapshot, Message } from './domain';
import {
  clearSubmittedDraft,
  initialSessionState,
  retainAvailableRoom,
  sessionReducer,
} from './session';

const snapshot: ClientSnapshot = {
  mode: 'local',
  profileName: 'tester',
  profileSymbol: 'T',
  profileId: 'tester',
  connection: { status: 'local', warningCode: null },
  communities: [{
    id: 'community-a',
    name: 'A',
    symbol: 'A',
    accent: 'coral',
    canInvite: true,
    summary: '',
    roomSummary: '',
    unread: 0,
    online: 0,
    rooms: [
      { id: 'room-a', name: 'A', unread: 0 },
      { id: 'room-b', name: 'B', unread: 0 },
    ],
  }],
  messagesByRoom: {},
};

const ownMessage: Message = {
  id: 'message',
  author: 'tester',
  avatar: 'T',
  tone: 'self',
  sentAt: 'now',
  body: 'hello',
  own: true,
};

describe('room selection', () => {
  it('retains a room that still exists after synchronization', () => {
    expect(retainAvailableRoom(snapshot, {
      communityId: 'community-a',
      roomId: 'room-b',
    })).toEqual({
      communityId: 'community-a',
      roomId: 'room-b',
    });
  });

  it('falls back only when the selected room disappeared', () => {
    expect(retainAvailableRoom(snapshot, {
      communityId: 'community-a',
      roomId: 'missing',
    })).toEqual({
      communityId: 'community-a',
      roomId: 'room-a',
    });
  });

  it('uses the latest selected room when a delayed snapshot arrives', () => {
    const selected = sessionReducer(initialSessionState, {
      type: 'selectRoom',
      selection: { communityId: 'community-a', roomId: 'room-b' },
    });
    const synchronized = sessionReducer(selected, {
      type: 'adoptSnapshot',
      snapshot,
    });

    expect(synchronized.selection.roomId).toBe('room-b');
  });
});

describe('room drafts and messages', () => {
  it('does not clear text typed after an earlier send started', () => {
    const drafts = { 'community-a:room-a': 'new text' };
    expect(clearSubmittedDraft(
      drafts,
      'community-a:room-a',
      'old text',
    )).toBe(drafts);
    expect(clearSubmittedDraft(
      drafts,
      'community-a:room-a',
      'new text',
    )).toEqual({ 'community-a:room-a': '' });
  });

  it('updates only the active room data named by each action', () => {
    const withDrafts = sessionReducer(
      sessionReducer(initialSessionState, {
        type: 'updateDraft',
        roomKey: 'community-a:room-a',
        value: 'draft A',
      }),
      {
        type: 'updateDraft',
        roomKey: 'community-a:room-b',
        value: 'draft B',
      },
    );
    const withMessage = sessionReducer(withDrafts, {
      type: 'appendMessage',
      roomKey: 'community-a:room-a',
      message: ownMessage,
    });

    expect(withMessage.drafts).toEqual({
      'community-a:room-a': 'draft A',
      'community-a:room-b': 'draft B',
    });
    expect(withMessage.messages['community-a:room-a']).toEqual([ownMessage]);
    expect(withMessage.messages['community-a:room-b']).toBeUndefined();
  });
});
