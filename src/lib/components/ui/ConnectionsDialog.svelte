<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { api, type SavedConnection } from '$lib/services/api';

  interface Props {
    open: boolean;
    onClose: () => void;
    onChanged: () => void;
  }
  let { open, onClose, onChanged }: Props = $props();

  let list = $state<SavedConnection[]>([]);
  let editing = $state<SavedConnection | null>(null);
  let error = $state('');
  let busy = $state(false);

  async function reload() {
    try { list = await api.listConnections(); } catch { list = []; }
  }

  $effect(() => { if (open) { editing = null; error = ''; reload(); } });

  function startAdd() {
    editing = { id: '', name: '', uri: '', db_name: '2c_platform' };
    error = '';
  }
  function startEdit(conn: SavedConnection) {
    editing = { ...conn };
    error = '';
  }
  function cancelEdit() { editing = null; error = ''; }

  async function save() {
    if (!editing) return;
    error = ''; busy = true;
    try {
      await api.saveConnection(editing);
      editing = null;
      await reload();
      onChanged();
    } catch (e) {
      error = typeof e === 'string' ? e : (e as Error)?.message ?? 'Ошибка сохранения';
    }
    busy = false;
  }

  async function remove(conn: SavedConnection) {
    busy = true;
    try {
      await api.deleteConnection(conn.id);
      await reload();
      onChanged();
    } catch (e) {
      error = typeof e === 'string' ? e : (e as Error)?.message ?? 'Ошибка удаления';
    }
    busy = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') { e.preventDefault(); editing ? cancelEdit() : onClose(); }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="fixed inset-0 z-[90] grid place-items-center bg-black/50" role="presentation">
    <div
      class="card w-[560px] max-w-[94vw] space-y-3 bg-surface-100-900 p-4 shadow-2xl"
      role="dialog"
      aria-modal="true"
      aria-label="Подключения к базе данных"
      onclick={(e) => e.stopPropagation()}
    >
      {#if !editing}
        <h3 class="flex items-center gap-2 text-sm font-semibold"><i class="fa-solid fa-database"></i> Подключения к базе данных</h3>
        <div class="max-h-72 space-y-1 overflow-y-auto">
          {#if list.length === 0}
            <p class="rounded-lg border border-dashed border-surface-300-700 p-4 text-center text-sm text-surface-500-500">
              Нет сохранённых подключений
            </p>
          {/if}
          {#each list as conn (conn.id)}
            <div class="flex items-center gap-2 rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2">
              <i class="fa-solid fa-server text-surface-400"></i>
              <div class="min-w-0 flex-1">
                <div class="truncate text-sm font-medium text-surface-900-100">{conn.name}</div>
                <div class="truncate text-xs text-surface-500-500">{conn.db_name}</div>
              </div>
              <button class="btn btn-sm btn-outline" onclick={() => startEdit(conn)} title="Изменить">
                <i class="fa-solid fa-pen"></i>
              </button>
              <button class="btn btn-sm preset-outlined-error-500" disabled={busy} onclick={() => remove(conn)} title="Удалить">
                <i class="fa-solid fa-trash"></i>
              </button>
            </div>
          {/each}
        </div>
        {#if error}<div class="rounded-lg bg-error-500/10 p-2 text-sm text-error-600" role="alert">{error}</div>{/if}
        <div class="flex justify-between pt-1">
          <button class="btn btn-sm preset-filled-primary-500" onclick={startAdd}>
            <i class="fa-solid fa-plus"></i> Добавить
          </button>
          <button class="btn btn-sm btn-outline" onclick={onClose}>Закрыть</button>
        </div>
      {:else}
        <h3 class="flex items-center gap-2 text-sm font-semibold">
          <i class="fa-solid fa-pen"></i> {editing.id ? 'Изменение подключения' : 'Новое подключение'}
        </h3>
        <form onsubmit={(e) => { e.preventDefault(); save(); }} class="space-y-3">
          <label class="block text-sm font-medium text-surface-700-300">
            Имя подключения
            <input bind:value={editing.name} class="input mt-1 w-full" placeholder="Рабочая база" />
          </label>
          <label class="block text-sm font-medium text-surface-700-300">
            URI подключения
            <input bind:value={editing.uri} class="input mt-1 w-full" placeholder="mongodb://user:pass@host:port" />
          </label>
          <label class="block text-sm font-medium text-surface-700-300">
            Имя базы данных
            <input bind:value={editing.db_name} class="input mt-1 w-full" placeholder="2c_platform" />
          </label>
          {#if error}<div class="rounded-lg bg-error-500/10 p-2 text-sm text-error-600" role="alert">{error}</div>{/if}
          <div class="flex justify-end gap-2 pt-1">
            <button type="button" class="btn btn-sm btn-outline" onclick={cancelEdit}>Отмена</button>
            <button type="submit" class="btn btn-sm preset-filled-primary-500" disabled={busy}>
              {#if busy}<i class="fa-solid fa-circle-notch fa-spin"></i>{/if}
              Сохранить
            </button>
          </div>
        </form>
      {/if}
    </div>
  </div>
{/if}
