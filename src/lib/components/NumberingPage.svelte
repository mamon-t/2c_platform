// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type NumberSequence, type EntityType } from '$lib/services/api';

  let sequences: NumberSequence[] = $state([]);
  let entityTypes: EntityType[] = $state([]);
  let loading = $state(true);
  let error = $state('');

  // Edit modal
  let showEditModal = $state(false);
  let editingSeq = $state<NumberSequence | null>(null);
  let editForm = $state({ prefix: '', padding: 6, suffix: '' });
  let saving = $state(false);

  // Reset modal
  let showResetModal = $state(false);
  let resetEntityTypeId = $state('');
  let resetEntityTypeName = $state('');
  let resetValue = $state<number | undefined>(undefined);

  async function load() {
    loading = true;
    error = '';
    try {
      [sequences, entityTypes] = await Promise.all([
        api.numberingList(),
        api.listEntityTypes(),
      ]);
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка загрузки';
    } finally {
      loading = false;
    }
  }

  function getEntityName(id: string): string {
    return entityTypes.find(e => e._id === id)?.name ?? id;
  }

  function openEdit(seq: NumberSequence) {
    editingSeq = seq;
    editForm = { prefix: seq.prefix, padding: seq.padding, suffix: seq.suffix };
    showEditModal = true;
  }

  async function saveFormat() {
    if (!editingSeq) return;
    saving = true;
    try {
      await api.numberingUpdateFormat(
        editingSeq.entity_type_id,
        editingSeq.entity_type_name || getEntityName(editingSeq.entity_type_id),
        { prefix: editForm.prefix, padding: editForm.padding, suffix: editForm.suffix }
      );
      showEditModal = false;
      await load();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка сохранения';
    } finally {
      saving = false;
    }
  }

  function openReset(seq: NumberSequence) {
    resetEntityTypeId = seq.entity_type_id;
    resetEntityTypeName = seq.entity_type_name || getEntityName(seq.entity_type_id);
    resetValue = undefined;
    showResetModal = true;
  }

  async function confirmReset() {
    try {
      await api.numberingReset(resetEntityTypeId, resetValue);
      showResetModal = false;
      await load();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка сброса';
    }
  }

  function previewNumber(seq: NumberSequence, value?: number): string {
    const v = value ?? seq.current_value + 1;
    const p = String(v).padStart(Math.max(seq.padding, 1), '0');
    return `${seq.prefix}${p}${seq.suffix}`;
  }

  onMount(load);
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h2 class="text-2xl font-bold text-surface-900-100">Нумерация документов</h2>
    <button onclick={load} class="rounded-lg border border-surface-300-700 px-3 py-1.5 text-sm text-surface-700-300 hover:bg-surface-200-800">
      <i class="fa-solid fa-arrows-rotate mr-1"></i>Обновить
    </button>
  </div>

  {#if error}
    <div class="rounded-lg bg-error-500/10 p-3 text-sm text-error-600">{error}</div>
  {/if}

  {#if loading}
    <div class="flex items-center justify-center py-12">
      <div class="h-8 w-8 animate-spin rounded-full border-4 border-primary-500 border-t-transparent"></div>
    </div>
  {:else if sequences.length === 0}
    <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">
      <i class="fa-solid fa-hashtag text-4xl mb-3 block text-surface-300-700"></i>
      <p>Последовательности нумерации ещё не созданы.</p>
      <p class="mt-1 text-sm">Они создаются автоматически при первом проведении документа.</p>
      <p class="mt-3 text-sm">Создайте документ и нажмите «Провести», чтобы инициализировать нумерацию.</p>
    </div>
  {:else}
    <div class="overflow-x-auto rounded-xl border border-surface-300-700">
      <table class="w-full text-left text-sm">
        <thead class="border-b border-surface-300-700 bg-surface-100-900 text-xs font-medium uppercase text-surface-500-500">
          <tr>
            <th class="px-4 py-3">Тип объекта</th>
            <th class="px-4 py-3">Префикс</th>
            <th class="px-4 py-3">Длина</th>
            <th class="px-4 py-3">Суффикс</th>
            <th class="px-4 py-3">Текущий №</th>
            <th class="px-4 py-3">Пример</th>
            <th class="px-4 py-3 text-right">Действия</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-surface-300-700">
          {#each sequences as seq (seq._id)}
            <tr class="hover:bg-surface-100-900/50">
              <td class="px-4 py-3">
                <div class="font-medium text-surface-900-100">{seq.entity_type_name || getEntityName(seq.entity_type_id)}</div>
                <div class="text-xs text-surface-500-500 font-mono">{seq.entity_type_id.slice(0, 8)}…</div>
              </td>
              <td class="px-4 py-3 font-mono text-sm text-surface-900-100">{seq.prefix || '—'}</td>
              <td class="px-4 py-3 text-surface-900-100">{seq.padding}</td>
              <td class="px-4 py-3 font-mono text-sm text-surface-900-100">{seq.suffix || '—'}</td>
              <td class="px-4 py-3 text-lg font-bold text-surface-900-100">{seq.current_value}</td>
              <td class="px-4 py-3 font-mono text-sm text-primary-500">{previewNumber(seq)}</td>
              <td class="px-4 py-3 text-right space-x-2">
                <button onclick={() => openEdit(seq)} class="rounded-lg px-3 py-1 text-xs font-medium bg-surface-200-800 text-surface-700-300 hover:bg-surface-300-700">
                  <i class="fa-solid fa-pen mr-1"></i>Формат
                </button>
                <button onclick={() => openReset(seq)} class="rounded-lg px-3 py-1 text-xs font-medium bg-warning-500/20 text-warning-700 hover:bg-warning-500/30">
                  <i class="fa-solid fa-rotate-left mr-1"></i>Сброс
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<!-- Edit format modal -->
{#if showEditModal && editingSeq}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onclick={() => { showEditModal = false; }}>
    <div class="w-full max-w-md rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 shadow-xl" onclick={(e) => e.stopPropagation()}>
      <h3 class="mb-4 font-semibold text-surface-900-100">Формат нумерации: {editingSeq.entity_type_name || getEntityName(editingSeq.entity_type_id)}</h3>
      <p class="mb-3 text-xs text-surface-500-500">Текущий номер: <span class="font-mono text-primary-500">{previewNumber(editingSeq)}</span></p>
      <div class="space-y-3">
        <label class="block text-sm text-surface-700-300">
          Префикс
          <input value={editForm.prefix} oninput={(e) => { editForm.prefix = (e.target as HTMLInputElement).value; }} class="mt-1 w-full rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 focus:border-primary-500 focus:outline-none" placeholder="напр. ОСВ-" />
        </label>
        <label class="block text-sm text-surface-700-300">
          Длина номера (цифр)
          <input type="number" min="1" max="20" value={editForm.padding} oninput={(e) => { editForm.padding = parseInt((e.target as HTMLInputElement).value) || 6; }} class="mt-1 w-full rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 focus:border-primary-500 focus:outline-none" />
        </label>
        <label class="block text-sm text-surface-700-300">
          Суффикс
          <input value={editForm.suffix} oninput={(e) => { editForm.suffix = (e.target as HTMLInputElement).value; }} class="mt-1 w-full rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 focus:border-primary-500 focus:outline-none" placeholder="напр. /2024" />
        </label>
        <div class="rounded-lg bg-surface-100-900 p-3 text-center">
          <div class="text-xs text-surface-500-500 mb-1">Будущий номер:</div>
          <div class="text-lg font-mono font-bold text-primary-500">{previewNumber(editingSeq, editingSeq.current_value + 1)}</div>
        </div>
      </div>
      <div class="mt-4 flex gap-2">
        <button onclick={saveFormat} disabled={saving} class="rounded-lg bg-primary-500 px-4 py-2 text-sm font-medium text-white hover:bg-primary-600 disabled:opacity-50">
          {saving ? 'Сохранение...' : 'Сохранить'}
        </button>
        <button onclick={() => { showEditModal = false; }} class="rounded-lg border border-surface-300-700 px-4 py-2 text-sm text-surface-700-300 hover:bg-surface-200-800">
          Отмена
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Reset modal -->
{#if showResetModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onclick={() => { showResetModal = false; }}>
    <div class="w-full max-w-sm rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 shadow-xl" onclick={(e) => e.stopPropagation()}>
      <h3 class="mb-2 font-semibold text-surface-900-100">Сброс нумерации</h3>
      <p class="mb-3 text-sm text-surface-500-500">Тип: <span class="font-medium text-surface-900-100">{resetEntityTypeName}</span></p>
      <label class="block text-sm text-surface-700-300">
        Новое значение (0 = сбросить сначала)
        <input type="number" min="0" bind:value={resetValue} class="mt-1 w-full rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 focus:border-primary-500 focus:outline-none" placeholder="0" />
      </label>
      <p class="mt-2 text-xs text-warning-600">
        <i class="fa-solid fa-triangle-exclamation mr-1"></i>
        Номера уже проведённых документов не изменятся.
      </p>
      <div class="mt-4 flex gap-2">
        <button onclick={confirmReset} class="rounded-lg bg-warning-500 px-4 py-2 text-sm font-medium text-white hover:bg-warning-600">
          Сбросить
        </button>
        <button onclick={() => { showResetModal = false; }} class="rounded-lg border border-surface-300-700 px-4 py-2 text-sm text-surface-700-300 hover:bg-surface-200-800">
          Отмена
        </button>
      </div>
    </div>
  </div>
{/if}
