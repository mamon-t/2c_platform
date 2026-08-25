<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api,
    type EntityType, type EntityField, type EntityState, type EntityTransition,
    type ObjectEntity, type ObjectSnapshot, type User, type Company,
    FIELD_KIND_META, OBJECT_STATE_META,
    type FieldKind, type ObjectStateTS,
  } from '$lib/services/api';
  import { auth, hasPermission } from '$lib/stores/auth';
  import { lastWeight, initDeviceEvents } from '$lib/stores/devices';
  import { parseFile, serializeFile, downloadText, readFileAsText,
    type FileFormat, type ParseResult } from '$lib/utils/fileConverter';
  import FieldMappingDialog from '$lib/components/FieldMappingDialog.svelte';
  import { confirmDialog } from '$lib/components/ui/dialog';

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
  let validationErrors: string[] = $state([]);
  let versions: ObjectSnapshot[] = $state([]);
  let showVersions = $state(false);
  let tab = $state<'form' | 'json' | 'versions'>('form');
  let jsonText = $state('');

  let referenceOptions: Record<string, ObjectEntity[]> = $state({});
  let allUsers = $state<User[]>([]);
  let allCompanies = $state<Company[]>([]);
  let loadingReferences: Record<string, boolean> = $state({});

  const entityType = $derived(entityTypes.find(t => t._id === object.entity_type_id));

  const groupedFields = $derived.by(() => {
    const groups: Record<string, EntityField[]> = {};
    for (const f of fields.sort((a, b) => a.order - b.order)) {
      const g = f.group_name ?? '';
      if (!groups[g]) groups[g] = [];
      groups[g].push(f);
    }
    return groups;
  });

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
      await loadReferences();
      api.listUsers().then((u) => (allUsers = u)).catch(() => {});
      api.listCompanies().then((c) => (allCompanies = c)).catch(() => {});
    } catch (e: any) {
      error = e?.toString() || 'Ошибка загрузки метаданных';
    }
  }

  async function loadReferences() {
    for (const field of fields) {
      if (field.field_kind === 'reference' && field.reference_entity) {
        loadingReferences[field.code] = true;
        try {
          const et = entityTypes.find(t => t.code === field.reference_entity || t._id === field.reference_entity);
          if (et) {
            const page = await api.listObjects({ entity_type_id: et._id, limit: 200 });
            referenceOptions[field.code] = page.objects;
          }
        } catch { /* ignore */ }
        loadingReferences[field.code] = false;
      }
    }
  }

  function initData() {
    data = { ...object.data };
    jsonText = JSON.stringify(data, null, 2);
  }

  let dirty = $state(false);
  $effect(() => { JSON.stringify(data); if (!loading) dirty = JSON.stringify(data) !== JSON.stringify(object.data); });

  async function requestClose() {
    if (dirty && !(await confirmDialog({ title: 'Закрыть без сохранения?', message: 'Имеются несохранённые изменения.', danger: true, confirmLabel: 'Закрыть' }))) return;
    onClosed?.();
  }

  function validateRequired(): string[] {
    const errors: string[] = [];
    for (const field of fields) {
      if (!field.is_required) continue;
      const val = data[field.code];
      if (val === undefined || val === null || val === '') {
        errors.push(`«${field.name}» — обязательное поле`);
      }
    }
    return errors;
  }

  async function handleSave() {
    const reqErrors = validateRequired();
    if (reqErrors.length > 0) {
      validationErrors = reqErrors;
      return;
    }
    validationErrors = [];
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
    const isDangerous = t.to_state === 'posted' || t.to_state === 'cancelled';
    if (isDangerous) {
      const label = t.to_state === 'posted' ? 'проведении' : 'отмене';
      if (!(await confirmDialog({ title: `Подтвердите ${label} документа`, danger: true, confirmLabel: 'Провести' }))) return;
    }

    saving = true;
    error = '';
    validationErrors = [];
    try {
      let saved: ObjectEntity;
      if (t.to_state === 'posted') {
        saved = await api.postObject(object._id, object.version);
      } else if (t.to_state === 'cancelled') {
        saved = await api.cancelObject(object._id, object.version);
      } else {
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
    if (!(await confirmDialog({ title: `Восстановить версию v${targetVersion}?`, danger: true }))) return;
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

  /** Поле веса: код содержит weight / вес / mass. */
  function isWeightField(code: string): boolean {
    return /weight|вес|mass/i.test(code);
  }

  /** Подставить последнее показание весов (г → кг для kg-полей). */
  function takeWeight(code: string, asKg: boolean) {
    const w = $lastWeight;
    if (!w) { return; }
    if (asKg || /kg|килограмм/i.test(code)) {
      setField(code, Math.round(w.grams) / 1000);
    } else {
      setField(code, Math.round(w.grams));
    }
  }

  function setField(code: string, value: unknown) {
    data = { ...data, [code]: value };
  }

  const isEditable = $derived(object.state === 'draft' || object.state === 'active');

  // Импорт/экспорт табличных данных
  let importTargetField = $state<string | null>(null);
  let importParsed = $state<ParseResult | null>(null);
  let showMapping = $state(false);

  function openImport(fieldCode: string, file: File) {
    const fmt: FileFormat = file.name.endsWith('.json') ? 'json' :
      file.name.endsWith('.xml') ? 'xml' : 'csv';
    readFileAsText(file).then((text) => {
      try {
        const parsed = parseFile(text, fmt);
        if (!parsed.rows.length) { error = 'Файл пуст или не распознан'; return; }
        importTargetField = fieldCode;
        importParsed = parsed;
        showMapping = true;
      } catch (e: any) {
        error = 'Ошибка парсинга: ' + (e?.message ?? String(e));
      }
    });
  }

  function applyImport(mapping: Record<string, string>) {
    if (!importParsed || !importTargetField) return;
    const mapped = importParsed.rows.map((row: Record<string, unknown>) => {
      const out: Record<string, unknown> = {};
      for (const [src, tgt] of Object.entries(mapping)) {
        if (tgt) out[tgt] = row[src];
      }
      return out;
    });
    setField(importTargetField, mapped);
    showMapping = false;
    importTargetField = null;
    importParsed = null;
  }

  function exportTable(fieldCode: string, format: FileFormat) {
    const val = data[fieldCode];
    const rows = Array.isArray(val) ? val as Record<string, unknown>[] : [];
    if (!rows.length) return;
    const content = serializeFile(rows, format);
    downloadText(content, `${object._id.slice(0,8)}_${fieldCode}.${format}`,
      format === 'csv' ? 'text/csv' : format === 'json' ? 'application/json' : 'text/xml');
  }

  function exportDocument(format: FileFormat) {
    const content = JSON.stringify({
      _id: object._id, entity_type_id: object.entity_type_id,
      state: object.state, number: object.number,
      date: object.date, data: object.data, version: object.version,
    }, null, 2);
    const ext = format === 'xml' ? 'xml' : 'json';
    downloadText(content, `${object._id.slice(0,8)}.${ext}`,
      format === 'xml' ? 'text/xml' : 'application/json');
  }

  onMount(async () => {
    initDeviceEvents();
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
    <button class="btn btn-sm preset-tonal-error ml-auto" onclick={requestClose} aria-label="Закрыть редактор">
      <i class="fa-solid fa-xmark"></i>
    </button>
  </div>

  {#if error}
    <div class="alert preset-tonal-error mx-3 mt-2 text-sm">{error}</div>
  {/if}

  {#if validationErrors.length > 0}
    <div class="alert preset-tonal-warning mx-3 mt-2 text-sm">
      <div class="font-medium mb-1">Заполните обязательные поля:</div>
      {#each validationErrors as ve}
        <div class="text-xs">• {ve}</div>
      {/each}
    </div>
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
    {#if isEditable && $auth && hasPermission($auth.permissions, 'documents', 'update')}
      <button class="btn btn-sm preset-filled-primary text-xs" disabled={saving} onclick={handleSave}>
        {saving ? '...' : 'Сохранить'}
      </button>
    {/if}
  </div>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto p-3">
    {#if !loading && isEditable}
      <div class="flex gap-2 mb-2">
        <span class="text-xs text-surface-400 self-center"><i class="fa-solid fa-download mr-1"></i>Выгрузить:</span>
        <button class="btn btn-xs btn-outline" onclick={() => exportDocument('json')}>JSON</button>
        <button class="btn btn-xs btn-outline" onclick={() => exportDocument('xml')}>XML</button>
      </div>
    {/if}

    {#if loading}
      <div class="text-center py-8 text-surface-500 text-sm"><i class="fa-solid fa-spinner fa-spin mr-1"></i>Загрузка...</div>

    {:else if tab === 'form'}
      {#if fields.length === 0}
        <div class="text-center py-8 text-surface-500 text-sm">Нет полей. Добавьте поля в метаданных.</div>
      {:else}
        <div class="space-y-4 max-w-2xl">
          {#each Object.entries(groupedFields) as [groupName, groupFields]}
            {#if groupName}
              <h4 class="text-xs font-semibold text-surface-500 uppercase tracking-wider mt-4">{groupName}</h4>
            {/if}
            <div class="space-y-3">
              {#each groupFields as field (field._id)}
                <label class="label">
                  <span class="label-text text-sm">
                    {field.name}
                    {#if field.is_required}<span class="text-error-500">*</span>{/if}
                    {#if field.field_kind === 'formula' || field.field_kind === 'computed'}
                      <i class="fa-solid fa-calculator text-xs text-surface-400 ml-1"></i>
                    {/if}
                  </span>

                  {#if field.field_kind === 'string'}
                    <input
                      class="input"
                      type="text"
                      placeholder={field.code}
                      disabled={field.is_readonly || !isEditable}
                      value={String(data[field.code] ?? '')}
                      oninput={(e) => setField(field.code, (e.target as HTMLInputElement).value)}
                    />

                  {:else if field.field_kind === 'text'}
                    <textarea
                      class="textarea"
                      placeholder={field.code}
                      disabled={field.is_readonly || !isEditable}
                      value={String(data[field.code] ?? '')}
                      oninput={(e) => setField(field.code, (e.target as HTMLTextAreaElement).value)}
                    ></textarea>

                  {:else if field.field_kind === 'integer'}
                    <div class="flex gap-1">
                      <input
                        class="input"
                        type="number"
                        step="1"
                        disabled={field.is_readonly || !isEditable}
                        value={data[field.code] ?? ''}
                        oninput={(e) => setField(field.code, parseInt((e.target as HTMLInputElement).value) || 0)}
                      />
                      {#if isWeightField(field.code)}
                        <button type="button" class="btn btn-sm btn-outline shrink-0" title="Взять вес с весов"
                          disabled={field.is_readonly || !isEditable}
                          onclick={() => takeWeight(field.code, false)}>
                          <i class="fa-solid fa-weight-scale"></i>
                        </button>
                      {/if}
                    </div>

                  {:else if field.field_kind === 'money'}
                    <div class="flex gap-1">
                      <input
                        class="input"
                        type="number"
                        step="0.01"
                        disabled={field.is_readonly || !isEditable}
                        value={data[field.code] ?? ''}
                        oninput={(e) => setField(field.code, parseFloat((e.target as HTMLInputElement).value) || 0)}
                      />
                      {#if isWeightField(field.code)}
                        <button type="button" class="btn btn-sm btn-outline shrink-0" title="Взять вес с весов"
                          disabled={field.is_readonly || !isEditable}
                          onclick={() => takeWeight(field.code, true)}>
                          <i class="fa-solid fa-weight-scale"></i>
                        </button>
                      {/if}
                    </div>

                  {:else if field.field_kind === 'date'}
                    <input
                      class="input"
                      type="date"
                      disabled={field.is_readonly || !isEditable}
                      value={String(data[field.code] ?? '')}
                      oninput={(e) => setField(field.code, (e.target as HTMLInputElement).value)}
                    />

                  {:else if field.field_kind === 'datetime'}
                    <input
                      class="input"
                      type="datetime-local"
                      disabled={field.is_readonly || !isEditable}
                      value={String(data[field.code] ?? '')}
                      oninput={(e) => setField(field.code, (e.target as HTMLInputElement).value)}
                    />

                  {:else if field.field_kind === 'boolean'}
                    <div class="flex items-center gap-2">
                      <input
                        class="checkbox"
                        type="checkbox"
                        disabled={field.is_readonly || !isEditable}
                        checked={Boolean(data[field.code])}
                        onchange={(e) => setField(field.code, (e.target as HTMLInputElement).checked)}
                      />
                      <span class="text-xs text-surface-500">{Boolean(data[field.code]) ? 'Да' : 'Нет'}</span>
                    </div>

                  {:else if field.field_kind === 'enum'}
                    <select
                      class="select"
                      disabled={field.is_readonly || !isEditable}
                      value={String(data[field.code] ?? '')}
                      onchange={(e) => setField(field.code, (e.target as HTMLSelectElement).value)}
                    >
                      <option value="">— не задано —</option>
                      {#each (field.enum_values ?? []) as opt}
                        <option value={opt}>{opt}</option>
                      {/each}
                    </select>

                  {:else if field.field_kind === 'reference'}
                    {#if referenceOptions[field.code]}
                      <select
                        class="select"
                        disabled={field.is_readonly || !isEditable}
                        value={String(data[field.code] ?? '')}
                        onchange={(e) => setField(field.code, (e.target as HTMLSelectElement).value || null)}
                      >
                        <option value="">— не задано —</option>
                        {#each referenceOptions[field.code] as refObj}
                          <option value={refObj._id}>{refObj.number ?? refObj._id} — {refObj.data?.name ?? refObj.data?.title ?? ''}</option>
                        {/each}
                      </select>
                    {:else if loadingReferences[field.code]}
                      <div class="text-xs text-surface-400 py-1"><i class="fa-solid fa-spinner fa-spin mr-1"></i>Загрузка...</div>
                    {:else}
                      <input
                        class="input"
                        type="text"
                        placeholder="UUID: {field.reference_entity ?? ''}"
                        disabled={field.is_readonly || !isEditable}
                        value={String(data[field.code] ?? '')}
                        oninput={(e) => setField(field.code, (e.target as HTMLInputElement).value || null)}
                      />
                    {/if}

                  {:else if field.field_kind === 'user'}
                    <select
                      class="select"
                      disabled={field.is_readonly || !isEditable}
                      value={String(data[field.code] ?? '')}
                      onchange={(e) => setField(field.code, (e.target as HTMLSelectElement).value || null)}
                    >
                      <option value="">— не задано —</option>
                      {#each allUsers as u (u._id)}
                        <option value={u._id}>{u.display_name || u.login}</option>
                      {/each}
                    </select>

                  {:else if field.field_kind === 'company'}
                    <select
                      class="select"
                      disabled={field.is_readonly || !isEditable}
                      value={String(data[field.code] ?? '')}
                      onchange={(e) => setField(field.code, (e.target as HTMLSelectElement).value || null)}
                    >
                      <option value="">— не задано —</option>
                      {#each allCompanies as c (c._id)}
                        <option value={c._id}>{c.name}</option>
                      {/each}
                    </select>

                  {:else if field.field_kind === 'array'}
                    <textarea
                      class="textarea font-mono text-xs"
                      placeholder='["элемент1", "элемент2"]'
                      disabled={field.is_readonly || !isEditable}
                      value={JSON.stringify(data[field.code] ?? [], null, 2)}
                      oninput={(e) => {
                        try { setField(field.code, JSON.parse((e.target as HTMLTextAreaElement).value)); }
                        catch { /* ждём валидный JSON */ }
                      }}
                    ></textarea>

                  {:else if field.field_kind === 'table'}
                    <div class="flex gap-1 mb-1">
                      <label class="btn btn-xs btn-outline cursor-pointer" title="Импорт табличных данных из файла">
                        <i class="fa-solid fa-file-import"></i> Загрузить
                        <input type="file" accept=".csv,.json,.xml,.txt" class="hidden"
                          onchange={(e) => {
                            const f = (e.target as HTMLInputElement).files?.[0];
                            if (f) openImport(field.code, f);
                          }} />
                      </label>
                      <button type="button" class="btn btn-xs btn-outline" title="Экспорт таблицы в файл"
                        onclick={() => exportTable(field.code, 'json')}>
                        <i class="fa-solid fa-file-export"></i> Выгрузить
                      </button>
                    </div>
                    <textarea
                      class="textarea font-mono text-xs"
                      placeholder='[{{"col1": "val1", "col2": "val2"}}]'
                      disabled={field.is_readonly || !isEditable}
                      value={JSON.stringify(data[field.code] ?? [], null, 2)}
                      oninput={(e) => {
                        try { setField(field.code, JSON.parse((e.target as HTMLTextAreaElement).value)); }
                        catch { /* ждём валидный JSON */ }
                      }}
                    ></textarea>

                  {:else if field.field_kind === 'json'}
                    <textarea
                      class="textarea font-mono text-xs"
                      placeholder={"{}"}
                      disabled={field.is_readonly || !isEditable}
                      value={JSON.stringify(data[field.code] ?? null, null, 2)}
                      oninput={(e) => {
                        try { setField(field.code, JSON.parse((e.target as HTMLTextAreaElement).value)); }
                        catch { /* ждём валидный JSON */ }
                      }}
                    ></textarea>

                  {:else if field.field_kind === 'formula' || field.field_kind === 'computed'}
                    <div class="input bg-surface-50-900 text-surface-500 cursor-not-allowed">
                      {data[field.code] ?? '—'}
                    </div>

                  {:else}
                    <input
                      class="input"
                      type="text"
                      placeholder="{FIELD_KIND_META[field.field_kind]?.label ?? field.field_kind}"
                      disabled={field.is_readonly || !isEditable}
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
        disabled={!isEditable}
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

{#if showMapping && importParsed}
  <FieldMappingDialog
    columns={importParsed.columns.map((c) => ({
      source: c,
      sample_value: String(importParsed!.rows[0]?.[c] ?? ''),
      matched_target: null,
    }))}
    targets={(fields.find((f) => f.code === importTargetField)?.field_kind === 'table')
      ? [{ code: 'nomenclature_id', name: 'Номенклатура' }, { code: 'qty', name: 'Количество' }, { code: 'price', name: 'Цена' }, { code: 'comment', name: 'Комментарий' }]
      : Object.keys(data).map((k) => ({ code: k, name: k }))
    }
    title="Сопоставление колонок"
    onApply={applyImport}
    onCancel={() => { showMapping = false; importTargetField = null; importParsed = null; }}
  />
{/if}
