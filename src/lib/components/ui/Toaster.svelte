<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { toasts, dismissToast, type ToastKind } from './toast';

  const META: Record<ToastKind, { cls: string; icon: string }> = {
    success: { cls: 'preset-tonal-success', icon: 'fa-solid fa-circle-check' },
    error: { cls: 'preset-tonal-error', icon: 'fa-solid fa-circle-exclamation' },
    warning: { cls: 'preset-tonal-warning', icon: 'fa-solid fa-triangle-exclamation' },
    info: { cls: 'preset-tonal-primary', icon: 'fa-solid fa-circle-info' },
  };
</script>

<div class="fixed left-4 top-16 z-[100] flex w-96 max-w-[92vw] flex-col gap-2" aria-live="polite">
  {#each $toasts as t (t.id)}
    <div
      class="alert {META[t.kind].cls} pointer-events-auto flex items-start gap-2 py-2 text-sm shadow-lg"
      role={t.kind === 'error' ? 'alert' : 'status'}
    >
      <i class="{META[t.kind].icon} mt-0.5 shrink-0"></i>
      <p class="flex-1 break-words whitespace-pre-wrap">{t.message}</p>
      <button class="shrink-0 opacity-60 hover:opacity-100" onclick={() => dismissToast(t.id)} aria-label="Закрыть уведомление">
        <i class="fa-solid fa-xmark text-xs"></i>
      </button>
    </div>
  {/each}
</div>
