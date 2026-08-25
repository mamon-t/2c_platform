<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api, type EntityType, type ObjectEntity,
    ENTITY_KIND_META, OBJECT_STATE_META,
  } from '$lib/services/api';
  import { auth, hasPermission } from '$lib/stores/auth';
  import { toastError, errText } from '$lib/components/ui/toast';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import ObjectEditor from './ObjectEditor.svelte';

  let entityTypes: EntityType[] = $state([]);
  let objects: ObjectEntity[] = $state([]);
  let totalCount = $state(0);
  let loading = $state(true);
  let error = $state('');
  let selectedType = $state<string>('');
  let selectedObj = $state<ObjectEntity | null>(null);
  let editing = $state(false);
  let creating = $state(false);
  let createTypeId = $state('');

  // Filters
  let filterState = $state('');
  let searchQuery = $state('');
  let filterOffset = $state(0);
  const PAGE = 50;

  const filtered = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return objects;
    return objects.filter((o) =>
      (o.number ?? '').toLowerCase().includes(q) || o._id.toLowerCase().includes(q)
    );
  });

  async function loadTypes() {
    try { entityTypes = await api.listEntityTypes(); }
    catch (e) { toastError(errText(e)); }
  }

  async function loadObjects() {
    loading = true;
    error = '';
    try {
      const page = await api.listObjects({
        entity_type_id: selectedType || undefined,
        state: filterState || undefined,
        limit: PAGE,
        offset: filterOffset,
      });
      objects = page.objects;
      totalCount = page.total_count;
    } catch (e) { error = errText(e); }
    finally { loading = false; }
  }

  function openEditor(obj: ObjectEntity) {
    selectedObj = obj;
    editing = true;
    creating = false;
  }

  async function startCreate() {
    if (!createTypeId) return;
    try {
      const obj = await api.createObject({ entity_type_id: createTypeId, data: {} });
      selectedObj = obj;
      editing = true;
      creating = false;
      await loadObjects();
    } catch (e) { toastError(errText(e, 'Ошибка создания')); }
  }

  function onEditorSaved(obj: ObjectEntity) {
    selectedObj = obj;
    loadObjects();
  }

  function onEditorClosed() {
    editing = false;
    selectedObj = null;
    loadObjects();
  }

  function formatTime(iso: string) {
    return new Date(iso).toLocaleString('ru-RU', { day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit' });
  }

  onMount(() => { loadTypes(); loadObjects(); });

  $effect(() => { selectedType; filterState; filterOffset = 0; loadObjects(); });
</script>

{#if editing && selectedObj}
  <ObjectEditor object={selectedObj} {entityTypes} onSaved={onEditorSaved} onClosed={onEditorClosed} />
{:else}
  <div class="flex h-full">
    <!-- Левая панель: типы -->
    <div class="w-52 shrink-0 space-y-0.5 overflow-y-auto border-r border-surface-300-700 p-2 text-sm">
      <button class="flex w-full items-center gap-1.5 rounded p-1.5 text-left hover:bg-surface-200-800" class:bg-primary-100-800={!selectedType} onclick={() => (selectedType = '')}>
        <i class="fa-solid fa-layer-group w-4 text-xs"></i>Все типы
      </button>
      {#each entityTypes as et (et._id)}
        <button class="flex w-full items-center gap-1.5 rounded p-1.5 text-left hover:bg-surface-200-800" class:bg-primary-100-800={selectedType === et._id} onclick={() => (selectedType = et._id)}>
          <i class="{ENTITY_KIND_META[et.kind]?.icon ?? 'fa-solid fa-cube'} w-4 text-xs"></i>{et.name}
        </button>
      {/each}
    </div>

    <!-- Основное -->
    <div class="flex flex-1 flex-col overflow-hidden">
      <!-- Toolbar -->
      <div class="flex flex-wrap items-center gap-2 border-b border-surface-300-700 px-3 py-2">
        <h2 class="text-sm font-semibold"><i class="fa-solid fa-cube mr-1 text-surface-400"></i>Объекты</h2>
        <input
          bind:value={searchQuery}
          class="input input-sm toolbar-input w-48"
          placeholder="Поиск по № / ID…"
          aria-label="Поиск объектов"
        />
        <select class="select select-sm max-w-[150px]" bind:value={filterState} aria-label="Фильтр по состоянию">
          <option value="">Все состояния</option>
          {#each Object.entries(OBJECT_STATE_META) as [k, v]}
            <option value={k}>{v.label}</option>
          {/each}
        </select>
        {#if selectedType}
          <span class="badge preset-tonal text-xs">{entityTypes.find((t) => t._id === selectedType)?.name ?? ''}</span>
        {/if}
        <span class="ml-auto text-xs text-surface-500">{filtered.length}{searchQuery ? ` из ${totalCount}` : ''}</span>

        {#if $auth && hasPermission($auth.permissions, 'documents', 'create')}
          <div class="ml-2 flex items-center gap-1">
            {#if creating}
              <select class="select select-sm max-w-[160px]" bind:value={createTypeId} aria-label="Тип нового объекта">
                <option value="">Выберите тип…</option>
                {#each entityTypes as et}<option value={et._id}>{et.name}</option>{/each}
              </select>
              <button class="btn btn-sm preset-filled-primary-500 text-xs" disabled={!createTypeId} onclick={startCreate}>OK</button>
              <button class="btn btn-sm preset-tonal text-xs" onclick={() => (creating = false)} aria-label="Отменить создание"><i class="fa-solid fa-xmark"></i></button>
            {:else}
              <button class="btn btn-sm preset-filled-primary-500 text-xs" onclick={() => (creating = true)}>
                <i class="fa-solid fa-plus mr-1"></i>Создать
              </button>
            {/if}
          </div>
        {/if}
      </div>

      {#if error}
        <div class="alert preset-tonal-error m-2 text-sm" role="alert">{error}</div>
      {/if}

      <!-- Таблица -->
      <div class="flex-1 overflow-y-auto">
        {#if loading}
          <Spinner />
        {:else if filtered.length === 0}
          <div class="py-10 text-center text-sm text-surface-400">
            {#if searchQuery}Ничего не найдено по «{searchQuery}»{:else}Нет объектов{/if}
          </div>
        {:else}
          <table class="table table-dense table-hover w-full text-left">
            <thead>
              <tr><th>№</th><th>Тип</th><th>Состояние</th><th>v</th><th>Обновлён</th></tr>
            </thead>
            <tbody>
              {#each filtered as obj (obj._id)}
                <tr class="cursor-pointer" tabindex="0" role="button"
                  onclick={() => openEditor(obj)}
                  onkeydown={(e) => { if (e.key === 'Enter') openEditor(obj); }}>
                  <td><code class="text-xs">{obj.number ?? obj._id.slice(0, 8) + '…'}</code></td>
                  <td class="text-xs">{entityTypes.find((t) => t._id === obj.entity_type_id)?.name ?? obj.entity_type_id.slice(0, 8)}</td>
                  <td>
                    <span class="inline-flex items-center gap-1 text-xs">
                      <span class="h-2 w-2 rounded-full {OBJECT_STATE_META[obj.state]?.color ?? ''}"></span>
                      {OBJECT_STATE_META[obj.state]?.label ?? obj.state}
                    </span>
                  </td>
                  <td class="text-xs">{obj.version}</td>
                  <td class="text-xs text-surface-500">{formatTime(obj.updated_at)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
          {#if totalCount > PAGE && !searchQuery}
            <div class="flex items-center justify-center gap-2 p-2 text-xs text-surface-500">
              <button class="btn btn-sm preset-tonal" disabled={filterOffset === 0} onclick={() => (filterOffset = Math.max(0, filterOffset - PAGE))}>
                <i class="fa-solid fa-chevron-left"></i> Назад
              </button>
              <span>{Math.floor(filterOffset / PAGE) + 1} / {Math.ceil(totalCount / PAGE)}</span>
              <button class="btn btn-sm preset-tonal" disabled={!objects.length || filterOffset + PAGE >= totalCount} onclick={() => (filterOffset += PAGE)}>
                Далее <i class="fa-solid fa-chevron-right"></i>
              </button>
            </div>
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}
