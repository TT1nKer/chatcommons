import { ClientBridgeError } from '../domain';

const previewDurationMs = 800;
const levelIntervalMs = 60;

export interface MicrophoneDevice {
  id: string;
  label: string;
}

interface MicrophonePreviewOptions {
  deviceId: string;
  signal: AbortSignal;
  onReady: (name: string) => void;
  onLevel: (level: number) => void;
}

function errorName(reason: unknown): string {
  if (
    typeof reason === 'object'
    && reason !== null
    && 'name' in reason
    && typeof reason.name === 'string'
  ) {
    return reason.name;
  }
  return '';
}

export function microphoneFailureCode(reason: unknown, userAgent: string): string {
  switch (errorName(reason)) {
    case 'NotAllowedError':
    case 'PermissionDeniedError':
    case 'SecurityError':
      if (/Windows/i.test(userAgent)) return 'voicePermissionWindows';
      if (/Macintosh|Mac OS/i.test(userAgent)) return 'voicePermissionMac';
      return 'voicePermission';
    case 'NotFoundError':
    case 'DevicesNotFoundError':
      return 'voiceMicrophoneMissing';
    case 'OverconstrainedError':
    case 'ConstraintNotSatisfiedError':
      return 'voiceMicrophoneSelectionMissing';
    case 'AbortError':
    case 'NotReadableError':
    case 'TrackStartError':
      return 'voiceMicrophoneBusy';
    case 'NotSupportedError':
    case 'TypeError':
      return 'voiceMicrophoneUnsupported';
    default:
      return 'voiceMicrophoneUnavailable';
  }
}

export async function listMicrophones(): Promise<MicrophoneDevice[]> {
  if (!navigator.mediaDevices?.enumerateDevices) return [];
  const devices = await navigator.mediaDevices.enumerateDevices();
  return devices
    .filter((device) => (
      device.kind === 'audioinput'
      && device.deviceId
      && device.deviceId !== 'default'
    ))
    .map((device) => ({
      id: device.deviceId,
      label: device.label.trim(),
    }));
}

async function sampleInputLevel(
  stream: MediaStream,
  signal: AbortSignal,
  onLevel: (level: number) => void,
): Promise<void> {
  const AudioContextConstructor = window.AudioContext;
  if (!AudioContextConstructor) return;

  let context: AudioContext;
  try {
    context = new AudioContextConstructor();
    if (context.state === 'suspended') await context.resume();
  } catch {
    // The audio track already proves the microphone works. Older WebViews may
    // lack a usable Web Audio analyser, so the visual meter is optional.
    return;
  }

  try {
    const source = context.createMediaStreamSource(stream);
    const analyser = context.createAnalyser();
    analyser.fftSize = 256;
    source.connect(analyser);
    const samples = new Uint8Array(analyser.fftSize);

    await new Promise<void>((resolve) => {
      let interval = 0;
      let timeout = 0;
      let finished = false;
      const finish = () => {
        if (finished) return;
        finished = true;
        window.clearInterval(interval);
        window.clearTimeout(timeout);
        signal.removeEventListener('abort', finish);
        resolve();
      };
      const updateLevel = () => {
        analyser.getByteTimeDomainData(samples);
        let sum = 0;
        for (const sample of samples) {
          const amplitude = (sample - 128) / 128;
          sum += amplitude * amplitude;
        }
        onLevel(Math.min(1, Math.sqrt(sum / samples.length) * 4));
      };

      interval = window.setInterval(updateLevel, levelIntervalMs);
      timeout = window.setTimeout(finish, previewDurationMs);
      signal.addEventListener('abort', finish, { once: true });
      if (signal.aborted) {
        finish();
      } else {
        updateLevel();
      }
    });
  } catch {
    // A working audio track is sufficient even when the optional level meter
    // fails after the AudioContext has been created.
  } finally {
    if (context.state !== 'closed') {
      // Failure to close an already-disposable analyser must not turn a
      // successful microphone check into a connection failure.
      await context.close().catch(() => {});
    }
  }
}

export async function previewMicrophone({
  deviceId,
  signal,
  onReady,
  onLevel,
}: MicrophonePreviewOptions): Promise<boolean> {
  if (!navigator.mediaDevices?.getUserMedia) {
    throw new ClientBridgeError(
      'voiceMicrophoneUnsupported',
      'microphone capture is unavailable in this client',
    );
  }

  let stream: MediaStream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      audio: deviceId ? { deviceId: { exact: deviceId } } : true,
      video: false,
    });
  } catch (reason) {
    throw new ClientBridgeError(
      microphoneFailureCode(reason, navigator.userAgent),
      'microphone preflight failed',
    );
  }

  try {
    if (signal.aborted) return false;
    const microphone = stream.getAudioTracks()[0];
    if (!microphone) {
      throw new ClientBridgeError(
        'voiceMicrophoneMissing',
        'microphone capture returned no audio track',
      );
    }
    onReady(microphone.label.trim());
    await sampleInputLevel(stream, signal, onLevel);
    return !signal.aborted;
  } finally {
    for (const track of stream.getTracks()) track.stop();
  }
}
