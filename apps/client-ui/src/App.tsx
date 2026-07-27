import {
  type ChangeEvent,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import type {
  ClientAdapter,
  ClientSnapshot,
  Community,
  FeedbackStatus,
  Locale,
  Message,
  Room,
  Screen,
} from './domain';
import { ClientBridgeError } from './domain';
import { copyFor, otherLocale } from './i18n';

const localeStorageKey = 'chatcommons-locale';
const screenshotLimit = 1_250_000;

function storedLocale(): Locale {
  try {
    return localStorage.getItem(localeStorageKey) === 'en' ? 'en' : 'zh-CN';
  } catch {
    return 'zh-CN';
  }
}

function roomKey(communityId: string, roomId: string): string {
  return `${communityId}:${roomId}`;
}

function failureCode(reason: unknown): string {
  return reason instanceof ClientBridgeError ? reason.code : 'unknown';
}

function readImage(file: File): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    const url = URL.createObjectURL(file);
    image.onload = () => {
      URL.revokeObjectURL(url);
      resolve(image);
    };
    image.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error('image-decode'));
    };
    image.src = url;
  });
}

async function prepareScreenshot(file: File): Promise<string> {
  if (!['image/png', 'image/jpeg'].includes(file.type)) {
    throw new Error('image-type');
  }
  const image = await readImage(file);
  const scale = Math.min(1, 1280 / Math.max(image.naturalWidth, 1));
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
  canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
  const context = canvas.getContext('2d');
  if (!context) throw new Error('image-canvas');
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  for (const quality of [0.78, 0.64, 0.5]) {
    const encoded = canvas.toDataURL('image/jpeg', quality);
    if (encoded.length <= screenshotLimit) return encoded;
  }
  throw new Error('image-size');
}

interface AppProps {
  adapter: ClientAdapter;
}

interface ReviewI18nBridge {
  setLocale?: (locale: Locale) => void;
}

interface ReviewWindow extends Window {
  chatcommonsI18n?: ReviewI18nBridge;
}

type Dialog = 'join' | 'feedback' | null;

interface ComposerKeyState {
  key: string;
  shiftKey: boolean;
  isComposing: boolean;
  keyCode: number;
}

export function shouldSubmitComposer(state: ComposerKeyState): boolean {
  return (
    state.key === 'Enter'
    && !state.shiftKey
    && !state.isComposing
    && state.keyCode !== 229
  );
}

