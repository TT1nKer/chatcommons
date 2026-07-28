import type {
  ClientAdapter,
  ClientSnapshot,
  CreateInvitationInput,
  FeedbackInput,
  FeedbackStatus,
  Invitation,
  Message,
  SendMessageInput,
  VoiceGrant,
  VoiceTokenInput,
} from '../domain';
import { ClientBridgeError } from '../domain';

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

async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke()<T>(command, args);
  } catch (reason) {
    if (
      reason
      && typeof reason === 'object'
      && 'code' in reason
      && typeof reason.code === 'string'
    ) {
      const detail = 'detail' in reason && typeof reason.detail === 'string'
        ? reason.detail
        : '';
      throw new ClientBridgeError(reason.code, detail);
    }
    throw new ClientBridgeError(
      'bridgeFailure',
      reason instanceof Error ? reason.message : String(reason),
    );
  }
}

export class TauriAdapter implements ClientAdapter {
  readonly kind = 'tauri' as const;

  load(): Promise<ClientSnapshot> {
    return call<ClientSnapshot>('client_snapshot');
  }

  sync(): Promise<ClientSnapshot> {
    return call<ClientSnapshot>('sync_client');
  }

  joinCommunity(inviteCode: string): Promise<ClientSnapshot> {
    return call<ClientSnapshot>('join_community', { inviteCode });
  }

  createInvitation(input: CreateInvitationInput): Promise<Invitation> {
    return call<Invitation>('create_invitation', { input });
  }

  sendMessage(input: SendMessageInput): Promise<Message> {
    return call<Message>('send_message', { input });
  }

  voiceToken(input: VoiceTokenInput): Promise<VoiceGrant> {
    return call<VoiceGrant>('voice_token', { input });
  }

  submitFeedback(input: FeedbackInput): Promise<FeedbackStatus> {
    return call<FeedbackStatus>('submit_feedback', { input });
  }

  feedbackStatus(): Promise<FeedbackStatus | null> {
    return call<FeedbackStatus | null>('feedback_status');
  }
}
