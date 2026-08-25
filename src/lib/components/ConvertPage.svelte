<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type EntityType, type EntityField, type ObjectEntity } from '$lib/services/api';
  import { parseFile, readFileAsText, downloadText, serializeFile, type FileFormat, type ParseResult } from '$lib/utils/fileConverter';
  import { validateObjectData, coerceForImport, type EntityFieldMeta, type FieldValidationError } from '$lib/utils/fieldValidation';
  import FieldMappingDialog from '$lib/components/FieldMappingDialog.svelte';
  import type { MappingColumn } from '$lib/components/FieldMappingDialog.svelte';

  type Step = 'select' | 'mapping' | 'preview' | 'result';

  // ── State ────────────────────────────────────────────────
  let step = $state<Step>('select');
  let entityTypes = $state<EntityType[]>([]);
  let targetFields = $state<EntityField[]>([]);
  let loading = $state(false);
  let error = $state('');
  let log = $state<{ time: string; msg: string; ok: boolean }[]>([]);

  // Step 1: select
  let targetEntityTypeId = $state('');
  let targetEntityKind = $state<'import' | 'export'>('import');
  let fileFormat = $state<FileFormat>('csv');
  let importFile = $state<File | null>(null);

  // Step 2: parsed data
  let parsedData = $state<ParseResult>({ rows: [], columns: [] });
  let columns = $state<MappingColumn[]>([]);
  let showMappingDialog = $state(false);
  let mapping = $state<Record<string, string>>({});

  // Step 3: preview
  let previewRows = $state<Record<string, unknown>[]>([]);
  let validationErrors = $state<FieldValidationError[]>([]);

  // Step 4: result
  let importResult = $state<{ created: number; errors: string[] } | null>(null);
  let importing = $state(false);

  // Export
  let exportData = $state('');
  let exportFilename = $state('');

  // Derived
  let targetType = $derived(entityTypes.find(e => e._id === targetEntityTypeId));

  function addLog(msg: string, ok: boolean) {
    const time = new Date().toLocaleTimeString('ru-RU');
    log = [{ time, msg, ok }, ...log].slice(0, 50);
  }

  async function loadEntityTypes() {
    try {
      entityTypes = await api.listEntityTypes();
    } catch (e: any) {
      error = e?.toString() || 'Ошибка загрузки типов';
    }
  }

  async function loadTargetFields(entityTypeId: string) {
    if (!entityTypeId) { targetFields = []; return; }
    try {
      targetFields = await api.listEntityFields(entityTypeId);
    } catch (e: any) {
      targetFields = [];
      addLog(`Ошибка загрузки полей: ${e}`, false);
    }
  }

  // ── Step 1 → 2: Parse file ──────────────────────────────

  async function handleFileSelect(e: Event) {
    const input = e.target as HTMLInputElement;
    importFile = input.files?.[0] ?? null;
    if (!importFile) return;
    await parseImportFile();
  }

  function onFileDrop(e: DragEvent) {
    e.preventDefault();
    const file = e.dataTransfer?.files[0];
    if (file) { importFile = file; parseImportFile(); }
  }

  async function parseImportFile() {
    if (!importFile) return;
    try {
      const text = await readFileAsText(importFile);
      parsedData = parseFile(text, fileFormat);
      buildColumns();
      step = 'mapping';
      addLog(`Файл: ${parsedData.rows.length} строк, ${parsedData.columns.length} колонок`, true);
    } catch (e: any) {
      addLog(`Ошибка парсинга: ${e}`, false);
    }
  }

  // ── Step 2: Auto-mapping ────────────────────────────────

  function buildColumns() {
    const targetMeta = targetFields
      .filter(f => f.field_kind !== 'formula' && f.field_kind !== 'computed')
      .map(f => ({ code: f.code, name: f.name }));

    // Auto-match by name similarity
    const matched = new Map<string, string>();
    for (const col of parsedData.columns) {
      const lower = col.toLowerCase().replace(/[_\s]+/g, '');
      for (const t of targetMeta) {
        const tLower = t.code.toLowerCase() + t.name.toLowerCase().replace(/[_\s]+/g, '');
        if (lower === t.code.toLowerCase() || lower.includes(t.code.toLowerCase()) || tLower.includes(lower)) {
          matched.set(col, t.code);
          break;
        }
      }
    }

    columns = parsedData.columns.map(col => ({
      source: col,
      sample_value: parsedData.rows[0]?.[col] != null ? String(parsedData.rows[0][col]).slice(0, 40) : '',
      matched_target: matched.get(col) ?? null,
    }));

    // Init mapping from auto-match
    mapping = {};
    for (const col of columns) {
      if (col.matched_target) mapping[col.source] = col.matched_target;
    }
  }

  function handleApplyMapping(m: Record<string, string>) {
    mapping = m;
    showMappingDialog = false;
    buildPreview();
  }

  // ── Step 3: Preview & validate ──────────────────────────

  function buildPreview() {
    const metaMap = new Map(targetFields.map(f => [f.code, f]));
    previewRows = parsedData.rows.slice(0, 20).map(row => {
      const out: Record<string, unknown> = {};
      for (const [sourceCol, targetCode] of Object.entries(mapping)) {
        const field = metaMap.get(targetCode);
        if (!field) continue;
        out[targetCode] = coerceForImport(row[sourceCol], field.field_kind);
      }
      return out;
    });

    // Validate preview
    const fieldsMeta: EntityFieldMeta[] = targetFields
      .filter(f => f.field_kind !== 'formula' && f.field_kind !== 'computed')
      .map(f => ({
        code: f.code,
        name: f.name,
        field_kind: f.field_kind,
        is_required: false, // preview only — don't enforce required
        is_readonly: false,
        enum_values: f.enum_values ?? undefined,
        reference_entity: f.reference_entity ?? undefined,
      }));

    validationErrors = [];
    for (let i = 0; i < previewRows.length; i++) {
      const errs = validateObjectData(previewRows[i], fieldsMeta);
      for (const err of errs) {
        validationErrors.push({ ...err, row: i + 1 });
      }
    }
    step = 'preview';
  }

  // ── Step 4: Import ──────────────────────────────────────

  async function handleImport() {
    if (!targetEntityTypeId || parsedData.rows.length === 0) return;
    importing = true;
    importResult = null;
    let created = 0;
    const errors: string[] = [];

    try {
      for (let i = 0; i < parsedData.rows.length; i++) {
        const row = parsedData.rows[i];
        const data: Record<string, unknown> = {};
        for (const [sourceCol, targetCode] of Object.entries(mapping)) {
          const field = targetFields.find(f => f.code === targetCode);
          if (!field) continue;
          data[targetCode] = coerceForImport(row[sourceCol], field.field_kind);
        }

        // Resolve references
        for (const field of targetFields) {
          if (field.field_kind === 'reference' && field.reference_entity && data[field.code] && typeof data[field.code] === 'string' && !isValidUuid(String(data[field.code]))) {
            // Search by name in the reference entity type
            const refType = entityTypes.find(t => t.code === field.reference_entity);
            if (refType) {
              try {
                const found = await api.searchObjectByField(refType._id, 'name', String(data[field.code]));
                if (found) {
                  data[field.code] = found._id;
                } else {
                  errors.push(`Строка ${i + 1}: «${data[field.code]}» не найден в ${field.reference_entity}`);
                  continue;
                }
              } catch {
                errors.push(`Строка ${i + 1}: ошибка поиска «${data[field.code]}»`);
                continue;
              }
            }
          }
        }

        try {
          await api.createObject({ entity_type_id: targetEntityTypeId, data });
          created++;
        } catch (e: any) {
          errors.push(`Строка ${i + 1}: ${e}`);
        }
      }

      importResult = { created, errors };
      step = 'result';
      addLog(`Импорт: ${created}/${parsedData.rows.length} объектов`, errors.length === 0);
    } catch (e: any) {
      addLog(`Ошибка импорта: ${e}`, false);
    } finally {
      importing = false;
    }
  }

  // ── Export ───────────────────────────────────────────────

  async function handleExport() {
    if (!targetEntityTypeId) return;
    loading = true;
    try {
      const page = await api.listObjects({ entity_type_id: targetEntityTypeId, limit: 1000 });
      const fieldsToExport = targetFields.filter(f => f.field_kind !== 'formula' && f.field_kind !== 'computed');

      const rows = page.objects.map(obj => {
        const row: Record<string, unknown> = {};
        for (const f of fieldsToExport) {
          row[f.code] = obj.data[f.code] ?? null;
        }
        return row;
      });

      exportData = serializeFile(rows, fileFormat);
      exportFilename = `${targetType?.code ?? 'export'}.${fileFormat}`;
      downloadText(exportData, exportFilename, 'text/plain');
      addLog(`Экспорт: ${rows.length} объектов → ${exportFilename}`, true);
    } catch (e: any) {
      addLog(`Ошибка экспорта: ${e}`, false);
    } finally {
      loading = false;
    }
  }

  function isValidUuid(s: string): boolean {
    return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(s);
  }

  function resetToSelect() {
    step = 'select';
    parsedData = { rows: [], columns: [] };
    columns = [];
    mapping = {};
    previewRows = [];
    validationErrors = [];
    importResult = null;
    importFile = null;
  }

  onMount(loadEntityTypes);