export function App({ adapter }: AppProps) {
  const [locale, setLocale] = useState<Locale>(storedLocale);
  const [snapshot, setSnapshot] = useState<ClientSnapshot | null>(null);
  const [screen, setScreen] = useState<Screen>('home');
  const [communityId, setCommunityId] = useState('');
  const [roomId, setRoomId] = useState('');
  const [messages, setMessages] = useState<Record<string, Message[]>>({});
  const [draft, setDraft] = useState('');
  const [loading, setLoading] = useState(true);
  const [sending, setSending] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [errorCode, setErrorCode] = useState('');
  const [toast, setToast] = useState('');
  const [dialog, setDialog] = useState<Dialog>(null);
  const [inviteCode, setInviteCode] = useState('');
  const [joining, setJoining] = useState(false);
  const [feedbackWhat, setFeedbackWhat] = useState('');
  const [feedbackExpected, setFeedbackExpected] = useState('');
  const [feedbackScreenshot, setFeedbackScreenshot] = useState('');
  const [feedbackScreenshotName, setFeedbackScreenshotName] = useState('');
  const [feedbackConfirmed, setFeedbackConfirmed] = useState(false);
  const [feedbackSending, setFeedbackSending] = useState(false);
  const [feedbackNotice, setFeedbackNotice] = useState('');
  const [feedbackReceipt, setFeedbackReceipt] = useState<FeedbackStatus | null>(null);
  const toastTimer = useRef<number | undefined>(undefined);
  const copy = copyFor(locale);

  const community = useMemo(
    () => snapshot?.communities.find((item) => item.id === communityId) ?? null,
    [snapshot, communityId],
  );
  const room = useMemo(
    () => community?.rooms.find((item) => item.id === roomId) ?? community?.rooms[0] ?? null,
    [community, roomId],
  );
  const currentMessages = community && room
    ? messages[roomKey(community.id, room.id)] ?? []
    : [];

  function announce(message: string) {
    setToast(message);
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(''), 2400);
  }

  function adoptSnapshot(next: ClientSnapshot) {
    setSnapshot(next);
    setMessages(next.messagesByRoom);
    const firstCommunity = next.communities[0];
    if (!firstCommunity) {
      setCommunityId('');
      setRoomId('');
      setScreen('home');
      return;
    }
    const retainedCommunity = next.communities.find((item) => item.id === communityId)
      ?? firstCommunity;
    const retainedRoom = retainedCommunity.rooms.find((item) => item.id === roomId)
      ?? retainedCommunity.rooms[0];
    setCommunityId(retainedCommunity.id);
    setRoomId(retainedRoom?.id ?? '');
  }

  async function load() {
    setLoading(true);
    setErrorCode('');
    try {
      adoptSnapshot(await adapter.load());
    } catch (reason) {
      setErrorCode(failureCode(reason));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
    return () => {
      if (toastTimer.current) window.clearTimeout(toastTimer.current);
    };
    // The adapter is selected once at boot.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [adapter]);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.title = screen === 'community' && community && room
      ? `${community.name} · ${room.name}`
      : `ChatCommons · ${copy.now}`;
    try {
      localStorage.setItem(localeStorageKey, locale);
    } catch {
      // The selected locale still applies for this session.
    }
  }, [community, copy.now, locale, room, screen]);

  useEffect(() => {
    document.documentElement.dataset.reviewScreen = screen === 'community' && community && room
      ? `community:${community.id}:room:${room.id}`
      : 'home';
    window.dispatchEvent(new Event('chatcommons:screen-change'));
  }, [community, room, screen]);

  function toggleLocale() {
    setLocale((current) => {
      const next = otherLocale(current);
      (window as ReviewWindow).chatcommonsI18n?.setLocale?.(next);
      return next;
    });
  }

  function openCommunity(next: Community) {
    setCommunityId(next.id);
    setRoomId(next.rooms[0]?.id ?? '');
    setScreen('community');
    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    window.scrollTo({ top: 0, behavior: reducedMotion ? 'auto' : 'smooth' });
  }

  async function synchronize() {
    if (syncing) return;
    setSyncing(true);
    try {
      const next = await adapter.sync();
      adoptSnapshot(next);
      announce(next.connection.status === 'degraded' ? copy.syncDegraded : copy.synchronized);
    } catch (reason) {
      announce(copy.errorMessage(failureCode(reason)));
    } finally {
      setSyncing(false);
    }
  }

  async function joinCommunity(event: FormEvent) {
    event.preventDefault();
    if (!inviteCode.trim() || joining) return;
    setJoining(true);
    setFeedbackNotice('');
    try {
      const next = await adapter.joinCommunity(inviteCode.trim());
      adoptSnapshot(next);
      setInviteCode('');
      setDialog(null);
      announce(copy.joined);
    } catch (reason) {
      setFeedbackNotice(copy.errorMessage(failureCode(reason)));
    } finally {
      setJoining(false);
    }
  }

  async function submitMessage(event: FormEvent) {
    event.preventDefault();
    if (!community || !room || !draft.trim() || sending) return;
    const body = draft.trim();
    setSending(true);
    try {
      const message = await adapter.sendMessage({
        communityId: community.id,
        roomId: room.id,
        body,
      });
      const key = roomKey(community.id, room.id);
      setMessages((current) => ({
        ...current,
        [key]: [...(current[key] ?? []), message],
      }));
      setDraft('');
      announce(adapter.kind === 'review' ? copy.demoSaved : copy.savedLocally);
      if (adapter.kind === 'tauri') {
        void adapter.sync().then(adoptSnapshot).catch(() => {
          announce(copy.syncDegraded);
        });
      }
    } catch (reason) {
      announce(copy.errorMessage(failureCode(reason)));
    } finally {
      setSending(false);
    }
  }

  function composerKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (shouldSubmitComposer({
      key: event.key,
      shiftKey: event.shiftKey,
      isComposing: event.nativeEvent.isComposing,
      keyCode: event.keyCode,
    })) {
      event.preventDefault();
      event.currentTarget.form?.requestSubmit();
    }
  }

  async function openFeedback() {
    setDialog('feedback');
    setFeedbackNotice('');
    try {
      setFeedbackReceipt(await adapter.feedbackStatus());
    } catch {
      // A missing receipt or offline status check must not block a new report.
    }
  }

  async function chooseScreenshot(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (!file) return;
    setFeedbackNotice(copy.preparingScreenshot);
    try {
      setFeedbackScreenshot(await prepareScreenshot(file));
      setFeedbackScreenshotName(file.name);
      setFeedbackNotice(copy.screenshotReady);
    } catch {
      setFeedbackScreenshot('');
      setFeedbackScreenshotName('');
      setFeedbackNotice(copy.screenshotInvalid);
    }
  }

  async function sendFeedback(event: FormEvent) {
    event.preventDefault();
    if (
      !feedbackWhat.trim()
      || !feedbackExpected.trim()
      || !feedbackConfirmed
      || feedbackSending
    ) {
      setFeedbackNotice(copy.feedbackIncomplete);
      return;
    }
    setFeedbackSending(true);
    setFeedbackNotice(copy.feedbackSending);
    try {
      const receipt = await adapter.submitFeedback({
        whatHappened: feedbackWhat,
        expected: feedbackExpected,
        screen: screen === 'community' && community && room
          ? `community:${community.id}:room:${room.id}`
          : 'home',
        screenshot: feedbackScreenshot,
        viewportWidth: window.innerWidth,
        viewportHeight: window.innerHeight,
        confirmed: feedbackConfirmed,
      });
      setFeedbackReceipt(receipt);
      setFeedbackWhat('');
      setFeedbackExpected('');
      setFeedbackScreenshot('');
      setFeedbackScreenshotName('');
      setFeedbackConfirmed(false);
      setFeedbackNotice(copy.feedbackDelivered);
    } catch (reason) {
      setFeedbackNotice(copy.errorMessage(failureCode(reason)));
    } finally {
      setFeedbackSending(false);
    }
  }

  if (loading) {
    return (
      <main className="client-state" aria-busy="true">
        <button className="state-language" type="button" onClick={toggleLocale}>
          {locale === 'zh-CN' ? 'EN' : '中文'}
        </button>
        <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>
        <p>{copy.loading}</p>
      </main>
    );
  }

  if (!snapshot || errorCode) {
    return (
      <div className="app-shell">
        <main className="client-state client-state-error">
          <button className="state-language" type="button" onClick={toggleLocale}>
            {locale === 'zh-CN' ? 'EN' : '中文'}
          </button>
          <h1>{copy.loadFailed}</h1>
          <p>{copy.errorMessage(errorCode)}</p>
          <div className="state-actions">
            <button className="primary-action" type="button" onClick={() => void load()}>
              {copy.retry}
            </button>
            <button className="secondary-action" type="button" onClick={() => void openFeedback()}>
              {copy.feedback}
            </button>
          </div>
        </main>
        {dialog === 'feedback' && (
          <FeedbackDialog
            copy={copy}
            whatHappened={feedbackWhat}
            expected={feedbackExpected}
            screenshotName={feedbackScreenshotName}
            confirmed={feedbackConfirmed}
            sending={feedbackSending}
            notice={feedbackNotice}
            receipt={feedbackReceipt}
            onWhatHappened={setFeedbackWhat}
            onExpected={setFeedbackExpected}
            onScreenshot={chooseScreenshot}
            onRemoveScreenshot={() => {
              setFeedbackScreenshot('');
              setFeedbackScreenshotName('');
            }}
            onConfirmed={setFeedbackConfirmed}
            onClose={() => setDialog(null)}
            onSubmit={sendFeedback}
          />
        )}
      </div>
    );
  }

  return (
    <div className="app-shell" data-adapter={adapter.kind}>
      <div className="client-frame">
        <Sidebar
          copy={copy}
          snapshot={snapshot}
          screen={screen}
          communityId={communityId}
          roomId={roomId}
          onHome={() => setScreen('home')}
          onCommunity={openCommunity}
          onRoom={(nextCommunity, nextRoom) => {
            setCommunityId(nextCommunity.id);
            setRoomId(nextRoom.id);
            setScreen('community');
          }}
          onJoin={() => setDialog('join')}
          onCreate={() => announce(copy.notConnectedYet)}
          onFeedback={() => void openFeedback()}
        />

        <section className="client-main">
          <header className="context-bar">
            <div className="context-copy">
              <span>{screen === 'community' && community ? community.name : copy.personalHome}</span>
              <strong>{screen === 'community' && room ? `# ${room.name}` : copy.now}</strong>
            </div>
            <nav className="context-actions" aria-label={copy.globalActions}>
              <span className={`connection-dot is-${snapshot.connection.status}`} title={copy.connection(snapshot.connection.status)}>
                <i aria-hidden="true" />
                <span>{copy.connection(snapshot.connection.status)}</span>
              </span>
              <button
                className="icon-control"
                type="button"
                disabled={syncing}
                onClick={() => void synchronize()}
                aria-label={syncing ? copy.synchronizing : copy.sync}
              >
                ↻
              </button>
              <button
                className="language-toggle compact-control"
                type="button"
                aria-label={locale === 'zh-CN' ? 'Switch to English' : '切换到中文'}
                onClick={toggleLocale}
              >
                {locale === 'zh-CN' ? 'EN' : '中'}
              </button>
              <button className="avatar-button compact-avatar" type="button" aria-label={copy.openProfile}>
                {snapshot.profileSymbol}
              </button>
            </nav>
          </header>

          <main className="client-content">
            {screen === 'home' ? (
              snapshot.communities.length > 0 ? (
                <HomeScreen
                  copy={copy}
                  snapshot={snapshot}
                  communities={snapshot.communities}
                  onCommunity={openCommunity}
                  onJoin={() => setDialog('join')}
                  onPendingAction={() => announce(copy.notConnectedYet)}
                />
              ) : (
                <EmptyState
                  copy={copy}
                  profileId={snapshot.profileId}
                  onJoin={() => setDialog('join')}
                  onPendingAction={() => announce(copy.notConnectedYet)}
                />
              )
            ) : community && room ? (
              <CommunityScreen
                copy={copy}
                room={room}
                messages={currentMessages}
                draft={draft}
                sending={sending}
                onDraft={setDraft}
                onSubmit={submitMessage}
                onComposerKeyDown={composerKeyDown}
                onPendingAction={() => announce(copy.notConnectedYet)}
              />
            ) : (
              <EmptyState
                copy={copy}
                profileId={snapshot.profileId}
                onJoin={() => setDialog('join')}
                onPendingAction={() => announce(copy.notConnectedYet)}
              />
            )}
          </main>
        </section>
      </div>

      {dialog === 'join' && (
        <JoinDialog
          copy={copy}
          inviteCode={inviteCode}
          joining={joining}
          notice={feedbackNotice}
          onInviteCode={setInviteCode}
          onClose={() => setDialog(null)}
          onSubmit={joinCommunity}
        />
      )}

      {dialog === 'feedback' && (
        <FeedbackDialog
          copy={copy}
          whatHappened={feedbackWhat}
          expected={feedbackExpected}
          screenshotName={feedbackScreenshotName}
          confirmed={feedbackConfirmed}
          sending={feedbackSending}
          notice={feedbackNotice}
          receipt={feedbackReceipt}
          onWhatHappened={setFeedbackWhat}
          onExpected={setFeedbackExpected}
          onScreenshot={chooseScreenshot}
          onRemoveScreenshot={() => {
            setFeedbackScreenshot('');
            setFeedbackScreenshotName('');
          }}
          onConfirmed={setFeedbackConfirmed}
          onClose={() => setDialog(null)}
          onSubmit={sendFeedback}
        />
      )}

      <div className={`toast ${toast ? 'show' : ''}`} role="status" aria-live="polite">
        {toast}
      </div>
    </div>
  );
}

