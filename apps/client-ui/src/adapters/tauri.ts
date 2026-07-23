import type {
  ClientAdapter,
  ClientSnapshot,
  Message,
  SendMessageInput,
} from '../domain';

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

interface TauriWindow extends Window {
  __TAURI__?: {
    core?: {
      invoke?: Invoke;
    };
  };
}

function invoke(): Invoke {
  const candidate = (window as TauriWindow).__TAURI__?.core?.invoke;
  if (!candidate) {
    throw new Error('The Tauri bridge is unavailable.');
  }
  return candidate;
}

export class TauriAdapter implements ClientAdapter {
  readonly kind = 'tauri' as const;

  load(): Promise<ClientSnapshot> {
    return invoke()<ClientSnapshot>('client_snapshot');
  }

  sendMessage(input: SendMessageInput): Promise<Message> {
    return invoke()<Message>('send_message', { input });
  }
}
