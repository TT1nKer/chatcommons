export type Locale = 'zh-CN' | 'en';
export type Screen = 'home' | 'community';
export type ConnectionStatus = 'local' | 'connected' | 'degraded';

export interface Room {
  id: string;
  name: string;
  unread: number;
}

export interface Community {
  id: string;
  name: string;
  symbol: string;
  accent: 'coral' | 'blue' | 'green';
  summary: string;
  roomSummary: string;
  unread: number;
  online: number;
  rooms: Room[];
}

export interface Message {
  id: string;
  author: string;
  avatar: string;
  tone: 'coral' | 'blue' | 'green' | 'self';
  sentAt: string;
  body: string;
  own: boolean;
}

export interface ClientSnapshot {
  mode: 'demo' | 'local';
  profileName: string;
  profileSymbol: string;
  profileId: string;
  connection: {
    status: ConnectionStatus;
    warningCode: string | null;
  };
  communities: Community[];
  messagesByRoom: Record<string, Message[]>;
}

export interface SendMessageInput {
  communityId: string;
  roomId: string;
  body: string;
}

export interface VoiceTokenInput {
  communityId: string;
  roomId: string;
}

export interface VoiceGrant {
  serverUrl: string;
  participantToken: string;
  expiresAtMs: number;
}

export interface FeedbackInput {
  whatHappened: string;
  expected: string;
  screen: string;
  screenshot: string;
  viewportWidth: number;
  viewportHeight: number;
  confirmed: boolean;
}

export interface FeedbackStatus {
  publicId: string;
  status: string;
  adminReply: string;
}

export class ClientBridgeError extends Error {
  readonly code: string;
  readonly detail: string;

  constructor(code: string, detail: string) {
    super(code);
    this.name = 'ClientBridgeError';
    this.code = code;
    this.detail = detail;
  }
}

export function clientFailureCode(reason: unknown): string {
  return reason instanceof ClientBridgeError ? reason.code : 'unknown';
}

export interface ClientAdapter {
  readonly kind: 'review' | 'tauri';
  load(): Promise<ClientSnapshot>;
  sync(): Promise<ClientSnapshot>;
  joinCommunity(inviteCode: string): Promise<ClientSnapshot>;
  sendMessage(input: SendMessageInput): Promise<Message>;
  voiceToken(input: VoiceTokenInput): Promise<VoiceGrant>;
  submitFeedback(input: FeedbackInput): Promise<FeedbackStatus>;
  feedbackStatus(): Promise<FeedbackStatus | null>;
}