type AppCopy = ReturnType<typeof copyFor>;

function Sidebar({
  copy,
  snapshot,
  screen,
  communityId,
  roomId,
  onHome,
  onCommunity,
  onRoom,
  onJoin,
  onCreate,
  onFeedback,
}: {
  copy: AppCopy;
  snapshot: ClientSnapshot;
  screen: Screen;
  communityId: string;
  roomId: string;
  onHome: () => void;
  onCommunity: (community: Community) => void;
  onRoom: (community: Community, room: Room) => void;
  onJoin: () => void;
  onCreate: () => void;
  onFeedback: () => void;
}) {
  return (
    <aside className="client-sidebar">
      <header className="sidebar-brand">
        <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>
        <span>
          <strong>ChatCommons</strong>
          <small>{snapshot.mode === 'demo' ? copy.prototype : copy.alpha}</small>
        </span>
      </header>

      <nav className="space-navigation" aria-label={copy.yourCommunities}>
        <button
          className={`now-link ${screen === 'home' ? 'is-active' : ''}`}
          type="button"
          onClick={onHome}
        >
          <span className="nav-symbol">⌁</span>
          <span><strong>{copy.now}</strong><small>{copy.personalHome}</small></span>
        </button>

        <div className="sidebar-section-heading">
          <span>{copy.yourCommunities}</span>
          <div>
            <button type="button" onClick={onJoin} aria-label={copy.join}>↗</button>
            <button type="button" onClick={onCreate} aria-label={copy.create}>＋</button>
          </div>
        </div>

        <div className="community-tree">
          {snapshot.communities.map((item) => {
            const active = item.id === communityId && screen === 'community';
            return (
              <section className={`tree-community ${active ? 'is-active' : ''}`} key={item.id}>
                <button className="tree-community-button" type="button" onClick={() => onCommunity(item)}>
                  <span className={`community-symbol symbol-${item.accent}`}>{item.symbol}</span>
                  <span>
                    <strong>{item.name}</strong>
                    <small>{item.roomSummary || copy.noRooms}</small>
                  </span>
                  {item.unread > 0 && <b>{item.unread}</b>}
                </button>
                <div className="tree-rooms">
                  {item.rooms.map((itemRoom) => (
                    <button
                      className={active && itemRoom.id === roomId ? 'is-active' : ''}
                      type="button"
                      key={itemRoom.id}
                      onClick={() => onRoom(item, itemRoom)}
                    >
                      <span>#</span>
                      <strong>{itemRoom.name}</strong>
                      {itemRoom.unread > 0 && <b>{itemRoom.unread}</b>}
                    </button>
                  ))}
                </div>
              </section>
            );
          })}
        </div>
      </nav>

      <footer className="sidebar-footer">
        <button className="sidebar-feedback" type="button" onClick={onFeedback}>
          <span>?</span>
          <strong>{copy.feedback}</strong>
        </button>
        <div className="identity-row">
          <span className="avatar-button compact-avatar">{snapshot.profileSymbol}</span>
          <span><strong>{snapshot.profileName}</strong><small>{copy.localIdentity}</small></span>
          <i className={`status-indicator is-${snapshot.connection.status}`} aria-label={copy.connection(snapshot.connection.status)} />
        </div>
      </footer>
    </aside>
  );
}

