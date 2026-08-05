import { useRef, useState } from 'react';
import type { ClientAdapter, Community } from '../domain';
import { clientFailureCode } from '../domain';
import type { Copy } from '../i18n';

export function useInvitationFlow(
  adapter: ClientAdapter,
  copy: Copy,
  announce: (message: string) => void,
) {
  const [community, setCommunity] = useState<Community | null>(null);
  const [code, setCode] = useState('');
  const [creating, setCreating] = useState(false);
  const [notice, setNotice] = useState('');
  const activeRequest = useRef(0);

  function open(next: Community) {
    if (!next.canInvite) return;
    activeRequest.current += 1;
    setCommunity(next);
    setCode('');
    setNotice('');
  }

  function close() {
    activeRequest.current += 1;
    setCommunity(null);
    setCode('');
    setCreating(false);
    setNotice('');
  }

  async function create() {
    if (!community || creating) return;
    const request = ++activeRequest.current;
    setCreating(true);
    setNotice('');
    try {
      const invitation = await adapter.createInvitation({
        communityId: community.id,
      });
      if (activeRequest.current === request) {
        setCode(invitation.code);
      }
    } catch (reason) {
      if (activeRequest.current === request) {
        setNotice(copy.errorMessage(clientFailureCode(reason)));
      }
    } finally {
      if (activeRequest.current === request) {
        setCreating(false);
      }
    }
  }

  async function copyCode() {
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code);
      announce(copy.inviteCopied);
    } catch {
      setNotice(copy.inviteCopyFailed);
    }
  }

  return {
    close,
    code,
    community,
    copyCode,
    create,
    creating,
    notice,
    open,
  };
}
