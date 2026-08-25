<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import type { SavedConnection } from '$lib/services/api';

  interface Props {
    onSubmit: (login: string, password: string) => Promise<string | null>;
    connections?: SavedConnection[];
    currentUri?: string;
    onSelectConnection?: (conn: SavedConnection) => Promise<string | null>;
    onOpenConnections?: () => void;
  }
  let { onSubmit, connections = [], currentUri = '', onSelectConnection, onOpenConnections }: Props = $props();

  let login = $state('');
  let password = $state('');
  let error = $state('');
  let busy = $state(false);
  let switching = $state(false);

  const currentConn = $derived(connections.find((c) => c.uri === currentUri) ?? null);

  async function selectConnection(e: Event) {
    const id = (e.currentTarget as HTMLSelectElement).value;
    const conn = connections.find((c) => c.id === id);
    if (!conn || !onSelectConnection) return;
    error = ''; switching = true;
    const err = await onSelectConnection(conn);
    if (err) error = err;
    switching = false;
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!login.trim() || !password) { error = 'Введите логин и пароль'; return; }
    error = ''; busy = true;
    const err = await onSubmit(login.trim(), password);
    if (err) error = err;
    busy = false;
  }
</script>

<div class="flex h-screen items-center justify-center bg-surface-50-950">
  <div class="w-full max-w-md space-y-6 rounded-2xl border border-surface-300-700 bg-surface-50-950 p-8 shadow-xl">
    <div class="text-center">
      <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-primary-500 text-2xl font-bold text-white">2C</div>
      <h1 class="mt-4 text-xl font-bold text-surface-900-100">Вход в систему</h1>
    </div>
    {#if onOpenConnections}
      <div>
        <label class="block text-sm font-medium text-surface-700-300" for="db-select">База данных</label>
        <div class="mt-1 flex gap-2">
          <select
            id="db-select"
            class="input w-full"
            disabled={switching}
            value={currentConn?.id ?? ''}
            onchange={selectConnection}
          >
            {#if connections.length === 0}
              <option value="">Текущее подключение</option>
            {/if}
            {#each connections as conn (conn.id)}
              <option value={conn.id}>{conn.name}</option>
            {/each}
          </select>
          <button
            type="button"
            class="btn preset-outlined-primary-500 shrink-0"
            onclick={onOpenConnections}
            title="Редактировать подключения"
            aria-label="Редактировать подключения"
          >
            <i class="fa-solid fa-gear"></i>
          </button>
        </div>
      </div>
    {/if}
    <form onsubmit={submit} class="space-y-4">
      <label class="block text-sm font-medium text-surface-700-300">
        Логин
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={login} class="input mt-1 w-full" placeholder="admin" autocomplete="username" autofocus />
      </label>
      <label class="block text-sm font-medium text-surface-700-300">
        Пароль
        <input bind:value={password} type="password" class="input mt-1 w-full" placeholder="••••••••" autocomplete="current-password" />
      </label>
      {#if error}<div class="rounded-lg bg-error-500/10 p-3 text-sm text-error-600" role="alert">{error}</div>{/if}
      <button type="submit" disabled={busy || switching} class="btn preset-filled-primary-500 w-full">
        {#if busy || switching}<i class="fa-solid fa-circle-notch fa-spin"></i>{/if}
        {switching ? 'Переключение базы…' : busy ? 'Вход…' : 'Войти'}
      </button>
    </form>
  </div>
</div>