interface HomeScreenProps {
  copy: AppCopy;
  snapshot: ClientSnapshot;
  communities: Community[];
  onCommunity: (community: Community) => void;
  onJoin: () => void;
  onPendingAction: () => void;
}

function HomeScreen({
  copy,
  snapshot,
  communities,
  onCommunity,
  onJoin,
  onPendingAction,
}: HomeScreenProps) {
  return (
    <section className="now-screen" aria-labelledby="home-title">
      <header className="now-heading">
        <div>
          <h1 id="home-title">{copy.now}</h1>
          <p>{snapshot.mode === 'demo' ? copy.homeLead : copy.realHomeLead}</p>
        </div>
        <button className="primary-action" type="button" onClick={onJoin}>{copy.join}</button>
      </header>

      <div className="now-columns">
        <section className="activity-column" aria-labelledby="activity-title">
          <header className="list-heading">
            <span>{copy.attention}</span>
            <h2 id="activity-title">{copy.mentionsUnreadInvites}</h2>
          </header>
          <div className="activity-list">
            {snapshot.mode === 'demo' ? (
              <>
                <button type="button" onClick={() => communities[1] && onCommunity(communities[1])}>
                  <span className="activity-mark accent">@</span>
                  <span><strong>{copy.mentionedYou}</strong><small>{copy.productDiscussion}</small></span>
                  <time>{copy.minutes12}</time>
                </button>
                <button type="button" onClick={() => communities[0] && onCommunity(communities[0])}>
                  <span className="activity-mark">3</span>
                  <span><strong>{copy.newWeekendMessages}</strong><small>{copy.continueWhereLeft}</small></span>
                  <time>{copy.justNow}</time>
                </button>
              </>
            ) : (
              <div className="activity-empty">
                <span className={`status-indicator is-${snapshot.connection.status}`} />
                <div>
                  <strong>{copy.connection(snapshot.connection.status)}</strong>
                  <small>{copy.identityStoredLocally}</small>
                </div>
              </div>
            )}
            <button type="button" onClick={onPendingAction}>
              <span className="activity-mark">＋</span>
              <span><strong>{copy.inviteFriend}</strong><small>{copy.onePersonOnly}</small></span>
              <time>{copy.createInvite}</time>
            </button>
          </div>
        </section>

        <section className="recent-column" aria-labelledby="recent-title">
          <header className="list-heading">
            <span>{copy.yourCommunities}</span>
            <h2 id="recent-title">{copy.chooseCommunity}</h2>
          </header>
          <div className="recent-list">
            {communities.map((item) => (
              <button type="button" key={item.id} onClick={() => onCommunity(item)}>
                <span className={`community-symbol symbol-${item.accent}`}>{item.symbol}</span>
                <span>
                  <strong>{item.name}</strong>
                  <small>{item.roomSummary || copy.noRooms}</small>
                  <p>{item.summary}</p>
                </span>
                <i>→</i>
              </button>
            ))}
          </div>
        </section>
      </div>
    </section>
  );
}

