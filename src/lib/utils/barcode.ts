// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

/**
 * Svelte-action: поле ввода штрихкода (keyboard-wedge сканер).
 *
 * Слушатель активен ТОЛЬКО пока фокус в этом поле — обычный набор
 * в остальных местах приложения не перехватывается.
 *
 * Эвристика: пауза > 80ms сбрасывает буфер, Enter завершает код ≥ 4 символов.
 * По завершении вызывается onCode(code); Enter не «проваливается» в форму.
 *
 * Использование:
 *   <input use:barcodeField={{ onCode: (code) => ... }} />
 */
export interface BarcodeFieldOptions {
  onCode: (code: string) => void;
}

export function barcodeField(node: HTMLInputElement, opts: BarcodeFieldOptions) {
  let buf = '';
  let last = 0;
  let active = false;
  let handler = opts.onCode;

  function onKey(e: KeyboardEvent) {
    if (!active) return;
    const now = Date.now();
    if (now - last > 80) buf = '';
    last = now;

    if (e.key === 'Enter') {
      e.preventDefault();
      if (buf.length >= 4) handler(buf);
      buf = '';
      node.select();
    } else if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
      buf += e.key;
    }
  }

  function onFocus() { active = true; }
  function onBlur() { active = false; buf = ''; }

  node.addEventListener('keydown', onKey);
  node.addEventListener('focus', onFocus);
  node.addEventListener('blur', onBlur);

  return {
    update(newOpts: BarcodeFieldOptions) { handler = newOpts.onCode; },
    destroy() {
      node.removeEventListener('keydown', onKey);
      node.removeEventListener('focus', onFocus);
      node.removeEventListener('blur', onBlur);
    },
  };
}
