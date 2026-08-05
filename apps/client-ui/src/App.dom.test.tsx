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

const voiceSdk = vi.hoisted(() => ({
  connect: vi.fn().mockResolvedValue(undefined),
  disconnect: vi.fn().mockResolvedValue(undefined),
  startAudio: vi.fn().mockResolvedValue(undefined),
  setMicrophoneEnabled: vi.fn().mockResolvedValue(undefined),
  switchActiveDevice: vi.fn().mockResolvedValue(undefined),
  listeners: new Map<string, (...args: unknown[]) => void>(),
}));
const microphoneSdk = vi.hoisted(() => ({
  addEventListener: vi.fn(),
  deviceChangeListeners: new Set<() => void>(),
  enumerateDevices: vi.fn(),
  getUserMedia: vi.fn(),
  removeEventListener: vi.fn(),
  stop: vi.fn(),
}));

vi.mock('livekit-client', () => {
  class TestRoom {
    activeSpeakers: unknown[] = [];
    remoteParticipants = new Map();
    localParticipant = {
      identity: 'local-user:local-device',
      name: 'Tester',
      setMicrophoneEnabled: voiceSdk.setMicrophoneEnabled,
    };

    on(event: string, listener: (...args: unknown[]) => void) {
      voiceSdk.listeners.set(event, listener);
      return this;
    }

    removeAllListeners() {}

    connect = voiceSdk.connect;

    disconnect = voiceSdk.disconnect;

    startAudio = voiceSdk.startAudio;

    switchActiveDevice = voiceSdk.switchActiveDevice;
  }

  return {
    Room: TestRoom,
    RoomEvent: {
      ParticipantConnected: 'participantConnected',
      ParticipantDisconnected: 'participantDisconnected',
      ParticipantNameChanged: 'participantNameChanged',
      ActiveSpeakersChanged: 'activeSpeakersChanged',
      Reconnecting: 'reconnecting',
      Reconnected: 'reconnected',
      Disconnected: 'disconnected',
      TrackSubscribed: 'trackSubscribed',
      TrackUnsubscribed: 'trackUnsubscribed',
      MediaDevicesError: 'mediaDevicesError',
    },
    Track: { Kind: { Audio: 'audio' } },
  };
});

