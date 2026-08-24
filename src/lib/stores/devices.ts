// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

/**
 * Общий стор событий устройств.
 * initDeviceEvents() идемпотентен — можно звать из любого компонента.
 */
import { writable } from 'svelte/store';
import type { DeviceEventTS } from '$lib/services/api';

export const lastWeight = writable<{ grams: number; at: number } | null>(null);
export const lastScan = writable<{ code: string; at: number } | null>(null);

let inited = false;

export async function initDeviceEvents(): Promise<void> {
  if (inited) return;
  inited = true;
  try {
    const { listen } = await import('@tauri-apps/api/event');
    await listen<{ device_id: string; event: DeviceEventTS }>('device-event', (e) => {
      const ev = e.payload?.event;
      if (!ev) return;
      if (ev.type === 'weighed') {
        lastWeight.set({ grams: ev.grams ?? 0, at: Date.now() });
      } else if (ev.type === 'scanned') {
        lastScan.set({ code: ev.code ?? '', at: Date.now() });
      }
    });
  } catch {
    // не tauri-окружение (тесты/dev) — тихо игнорируем
  }
}

/** Вес в килограммах (3 знака) или null. */
export function weightToKg(grams: number): number {
  return Math.round(grams) / 1000;
}
