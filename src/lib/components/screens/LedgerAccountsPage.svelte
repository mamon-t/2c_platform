<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/services/api';
  import { toastSuccess, toastError } from '$lib/components/ui/toast';

  interface Account {
    id: string;
    code: string;
    name: string;
    account_type: string;
    parent_code: string | null;
    is_active: boolean;
  }

  let accounts = $state<Account[]>([]);
  let loading = $state(true);
  let error = $state('');

  // Modal
  let showModal = $state(false);
  let editCode = $state('');
  let editName = $state('');
  let editType = $state('asset');
  let editParent = $state('');
  let editActive = $state(true);
  let isNew = $state(true);

  const typeOptions = [
    { value: 'asset', label: 'Актив' },
    { value: 'liability', label: 'Пассив' },
    { value: 'equity', label: 'Капитал' },
    { value: 'revenue', label: 'Доход' },
    { value: 'expense', label: 'Расход' },
    { value: 'off_balance', label: 'Забалансовый' },
  ];

  function typeLabel(t: string): string {
    return typeOptions.find(o => o.value === t)?.label ?? t;
  }

  async function load() {
    loading = true; error = '';
    try {
      accounts = (await api.ledgerAccountsList()) as Account[];
    } catch (e: any) {
      error = String(e);
    } finally { loading = false; }
  }

  function openNew() {
    isNew = true;
    editCode = '';
    editName = '';
    editType = 'asset';
    editParent = '';
    editActive = true;
    showModal = true;
  }

  function openEdit(a: Account) {
    isNew = false;
    editCode = a.code;
    editName = a.name;
    editType = a.account_type;
    editParent = a.parent_code ?? '';
    editActive = a.is_active;
    showModal = true;
  }

  async function handleSave() {
    try {
      if (isNew) {
        await api.ledgerAccountCreate(editCode, editName, editType, editParent || undefined);
        toastSuccess(`Счёт ${editCode} создан`);
      } else {
        await api.ledgerAccountUpdate(editCode, editName, editActive);
        toastSuccess(`Счёт ${editCode} обновлён`);
      }
      showModal = false;
      await load();
    } catch (e: any) {
      toastError(String(e));
    }
  }

  onMount(load);
</script>

<div class="container mx-auto p-4 space-y-4">
  <div class="flex items-center justify-between">
    <h2 class="h4 flex items-center gap-2">
      <i class="fa-solid fa-list-check"></i> План счетов
    </h2>
    <button class="btn btn-sm preset-filled-primary" onclick={openNew}>
      <i class="fa-solid fa-plus mr-1"></i>Добавить счёт
    </button>
  </div>

  {#if error}
    <div class="alert preset-tonal-error text-sm">{error}</div>
  {/if}

  {#if loading}
    <div class="flex flex-col items-center justify-center py-16 gap-3">
      <div class="h-10 w-10 animate-spin rounded-full border-4 border-primary-500 border-t-transparent"></div>
      <span class="text-sm text-surface-500">Загрузка плана счетов...</span>
    </div>
  {:else}
    <div class="overflow-x-auto">
      <table class="table table-sm text-xs">
        <thead>
          <tr>
            <th class="w-20">Код</th>
            <th>Название</th>
            <th class="w-24">Тип</th>
            <th class="w-20">Родитель</th>
            <th class="w-20 text-center">Активен</th>
            <th class="w-20"></th>
          </tr>
        </thead>
        <tbody>
          {#each accounts as a (a.id)}
            <tr class={!a.is_active ? 'opacity-50' : ''}>
              <td class="font-mono font-medium">{a.code}</td>
              <td>{a.name}</td>
              <td class="text-surface-400">{typeLabel(a.account_type)}</td>
              <td class="font-mono text-surface-400">{a.parent_code ?? '—'}</td>
              <td class="text-center">
                {#if a.is_active}
                  <i class="fa-solid fa-check text-green-500"></i>
                {:else}
                  <i class="fa-solid fa-xmark text-surface-400"></i>
                {/if}
              </td>
              <td>
                <button class="btn btn-xs preset-tonal" onclick={() => openEdit(a)}>
                  <i class="fa-solid fa-pen"></i>
                </button>
              </td>
            </tr>
          {:else}
            <tr>
              <td colspan="6" class="text-center text-surface-400 py-4">План счетов пуст</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

{#if showModal}
  <div class="fixed inset-0 bg-black/50 z-[60] grid place-items-center" role="presentation" onclick={() => showModal = false}>
    <div class="card p-5 w-96 space-y-3 bg-surface-50-950" onclick={(e) => e.stopPropagation()} role="dialog">
      <h3 class="font-semibold text-sm">
        {isNew ? 'Новый счёт' : `Редактирование: ${editCode}`}
      </h3>

      {#if isNew}
        <label class="label">
          <span class="label-text text-xs">Код</span>
          <input class="input input-sm" bind:value={editCode} placeholder="41, 60, 90.1…" />
        </label>
      {/if}

      <label class="label">
        <span class="label-text text-xs">Название</span>
        <input class="input input-sm" bind:value={editName} />
      </label>

      {#if isNew}
        <label class="label">
          <span class="label-text text-xs">Тип</span>
          <select class="select select-sm" bind:value={editType}>
            {#each typeOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </label>

        <label class="label">
          <span class="label-text text-xs">Родительский счёт</span>
          <input class="input input-sm" bind:value={editParent} placeholder="необязательно" />
        </label>
      {:else}
        <label class="label flex items-center gap-2">
          <input type="checkbox" class="checkbox checkbox-sm" bind:checked={editActive} />
          <span class="label-text text-xs">Активен</span>
        </label>
      {/if}

      <div class="flex justify-end gap-2 pt-2">
        <button class="btn btn-outline" onclick={() => showModal = false}>Отмена</button>
        <button class="btn btn-primary" onclick={handleSave}>
          <i class="fa-solid fa-check mr-1"></i>{isNew ? 'Создать' : 'Сохранить'}
        </button>
      </div>
    </div>
  </div>
{/if}
