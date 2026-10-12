import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export const desktop = isTauri();

export function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!desktop) return Promise.reject(new Error('Bu ekranı Game Library masaüstü uygulamasında aç.'));
  return invoke<T>(name, args);
}

export function subscribe<T>(name: string, handler: (value: T) => void) {
  if (!desktop) return Promise.resolve(() => {});
  return listen<T>(name, event => handler(event.payload));
}

export const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);
