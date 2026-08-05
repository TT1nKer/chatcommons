import type { FormEvent } from 'react';
import { DialogFrame } from '../components/DialogFrame';
import type { Copy } from '../i18n';

interface JoinCommunityDialogProps {
  copy: Copy;
  inviteCode: string;
  joining: boolean;
  notice: string;
  onInviteCode: (value: string) => void;
  onClose: () => void;
  onSubmit: (event: FormEvent) => void;
}

export function JoinCommunityDialog({
  copy,
  inviteCode,
  joining,
  notice,
  onInviteCode,
  onClose,
  onSubmit,
}: JoinCommunityDialogProps) {
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

interface CreateInvitationDialogProps {
  copy: Copy;
  communityName: string;
  invitationCode: string;
  creating: boolean;
  notice: string;
  onCreate: () => void;
  onCopy: () => void;
  onClose: () => void;
}

function invitationFingerprint(code: string): string {
  return code.length > 28
    ? `${code.slice(0, 16)}…${code.slice(-10)}`
    : code;
}

export function CreateInvitationDialog({
  copy,
  communityName,
  invitationCode,
  creating,
  notice,
  onCreate,
  onCopy,
  onClose,
}: CreateInvitationDialogProps) {
  return (
    <DialogFrame title={copy.createInviteTitle} closeLabel={copy.close} onClose={onClose}>
      <div className="dialog-form invitation-dialog">
        <div className="dialog-scroll">
          <p>{copy.createInviteLead(communityName)}</p>
          <div className="invitation-rule">
            <span aria-hidden="true">1</span>
            <div>
              <strong>{copy.onePersonOnly}</strong>
              <small>{copy.inviteRequiresServer}</small>
            </div>
          </div>

          {invitationCode ? (
            <section className="invitation-result" aria-live="polite">
              <span>{copy.inviteReady}</span>
              <strong>{invitationFingerprint(invitationCode)}</strong>
              <p>{copy.inviteReadyLead}</p>
              <details>
                <summary>{copy.showInviteCode}</summary>
                <textarea readOnly rows={5} value={invitationCode} aria-label={copy.onePersonInvite} />
              </details>
            </section>
          ) : null}
          {notice && <p className="dialog-notice" role="alert">{notice}</p>}
        </div>
        <footer>
          <button className="secondary-action" type="button" onClick={onClose}>{copy.close}</button>
          {invitationCode ? (
            <button className="primary-action" type="button" onClick={onCopy}>
              {copy.copyInvite}
            </button>
          ) : (
            <button className="primary-action" type="button" disabled={creating} onClick={onCreate}>
              {creating ? copy.creatingInvite : copy.createInvite}
            </button>
          )}
        </footer>
      </div>
    </DialogFrame>
  );
}
