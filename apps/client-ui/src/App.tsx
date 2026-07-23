import {
  type FormEvent,
  type KeyboardEvent,
  useEffect,
  useMemo,
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
import { copyFor, otherLocale } from './i18n';

const localeStorageKey = 'chatcommons-locale';

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

interface AppProps {
  adapter: ClientAdapter;
}

interface ReviewI18nBridge {
  setLocale?: (locale: Locale) => void;
}

interface ReviewWindow extends Window {
  chatcommonsI18n?: ReviewI18nBridge;
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
  const [error, setError] = useState('');
  const [toast, setToast] = useState('');
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

  async function load() {
    setLoading(true);
    setError('');
    try {
      const next = await adapter.load();
      setSnapshot(next);
      setMessages(next.messagesByRoom);
      const firstCommunity = next.communities[0];
      if (firstCommunity) {
        setCommunityId((current) => current || firstCommunity.id);
        setRoomId((current) => current || firstCommunity.rooms[0]?.id || '');
      }
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : copy.loadFailed);
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

  function selectRoom(next: Room) {
    setRoomId(next.id);
    announce(copy.switchedRoom(next.name));
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
      announce(adapter.kind === 'review' ? copy.demoSaved : copy.justNow);
    } catch {
      announce(copy.messageFailed);
    } finally {
      setSending(false);
    }
  }

  function composerKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      event.currentTarget.form?.requestSubmit();
    }
  }

  if (loading) {
    return (
      <main className="client-state" aria-busy="true">
        <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>
        <p>{copy.loading}</p>
      </main>
    );
  }

  if (!snapshot || error) {
    return (
      <main className="client-state client-state-error">
        <h1>{copy.loadFailed}</h1>
        <p>{error}</p>
        <button className="primary-action" type="button" onClick={() => void load()}>
          {copy.retry}
        </button>
      </main>
    );
  }

  return (
    <div className="app-shell" data-adapter={adapter.kind}>
      <header className="topbar">
        <button
          className="brand"
          type="button"
          aria-label={copy.backHome}
          onClick={() => setScreen('home')}
        >
          <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>
          <span>ChatCommons</span>
          <small><span>{copy.prototype}</span><b>v0.1.0-alpha.3</b></small>
        </button>
        <nav className="top-actions" aria-label={copy.globalActions}>
          <button className="search-button" type="button" onClick={() => announce(copy.notConnectedYet)}>
            <span>⌘ K</span> {copy.search}
          </button>
          <button className="quiet-button" type="button" onClick={() => announce(copy.notConnectedYet)}>
            {copy.customize}
          </button>
          <button
            className="language-toggle"
            type="button"
            aria-label={locale === 'zh-CN' ? 'Switch to English' : '切换到中文'}
            onClick={toggleLocale}
          >
            {locale === 'zh-CN' ? 'EN' : '中文'}
          </button>
          <button className="avatar-button" type="button" aria-label={copy.openProfile}>
            {snapshot.profileSymbol}
          </button>
        </nav>
      </header>

      <main className="workspace client-workspace">
        {screen === 'home' ? (
          <HomeScreen
            copy={copy}
            communities={snapshot.communities}
            onCommunity={openCommunity}
            onPendingAction={() => announce(copy.notConnectedYet)}
          />
        ) : community && room ? (
          <CommunityScreen
            copy={copy}
            community={community}
            room={room}
            messages={currentMessages}
            draft={draft}
            sending={sending}
            onBack={() => setScreen('home')}
            onRoom={selectRoom}
            onDraft={setDraft}
            onSubmit={submitMessage}
            onComposerKeyDown={composerKeyDown}
            onPendingAction={() => announce(copy.notConnectedYet)}
          />
        ) : (
          <EmptyState copy={copy} onPendingAction={() => announce(copy.notConnectedYet)} />
        )}
      </main>

      <div className={`toast ${toast ? 'show' : ''}`} role="status" aria-live="polite">
        {toast}
      </div>
    </div>
  );
}

type AppCopy = ReturnType<typeof copyFor>;

interface HomeScreenProps {
  copy: AppCopy;
  communities: Community[];
  onCommunity: (community: Community) => void;
  onPendingAction: () => void;
}

