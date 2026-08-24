// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

import { writable } from 'svelte/store';

export type ToastKind = 'success' | 'error' | 'info' | 'warning';

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

let nextId = 1;

export const toasts = writable<ToastItem[]>([]);

export function toast(kind: ToastKind, message: string, timeoutMs = 4000): void {
  if (!message) return;
  const id = nextId++;
  toasts.update((list) => [...list.slice(-7), { id, kind, message }]);
  if (timeoutMs > 0) setTimeout(() => dismissToast(id), timeoutMs);
}

export const toastSuccess = (m: string) => toast('success', m);
export const toastError = (m: string) => toast('error', m, 7000);
export const toastInfo = (m: string) => toast('info', m);
export const toastWarning = (m: string) => toast('warning', m, 6000);

export function dismissToast(id: number): void {
  toasts.update((list) => list.filter((t) => t.id !== id));
}

export function errText(e: unknown, fallback = 'Ошибка'): string {
  return typeof e === 'string' ? e : (e as Error)?.message ?? fallback;
}
