<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api,
    type EntityType, type EntityField, type EntityState, type EntityTransition,
    type ObjectEntity, type ObjectSnapshot,
    FIELD_KIND_META, OBJECT_STATE_META,
    type FieldKind, type ObjectStateTS,
  } from '$lib/services/api';
  import { auth, hasPermission } from '$lib/stores/auth';

  interface Props {
    object: ObjectEntity;
    entityTypes: EntityType[];
    onSaved?: (obj: ObjectEntity) => void;
    onClosed?: () => void;
  }

  let { object, entityTypes, onSaved, onClosed }: Props = $props();

  let fields: EntityField[] = $state([]);
  let states: EntityState[] = $state([]);
  let transitions: EntityTransition[] = $state([]);
  let data: Record<string, unknown> = $state({});
  let loading = $state(true);
  let saving = $state(false);
  let error = $state('');
  let versions: ObjectSnapshot[] = $state([]);
  let showVersions = $state(false);
  let tab = $state<'form' | 'json' | 'versions'>('form');
  let jsonText = $state('');

  const entityType = $derived(entityTypes.find(t => t._id === object.entity_type_id));

  // Группировка полей
  const groupedFields = $derived(() => {
    const groups: Record<string, EntityField[]> = {};
    for (const f of fields.sort((a, b) => a.order - b.order)) {
      const g = f.group_name ?? '';
      if (!groups[g]) groups[g] = [];
      groups[g].push(f);
    }
    return groups;
  });

  // Доступные переходы из текущего состояния (с учётом RBAC)
  const availableTransitions = $derived(
    transitions.filter(t => {
      if (t.to_state === object.state) return false;
      if (!$auth) return false;
      if (t.to_state === 'posted') return hasPermission($auth.permissions, 'documents', 'approve');
      if (t.to_state === 'cancelled') return hasPermission($auth.permissions, 'documents', 'cancel');
      return true;
    }).filter(t => t.from_state === object.state)
  );

  async function loadMetadata() {
    if (!object.entity_type_id) return;
    try {
      [fields, states, transitions] = await Promise.all([
        api.listEntityFields(object.entity_type_id),
        api.listEntityStates(object.entity_type_id),
        api.listEntityTransitions(object.entity_type_id),
      ]);
    } catch (e: any) {
      error = e?.toString() || 'Ошибка загрузки метаданных';
    }
  }

  function initData() {
    data = { ...object.data };
    jsonText = JSON.stringify(data, null, 2);
  }

  async function handleSave() {
    saving = true;
    error = '';
    try {
      const saved = await api.updateObject(object._id, {
        data: tab === 'json' ? JSON.parse(jsonText) : data,
        version: object.version,
        reason: 'Обновление через форму',
      });
      object = saved;
      initData();
      onSaved?.(saved);
    } catch (e: any) {
      error = e?.toString() || 'Ошибка сохранения';
    } finally {
      saving = false;
    }
  }

  async function handleTransition(t: EntityTransition) {
    saving = true;
    error = '';
    try {
      let saved: ObjectEntity;
      // Для post и cancel используем специальные команды
      if (t.to_state === 'posted') {
        saved = await api.postObject(object._id, object.version);
      } else if (t.to_state === 'cancelled') {
        saved = await api.cancelObject(object._id, object.version);
      } else {
        // Generic: update + state change через данные
        saved = await api.updateObject(object._id, {
          data: { ...data, _state_transition: t.code },
          version: object.version,
          reason: `Переход: ${t.name}`,
        });
      }
      object = saved;
      initData();
      onSaved?.(saved);
    } catch (e: any) {
      error = e?.toString() || 'Ошибка перехода';
    } finally {
      saving = false;
    }
  }

  async function loadVersions() {
    try {
      versions = await api.listObjectVersions(object._id);
      showVersions = true;
    } catch (e: any) { error = e?.toString() || 'Ошибка'; }
  }

  async function handleRestore(targetVersion: number) {
    saving = true;
    try {
      const saved = await api.restoreObjectVersion(object._id, targetVersion);
      object = saved;
      initData();
      showVersions = false;
      onSaved?.(saved);
    } catch (e: any) { error = e?.toString() || 'Ошибка'; }
    finally { saving = false; }
  }

  function formatTime(iso: string) {
    return new Date(iso).toLocaleString('ru-RU', { day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit' });
  }

  function setField(code: string, value: unknown) {
    data = { ...data, [code]: value };
  }

  onMount(async () => {
    loading = true;
    initData();
    await loadMetadata();
    loading = false;
  });

  $effect(() => { object; initData(); });
</script>

<div class="flex flex-col h-full">
  <!-- Header -->
  <div class="flex items-center gap-2 p-3 border-b border-surface-300-700 shrink-0">
    <i class="{FIELD_KIND_META['string']?.icon ?? 'fa-solid fa-cube'} text-lg"></i>
    <h3 class="font-semibold text-sm">
      {object.number ?? 'Черновик'} — {entityType?.name ?? object.entity_type_id}
    </h3>
    <span class="badge preset-tonal text-xs">{OBJECT_STATE_META[object.state]?.label ?? object.state}</span>
    <span class="text-xs text-surface-500">v{object.version}</span>
    <button class="btn btn-sm preset-tonal-error ml-auto" onclick={() => onClosed?.()}>
      <i class="fa-solid fa-xmark"></i>
    </button>
  </div>

  {#if error}
    <div class="alert preset-tonal-error mx-3 mt-2 text-sm">{error}</div>
  {/if}

  <!-- Transition buttons -->
  {#if availableTransitions.length > 0}
    <div class="flex items-center gap-2 px-3 py-2 border-b border-surface-300-700 shrink-0">
      <span class="text-xs text-surface-500">Действия:</span>
      {#each availableTransitions as t}
        <button
          class="btn btn-sm text-xs"
          class:preset-filled-success={t.to_state === 'posted'}
          class:preset-filled-error={t.to_state === 'cancelled'}
          class:preset-tonal={t.to_state !== 'posted' && t.to_state !== 'cancelled'}
          disabled={saving}
          onclick={() => handleTransition(t)}
        >
          {t.name} → {states.find(s => s.code === t.to_state)?.name ?? t.to_state}
        </button>
      {/each}
    </div>
  {/if}

  <!-- Tabs -->
  <div class="flex items-center gap-1 px-3 py-1.5 border-b border-surface-300-700 shrink-0">
    <button class="btn btn-sm text-xs" class:preset-tonal={tab === 'form'} onclick={() => tab = 'form'}>Форма</button>
    <button class="btn btn-sm text-xs" class:preset-tonal={tab === 'json'} onclick={() => { tab = 'json'; jsonText = JSON.stringify(data, null, 2); }}>JSON</button>
    <button class="btn btn-sm text-xs" class:preset-tonal={tab === 'versions'} onclick={() => { tab = 'versions'; loadVersions(); }}>История ({versions.length})</button>
    <div class="flex-1"></div>
    {#if object.state === 'draft' && $auth && hasPermission($auth.permissions, 'documents', 'update')}
      <button class="btn btn-sm preset-filled-primary text-xs" disabled={saving} onclick={handleSave}>
        {saving ? '...' : 'Сохранить'}
      </button>
    {/if}
  </div>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto p-3">
    {#if loading}
      <div class="text-center py-8 text-surface-500 text-sm"><i class="fa-solid fa-spinner fa-spin mr-1"></i>Загрузка...</div>

    {:else if tab === 'form'}
      {#if fields.length === 0}
        <div class="text-center py-8 text-surface-500 text-sm">Нет полей. Добавьте поля в метаданных.</div>
      {:else}
        <div class="space-y-4 max-w-2xl">
          {#each Object.entries(groupedFields()) as [groupName, groupFields]}
            {#if groupName}
              <h4 class="text-xs font-semibold text-surface-500 uppercase tracking-wider mt-4">{groupName}</h4>
            {/if}
            <div class="space-y-3">
              {#each groupFields as field (field._id)}
                <label class="label">
                  <span class="label-text text-sm">
                    {field.name}
                    {#if field.is_required}<span class="text-error-500">*</span>{/if}
                  </span>

                  {#if field.field_kind === 'string'}
                    <input
                      class="input"
                      type="text"
                      placeholder={field.code}
                      disabled={field.is_readonly || object.state !== 'draft'}
                      value={String(data[field.code] ?? '')}
                      oninput={(e) => setField(field.code, (e.target as HTMLInputElement).value)}
                    />

                  {:else if field.field_kind === 'text'}
                    <textarea
                      class="textarea"
                      placeholder={field.code}
                      disabled={field.is_readonly || object.state !== 'draft'}
                      value={String(data[field.code] ?? '')}
                      oninput={(e) => setField(field.code, (e.target as HTMLTextAreaElement).value)}
                    ></textarea>

                  {:else if field.field_kind === 'integer'}
                    <input
                      class="input"
                      type="number"
                      step="1"
                      disabled={field.is_readonly || object.state !== 'draft'}
                      value={data[field.code] ?? ''}
                      oninput={(e) => setField(field.code, parseInt((e.target as HTMLInputElement).value) || 0)}
                    />

                  {:else if field.field_kind === 'money'}
                    <input
                      class="input"
                      type="number"
                      step="0.01"
                      disabled={field.is_readonly || object.state !== 'draft'}
                      value={data[field.code] ?? ''}
                      oninput={(e) => setField(field.code, parseFloat((e.target as HTMLInputElement).value) || 0)}
                    />

                  {:else if field.field_kind === 'date'}
                    <input
                      class="input"
                      type="date"
                      disabled={field.is_readonly || object.state !== 'draft'}
                      value={String(data[field.code] ?? '')}
                      oninput={(e) => setField(field.code, (e.target as HTMLInputElement).value)}
                    />

                  {:else if field.field_kind === 'datetime'}
                    <input
                      class="input"
                      type="datetime-local"
                      disabled={field.is_readonly || object.state !== 'draft'}
                      value={String(data[field.code] ?? '')}
                      oninput={(e) => setField(field.code, (e.target as HTMLInputElement).value)}
                    />

                  {:else if field.field_kind === 'boolean'}
                    <div class="flex items-center gap-2">
                      <input
                        class="checkbox"
                        type="checkbox"
                        disabled={field.is_readonly || object.state !== 'draft'}
                        checked={Boolean(data[field.code])}
                        onchange={(e) => setField(field.code, (e.target as HTMLInputElement).checked)}
                      />
                      <span class="text-xs text-surface-500">{Boolean(data[field.code]) ? 'Да' : 'Нет'}</span>
                    </div>

                  {:else if field.field_kind === 'enum'}
                    <select
                      class="select"
                      disabled={field.is_readonly || object.state !== 'draft'}
                      value={String(data[field.code] ?? '')}
                      onchange={(e) => setField(field.code, (e.target as HTMLSelectElement).value)}
                    >
                      <option value="">— не задано —</option>
                      {#each (field.enum_values ?? []) as opt}
                        <option value={opt}>{opt}</option>
                      {/each}
                    </select>

                  {:else if field.field_kind === 'reference'}
                    <input
                      class="input"
                      type="text"
                      placeholder="ID ссылки: {field.reference_entity ?? ''}"
                      disabled={field.is_readonly || object.state !== 'draft'}
                      value={String(data[field.code] ?? '')}
                      oninput={(e) => setField(field.code, (e.target as HTMLInputElement).value)}
                    />

                  {:else}
                    <input
                      class="input"
                      type="text"
                      placeholder="{FIELD_KIND_META[field.field_kind]?.label ?? field.field_kind}"
                      disabled={field.is_readonly || object.state !== 'draft'}
                      value={String(data[field.code] ?? '')}
                      oninput={(e) => setField(field.code, (e.target as HTMLInputElement).value)}
                    />
                  {/if}
                </label>
              {/each}
            </div>
          {/each}
        </div>
      {/if}

    {:else if tab === 'json'}
      <textarea
        class="textarea font-mono text-xs w-full h-full min-h-[300px]"
        disabled={object.state !== 'draft'}
        bind:value={jsonText}
      ></textarea>

    {:else if tab === 'versions'}
      {#if versions.length === 0}
        <div class="text-xs text-surface-500 py-4">Нет сохранённых версий</div>
      {:else}
        <div class="space-y-1 max-w-lg">
          {#each versions as v (v._id)}
            <div class="card p-2 text-xs space-y-0.5">
              <div class="flex items-center justify-between">
                <span class="font-medium">v{v.version} — {OBJECT_STATE_META[v.state as ObjectStateTS]?.label ?? v.state}</span>
                <span class="text-surface-500">{formatTime(v.created_at)}</span>
              </div>
              {#if v.reason}
                <div class="text-surface-500">{v.reason}</div>
              {/if}
              {#if v.version < object.version && $auth && hasPermission($auth.permissions, 'documents', 'update')}
                <button class="btn btn-xs preset-tonal text-xs" disabled={saving} onclick={() => handleRestore(v.version)}>
                  <i class="fa-solid fa-rotate-left mr-1"></i>Восстановить v{v.version}
                </button>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>