function HomeScreen({ copy, communities, onCommunity, onPendingAction }: HomeScreenProps) {
  return (
    <section className="screen screen-home is-active" aria-labelledby="home-title">
      <div className="home-heading client-home-heading">
        <div>
          <p className="kicker">{copy.personalHome}</p>
          <h1 id="home-title">{copy.now}</h1>
          <p>{copy.homeLead}</p>
        </div>
        <div className="primary-actions">
          <button className="primary-action" type="button" onClick={onPendingAction}>{copy.join}</button>
          <button className="secondary-action" type="button" onClick={onPendingAction}>{copy.create}</button>
        </div>
      </div>

      <section className="pulse-section" aria-labelledby="pulse-title">
        <div className="section-title">
          <div><p>{copy.attention}</p><h2 id="pulse-title">{copy.mentionsUnreadInvites}</h2></div>
        </div>
        <div className="pulse-list">
          <button className="pulse-priority" type="button" onClick={() => communities[1] && onCommunity(communities[1])}>
            <span className="pulse-icon">@</span>
            <span><strong>{copy.mentionedYou}</strong><small>{copy.productDiscussion}</small></span>
            <time>{copy.minutes12}</time>
          </button>
          <button type="button" onClick={() => communities[0] && onCommunity(communities[0])}>
            <span className="pulse-icon">3</span>
            <span><strong>{copy.newWeekendMessages}</strong><small>{copy.continueWhereLeft}</small></span>
            <time>{copy.justNow}</time>
          </button>
          <button type="button" onClick={onPendingAction}>
            <span className="pulse-icon">＋</span>
            <span><strong>{copy.inviteFriend}</strong><small>{copy.onePersonOnly}</small></span>
            <time>{copy.createInvite}</time>
          </button>
        </div>
      </section>

      <section className="continue-section" aria-labelledby="continue-title">
        <div className="section-title">
          <div><p>{copy.yourCommunities}</p><h2 id="continue-title">{copy.chooseCommunity}</h2></div>
          <button className="text-button" type="button" onClick={onPendingAction}>
            {copy.findFilter} <span>{communities.length}</span>
          </button>
        </div>
        <div className="community-grid">
          {communities.map((community, index) => (
            <button
              className={`community-card ${index === 0 ? 'card-featured' : ''} ${community.unread === 0 ? 'card-muted' : ''}`}
              type="button"
              key={community.id}
              aria-label={`${copy.openCommunity}: ${community.name}`}
              onClick={() => onCommunity(community)}
            >
              <span className={`community-symbol symbol-${community.accent}`}>{community.symbol}</span>
              <span className="community-copy">
                <small>{community.name} · {community.roomSummary}</small>
                <strong>{community.summary}</strong>
                <span>{community.unread > 0 ? `${community.unread} ${copy.messages}` : community.roomSummary}</span>
              </span>
              <span className="presence-stack" aria-label={copy.peopleOnline(community.online)}>
                {Array.from({ length: Math.min(community.online, 3) }, (_, person) => (
                  <i className={`person ${community.accent}`} key={person}>{community.symbol}</i>
                ))}
              </span>
            </button>
          ))}
        </div>
      </section>
    </section>
  );
}

interface CommunityScreenProps {
  copy: AppCopy;
  community: Community;
  room: Room;
  messages: Message[];
  draft: string;
  sending: boolean;
  onBack: () => void;
  onRoom: (room: Room) => void;
  onDraft: (value: string) => void;
  onSubmit: (event: FormEvent) => void;
  onComposerKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => void;
  onPendingAction: () => void;
}

function CommunityScreen({
  copy,
  community,
  room,
  messages,
  draft,
  sending,
  onBack,
  onRoom,
  onDraft,
  onSubmit,
  onComposerKeyDown,
  onPendingAction,
}: CommunityScreenProps) {
  return (
    <section className="screen screen-community is-active" aria-labelledby="community-title">
      <header className="community-header">
        <button className="back-button" type="button" onClick={onBack} aria-label={copy.backHome}>←</button>
        <span className={`community-symbol symbol-${community.accent}`}>{community.symbol}</span>
        <div className="community-title-wrap"><p>{copy.community}</p><h1 id="community-title">{community.name}</h1></div>
        <div className="community-people">
          <span className="presence-stack large" aria-label={copy.peopleOnline(community.online)}>
            {Array.from({ length: Math.min(community.online, 3) }, (_, person) => (
              <i className={`person ${community.accent}`} key={person}>{community.symbol}</i>
            ))}
            <span>{copy.peopleOnline(community.online)}</span>
          </span>
          <button className="secondary-action compact" type="button" onClick={onPendingAction}>{copy.invite}</button>
        </div>
      </header>

      <div className="room-strip" role="tablist" aria-label={copy.rooms}>
        {community.rooms.map((item) => (
          <button
            className={`room-tab ${item.id === room.id ? 'is-active' : ''}`}
            type="button"
            role="tab"
            aria-selected={item.id === room.id}
            key={item.id}
            onClick={() => onRoom(item)}
          >
            {item.name} {item.unread > 0 && <span>{item.unread}</span>}
          </button>
        ))}
        <button className="room-browser" type="button" onClick={onPendingAction}>{copy.browseRooms} <span>⌄</span></button>
      </div>

      <div className="conversation-layout">
        <section className="conversation" aria-label={copy.messages}>
          <div className="conversation-heading">
            <div><p>{copy.today}</p><h2>{room.name}</h2></div>
            <button className="icon-button" type="button" onClick={onPendingAction} aria-label={copy.roomInfo}>···</button>
          </div>
          <div className="message-list" aria-live="polite">
            {messages.map((message) => (
              <article className={`message ${message.own ? 'message-self' : ''}`} key={message.id}>
                <span className={`message-avatar ${message.tone}`}>{message.avatar}</span>
                <div>
                  <header><strong>{message.author}</strong><time>{message.sentAt}</time></header>
                  <p>{message.body}</p>
                </div>
              </article>
            ))}
          </div>
          <form className="composer" onSubmit={onSubmit}>
            <button type="button" className="composer-add" onClick={onPendingAction} aria-label={copy.customize}>＋</button>
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
      </div>
    </section>
  );
}

function EmptyState({ copy, onPendingAction }: { copy: AppCopy; onPendingAction: () => void }) {
  return (
    <section className="client-empty">
      <p className="kicker">{copy.personalHome}</p>
      <h1>{copy.noCommunity}</h1>
      <p>{copy.noCommunityLead}</p>
      <button className="primary-action" type="button" onClick={onPendingAction}>{copy.join}</button>
    </section>
  );
}