interface CommunityScreenProps {
  copy: AppCopy;
  room: Room;
  messages: Message[];
  draft: string;
  sending: boolean;
  onDraft: (value: string) => void;
  onSubmit: (event: FormEvent) => void;
  onComposerKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => void;
  onPendingAction: () => void;
}

function CommunityScreen({
  copy,
  room,
  messages,
  draft,
  sending,
  onDraft,
  onSubmit,
  onComposerKeyDown,
  onPendingAction,
}: CommunityScreenProps) {
  return (
    <section className="conversation-screen" aria-label={copy.messages}>
      <div className="message-list" aria-live="polite">
        {messages.length === 0 && (
          <div className="conversation-empty">
            <span>#</span>
            <strong>{copy.channelBeginning}</strong>
            <small>{copy.sendFirstMessage}</small>
          </div>
        )}
        {messages.map((message) => (
          <article className={`message ${message.own ? 'message-self' : ''}`} key={message.id}>
            <span className={`message-avatar ${message.tone}`}>{message.avatar}</span>
            <div>
              <header>
                <strong>{message.own ? copy.you : message.author}</strong>
                <time>{message.sentAt === 'now' ? copy.justNow : message.sentAt}</time>
              </header>
              <p>{message.body}</p>
            </div>
          </article>
        ))}
      </div>
      <form className="composer" onSubmit={onSubmit}>
        <button type="button" className="composer-add" onClick={onPendingAction} aria-label={copy.addAttachment}>＋</button>
        <label>
          <span className="sr-only">{copy.composerLabel}</span>
          <textarea
            rows={1}
            value={draft}
            placeholder={copy.composer(room.name)}
            onChange={(event) => onDraft(event.target.value)}
            onKeyDown={onComposerKeyDown}
          />
        </label>
        <span className="composer-hint">{copy.composerHint}</span>
        <button type="submit" className="send-button" disabled={sending || !draft.trim()}>
          {sending ? copy.sending : copy.send}
        </button>
      </form>
    </section>
  );
}

