import type { ClientAdapter } from './domain';
import { DemoAdapter } from './adapters/demo';
import { TauriAdapter } from './adapters/tauri';

export function selectAdapter(): ClientAdapter {
  const requested = new URLSearchParams(window.location.search).get('adapter');
  if (requested === 'tauri' || '__TAURI__' in window) {
    return new TauriAdapter();
  }
  return new DemoAdapter();
}
