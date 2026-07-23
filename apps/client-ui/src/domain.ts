export type Locale = 'zh-CN' | 'en';
export type Screen = 'home' | 'community';

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
  profileName: string;
  profileSymbol: string;
  communities: Community[];
  messagesByRoom: Record<string, Message[]>;
}

export interface SendMessageInput {
  communityId: string;
  roomId: string;
  body: string;
}

export interface ClientAdapter {
  readonly kind: 'review' | 'tauri';
  load(): Promise<ClientSnapshot>;
  sendMessage(input: SendMessageInput): Promise<Message>;
}