function EmptyState({
  copy,
  profileId,
  onJoin,
  onPendingAction,
}: {
  copy: AppCopy;
  profileId: string;
  onJoin: () => void;
  onPendingAction: () => void;
}) {
  return (
    <section className="client-empty">
      <span className="empty-glyph">↗</span>
      <p className="kicker">{copy.identityReady}</p>
      <h1>{copy.noCommunity}</h1>
      <p>{copy.noCommunityLead}</p>
      <div className="state-actions">
        <button className="primary-action" type="button" onClick={onJoin}>{copy.join}</button>
        <button className="secondary-action" type="button" onClick={onPendingAction}>{copy.create}</button>
      </div>
      <small>{copy.localIdentity} · {profileId.slice(0, 10)}</small>
    </section>
  );
}

function DialogFrame({
  title,
  closeLabel,
  onClose,
  children,
}: {
  title: string;
  closeLabel: string;
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <div className="client-dialog-backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}>
      <section className="client-dialog" role="dialog" aria-modal="true" aria-labelledby="dialog-title">
        <header>
          <h2 id="dialog-title">{title}</h2>
          <button type="button" onClick={onClose} aria-label={closeLabel}>×</button>
        </header>
        {children}
      </section>
    </div>
  );
}

function JoinDialog({
  copy,
  inviteCode,
  joining,
  notice,
  onInviteCode,
  onClose,
  onSubmit,
}: {
  copy: AppCopy;
  inviteCode: string;
  joining: boolean;
  notice: string;
  onInviteCode: (value: string) => void;
  onClose: () => void;
  onSubmit: (event: FormEvent) => void;
}) {
  return (
    <DialogFrame title={copy.joinCommunity} closeLabel={copy.close} onClose={onClose}>
      <form className="dialog-form" onSubmit={onSubmit}>
        <div className="dialog-scroll">
          <p>{copy.joinLead}</p>
          <label>
            <span>{copy.onePersonInvite}</span>
            <textarea
              rows={7}
              value={inviteCode}
              placeholder="cc1_…"
              spellCheck={false}
              autoFocus
              onChange={(event) => onInviteCode(event.target.value)}
            />
          </label>
          <small>{copy.invitePrivacy}</small>
          {notice && <p className="dialog-notice" role="alert">{notice}</p>}
        </div>
        <footer>
          <button className="secondary-action" type="button" onClick={onClose}>{copy.cancel}</button>
          <button className="primary-action" type="submit" disabled={joining || !inviteCode.trim()}>
            {joining ? copy.joining : copy.joinAction}
          </button>
        </footer>
      </form>
    </DialogFrame>
  );
}

