<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  interface Props {
    onSubmit: (uri: string, dbName: string) => Promise<string | null>;
    initialUri?: string;
    initialName?: string;
  }
  let { onSubmit, initialUri = '', initialName = '2c_platform' }: Props = $props();

  let uri = $state(initialUri);
  let name = $state(initialName);
  let error = $state('');
  let busy = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!uri.trim()) { error = 'Укажите URI подключения'; return; }
    error = ''; busy = true;
    const err = await onSubmit(uri.trim(), name.trim() || '2c_platform');
    if (err) error = err;
    busy = false;
  }
</script>

<div class="flex h-screen items-center justify-center bg-surface-50-950">
  <div class="w-full max-w-md space-y-6 rounded-2xl border border-surface-300-700 bg-surface-50-950 p-8 shadow-xl">
    <div class="text-center">
      <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-primary-500 text-2xl font-bold text-white">2C</div>
      <h1 class="mt-4 text-xl font-bold text-surface-900-100">Подключение к базе данных</h1>
      <p class="mt-1 text-sm text-surface-500-500">Введите параметры подключения к MongoDB</p>
    </div>
    <form onsubmit={submit} class="space-y-4">
      <label class="block text-sm font-medium text-surface-700-300">
        URI подключения
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={uri} class="input mt-1 w-full" placeholder="mongodb://user:pass@host:port" autofocus />
      </label>
      <label class="block text-sm font-medium text-surface-700-300">
        Имя базы данных
        <input bind:value={name} class="input mt-1 w-full" placeholder="2c_platform" />
      </label>
      {#if error}<div class="rounded-lg bg-error-500/10 p-3 text-sm text-error-600" role="alert">{error}</div>{/if}
      <button type="submit" disabled={busy} class="btn preset-filled-primary-500 w-full">
        {#if busy}<i class="fa-solid fa-circle-notch fa-spin"></i>{/if}
        {busy ? 'Подключение…' : 'Подключиться'}
      </button>
    </form>
  </div>
</div>
