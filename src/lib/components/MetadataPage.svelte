// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api,
    type EntityType, type EntityField, type EntityState, type EntityTransition,
    type EntityKind, type FieldKind,
    ENTITY_KIND_META, FIELD_KIND_META,
  } from '$lib/services/api';

  let entityTypes: EntityType[] = $state([]);
  let loading = $state(true);
  let error = $state('');
  let selectedId = $state<string | null>(null);
  let fields: EntityField[] = $state([]);
  let states: EntityState[] = $state([]);
  let transitions: EntityTransition[] = $state([]);
  let tab = $state<'fields' | 'states' | 'transitions'>('fields');

  // Create form
  let showCreate = $state(false);
  let newCode = $state('');
  let newName = $state('');
  let newKind = $state<EntityKind>('document');

  // Field form
  let showFieldForm = $state(false);
  let fieldCode = $state('');
  let fieldName = $state('');
  let fieldKind = $state<FieldKind>('string');

  // State form
  let showStateForm = $state(false);
  let stateCode = $state('');
  let stateName = $state('');

  async function loadTypes() {
    loading = true;
    try { entityTypes = await api.listEntityTypes(); }
    catch (e: any) { error = e?.toString() || 'Ошибка'; }
    finally { loading = false; }
  }

  async function selectType(id: string) {
    selectedId = id;
    tab = 'fields';
    await loadDetails(id);
  }

  async function loadDetails(id: string) {
    try {
      fields = await api.listEntityFields(id);
      states = await api.listEntityStates(id);
      transitions = await api.listEntityTransitions(id);
    } catch (e: any) { error = e?.toString() || 'Ошибка'; }
  }

  async function createType() {
    if (!newCode || !newName) return;
    try {
      await api.createEntityType({ code: newCode, name: newName, kind: newKind });
      showCreate = false; newCode = ''; newName = '';
      await loadTypes();
    } catch (e: any) { error = e?.toString() || 'Ошибка создания'; }
  }

  async function deleteType(id: string) {
    if (!confirm('Удалить тип сущности и все вложенные данные?')) return;
    try {
      await api.deleteEntityType(id);
      if (selectedId === id) { selectedId = null; fields = []; states = []; transitions = []; }
      await loadTypes();
    } catch (e: any) { error = e?.toString() || 'Ошибка удаления'; }
  }

  async function createField() {
    if (!selectedId || !fieldCode || !fieldName) return;
    try {
      await api.createEntityField({ entity_type_id: selectedId, code: fieldCode, name: fieldName, field_kind: fieldKind });
      showFieldForm = false; fieldCode = ''; fieldName = '';
      await loadDetails(selectedId);
    } catch (e: any) { error = e?.toString() || 'Ошибка'; }
  }

  async function deleteField(id: string) {
    try { await api.deleteEntityField(id); if (selectedId) await loadDetails(selectedId); }
    catch (e: any) { error = e?.toString() || 'Ошибка'; }
  }

  async function createState() {
    if (!selectedId || !stateCode || !stateName) return;
    try {
      const existing = states.length;
      await api.createEntityState({ entity_type_id: selectedId, code: stateCode, name: stateName, is_initial: existing === 0 });
      showStateForm = false; stateCode = ''; stateName = '';
      await loadDetails(selectedId);
    } catch (e: any) { error = e?.toString() || 'Ошибка'; }
  }

  async function deleteState(id: string) {
    try { await api.deleteEntityState(id); if (selectedId) await loadDetails(selectedId); }
    catch (e: any) { error = e?.toString() || 'Ошибка'; }
  }

  async function deleteTransition(id: string) {
    try { await api.deleteEntityTransition(id); if (selectedId) await loadDetails(selectedId); }
    catch (e: any) { error = e?.toString() || 'Ошибка'; }
  }

  onMount(() => loadTypes());
</script>