</script>

<div class="flex h-full">
  <!-- Left panel: entity types -->
  <div class="w-56 border-r border-surface-300-700 flex flex-col">
    <div class="p-3 border-b border-surface-300-700">
      <h3 class="font-semibold text-sm mb-2">Типы данных</h3>
    </div>
    <div class="flex-1 overflow-y-auto p-2 space-y-0.5">
      {#each entityTypes as et (et._id)}
        <button
          class="btn btn-sm w-full text-xs text-left justify-start"
          class:preset-tonal={targetEntityTypeId === et._id}
          onclick={() => { targetEntityTypeId = et._id; loadTargetFields(et._id); }}
        >
          <i class="fa-solid fa-cube mr-1"></i>{et.name}
        </button>
      {/each}
    </div>
  </div>

  <!-- Main area -->
  <div class="flex-1 flex flex-col overflow-hidden">
    <div class="flex items-center justify-between p-3 border-b border-surface-300-700">
      <h2 class="h3 text-sm">
        <i class="fa-solid fa-right-left mr-1"></i>Конвертация данных
      </h2>
      {#if targetType}
        <span class="text-xs text-surface-500">{targetType.name}</span>
      {/if}
    </div>

    {#if error}
      <div class="alert preset-tonal-error mx-3 mt-2 text-sm">{error}</div>
    {/if}

    <div class="flex-1 overflow-y-auto p-3">
      {#if !targetEntityTypeId}
        <div class="text-center py-12 text-surface-500 text-sm">
          <i class="fa-solid fa-arrow-left mr-1"></i>Выберите тип данных слева
        </div>

      {:else if step === 'select'}
        <div class="max-w-lg space-y-4">
          <div class="card p-4 space-y-3">
            <h3 class="font-semibold text-sm">Импорт данных</h3>
            <label class="label">
              <span class="label-text text-xs">Формат файла</span>
              <select class="select select-sm" bind:value={fileFormat}>
                <option value="csv">CSV</option>
                <option value="json">JSON</option>
                <option value="xml">XML</option>
              </select>
            </label>

            <div
              class="border-2 border-dashed border-surface-300-600 rounded p-4 text-center text-xs text-surface-500 cursor-pointer hover:border-primary-500 transition-colors"
              ondrop={onFileDrop}
              ondragover={(e) => e.preventDefault()}
              onclick={() => document.getElementById('file-input-convert')?.click()}
            >
              {#if importFile}
                <p class="text-surface-200">{importFile.name}</p>
              {:else}
                <p>Перетащите файл или нажмите</p>
              {/if}
            </div>
            <input id="file-input-convert" type="file" class="hidden" accept=".csv,.json,.xml" onchange={handleFileSelect} />
          </div>

          <div class="card p-4 space-y-3">
            <h3 class="font-semibold text-sm">Экспорт данных</h3>
            <label class="label">
              <span class="label-text text-xs">Формат</span>
              <select class="select select-sm" bind:value={fileFormat}>
                <option value="csv">CSV</option>
                <option value="json">JSON</option>
                <option value="xml">XML</option>
              </select>
            </label>
            <button
              class="btn btn-sm preset-filled-primary w-full text-xs"
              disabled={loading}
              onclick={handleExport}
            >
              {loading ? 'Выгрузка...' : 'Экспортировать'}
            </button>
          </div>
        </div>

      {:else if step === 'mapping'}
        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <h3 class="font-semibold text-sm">
              Сопоставление полей — {parsedData.rows.length} строк
            </h3>
            <button class="btn btn-sm text-xs" onclick={resetToSelect}>
              <i class="fa-solid fa-arrow-left mr-1"></i>Назад
            </button>
          </div>

          <div class="card p-3">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs text-surface-500">
                Сопоставлено: {Object.keys(mapping).length} из {parsedData.columns.length} колонок
              </span>
              <button class="btn btn-sm preset-tonal text-xs" onclick={() => showMappingDialog = true}>
                <i class="fa-solid fa-right-left mr-1"></i>Изменить
              </button>
            </div>

            <table class="table table-sm text-xs">
              <thead>
                <tr><th>Колонка файла</th><th>Пример</th><th>Поле объекта</th><th>Тип</th></tr>
              </thead>
              <tbody>
                {#each columns as col (col.source)}
                  {@const target = targetFields.find(f => f.code === mapping[col.source])}
                  <tr>
                    <td class="font-mono">{col.source}</td>
                    <td class="text-surface-400 truncate max-w-32">{col.sample_value}</td>
                    <td>{target?.name ?? '—'}</td>
                    <td class="text-surface-400">{target?.field_kind ?? ''}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>

          <div class="flex justify-end gap-2">
            <button class="btn btn-sm preset-tonal text-xs" onclick={resetToSelect}>Отмена</button>
            <button
              class="btn btn-sm preset-filled-primary text-xs"
              disabled={Object.keys(mapping).length === 0}
              onclick={buildPreview}
            >
              Далее: предпросмотр
            </button>
          </div>
        </div>

      {:else if step === 'preview'}
        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <h3 class="font-semibold text-sm">
              Предпросмотр — {previewRows.length} строк
              {#if validationErrors.length > 0}
                <span class="text-red-400 ml-2">({validationErrors.length} ошибок)</span>
              {/if}
            </h3>
            <button class="btn btn-sm text-xs" onclick={() => step = 'mapping'}>
              <i class="fa-solid fa-arrow-left mr-1"></i>Назад
            </button>
          </div>

          {#if validationErrors.length > 0}
            <div class="alert preset-tonal-error text-xs max-h-32 overflow-y-auto">
              {#each validationErrors.slice(0, 10) as err}
                <div>Строка {err.row ?? '?'}: поле «{err.field_name}» — {err.message}</div>
              {/each}
              {#if validationErrors.length > 10}
                <div class="text-surface-500">...ещё {validationErrors.length - 10}</div>
              {/if}
            </div>
          {/if}

          <div class="card overflow-x-auto">
            <table class="table table-sm text-xs">
              <thead>
                <tr>
                  <th>#</th>
                  {#each Object.keys(previewRows[0] ?? {}) as key}
                    <th>{targetFields.find(f => f.code === key)?.name ?? key}</th>
                  {/each}
                </tr>
              </thead>
              <tbody>
                {#each previewRows as row, i}
                  <tr>
                    <td class="text-surface-400">{i + 1}</td>
                    {#each Object.values(row) as val}
                      <td>{val != null ? String(val) : '—'}</td>
                    {/each}
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>

          <div class="flex justify-end gap-2">
            <button class="btn btn-sm preset-tonal text-xs" onclick={() => step = 'mapping'}>Назад</button>
            <button
              class="btn btn-sm preset-filled-primary text-xs"
              disabled={importing}
              onclick={handleImport}
            >
              {importing ? 'Импорт...' : `Импортировать ${parsedData.rows.length} строк`}
            </button>
          </div>
        </div>

      {:else if step === 'result'}
        <div class="max-w-lg space-y-3">
          <h3 class="font-semibold text-sm">Результат</h3>
          {#if importResult}
            <div class="alert text-xs" class:preset-tonal-success={importResult.errors.length === 0} class:preset-tonal-error={importResult.errors.length > 0}>
              Импортировано: {importResult.created}/{parsedData.rows.length}
            </div>
            {#if importResult.errors.length > 0}
              <div class="card p-3 text-xs max-h-40 overflow-y-auto space-y-1">
                {#each importResult.errors as err}
                  <div class="text-red-400">{err}</div>
                {/each}
              </div>
            {/if}
          {/if}
          <button class="btn btn-sm preset-tonal text-xs" onclick={resetToSelect}>
            <i class="fa-solid fa-arrow-left mr-1"></i>Новый импорт
          </button>
        </div>
      {/if}
    </div>

    <!-- Log -->
    {#if log.length > 0}
      <div class="border-t border-surface-300-700 max-h-36 overflow-y-auto p-2">
        <h4 class="text-xs font-semibold text-surface-500 mb-1">Журнал</h4>
        {#each log as entry}
          <div class="text-xs flex items-center gap-2">
            <span class="text-surface-500">{entry.time}</span>
            <span class={entry.ok ? 'text-green-400' : 'text-red-400'}>
              <i class="fa-solid {entry.ok ? 'fa-check' : 'fa-xmark'}"></i>
            </span>
            <span>{entry.msg}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

{#if showMappingDialog}
  <FieldMappingDialog
    {columns}
    targets={targetFields.filter(f => f.field_kind !== 'formula' && f.field_kind !== 'computed').map(f => ({ code: f.code, name: f.name }))}
    onApply={handleApplyMapping}
    onCancel={() => showMappingDialog = false}
  />
{/if}
