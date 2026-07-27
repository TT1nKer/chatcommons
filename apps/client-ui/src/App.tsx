import {
  type FormEvent,
  type KeyboardEvent,
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from 'react';
import type {
  ClientAdapter,
  ClientSnapshot,
  Community,
  Locale,
  Message,
  Room,
  Screen,
} from './domain';
import { clientFailureCode } from './domain';
import { DialogFrame } from './components/DialogFrame';
import { FeedbackFlow } from './feedback/FeedbackFlow';
import { copyFor, otherLocale } from './i18n';
import {
  initialSessionState,
  roomKey,
  sessionReducer,
  type RoomSelection,
} from './session';

const localeStorageKey = 'chatcommons-locale';
const automaticSyncDelayMs = 2_000;

function storedLocale(): Locale {
  try {
    return localStorage.getItem(localeStorageKey) === 'en' ? 'en' : 'zh-CN';
  } catch {
    return 'zh-CN';
  }
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

export function shouldScrollToLatest(
  previousRoomKey: string,
  activeRoomKey: string,
  previousMessageCount: number,
  messages: Message[],
): boolean {
  return previousRoomKey !== activeRoomKey
    || (
      messages.length > previousMessageCount
      && messages.at(-1)?.own === true
    );
}

export function App({ adapter }: AppProps) {
  const [locale, setLocale] = useState<Locale>(storedLocale);
  const [session, dispatchSession] = useReducer(sessionReducer, initialSessionState);
  const [screen, setScreen] = useState<Screen>('home');
  const [loading, setLoading] = useState(true);
  const [sending, setSending] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [errorCode, setErrorCode] = useState('');
  const [toast, setToast] = useState('');
  const [dialog, setDialog] = useState<Dialog>(null);
  const [inviteCode, setInviteCode] = useState('');
  const [joining, setJoining] = useState(false);
  const [joinNotice, setJoinNotice] = useState('');
  const toastTimer = useRef<number | undefined>(undefined);
  const syncInFlight = useRef(false);
  const copy = copyFor(locale);
  const {
    snapshot,
    selection,
    messages,
    drafts,
  } = session;
  const { communityId, roomId } = selection;

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
  const activeRoomKey = community && room ? roomKey(community.id, room.id) : '';
  const draft = activeRoomKey ? drafts[activeRoomKey] ?? '' : '';

  const announce = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(''), 2400);
  }, []);

  const adoptSnapshot = useCallback((next: ClientSnapshot) => {
    dispatchSession({ type: 'adoptSnapshot', snapshot: next });
    if (next.communities.length === 0) {
      setScreen('home');
    }
  }, []);

  function setActiveRoom(next: RoomSelection) {
    dispatchSession({ type: 'selectRoom', selection: next });
  }

  function setActiveDraft(value: string) {
    if (!activeRoomKey) return;
    dispatchSession({
      type: 'updateDraft',
      roomKey: activeRoomKey,
      value,
    });
  }

  const synchronize = useCallback(async (showSuccess = true) => {
    if (syncInFlight.current) return;
    syncInFlight.current = true;
    setSyncing(true);
    try {
      const next = await adapter.sync();
      adoptSnapshot(next);
      if (showSuccess) {
        announce(next.connection.status === 'degraded' ? copy.syncDegraded : copy.synchronized);
      }
    } catch (reason) {
      if (showSuccess) {
        announce(copy.errorMessage(clientFailureCode(reason)));
      }
    } finally {
      syncInFlight.current = false;
      setSyncing(false);
    }
  }, [adapter, adoptSnapshot, announce, copy]);

  async function load() {
    setLoading(true);
    setErrorCode('');
    try {
      const initial = await adapter.load();
      adoptSnapshot(initial);
      if (adapter.kind === 'tauri' && initial.communities.length > 0) {
        void synchronize(false);
      }
    } catch (reason) {
      setErrorCode(clientFailureCode(reason));
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
    if (adapter.kind !== 'tauri' || !snapshot?.communities.length) {
      return undefined;
    }

    let stopped = false;
    let timer: number | undefined;
    const schedule = () => {
      timer = window.setTimeout(() => {
        void synchronize(false).finally(() => {
          if (!stopped) schedule();
        });
      }, automaticSyncDelayMs);
    };
    schedule();

    return () => {
      stopped = true;
      if (timer) window.clearTimeout(timer);
    };
  }, [adapter.kind, snapshot?.communities.length, synchronize]);

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
    setActiveRoom({
      communityId: next.id,
      roomId: next.rooms[0]?.id ?? '',
    });
    setScreen('community');
    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    window.scrollTo({ top: 0, behavior: reducedMotion ? 'auto' : 'smooth' });
  }

  async function joinCommunity(event: FormEvent) {
    event.preventDefault();
    if (!inviteCode.trim() || joining) return;
    setJoining(true);
    setJoinNotice('');
    try {
      const next = await adapter.joinCommunity(inviteCode.trim());
      adoptSnapshot(next);
      setInviteCode('');
      setDialog(null);
      if (next.communities.length > 0) {
        setScreen('community');
      }
      announce(copy.joined);
    } catch (reason) {
      setJoinNotice(copy.errorMessage(clientFailureCode(reason)));
    } finally {
      setJoining(false);
    }
  }

  async function submitMessage(event: FormEvent) {
    event.preventDefault();
    if (!community || !room || !activeRoomKey || !draft.trim() || sending) return;
    const submittedDraft = draft;
    const body = draft.trim();
    const submittedRoomKey = activeRoomKey;
    setSending(true);
    try {
      const message = await adapter.sendMessage({
        communityId: community.id,
        roomId: room.id,
        body,
      });
      dispatchSession({
        type: 'appendMessage',
        roomKey: submittedRoomKey,
        message,
      });
      dispatchSession({
        type: 'clearSubmittedDraft',
        roomKey: submittedRoomKey,
        submittedDraft,
      });
      announce(adapter.kind === 'review' ? copy.demoSaved : copy.savedLocally);
      if (adapter.kind === 'tauri') {
        void synchronize(false);
      }
    } catch (reason) {
      announce(copy.errorMessage(clientFailureCode(reason)));
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

  const feedbackScreen = errorCode
    ? `error:${errorCode}`
    : screen === 'community' && community && room
      ? `community:${community.id}:room:${room.id}`
      : 'home';
  const feedbackFlow = (
    <FeedbackFlow
      open={dialog === 'feedback'}
      adapter={adapter}
      copy={copy}
      screenContext={feedbackScreen}
      onClose={() => setDialog(null)}
    />
  );

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
            <button
              className="secondary-action"
              type="button"
              onClick={() => setDialog('feedback')}
            >
              {copy.feedback}
            </button>
          </div>
        </main>
        {feedbackFlow}
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
            setActiveRoom({
              communityId: nextCommunity.id,
              roomId: nextRoom.id,
            });
            setScreen('community');
          }}
          onJoin={() => setDialog('join')}
          onCreate={() => announce(copy.notConnectedYet)}
          onFeedback={() => setDialog('feedback')}
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
                activeRoomKey={activeRoomKey}
                messages={currentMessages}
                draft={draft}
                sending={sending}
                onDraft={setActiveDraft}
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
          notice={joinNotice}
          onInviteCode={setInviteCode}
          onClose={() => setDialog(null)}
          onSubmit={joinCommunity}
        />
      )}

      {feedbackFlow}

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
  activeRoomKey: string;
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
  activeRoomKey,
  messages,
  draft,
  sending,
  onDraft,
  onSubmit,
  onComposerKeyDown,
  onPendingAction,
}: CommunityScreenProps) {
  const messageListRef = useRef<HTMLDivElement>(null);
  const previousRoomKey = useRef('');
  const previousMessageCount = useRef(0);

  useEffect(() => {
    if (shouldScrollToLatest(
      previousRoomKey.current,
      activeRoomKey,
      previousMessageCount.current,
      messages,
    )) {
      const list = messageListRef.current;
      if (list) list.scrollTop = list.scrollHeight;
    }
    previousRoomKey.current = activeRoomKey;
    previousMessageCount.current = messages.length;
  }, [activeRoomKey, messages]);

  return (
    <section className="conversation-screen" aria-label={copy.messages}>
      <div className="message-list" aria-live="polite" ref={messageListRef}>
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
