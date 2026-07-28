import {
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';
import { listMicrophones, type MicrophoneDevice } from './microphone';

const preferredMicrophoneStorageKey = 'chatcommons-preferred-microphone';
const maximumStoredDeviceIdLength = 512;

function storedMicrophoneId(): string {
  try {
    const stored = localStorage.getItem(preferredMicrophoneStorageKey) ?? '';
    return stored.length <= maximumStoredDeviceIdLength ? stored : '';
  } catch {
    return '';
  }
}

function persistMicrophoneId(deviceId: string): void {
  try {
    if (deviceId) {
      localStorage.setItem(preferredMicrophoneStorageKey, deviceId);
    } else {
      localStorage.removeItem(preferredMicrophoneStorageKey);
    }
  } catch {
    // Device selection remains valid for this session when local storage is unavailable.
  }
}

export function useMicrophoneSelection() {
  const [microphones, setMicrophones] = useState<MicrophoneDevice[]>([]);
  const [selectedMicrophoneId, setSelectedMicrophoneId] = useState(storedMicrophoneId);
  const hasObservedMicrophones = useRef(false);
  const refreshGeneration = useRef(0);

  const refreshMicrophones = useCallback(async () => {
    const generation = refreshGeneration.current + 1;
    refreshGeneration.current = generation;
    try {
      const availableMicrophones = await listMicrophones();
      if (refreshGeneration.current !== generation) return;
      setMicrophones(availableMicrophones);
      const availabilityIsKnown = (
        availableMicrophones.length > 0
        || hasObservedMicrophones.current
      );
      if (availableMicrophones.length > 0) {
        hasObservedMicrophones.current = true;
      }
      setSelectedMicrophoneId((current) => {
        if (
          !current
          || !availabilityIsKnown
          || availableMicrophones.some((microphone) => microphone.id === current)
        ) {
          return current;
        }
        persistMicrophoneId('');
        return '';
      });
    } catch {
      if (refreshGeneration.current !== generation) return;
      setMicrophones([]);
      setSelectedMicrophoneId('');
      persistMicrophoneId('');
      // Enumeration is an optional convenience. Falling back to the default
      // keeps the visible selection aligned with the device used by getUserMedia.
    }
  }, []);

  useEffect(() => {
    void refreshMicrophones();
    const mediaDevices = navigator.mediaDevices;
    if (!mediaDevices?.addEventListener) return undefined;

    const refreshAfterDeviceChange = () => {
      void refreshMicrophones();
    };
    mediaDevices.addEventListener('devicechange', refreshAfterDeviceChange);
    return () => {
      mediaDevices.removeEventListener('devicechange', refreshAfterDeviceChange);
    };
  }, [refreshMicrophones]);

  const selectMicrophone = useCallback((deviceId: string) => {
    if (
      deviceId
      && !microphones.some((microphone) => microphone.id === deviceId)
    ) {
      return;
    }
    setSelectedMicrophoneId(deviceId);
    persistMicrophoneId(deviceId);
  }, [microphones]);

  return {
    microphones,
    selectedMicrophoneId,
    selectMicrophone,
    refreshMicrophones,
  };
}
