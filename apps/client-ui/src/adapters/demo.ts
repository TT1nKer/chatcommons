import type {
  ClientAdapter,
  ClientSnapshot,
  Message,
  SendMessageInput,
} from '../domain';

const snapshot: ClientSnapshot = {
  profileName: '林间',
  profileSymbol: '林',
  communities: [
    {
      id: 'weekend',
      name: '周末游戏组',
      symbol: '周',
      accent: 'coral',
      summary: '今晚八点还是九点？',
      roomSummary: '闲聊 · 阿岚刚刚',
      unread: 3,
      online: 3,
      rooms: [
        { id: 'general', name: '闲聊', unread: 3 },
        { id: 'games', name: '游戏', unread: 0 },
        { id: 'gear', name: '装备讨论', unread: 0 },
      ],
    },
    {
      id: 'opensource',
      name: '开源小组',
      symbol: '开',
      accent: 'blue',
      summary: '首页不应该是一排服务器图标',
      roomSummary: '产品讨论 · 小陈 12 分钟前',
      unread: 1,
      online: 2,
      rooms: [{ id: 'product', name: '产品讨论', unread: 1 }],
    },
    {
      id: 'family',
      name: '家里人',
      symbol: '家',
      accent: 'green',
      summary: '照片我晚点发到群里',
      roomSummary: '日常 · 昨天',
      unread: 0,
      online: 1,
      rooms: [{ id: 'daily', name: '日常', unread: 0 }],
    },
  ],
  messagesByRoom: {
    'weekend:general': [
      {
        id: 'm1',
        author: '阿岚',
        avatar: '岚',
        tone: 'coral',
        sentAt: '19:42',
        body: '今晚八点还是九点？我想先把新地图开一下。',
        own: false,
      },
      {
        id: 'm2',
        author: '小陈',
        avatar: '陈',
        tone: 'blue',
        sentAt: '19:44',
        body: '八点半吧，我回家以后正好能赶上。',
        own: false,
      },
      {
        id: 'm3',
        author: '你',
        avatar: '林',
        tone: 'self',
        sentAt: '19:45',
        body: '可以，我先开着房间等你们。',
        own: true,
      },
      {
        id: 'm4',
        author: '木木',
        avatar: '木',
        tone: 'green',
        sentAt: '刚刚',
        body: '我也来。今晚可以顺便试一下新的聊天原型。',
        own: false,
      },
    ],
  },
};

export class DemoAdapter implements ClientAdapter {
  readonly kind = 'review' as const;

  async load(): Promise<ClientSnapshot> {
    return structuredClone(snapshot);
  }

  async sendMessage(input: SendMessageInput): Promise<Message> {
    return {
      id: `demo-${Date.now()}`,
      author: '你',
      avatar: '林',
      tone: 'self',
      sentAt: '刚刚',
      body: input.body,
      own: true,
    };
  }
}
