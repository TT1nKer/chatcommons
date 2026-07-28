import type {
  Participant,
  RemoteTrack,
  Room as LiveKitRoom,
} from 'livekit-client';
import {
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';
import {
  ClientBridgeError,
  clientFailureCode,
  type ClientAdapter,
  type VoiceGrant,
} from '../domain';
import { microphoneFailureCode, previewMicrophone } from './microphone';
import { useMicrophoneSelection } from './useMicrophoneSelection';

const minimumTokenLifetimeMs = 5_000;
const maximumTokenBytes = 16 * 1024;

export type VoiceStatus =
  | 'idle'
  | 'checking'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'error';

export interface VoiceParticipant {
  id: string;
  name: string;
  symbol: string;
  local: boolean;
  speaking: boolean;
}

interface VoiceSessionState {
  status: VoiceStatus;
  roomKey: string;
  roomName: string;
  muted: boolean;
  switchingMicrophone: boolean;
  microphoneName: string;
  microphoneLevel: number;
  participants: VoiceParticipant[];
  errorCode: string;
}

const initialState: VoiceSessionState = {
  status: 'idle',
  roomKey: '',
  roomName: '',
  muted: false,
  switchingMicrophone: false,
  microphoneName: '',
  microphoneLevel: 0,
  participants: [],
  errorCode: '',
};

function validateGrant(grant: VoiceGrant): void {
  let serverUrl: URL;
  try {
    serverUrl = new URL(grant.serverUrl);
  } catch {
    throw new ClientBridgeError('voiceGrantInvalid', 'voice server URL is invalid');
  }
  if (
    serverUrl.protocol !== 'wss:'
    || !grant.participantToken
    || grant.participantToken.length > maximumTokenBytes
    || /\s/.test(grant.participantToken)
    || grant.expiresAtMs < Date.now() + minimumTokenLifetimeMs
  ) {
    throw new ClientBridgeError('voiceGrantInvalid', 'voice grant failed validation');
  }
}

function participantView(
  participant: Participant,
  local: boolean,
  activeSpeakers: Set<string>,
): VoiceParticipant {
  const fallbackName = participant.identity.split(':')[0] || participant.identity;
  const name = participant.name?.trim() || fallbackName.slice(0, 12);
  return {
    id: participant.identity,
    name,
    symbol: name.slice(0, 1).toUpperCase() || '?',
    local,
    speaking: activeSpeakers.has(participant.identity),
  };
}

export function useVoiceSession(adapter: ClientAdapter) {
  const [state, setState] = useState<VoiceSessionState>(initialState);
  const microphoneSelection = useMicrophoneSelection();
  const roomRef = useRef<LiveKitRoom | null>(null);
  const audioElements = useRef(new Set<HTMLMediaElement>());
  const previewAbort = useRef<AbortController | null>(null);
  const microphoneSwitchInFlight = useRef(false);
  const generation = useRef(0);

  const removeAudio = useCallback(() => {
    for (const element of audioElements.current) {
      element.pause();
      element.remove();
    }
    audioElements.current.clear();
  }, []);

  const disconnectCurrent = useCallback(() => {
    const current = roomRef.current;
    roomRef.current = null;
    if (current) {
      current.removeAllListeners();
      void current.disconnect();
    }
    removeAudio();
  }, [removeAudio]);

  const stopMicrophonePreview = useCallback(() => {
    previewAbort.current?.abort();
    previewAbort.current = null;
  }, []);

  const leave = useCallback(() => {
    generation.current += 1;
    stopMicrophonePreview();
    disconnectCurrent();
    setState(initialState);
  }, [disconnectCurrent, stopMicrophonePreview]);

  const join = useCallback(async ({
    communityId,
    roomId,
    roomName,
  }: {
    communityId: string;
    roomId: string;
    roomName: string;
  }) => {
    const nextRoomKey = `${communityId}:${roomId}`;
    if (
      state.roomKey === nextRoomKey
      && (
        state.status === 'checking'
        || state.status === 'connecting'
        || state.status === 'connected'
        || state.status === 'reconnecting'
      )
    ) {
      return;
    }

    const attempt = generation.current + 1;
    generation.current = attempt;
    stopMicrophonePreview();
    disconnectCurrent();
    setState({
      ...initialState,
      status: 'checking',
      roomKey: nextRoomKey,
      roomName,
    });

    try {
      const abortController = new AbortController();
      previewAbort.current = abortController;
      let microphoneReady = false;
      try {
        microphoneReady = await previewMicrophone({
          deviceId: microphoneSelection.selectedMicrophoneId,
          signal: abortController.signal,
          onReady: (microphoneName) => {
            if (generation.current !== attempt) return;
            void microphoneSelection.refreshMicrophones();
            setState((current) => ({
              ...current,
              microphoneName,
              microphoneLevel: 0,
            }));
          },
          onLevel: (microphoneLevel) => {
            if (generation.current !== attempt) return;
            setState((current) => ({ ...current, microphoneLevel }));
          },
        });
      } finally {
        if (previewAbort.current === abortController) {
          previewAbort.current = null;
        }
      }
      if (!microphoneReady || generation.current !== attempt) return;
      setState((current) => ({
        ...current,
        status: 'connecting',
        microphoneLevel: 0,
      }));

      const grant = await adapter.voiceToken({ communityId, roomId });
      validateGrant(grant);
      if (generation.current !== attempt) return;

      const { Room, RoomEvent, Track } = await import('livekit-client');
      if (generation.current !== attempt) return;
      const liveRoom = new Room({
        adaptiveStream: false,
        dynacast: false,
        disconnectOnPageLeave: true,
      });
      roomRef.current = liveRoom;

      const refreshParticipants = () => {
        if (generation.current !== attempt) return;
        const activeSpeakers = new Set(
          liveRoom.activeSpeakers.map((participant) => participant.identity),
        );
        const participants = [
          participantView(liveRoom.localParticipant, true, activeSpeakers),
          ...[...liveRoom.remoteParticipants.values()].map((participant) => (
            participantView(participant, false, activeSpeakers)
          )),
        ];
        setState((current) => ({ ...current, participants }));
      };

      liveRoom
        .on(RoomEvent.ParticipantConnected, refreshParticipants)
        .on(RoomEvent.ParticipantDisconnected, refreshParticipants)
        .on(RoomEvent.ParticipantNameChanged, refreshParticipants)
        .on(RoomEvent.ActiveSpeakersChanged, refreshParticipants)
        .on(RoomEvent.Reconnecting, () => {
          setState((current) => ({ ...current, status: 'reconnecting' }));
        })
        .on(RoomEvent.Reconnected, () => {
          setState((current) => ({ ...current, status: 'connected', errorCode: '' }));
          refreshParticipants();
        })
        .on(RoomEvent.Disconnected, () => {
          if (generation.current !== attempt) return;
          roomRef.current = null;
          removeAudio();
          setState((current) => (
            current.status === 'error' ? current : initialState
          ));
        })
        .on(RoomEvent.TrackSubscribed, (track: RemoteTrack) => {
          if (track.kind !== Track.Kind.Audio) return;
          const element = track.attach();
          element.dataset.chatcommonsVoiceAudio = 'true';
          element.hidden = true;
          document.body.append(element);
          audioElements.current.add(element);
        })
        .on(RoomEvent.TrackUnsubscribed, (track: RemoteTrack) => {
          for (const element of track.detach()) {
            audioElements.current.delete(element);
            element.remove();
          }
        })
        .on(RoomEvent.MediaDevicesError, () => {
          setState((current) => ({ ...current, status: 'error', errorCode: 'voicePermission' }));
        });

      await liveRoom.connect(grant.serverUrl, grant.participantToken, {
        autoSubscribe: true,
      });
      if (generation.current !== attempt) {
        await liveRoom.disconnect();
        return;
      }
      await liveRoom.startAudio();
      if (microphoneSelection.selectedMicrophoneId) {
        await liveRoom.localParticipant.setMicrophoneEnabled(
          true,
          { deviceId: microphoneSelection.selectedMicrophoneId },
        );
      } else {
        await liveRoom.localParticipant.setMicrophoneEnabled(true);
      }
      refreshParticipants();
      setState((current) => ({
        ...current,
        status: 'connected',
        muted: false,
        errorCode: '',
      }));
    } catch (reason) {
      if (generation.current !== attempt) return;
      disconnectCurrent();
      stopMicrophonePreview();
      const code = clientFailureCode(reason);
      setState((current) => ({
        ...current,
        status: 'error',
        errorCode: code === 'unknown' ? 'voiceConnect' : code,
      }));
    }
  }, [
    adapter,
    disconnectCurrent,
    removeAudio,
    microphoneSelection.refreshMicrophones,
    microphoneSelection.selectedMicrophoneId,
    state.roomKey,
    state.status,
    stopMicrophonePreview,
  ]);

  const toggleMute = useCallback(async () => {
    const current = roomRef.current;
    if (!current || state.status !== 'connected') return;
    const nextMuted = !state.muted;
    try {
      await current.localParticipant.setMicrophoneEnabled(!nextMuted);
      setState((previous) => ({ ...previous, muted: nextMuted, errorCode: '' }));
    } catch {
      disconnectCurrent();
      setState((previous) => ({
        ...previous,
        status: 'error',
        errorCode: 'voicePermission',
      }));
    }
  }, [disconnectCurrent, state.muted, state.status]);

  const selectMicrophone = useCallback(async (deviceId: string) => {
    if (
      deviceId === microphoneSelection.selectedMicrophoneId
      || microphoneSwitchInFlight.current
      || state.status === 'checking'
      || state.status === 'connecting'
      || state.status === 'reconnecting'
    ) {
      return;
    }

    const current = roomRef.current;
    if (!current || state.status !== 'connected') {
      microphoneSelection.selectMicrophone(deviceId);
      return;
    }

    microphoneSwitchInFlight.current = true;
    setState((previous) => ({
      ...previous,
      switchingMicrophone: true,
      errorCode: '',
    }));
    try {
      await current.switchActiveDevice('audioinput', deviceId || 'default');
      if (roomRef.current !== current) return;
      microphoneSelection.selectMicrophone(deviceId);
      const selected = microphoneSelection.microphones.find(
        (microphone) => microphone.id === deviceId,
      );
      setState((previous) => ({
        ...previous,
        switchingMicrophone: false,
        microphoneName: selected?.label ?? '',
        errorCode: '',
      }));
    } catch (reason) {
      if (roomRef.current !== current) return;
      setState((previous) => ({
        ...previous,
        switchingMicrophone: false,
        errorCode: microphoneFailureCode(reason, navigator.userAgent),
      }));
    } finally {
      microphoneSwitchInFlight.current = false;
    }
  }, [
    microphoneSelection.microphones,
    microphoneSelection.selectMicrophone,
    microphoneSelection.selectedMicrophoneId,
    state.status,
  ]);

  useEffect(() => () => {
    generation.current += 1;
    stopMicrophonePreview();
    disconnectCurrent();
  }, [disconnectCurrent, stopMicrophonePreview]);

  return {
    ...state,
    microphones: microphoneSelection.microphones,
    selectedMicrophoneId: microphoneSelection.selectedMicrophoneId,
    selectMicrophone,
    join,
    leave,
    toggleMute,
  };
}
