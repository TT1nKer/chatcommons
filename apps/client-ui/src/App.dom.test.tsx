// @vitest-environment jsdom
// @vitest-environment-options {"url":"http://localhost/"}

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest';
import { App } from './App';
import type {
  ClientAdapter,
  ClientSnapshot,
  FeedbackInput,
  Message,
} from './domain';

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason?: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function clientSnapshot(): ClientSnapshot {
  return {
    mode: 'local',
    profileName: 'tester',
    profileSymbol: 'T',
    profileId: 'tester',
    connection: { status: 'local', warningCode: null },
    communities: [{
      id: 'community-a',
      name: 'Community A',
      symbol: 'A',
      accent: 'coral',
      summary: '',
      roomSummary: 'Room A · Room B',
      unread: 0,
      online: 1,
      rooms: [
        { id: 'room-a', name: 'Room A', unread: 0 },
        { id: 'room-b', name: 'Room B', unread: 0 },
      ],
    }],
    messagesByRoom: {
      'community-a:room-a': [],
      'community-a:room-b': [],
    },
  };
}

function emptyClientSnapshot(): ClientSnapshot {
  return {
    ...clientSnapshot(),
    communities: [],
    messagesByRoom: {},
  };
}

function sentMessage(body: string): Message {
  return {
    id: `message-${body}`,
    author: 'tester',
    avatar: 'T',
    tone: 'self',
    sentAt: 'now',
    body,
    own: true,
  };
}

function createAdapter(
  overrides: Partial<ClientAdapter> = {},
): ClientAdapter {
  const snapshot = clientSnapshot();
  return {
    kind: 'review',
    load: vi.fn().mockResolvedValue(snapshot),
    sync: vi.fn().mockResolvedValue(snapshot),
    joinCommunity: vi.fn().mockResolvedValue(snapshot),
    sendMessage: vi.fn(async ({ body }) => sentMessage(body)),
    submitFeedback: vi.fn().mockResolvedValue({
      publicId: 'feedback-1',
      status: 'received',
      adminReply: '',
    }),
    feedbackStatus: vi.fn().mockResolvedValue(null),
    ...overrides,
  };
}

function click(element: Element | null) {
  if (!(element instanceof HTMLElement)) {
    throw new Error('expected clickable element');
  }
  act(() => element.click());
}