import { App } from './App';
import { microphoneFailureCode } from './voice/microphone';
import {
  ClientBridgeError,
  type ClientAdapter,
  type ClientSnapshot,
  type FeedbackInput,
  type Message,
  type VoiceGrant,
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
      canInvite: true,
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

function mediaDevice(
  deviceId: string,
  label: string,
  kind: MediaDeviceKind = 'audioinput',
): MediaDeviceInfo {
  return {
    deviceId,
    groupId: `${deviceId}-group`,
    kind,
    label,
    toJSON: () => ({}),
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
    createInvitation: vi.fn().mockResolvedValue({ code: 'cc1_test_invitation' }),
    sendMessage: vi.fn(async ({ body }) => sentMessage(body)),
    voiceToken: vi.fn().mockRejectedValue(
      new ClientBridgeError('voiceDesktopOnly', 'voice requires the desktop client'),
    ),
    recordLatency: vi.fn().mockResolvedValue(undefined),
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

function selectOption(element: Element | null, value: string) {
  if (!(element instanceof HTMLSelectElement)) {
    throw new Error('expected select element');
  }
  const setter = Object.getOwnPropertyDescriptor(
    HTMLSelectElement.prototype,
    'value',
  )?.set;
  if (!setter) throw new Error('select value setter unavailable');
  act(() => {
    setter.call(element, value);
    element.dispatchEvent(new Event('change', { bubbles: true }));
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
    vi.clearAllMocks();
    voiceSdk.listeners.clear();
    microphoneSdk.deviceChangeListeners.clear();
    microphoneSdk.enumerateDevices.mockResolvedValue([]);
    microphoneSdk.addEventListener.mockImplementation((
      type: string,
      listener: () => void,
    ) => {
      if (type === 'devicechange') microphoneSdk.deviceChangeListeners.add(listener);
    });
    microphoneSdk.removeEventListener.mockImplementation((
      type: string,
      listener: () => void,
    ) => {
      if (type === 'devicechange') microphoneSdk.deviceChangeListeners.delete(listener);
    });
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
    const microphoneTrack = {
      label: 'Test microphone',
      stop: microphoneSdk.stop,
    } as unknown as MediaStreamTrack;
    const microphoneStream = {
      getAudioTracks: () => [microphoneTrack],
      getTracks: () => [microphoneTrack],
    } as unknown as MediaStream;
    microphoneSdk.getUserMedia.mockResolvedValue(microphoneStream);
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: {
        addEventListener: microphoneSdk.addEventListener,
        enumerateDevices: microphoneSdk.enumerateDevices,
        getUserMedia: microphoneSdk.getUserMedia,
        removeEventListener: microphoneSdk.removeEventListener,
      },
    });
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });
    Object.defineProperty(window, 'AudioContext', {
      configurable: true,
      value: undefined,
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

  it('creates and copies a one-person invitation for an administered community', async () => {
    const createInvitation = vi.fn().mockResolvedValue({
      code: 'cc1_private_invitation_for_one_friend',
    });
    await render(createAdapter({ createInvitation }));

    click(container.querySelector('.invite-action'));
    expect(container.querySelector('.client-dialog')?.textContent)
      .toContain('Invite one friend');
    click(container.querySelector('.invitation-dialog .primary-action'));
    await settle();

    expect(createInvitation).toHaveBeenCalledWith({ communityId: 'community-a' });
    expect(container.querySelector('.invitation-result')?.textContent)
      .toContain('Invite ready');

    click(container.querySelector('.invitation-dialog .primary-action'));
    await settle();
    expect(navigator.clipboard.writeText)
      .toHaveBeenCalledWith('cc1_private_invitation_for_one_friend');
  });

  it('does not expose invitation controls to a regular member', async () => {
    const snapshot = clientSnapshot();
    snapshot.communities[0].canInvite = false;
    await render(createAdapter({
      load: vi.fn().mockResolvedValue(snapshot),
      sync: vi.fn().mockResolvedValue(snapshot),
    }));

    expect(container.querySelector('.invite-action')).toBeNull();
    click(container.querySelector('.recent-list button'));
    expect(container.querySelector('.context-invite')).toBeNull();
  });

  it('explains when the Home Server cannot create an invitation', async () => {
    await render(createAdapter({
      createInvitation: vi.fn().mockRejectedValue(
        new ClientBridgeError('inviteServerUnavailable', 'offline'),
      ),
    }));

    click(container.querySelector('.invite-action'));
    click(container.querySelector('.invitation-dialog .primary-action'));
    await settle();

    expect(container.querySelector('.dialog-notice')?.textContent)
      .toContain('Community Home Server is unavailable');
  });

  it('ignores an invitation result after its dialog was closed and reopened', async () => {
    const pendingInvitation = deferred<{ code: string }>();
    await render(createAdapter({
      createInvitation: vi.fn(() => pendingInvitation.promise),
    }));

    click(container.querySelector('.invite-action'));
    click(container.querySelector('.invitation-dialog .primary-action'));
    click(container.querySelector('.client-dialog > header button'));
    click(container.querySelector('.invite-action'));

    await act(async () => {
      pendingInvitation.resolve({ code: 'cc1_stale_invitation' });
      await pendingInvitation.promise;
    });

    expect(container.querySelector('.invitation-result')).toBeNull();
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

  it('starts voice authorization while checking and releasing the microphone', async () => {
    vi.useFakeTimers();
    Object.defineProperty(window, 'AudioContext', {
      configurable: true,
      value: class {
        state = 'running';

        createMediaStreamSource() {
          return { connect: vi.fn() };
        }

        createAnalyser() {
          return {
            fftSize: 0,
            getByteTimeDomainData: (samples: Uint8Array) => samples.fill(192),
          };
        }

        close = vi.fn().mockResolvedValue(undefined);

        resume = vi.fn().mockResolvedValue(undefined);
      },
    });
    const pendingGrant = deferred<VoiceGrant>();
    const voiceToken = vi.fn().mockReturnValue(pendingGrant.promise);
    const grantedVoice = {
      serverUrl: 'wss://voice.example.test',
      participantToken: 'signed-participant-token',
      expiresAtMs: Date.now() + 60_000,
    };
    const recordLatency = vi.fn().mockResolvedValue(undefined);
    await render(createAdapter({ kind: 'tauri', voiceToken, recordLatency }));

    click(roomButton(container, 'Room A'));
    click(container.querySelector('.voice-join'));
    await settle();
    await settle();

    expect(container.querySelector('.voice-dock')?.textContent)
      .toContain('Listening to Test microphone');
    expect(voiceToken).toHaveBeenCalledWith({
      communityId: 'community-a',
      roomId: 'room-a',
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });
    pendingGrant.resolve(grantedVoice);
    await settle();
    await settle();

    expect(microphoneSdk.getUserMedia).toHaveBeenCalledWith({
      audio: true,
      video: false,
    });
    expect(microphoneSdk.stop).toHaveBeenCalledOnce();
    expect(voiceSdk.connect).toHaveBeenCalledWith(
      'wss://voice.example.test',
      'signed-participant-token',
      { autoSubscribe: true },
    );
    expect(voiceSdk.setMicrophoneEnabled).toHaveBeenCalledWith(true);
    expect(container.querySelector('.voice-dock')?.textContent)
      .toContain('Voice connected');
    expect(recordLatency.mock.calls.map(([mark]) => mark.phase)).toEqual([
      'voice_join_started',
      'voice_microphone_ready',
      'voice_grant_ready',
      'voice_sfu_connected',
      'voice_audio_started',
    ]);

    const audio = document.createElement('audio');
    Object.defineProperty(audio, 'pause', { value: vi.fn() });
    act(() => {
      voiceSdk.listeners.get('trackSubscribed')?.({
        kind: 'audio',
        attach: () => audio,
        detach: () => [audio],
      });
      voiceSdk.listeners.get('trackSubscribed')?.({
        kind: 'audio',
        attach: () => audio,
        detach: () => [audio],
      });
    });
    expect(recordLatency.mock.calls.at(-1)?.[0].phase)
      .toBe('voice_remote_track');
    expect(recordLatency.mock.calls.filter(([mark]) => (
      mark.phase === 'voice_remote_track'
    ))).toHaveLength(1);

    click(container.querySelector('.voice-leave'));
    expect(voiceSdk.disconnect).toHaveBeenCalled();
    expect(container.querySelector('.voice-dock')).toBeNull();
  });

  it('uses the selected microphone for preflight and LiveKit capture', async () => {
    microphoneSdk.enumerateDevices.mockResolvedValue([
      mediaDevice('usb-microphone', 'USB microphone'),
      mediaDevice('hidden-label-microphone', ''),
      mediaDevice('camera', 'Camera', 'videoinput'),
    ]);
    const voiceToken = vi.fn().mockResolvedValue({
      serverUrl: 'wss://voice.example.test',
      participantToken: 'signed-participant-token',
      expiresAtMs: Date.now() + 60_000,
    });
    await render(createAdapter({ kind: 'tauri', voiceToken }));
    await settle();

    click(roomButton(container, 'Room A'));
    const microphoneSelect = container.querySelector('.microphone-picker select');
    expect(microphoneSelect?.textContent).toContain('USB microphone');
    expect(microphoneSelect?.textContent).toContain('Microphone 2');
    selectOption(microphoneSelect, 'usb-microphone');
    expect(window.localStorage.getItem('chatcommons-preferred-microphone'))
      .toBe('usb-microphone');

    click(container.querySelector('.voice-join'));
    await settle();
    await settle();

    expect(microphoneSdk.getUserMedia).toHaveBeenCalledWith({
      audio: { deviceId: { exact: 'usb-microphone' } },
      video: false,
    });
    expect(voiceSdk.setMicrophoneEnabled).toHaveBeenCalledWith(
      true,
      { deviceId: 'usb-microphone' },
    );
    expect(container.querySelector('.microphone-picker select'))
      .toHaveProperty('disabled', false);

    selectOption(container.querySelector('.microphone-picker select'), '');
    await settle();
    expect(voiceSdk.switchActiveDevice)
      .toHaveBeenCalledWith('audioinput', 'default');
    expect(window.localStorage.getItem('chatcommons-preferred-microphone')).toBeNull();

    voiceSdk.switchActiveDevice.mockRejectedValueOnce(
      new DOMException('Permission denied', 'NotAllowedError'),
    );
    selectOption(container.querySelector('.microphone-picker select'), 'usb-microphone');
    await settle();
    await settle();

    expect(container.querySelector<HTMLSelectElement>('.microphone-picker select')?.value)
      .toBe('');
    expect(window.localStorage.getItem('chatcommons-preferred-microphone')).toBeNull();
    expect(container.querySelector('.voice-dock')?.textContent)
      .toContain('The microphone is unavailable');
  });

  it('returns to the system default when the saved microphone is unplugged', async () => {
    microphoneSdk.enumerateDevices.mockResolvedValue([
      mediaDevice('usb-microphone', 'USB microphone'),
    ]);
    await render(createAdapter());
    await settle();

    click(roomButton(container, 'Room A'));
    const microphoneSelect = container.querySelector('.microphone-picker select');
    selectOption(microphoneSelect, 'usb-microphone');
    microphoneSdk.enumerateDevices.mockResolvedValue([]);
    act(() => {
      for (const listener of microphoneSdk.deviceChangeListeners) listener();
    });
    await settle();

    expect(container.querySelector<HTMLSelectElement>('.microphone-picker select')?.value)
      .toBe('');
    expect(window.localStorage.getItem('chatcommons-preferred-microphone')).toBeNull();
  });

  it('ignores a stale device list that resolves after a newer refresh', async () => {
    const staleDevices = deferred<MediaDeviceInfo[]>();
    microphoneSdk.enumerateDevices
      .mockImplementationOnce(() => staleDevices.promise)
      .mockResolvedValueOnce([mediaDevice('new-microphone', 'New microphone')]);
    await render(createAdapter());

    act(() => {
      for (const listener of microphoneSdk.deviceChangeListeners) listener();
    });
    await settle();
    click(roomButton(container, 'Room A'));
    expect(container.querySelector('.microphone-picker select')?.textContent)
      .toContain('New microphone');

    await act(async () => {
      staleDevices.resolve([mediaDevice('old-microphone', 'Old microphone')]);
      await staleDevices.promise;
    });
    await settle();

    const options = container.querySelector('.microphone-picker select')?.textContent;
    expect(options).toContain('New microphone');
    expect(options).not.toContain('Old microphone');
  });

  it('uses the visible system default when device enumeration fails', async () => {
    window.localStorage.setItem(
      'chatcommons-preferred-microphone',
      'unavailable-microphone',
    );
    microphoneSdk.enumerateDevices.mockRejectedValue(
      new DOMException('Enumeration failed', 'NotAllowedError'),
    );
    await render(createAdapter());
    await settle();
    click(roomButton(container, 'Room A'));

    expect(container.querySelector<HTMLSelectElement>('.microphone-picker select')?.value)
      .toBe('');
    expect(window.localStorage.getItem('chatcommons-preferred-microphone')).toBeNull();
  });

  it('keeps a saved microphone while labels are hidden before permission', async () => {
    window.localStorage.setItem(
      'chatcommons-preferred-microphone',
      'saved-microphone',
    );
    microphoneSdk.enumerateDevices.mockResolvedValue([]);
    await render(createAdapter());
    await settle();
    click(roomButton(container, 'Room A'));

    const microphoneSelect = container.querySelector<HTMLSelectElement>(
      '.microphone-picker select',
    );
    expect(microphoneSelect?.value).toBe('saved-microphone');
    expect(microphoneSelect?.textContent).toContain('Saved microphone');
    expect(window.localStorage.getItem('chatcommons-preferred-microphone'))
      .toBe('saved-microphone');
  });

  it('does not request a voice grant when no microphone is available', async () => {
    const voiceToken = vi.fn();
    microphoneSdk.getUserMedia.mockRejectedValue(
      new DOMException('No input device', 'NotFoundError'),
    );
    await render(createAdapter({ kind: 'tauri', voiceToken }));

    click(roomButton(container, 'Room A'));
    click(container.querySelector('.voice-join'));
    await settle();
    await settle();

    expect(voiceToken).not.toHaveBeenCalled();
    expect(container.querySelector('.voice-dock')?.textContent)
      .toContain('No microphone was found');
  });

  it('classifies platform permission and device startup failures', () => {
    expect(microphoneFailureCode(
      { name: 'NotAllowedError' },
      'Mozilla/5.0 (Macintosh; Intel Mac OS X)',
    )).toBe('voicePermissionMac');
    expect(microphoneFailureCode(
      { name: 'NotAllowedError' },
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64)',
    )).toBe('voicePermissionWindows');
    expect(microphoneFailureCode(
      { name: 'NotReadableError' },
      'Mozilla/5.0',
    )).toBe('voiceMicrophoneBusy');
    expect(microphoneFailureCode(
      { name: 'OverconstrainedError' },
      'Mozilla/5.0',
    )).toBe('voiceMicrophoneSelectionMissing');
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
