// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api, type EntityType, type ObjectEntity, type EntityField,
    ENTITY_KIND_META, OBJECT_STATE_META,
  } from '$lib/services/api';
  import { auth, hasPermission } from '$lib/stores/auth';
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
  let filterOffset = $state(0);
  const PAGE = 50;

  async function loadTypes() {
    try { entityTypes = await api.listEntityTypes(); } catch (e: any) { error = e?.toString() || 'Ошибка'; }
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
    } catch (e: any) { error = e?.toString() || 'Ошибка'; }
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
    } catch (e: any) { error = e?.toString() || 'Ошибка создания'; }
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
    <!-- Left sidebar: types -->
    <div class="w-48 border-r border-surface-300-700 overflow-y-auto p-2 space-y-1 text-sm">
      <button class="w-full text-left p-1.5 rounded hover:bg-surface-200-800" class:bg-primary-100-800={!selectedType} onclick={() => selectedType = ''}>
        Все типы ({totalCount})
      </button>
      {#each entityTypes as et (et._id)}
        <button class="w-full text-left p-1.5 rounded hover:bg-surface-200-800 flex items-center gap-1.5" class:bg-primary-100-800={selectedType === et._id} onclick={() => selectedType = et._id}>
          <i class="{ENTITY_KIND_META[et.kind]?.icon ?? 'fa-solid fa-cube'} text-xs"></i>
          {et.name}
        </button>
      {/each}
    </div>

    <!-- Main -->
    <div class="flex-1 flex flex-col overflow-hidden">
      <!-- Top bar -->
      <div class="flex items-center gap-2 p-3 border-b border-surface-300-700">
        <h2 class="h3 text-sm"><i class="fa-solid fa-cube mr-1"></i>Объекты</h2>
        <select class="select select-sm max-w-[150px]" bind:value={filterState}>
          <option value="">Все состояния</option>
          {#each Object.entries(OBJECT_STATE_META) as [k, v]}
            <option value={k}>{v.label}</option>
          {/each}
        </select>
        {#if selectedType}
          <span class="badge preset-tonal text-xs">{entityTypes.find(t => t._id === selectedType)?.name ?? ''}</span>
        {/if}
        <span class="text-xs text-surface-500 ml-auto">{totalCount} объектов</span>

        <!-- Create -->
        {#if $auth && hasPermission($auth.permissions, 'documents', 'create')}
        <div class="flex items-center gap-1 ml-3">
          {#if creating}
            <select class="select select-sm max-w-[160px]" bind:value={createTypeId}>
              <option value="">Выберите тип…</option>
              {#each entityTypes as et}
                <option value={et._id}>{et.name}</option>
              {/each}
            </select>
            <button class="btn btn-sm preset-filled-primary text-xs" disabled={!createTypeId} onclick={startCreate}>OK</button>
            <button class="btn btn-sm preset-tonal text-xs" onclick={() => creating = false}>✕</button>
          {:else}
            <button class="btn btn-sm preset-filled-primary text-xs" onclick={() => creating = true}>
              <i class="fa-solid fa-plus mr-1"></i>Создать
            </button>
          {/if}
        </div>
        {/if}
      </div>

      {#if error}
        <div class="alert preset-tonal-error m-2 text-sm">{error}</div>
      {/if}

      <!-- Table -->
      <div class="flex-1 overflow-y-auto">
        {#if loading}
          <div class="text-center py-8 text-surface-500 text-sm"><i class="fa-solid fa-spinner fa-spin mr-1"></i>Загрузка...</div>
        {:else if objects.length === 0}
          <div class="text-center py-8 text-surface-500 text-sm">Нет объектов</div>
        {:else}
          <table class="table-hover table text-sm">
            <thead>
              <tr>
                <th>№</th>
                <th>Тип</th>
                <th>Состояние</th>
                <th>v</th>
                <th>Обновлён</th>
              </tr>
            </thead>
            <tbody>
              {#each objects as obj (obj._id)}
                <tr class="cursor-pointer hover:bg-surface-100-800" onclick={() => openEditor(obj)}>
                  <td><code class="text-xs">{obj.number ?? obj._id.slice(0, 8) + '…'}</code></td>
                  <td class="text-xs">{entityTypes.find(t => t._id === obj.entity_type_id)?.name ?? obj.entity_type_id}</td>
                  <td>
                    <span class="inline-flex items-center gap-1 text-xs">
                      <span class="w-2 h-2 rounded-full {OBJECT_STATE_META[obj.state]?.color ?? ''}"></span>
                      {OBJECT_STATE_META[obj.state]?.label ?? obj.state}
                    </span>
                  </td>
                  <td class="text-xs">{obj.version}</td>
                  <td class="text-xs text-surface-500">{formatTime(obj.updated_at)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
          {#if totalCount > PAGE}
            <div class="flex justify-center gap-2 p-2">
              <button class="btn btn-sm preset-tonal" disabled={filterOffset === 0} onclick={() => filterOffset = Math.max(0, filterOffset - PAGE)}>← Назад</button>
              <button class="btn btn-sm preset-tonal" disabled={!objects.length || filterOffset + PAGE >= totalCount} onclick={() => filterOffset += PAGE}>Далее →</button>
            </div>
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}
