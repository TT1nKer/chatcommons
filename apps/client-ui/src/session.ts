import type { ClientSnapshot, Message } from './domain';

export interface RoomSelection {
  communityId: string;
  roomId: string;
}

export interface SessionState {
  snapshot: ClientSnapshot | null;
  selection: RoomSelection;
  messages: Record<string, Message[]>;
  drafts: Record<string, string>;
}

type SessionAction =
  | { type: 'adoptSnapshot'; snapshot: ClientSnapshot }
  | { type: 'selectRoom'; selection: RoomSelection }
  | { type: 'updateDraft'; roomKey: string; value: string }
  | { type: 'appendMessage'; roomKey: string; message: Message }
  | { type: 'clearSubmittedDraft'; roomKey: string; submittedDraft: string };

export const initialSessionState: SessionState = {
  snapshot: null,
  selection: {
    communityId: '',
    roomId: '',
  },
  messages: {},
  drafts: {},
};

export function roomKey(communityId: string, roomId: string): string {
  return `${communityId}:${roomId}`;
}

export function retainAvailableRoom(
  snapshot: ClientSnapshot,
  current: RoomSelection,
): RoomSelection {
  const community = snapshot.communities.find((item) => item.id === current.communityId)
    ?? snapshot.communities[0];
  if (!community) return { communityId: '', roomId: '' };
  const room = community.rooms.find((item) => item.id === current.roomId)
    ?? community.rooms[0];
  return {
    communityId: community.id,
    roomId: room?.id ?? '',
  };
}

export function clearSubmittedDraft(
  drafts: Record<string, string>,
  key: string,
  submittedDraft: string,
): Record<string, string> {
  if (drafts[key] !== submittedDraft) return drafts;
  return { ...drafts, [key]: '' };
}

export function sessionReducer(
  state: SessionState,
  action: SessionAction,
): SessionState {
  switch (action.type) {
    case 'adoptSnapshot':
      return {
        ...state,
        snapshot: action.snapshot,
        selection: retainAvailableRoom(action.snapshot, state.selection),
        messages: action.snapshot.messagesByRoom,
      };
    case 'selectRoom':
      return { ...state, selection: action.selection };
    case 'updateDraft':
      return {
        ...state,
        drafts: { ...state.drafts, [action.roomKey]: action.value },
      };
    case 'appendMessage':
      return {
        ...state,
        messages: {
          ...state.messages,
          [action.roomKey]: [
            ...(state.messages[action.roomKey] ?? []),
            action.message,
          ],
        },
      };
    case 'clearSubmittedDraft':
      return {
        ...state,
        drafts: clearSubmittedDraft(
          state.drafts,
          action.roomKey,
          action.submittedDraft,
        ),
      };
  }
}