function setText(element: Element | null, value: string) {
  if (!(element instanceof HTMLTextAreaElement)) {
    throw new Error('expected textarea');
  }
  const setter = Object.getOwnPropertyDescriptor(
    HTMLTextAreaElement.prototype,
    'value',
  )?.set;
  if (!setter) throw new Error('textarea value setter unavailable');
  act(() => {
    setter.call(element, value);
    element.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

function roomButton(container: HTMLElement, roomName: string): HTMLButtonElement {
  const button = [...container.querySelectorAll<HTMLButtonElement>('.tree-rooms button')]
    .find((item) => item.textContent?.includes(roomName));
  if (!button) throw new Error(`room button not found: ${roomName}`);
  return button;
}

function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() {
      return values.size;
    },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => {
      values.delete(key);
    },
    setItem: (key, value) => {
      values.set(key, value);
    },
  };
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('App behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
    const storage = memoryStorage();
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      value: storage,
    });
    Object.defineProperty(window, 'localStorage', {
      configurable: true,
      value: storage,
    });
    window.localStorage.clear();
    window.localStorage.setItem('chatcommons-locale', 'en');
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: vi.fn().mockReturnValue({
        matches: true,
        media: '(prefers-reduced-motion: reduce)',
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }),
    });
    Object.defineProperty(window, 'scrollTo', {
      configurable: true,
      value: vi.fn(),
    });
    container = document.createElement('div');
    document.body.append(container);
    root = createRoot(container);
  });

  afterEach(() => {
    if (root) act(() => root.unmount());
    container?.remove();
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  async function render(adapter: ClientAdapter) {
    await act(async () => {
      root.render(<App adapter={adapter} />);
    });
    await settle();
  }

  it('keeps drafts isolated when the user switches rooms', async () => {
    await render(createAdapter());

    click(roomButton(container, 'Room A'));
    setText(container.querySelector('.composer textarea'), 'draft A');
    click(roomButton(container, 'Room B'));
    expect(container.querySelector<HTMLTextAreaElement>('.composer textarea')?.value)
      .toBe('');
    setText(container.querySelector('.composer textarea'), 'draft B');
    click(roomButton(container, 'Room A'));
    expect(container.querySelector<HTMLTextAreaElement>('.composer textarea')?.value)
      .toBe('draft A');
    click(roomButton(container, 'Room B'));
    expect(container.querySelector<HTMLTextAreaElement>('.composer textarea')?.value)
      .toBe('draft B');
  });

  it('does not clear a different room draft when an earlier send finishes', async () => {
    const pendingSend = deferred<Message>();
    const adapter = createAdapter({
      sendMessage: vi.fn(() => pendingSend.promise),
    });
    await render(adapter);

    click(roomButton(container, 'Room A'));
    setText(container.querySelector('.composer textarea'), 'sent from A');
    click(container.querySelector('.send-button'));
    click(roomButton(container, 'Room B'));
    setText(container.querySelector('.composer textarea'), 'still typing in B');

    await act(async () => {
      pendingSend.resolve(sentMessage('sent from A'));
      await pendingSend.promise;
    });

    expect(container.querySelector<HTMLTextAreaElement>('.composer textarea')?.value)
      .toBe('still typing in B');
    click(roomButton(container, 'Room A'));
    expect(container.querySelector<HTMLTextAreaElement>('.composer textarea')?.value)
      .toBe('');
  });

  it('does not let a delayed synchronization change the selected room', async () => {
    const pendingSync = deferred<ClientSnapshot>();
    const adapter = createAdapter({
      kind: 'tauri',
      sync: vi.fn(() => pendingSync.promise),
    });
    await render(adapter);

    click(roomButton(container, 'Room B'));
    await act(async () => {
      pendingSync.resolve(clientSnapshot());
      await pendingSync.promise;
    });

    expect(
      container.querySelector<HTMLButtonElement>('.tree-rooms button.is-active')
        ?.textContent,
    ).toContain('Room B');
  });

  it('enters the first room after joining with an invite', async () => {
    const joinCommunity = vi.fn().mockResolvedValue(clientSnapshot());
    const adapter = createAdapter({
      kind: 'tauri',
      load: vi.fn().mockResolvedValue(emptyClientSnapshot()),
      joinCommunity,
    });
    await render(adapter);

    click(container.querySelector('.client-empty .primary-action'));
    setText(container.querySelector('.dialog-form textarea'), 'cc1_friend_invite');
    click(container.querySelector('.dialog-form button[type="submit"]'));
    await settle();

    expect(joinCommunity).toHaveBeenCalledWith('cc1_friend_invite');
    expect(container.querySelector('.conversation-screen')).not.toBeNull();
    expect(
      container.querySelector<HTMLButtonElement>('.tree-rooms button.is-active')
        ?.textContent,
    ).toContain('Room A');
  });

  it('polls without overlap and renders a newly synchronized message', async () => {
    vi.useFakeTimers();
    const firstSync = deferred<ClientSnapshot>();
    const synchronized = clientSnapshot();
    synchronized.messagesByRoom['community-a:room-a'] = [{
      id: 'friend-message',
      author: 'friend',
      avatar: 'F',
      tone: 'blue',
      sentAt: 'now',
      body: 'message from friend',
      own: false,
    }];
    const sync = vi.fn()
      .mockImplementationOnce(() => firstSync.promise)
      .mockResolvedValue(synchronized);
    await render(createAdapter({ kind: 'tauri', sync }));
    click(roomButton(container, 'Room A'));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(6_000);
    });
    expect(sync).toHaveBeenCalledTimes(1);

    await act(async () => {
      firstSync.resolve(synchronized);
      await firstSync.promise;
    });
    expect(container.querySelector('.message-list')?.textContent)
      .toContain('message from friend');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });
    expect(sync).toHaveBeenCalledTimes(2);
  });

  it('does not send while an input method is confirming composition', async () => {
    const sendMessage = vi.fn(async ({ body }) => sentMessage(body));
    await render(createAdapter({ sendMessage }));

    click(roomButton(container, 'Room A'));
    const composer = container.querySelector<HTMLTextAreaElement>('.composer textarea');
    setText(composer, 'composing text');
    const compositionEnter = new KeyboardEvent('keydown', {
      bubbles: true,
      key: 'Enter',
    });
    Object.defineProperties(compositionEnter, {
      isComposing: { value: true },
      keyCode: { value: 13 },
    });
    act(() => composer?.dispatchEvent(compositionEnter));

    expect(sendMessage).not.toHaveBeenCalled();
    expect(composer?.value).toBe('composing text');
  });

  it('scrolls to a message sent by the current user', async () => {
    await render(createAdapter());

    click(roomButton(container, 'Room A'));
    const messageList = container.querySelector<HTMLDivElement>('.message-list');
    if (!messageList) throw new Error('message list not found');
    Object.defineProperty(messageList, 'scrollHeight', {
      configurable: true,
      value: 480,
    });
    messageList.scrollTop = 0;
    setText(container.querySelector('.composer textarea'), 'new message');
    click(container.querySelector('.send-button'));
    await settle();

    expect(messageList.scrollTop).toBe(480);
  });

  it('submits long feedback without truncating it', async () => {
    const submitted: FeedbackInput[] = [];
    const adapter = createAdapter({
      submitFeedback: vi.fn(async (input) => {
        submitted.push(input);
        return {
          publicId: 'feedback-long',
          status: 'received',
          adminReply: '',
        };
      }),
    });
    await render(adapter);

    click(container.querySelector('.sidebar-feedback'));
    const fields = container.querySelectorAll<HTMLTextAreaElement>(
      '.feedback-form textarea',
    );
    const whatHappened = 'what happened '.repeat(1_000);
    const expected = 'what I expected '.repeat(1_000);
    setText(fields[0], whatHappened);
    setText(fields[1], expected);
    click(container.querySelector('.confirmation input'));
    click(container.querySelector('.feedback-form button[type="submit"]'));
    await settle();

    expect(submitted).toHaveLength(1);
    expect(submitted[0].whatHappened).toBe(whatHappened);
    expect(submitted[0].expected).toBe(expected);
    expect(submitted[0].screen).toBe('home');
  });

  it('does not let an older status request hide a new feedback receipt', async () => {
    const pendingStatus = deferred<Awaited<ReturnType<ClientAdapter['feedbackStatus']>>>();
    const adapter = createAdapter({
      feedbackStatus: vi.fn(() => pendingStatus.promise),
    });
    await render(adapter);

    click(container.querySelector('.sidebar-feedback'));
    const fields = container.querySelectorAll<HTMLTextAreaElement>(
      '.feedback-form textarea',
    );
    setText(fields[0], 'current problem');
    setText(fields[1], 'current expectation');
    click(container.querySelector('.confirmation input'));
    click(container.querySelector('.feedback-form button[type="submit"]'));
    await settle();
    await act(async () => {
      pendingStatus.resolve({
        publicId: 'feedback-old',
        status: 'received',
        adminReply: '',
      });
      await pendingStatus.promise;
    });

    expect(container.querySelector('.feedback-receipt')?.textContent)
      .toContain('feedback-1');
    expect(container.querySelector('.feedback-receipt')?.textContent)
      .not.toContain('feedback-old');
  });
});
