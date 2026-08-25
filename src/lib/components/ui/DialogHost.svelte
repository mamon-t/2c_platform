<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { activeDialog, closeDialog } from './dialog';

  let value = $state('');
  let inputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    const d = $activeDialog;
    if (d?.kind === 'prompt') {
      value = d.initialValue ?? '';
      setTimeout(() => inputEl?.focus(), 30);
    }
  });

  function onKeydown(e: KeyboardEvent) {
    if (!$activeDialog) return;
    if (e.key === 'Escape') { e.preventDefault(); closeDialog($activeDialog.kind === 'prompt' ? null : false); }
    else if (e.key === 'Enter' && $activeDialog.kind === 'prompt') { e.preventDefault(); submit(); }
  }

  function submit() {
    if ($activeDialog?.kind === 'prompt') closeDialog(value.trim() || null);
    else closeDialog(true);
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if $activeDialog}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="fixed inset-0 z-[90] grid place-items-center bg-black/50"
    role="presentation"
    onclick={() => closeDialog($activeDialog.kind === 'prompt' ? null : false)}
  >
    <div
      class="card w-[420px] max-w-[92vw] space-y-3 bg-surface-100-900 p-4 shadow-2xl"
      role="alertdialog"
      aria-modal="true"
      aria-label={$activeDialog.title}
      onclick={(e) => e.stopPropagation()}
    >
      <h3 class="flex items-center gap-2 text-sm font-semibold {$activeDialog.danger ? 'text-error-600' : ''}">
        {#if $activeDialog.danger}<i class="fa-solid fa-triangle-exclamation"></i>{/if}
        {$activeDialog.title}
      </h3>

      {#if $activeDialog.message}
        <p class="text-sm whitespace-pre-wrap text-surface-600-400">{$activeDialog.message}</p>
      {/if}

      {#if $activeDialog.kind === 'prompt'}
        <input
          bind:this={inputEl}
          bind:value
          class="input input-sm w-full"
          type={$activeDialog.inputType ?? 'text'}
          placeholder={$activeDialog.placeholder ?? ''}
        />
      {/if}

      <div class="flex justify-end gap-2 pt-1">
        <button class="btn btn-sm btn-outline" onclick={() => closeDialog($activeDialog.kind === 'prompt' ? null : false)}>
          {$activeDialog.cancelLabel ?? 'Отмена'}
        </button>
        <button
          class="btn btn-sm {$activeDialog.danger ? 'preset-filled-error-500' : 'preset-filled-primary-500'}"
          onclick={submit}
        >
          {$activeDialog.confirmLabel ?? ($activeDialog.kind === 'prompt' ? 'ОК' : 'Подтвердить')}
        </button>
      </div>
    </div>
  </div>
{/if}
