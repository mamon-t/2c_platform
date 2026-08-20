<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api, type EntityType, type ObjectEntity, type ObjectSnapshot, type ObjectStateTS,
    ENTITY_KIND_META, OBJECT_STATE_META,
  } from '$lib/services/api';

  let entityTypes: EntityType[] = $state([]);
  let objects: ObjectEntity[] = $state([]);
  let totalCount = $state(0);
  let loading = $state(true);
  let error = $state('');
  let selectedType = $state<string>('');
  let selectedObj = $state<ObjectEntity | null>(null);
  let versions: ObjectSnapshot[] = $state([]);
  let showVersions = $state(false);
  let tab = $state<'data' | 'versions'>('data');
  let dataJson = $state('');

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

  async function selectObject(obj: ObjectEntity) {
    selectedObj = obj;
    dataJson = JSON.stringify(obj.data, null, 2);
    tab = 'data';
    showVersions = false;
  }

  async function loadVersions() {
    if (!selectedObj) return;
    try {
      versions = await api.listObjectVersions(selectedObj._id);
      showVersions = true;
    } catch (e: any) { error = e?.toString() || 'Ошибка'; }
  }

  async function handlePost() {
    if (!selectedObj) return;
    try {
      selectedObj = await api.postObject(selectedObj._id, selectedObj.version);
      dataJson = JSON.stringify(selectedObj.data, null, 2);
      await loadObjects();
    } catch (e: any) { error = e?.toString() || 'Ошибка'; }
  }

  async function handleCancel() {
    if (!selectedObj) return;
    try {
      selectedObj = await api.cancelObject(selectedObj._id, selectedObj.version);
      dataJson = JSON.stringify(selectedObj.data, null, 2);
      await loadObjects();
    } catch (e: any) { error = e?.toString() || 'Ошибка'; }
  }

  async function handleRestore(targetVersion: number) {
    if (!selectedObj) return;
    try {
      selectedObj = await api.restoreObjectVersion(selectedObj._id, targetVersion);
      dataJson = JSON.stringify(selectedObj.data, null, 2);
      showVersions = false;
      await loadObjects();
    } catch (e: any) { error = e?.toString() || 'Ошибка'; }
  }

  function formatTime(iso: string) {
    return new Date(iso).toLocaleString('ru-RU', { day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit' });
  }

  onMount(() => { loadTypes(); loadObjects(); });

  $effect(() => { selectedType; filterState; filterOffset = 0; loadObjects(); });
</script>

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

  <!-- Main: list + details -->
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
    </div>

    {#if error}
      <div class="alert preset-tonal-error m-2 text-sm">{error}</div>
    {/if}

    <div class="flex-1 flex overflow-hidden">
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
                <tr class="cursor-pointer" class:bg-primary-50-950={selectedObj?._id === obj._id} onclick={() => selectObject(obj)}>
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

      <!-- Right panel: detail -->
      {#if selectedObj}
        <div class="w-[400px] border-l border-surface-300-700 overflow-y-auto p-3 space-y-3">
          <div class="flex items-center gap-2">
            <h3 class="font-semibold text-sm truncate">{selectedObj.number ?? 'Черновик'}</h3>
            <span class="badge preset-tonal text-xs">{OBJECT_STATE_META[selectedObj.state]?.label ?? selectedObj.state}</span>
            <button class="btn btn-sm preset-tonal-error ml-auto" onclick={() => { selectedObj = null; showVersions = false; }}>
              <i class="fa-solid fa-xmark text-xs"></i>
            </button>
          </div>

          <div class="text-xs text-surface-500 space-y-0.5">
            <div>ID: <code>{selectedObj._id}</code></div>
            <div>Версия: <strong>v{selectedObj.version}</strong></div>
            <div>Создан: {formatTime(selectedObj.created_at)}</div>
            <div>Обновлён: {formatTime(selectedObj.updated_at)}</div>
          </div>

          <!-- Actions -->
          <div class="flex flex-wrap gap-1">
            {#if selectedObj.state === 'draft'}
              <button class="btn btn-sm preset-filled-success" onclick={handlePost}>
                <i class="fa-solid fa-check-double mr-1"></i>Провести
              </button>
            {/if}
            {#if selectedObj.state === 'posted'}
              <button class="btn btn-sm preset-filled-error" onclick={handleCancel}>
                <i class="fa-solid fa-xmark mr-1"></i>Отменить
              </button>
            {/if}
            <button class="btn btn-sm preset-tonal" onclick={loadVersions}>
              <i class="fa-solid fa-clock-rotate-left mr-1"></i>Версии
            </button>
          </div>

          <!-- Tabs -->
          <div class="flex gap-1 border-b border-surface-300-700 pb-1">
            <button class="btn btn-sm text-xs" class:preset-tonal={tab === 'data'} onclick={() => tab = 'data'}>Данные</button>
            <button class="btn btn-sm text-xs" class:preset-tonal={tab === 'versions'} onclick={() => { tab = 'versions'; loadVersions(); }}>История</button>
          </div>

          {#if tab === 'data'}
            <pre class="text-xs bg-surface-100-800 p-2 rounded overflow-x-auto max-h-[500px]">{dataJson}</pre>
          {:else if tab === 'versions'}
            {#if versions.length === 0}
              <div class="text-xs text-surface-500 py-2">Нет сохранённых версий</div>
            {:else}
              <div class="space-y-1">
                {#each versions as v (v._id)}
                  <div class="card p-2 text-xs space-y-0.5">
                    <div class="flex items-center justify-between">
                      <span class="font-medium">v{v.version}</span>
                      <span class="text-surface-500">{formatTime(v.created_at)}</span>
                    </div>
                    {#if v.reason}
                      <div class="text-surface-500">{v.reason}</div>
                    {/if}
                    {#if v.version < selectedObj!.version}
                      <button class="btn btn-xs preset-tonal" onclick={() => handleRestore(v.version)}>
                        <i class="fa-solid fa-rotate-left mr-1"></i>Восстановить
                      </button>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          {/if}
        </div>
      {/if}
    </div>
  </div>
</div>
