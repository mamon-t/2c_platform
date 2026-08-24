// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

import { writable } from 'svelte/store';

export interface DialogOptions {
  title: string;
  message?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  placeholder?: string;
  initialValue?: string;
  inputType?: 'text' | 'password';
}

interface ActiveDialog extends DialogOptions {
  kind: 'confirm' | 'prompt';
  resolve: (value: string | boolean | null) => void;
}

export const activeDialog = writable<ActiveDialog | null>(null);

function open(kind: 'confirm' | 'prompt', opts: DialogOptions): Promise<string | boolean | null> {
  return new Promise((resolve) => {
    activeDialog.set({ kind, ...opts, resolve });
  });
}

/** Замена нативного confirm(): true = подтверждено */
export const confirmDialog = (opts: DialogOptions | string) =>
  open('confirm', typeof opts === 'string' ? { title: opts } : opts) as Promise<boolean>;

/** Замена нативного prompt(): строка или null при отмене */
export const promptDialog = (opts: DialogOptions | string) =>
  open('prompt', typeof opts === 'string' ? { title: opts } : opts) as Promise<string | null>;

export function closeDialog(value: string | boolean | null): void {
  activeDialog.update((d) => {
    d?.resolve(value);
    return null;
  });
}