function FeedbackDialog({
  copy,
  whatHappened,
  expected,
  screenshotName,
  confirmed,
  sending,
  notice,
  receipt,
  onWhatHappened,
  onExpected,
  onScreenshot,
  onRemoveScreenshot,
  onConfirmed,
  onClose,
  onSubmit,
}: {
  copy: AppCopy;
  whatHappened: string;
  expected: string;
  screenshotName: string;
  confirmed: boolean;
  sending: boolean;
  notice: string;
  receipt: FeedbackStatus | null;
  onWhatHappened: (value: string) => void;
  onExpected: (value: string) => void;
  onScreenshot: (event: ChangeEvent<HTMLInputElement>) => void;
  onRemoveScreenshot: () => void;
  onConfirmed: (value: boolean) => void;
  onClose: () => void;
  onSubmit: (event: FormEvent) => void;
}) {
  return (
    <DialogFrame title={copy.feedbackTitle} closeLabel={copy.close} onClose={onClose}>
      <form className="dialog-form feedback-form" onSubmit={onSubmit}>
        <div className="dialog-scroll">
          <p>{copy.feedbackPrivacy}</p>
          <label>
            <span>{copy.whatHappened}</span>
            <textarea
              rows={6}
              value={whatHappened}
              placeholder={copy.whatHappenedHint}
              onChange={(event) => onWhatHappened(event.target.value)}
            />
          </label>
          <label>
            <span>{copy.whatExpected}</span>
            <textarea
              rows={5}
              value={expected}
              placeholder={copy.whatExpectedHint}
              onChange={(event) => onExpected(event.target.value)}
            />
          </label>
          <div className="screenshot-control">
            <span>{copy.optionalScreenshot}</span>
            {screenshotName ? (
              <div>
                <strong>{screenshotName}</strong>
                <button type="button" onClick={onRemoveScreenshot}>{copy.remove}</button>
              </div>
            ) : (
              <label className="secondary-action">
                {copy.chooseScreenshot}
                <input type="file" accept="image/png,image/jpeg" onChange={onScreenshot} />
              </label>
            )}
            <small>{copy.screenshotPrivacy}</small>
          </div>
          {receipt && (
            <div className="feedback-receipt">
              <strong>{copy.feedbackReference} · {receipt.publicId}</strong>
              <span>{copy.feedbackState(receipt.status)}</span>
              {receipt.adminReply && <p>{copy.feedbackReply}: {receipt.adminReply}</p>}
            </div>
          )}
          {notice && <p className="dialog-notice" role="status">{notice}</p>}
        </div>
        <footer className="feedback-footer">
          <label className="confirmation">
            <input
              type="checkbox"
              checked={confirmed}
              onChange={(event) => onConfirmed(event.target.checked)}
            />
            <span>{copy.feedbackConfirmation}</span>
          </label>
          <div>
            <button className="secondary-action" type="button" onClick={onClose}>{copy.cancel}</button>
            <button
              className="primary-action"
              type="submit"
              disabled={sending || !confirmed || !whatHappened.trim() || !expected.trim()}
            >
              {sending ? copy.feedbackSending : copy.sendFeedback}
            </button>
          </div>
        </footer>
      </form>
    </DialogFrame>
  );
}