<div class="flex h-full">
  <!-- Left: list -->
  <div class="w-64 border-r border-surface-300-700 overflow-y-auto p-3 space-y-2">
    <div class="flex items-center justify-between mb-2">
      <h3 class="h3 text-sm">Типы сущностей</h3>
      <button class="btn btn-sm preset-filled-primary" onclick={() => showCreate = true}>
        <i class="fa-solid fa-plus text-xs"></i>
      </button>
    </div>

    {#if showCreate}
      <div class="card p-3 space-y-2 text-sm">
        <select class="select" bind:value={newKind}>
          {#each Object.entries(ENTITY_KIND_META) as [k, v]}
            <option value={k}>{v.label}</option>
          {/each}
        </select>
        <input class="input" placeholder="Код (INVOICE)" bind:value={newCode} />
        <input class="input" placeholder="Название" bind:value={newName} />
        <div class="flex gap-1">
          <button class="btn btn-sm preset-filled-primary" onclick={createType}>OK</button>
          <button class="btn btn-sm preset-tonal" onclick={() => showCreate = false}>✕</button>
        </div>
      </div>
    {/if}

    {#if loading}
      <div class="text-sm text-surface-500 py-4"><i class="fa-solid fa-spinner fa-spin mr-1"></i>Загрузка...</div>
    {:else if entityTypes.length === 0}
      <div class="text-sm text-surface-500 py-4">Нет типов сущностей</div>
    {:else}
      {#each entityTypes as et (et._id)}
        <button
          class="w-full text-left p-2 rounded-lg text-sm hover:bg-surface-200-800 transition-colors"
          class:bg-primary-100-800={selectedId === et._id}
          onclick={() => selectType(et._id)}
        >
          <div class="flex items-center gap-2">
            <i class="{ENTITY_KIND_META[et.kind]?.icon ?? 'fa-solid fa-cube'} text-xs"></i>
            <span class="font-medium">{et.name}</span>
          </div>
          <div class="text-xs text-surface-500 ml-5">{et.code} · {et.kind}</div>
        </button>
      {/each}
    {/if}
  </div>

  <!-- Right: details -->
  <div class="flex-1 overflow-y-auto p-4">
    {#if error}
      <div class="alert preset-tonal-error mb-4">{error}</div>
    {/if}

    {#if !selectedId}
      <div class="text-center py-12 text-surface-500">Выберите тип сущности слева</div>
    {:else}
      {@const et = entityTypes.find(t => t._id === selectedId)}
      {#if et}
        <div class="flex items-center gap-3 mb-4">
          <i class="{ENTITY_KIND_META[et.kind]?.icon ?? 'fa-solid fa-cube'} text-xl"></i>
          <h2 class="h2">{et.name}</h2>
          <span class="badge preset-tonal">{et.code}</span>
          <span class="badge preset-tonal">{et.kind}</span>
          <button class="btn btn-sm preset-tonal-error ml-auto" onclick={() => deleteType(et._id)}>
            <i class="fa-solid fa-trash"></i>
          </button>
        </div>

        <!-- Tabs -->
        <div class="flex gap-1 mb-4 border-b border-surface-300-700 pb-1">
          <button class="btn btn-sm" class:preset-tonal={tab === 'fields'} onclick={() => tab = 'fields'}>
            Поля ({fields.length})
          </button>
          <button class="btn btn-sm" class:preset-tonal={tab === 'states'} onclick={() => tab = 'states'}>
            Состояния ({states.length})
          </button>
          <button class="btn btn-sm" class:preset-tonal={tab === 'transitions'} onclick={() => tab = 'transitions'}>
            Переходы ({transitions.length})
          </button>
        </div>

        {#if tab === 'fields'}
          <div class="mb-3">
            <button class="btn btn-sm preset-filled-primary" onclick={() => showFieldForm = !showFieldForm}>
              <i class="fa-solid fa-plus mr-1"></i>Добавить поле
            </button>
          </div>
          {#if showFieldForm}
            <div class="card p-3 mb-3 space-y-2 text-sm">
              <input class="input" placeholder="Код (title)" bind:value={fieldCode} />
              <input class="input" placeholder="Название" bind:value={fieldName} />
              <select class="select" bind:value={fieldKind}>
                {#each Object.entries(FIELD_KIND_META) as [k, v]}
                  <option value={k}>{v.label}</option>
                {/each}
              </select>
              <div class="flex gap-1">
                <button class="btn btn-sm preset-filled-primary" onclick={createField}>OK</button>
                <button class="btn btn-sm preset-tonal" onclick={() => showFieldForm = false}>✕</button>
              </div>
            </div>
          {/if}
          {#if fields.length === 0}
            <div class="text-sm text-surface-500 py-4">Нет полей</div>
          {:else}
            <div class="overflow-x-auto">
              <table class="table-hover table text-sm">
                <thead><tr><th>#</th><th>Код</th><th>Название</th><th>Тип</th><th></th></tr></thead>
                <tbody>
                  {#each fields as f (f._id)}
                    <tr>
                      <td class="text-surface-500">{f.order}</td>
                      <td><code>{f.code}</code></td>
                      <td>{f.name}</td>
                      <td><span class="badge preset-tonal text-xs">{FIELD_KIND_META[f.field_kind]?.label ?? f.field_kind}</span></td>
                      <td>
                        <button class="btn btn-sm preset-tonal-error" onclick={() => deleteField(f._id)}>
                          <i class="fa-solid fa-trash text-xs"></i>
                        </button>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}

        {:else if tab === 'states'}
          <div class="mb-3">
            <button class="btn btn-sm preset-filled-primary" onclick={() => showStateForm = !showStateForm}>
              <i class="fa-solid fa-plus mr-1"></i>Добавить состояние
            </button>
          </div>
          {#if showStateForm}
            <div class="card p-3 mb-3 space-y-2 text-sm">
              <input class="input" placeholder="Код (draft)" bind:value={stateCode} />
              <input class="input" placeholder="Название" bind:value={stateName} />
              <div class="flex gap-1">
                <button class="btn btn-sm preset-filled-primary" onclick={createState}>OK</button>
                <button class="btn btn-sm preset-tonal" onclick={() => showStateForm = false}>✕</button>
              </div>
            </div>
          {/if}
          {#if states.length === 0}
            <div class="text-sm text-surface-500 py-4">Нет состояний</div>
          {:else}
            <div class="flex flex-wrap gap-2">
              {#each states as s (s._id)}
                <div class="card p-3 flex items-center gap-2 text-sm">
                  <span class="w-3 h-3 rounded-full" style="background-color: {s.color ?? '#94a3b8'}"></span>
                  <span class="font-medium">{s.name}</span>
                  <code class="text-xs text-surface-500">{s.code}</code>
                  {#if s.is_initial}<span class="badge preset-tonal-success text-xs">начальное</span>{/if}
                  {#if s.is_final}<span class="badge preset-tonal-warning text-xs">конечное</span>{/if}
                  <button class="btn btn-sm preset-tonal-error" onclick={() => deleteState(s._id)}>
                    <i class="fa-solid fa-trash text-xs"></i>
                  </button>
                </div>
              {/each}
            </div>
          {/if}

        {:else if tab === 'transitions'}
          {#if transitions.length === 0}
            <div class="text-sm text-surface-500 py-4">Нет переходов</div>
          {:else}
            <div class="overflow-x-auto">
              <table class="table-hover table text-sm">
                <thead><tr><th>Код</th><th>Название</th><th>Из</th><th>→</th><th></th></tr></thead>
                <tbody>
                  {#each transitions as t (t._id)}
                    <tr>
                      <td><code>{t.code}</code></td>
                      <td>{t.name}</td>
                      <td><span class="badge preset-tonal">{t.from_state}</span></td>
                      <td><i class="fa-solid fa-arrow-right text-xs text-surface-500"></i></td>
                      <td><span class="badge preset-tonal">{t.to_state}</span></td>
                      <td>
                        <button class="btn btn-sm preset-tonal-error" onclick={() => deleteTransition(t._id)}>
                          <i class="fa-solid fa-trash text-xs"></i>
                        </button>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        {/if}
      {/if}
    {/if}
  </div>
</div>
