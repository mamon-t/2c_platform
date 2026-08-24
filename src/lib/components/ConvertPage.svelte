<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api,
    type WasmModuleInfo,
    type PluginFunction,
    type EntityType,
  } from '$lib/services/api';

  interface ImportResult {
    created: number;
    total: number;
    errors: string[];
  }

  interface ExportResult {
    data: number[];
    filename: string;
    content_type: string;
  }

  let modules: WasmModuleInfo[] = $state([]);
  let entityTypes: EntityType[] = $state([]);
  let loading = $state(true);
  let error = $state('');
  let log: { time: string; msg: string; ok: boolean }[] = $state([]);

  // Module selection
  let importModuleId = $state('');
  let exportModuleId = $state('');

  // Import state
  let importFormat = $state('csv');
  let importEntityTypeId = $state('');
  let importFile = $state<File | null>(null);
  let importResult = $state<ImportResult | null>(null);
  let importing = $state(false);

  // Export state
  let exportFormat = $state('csv');
  let exportEntityTypeId = $state('');
  let exporting = $state(false);
  let exportResult = $state<ExportResult | null>(null);

  // Derived: selected modules and their functions
  let importModule = $derived(modules.find(m => m.id === importModuleId));
  let exportModule = $derived(modules.find(m => m.id === exportModuleId));
  let importFn = $derived(importModule?.functions.find(f => f.name === 'import_data'));
  let exportFn = $derived(exportModule?.functions.find(f => f.name === 'export_data'));
  let importFormats = $derived(
    (importFn?.input_schema as any)?.properties?.format?.enum ?? []
  );
  let exportFormats = $derived(
    (exportFn?.input_schema as any)?.properties?.format?.enum ?? []
  );

  function addLog(msg: string, ok: boolean) {
    const time = new Date().toLocaleTimeString('ru-RU');
    log = [{ time, msg, ok }, ...log].slice(0, 50);
  }

  async function loadModules() {
    try {
      modules = await api.listWasmModules();
      entityTypes = await api.listEntityTypes();
    } catch (e: any) {
      error = e?.toString() || 'Ошибка загрузки';
    } finally {
      loading = false;
    }
  }

  async function handleLoadModule() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.wasm';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      try {
        const arrayBuffer = await file.arrayBuffer();
        const bytes = Array.from(new Uint8Array(arrayBuffer));
        const info = await api.loadWasmModule(bytes, file.name.replace(/\.wasm$/i, ''));
        modules = [...modules, info];
        const fns = info.functions.map(f => f.name).join(', ');
        addLog(`Модуль загружен: ${info.name} v${info.version} [${fns}]`, true);
      } catch (e: any) {
        addLog(`Ошибка загрузки: ${e}`, false);
      }
    };
    input.click();
  }

  async function handleUnloadModule(id: string) {
    try {
      await api.unloadWasmModule(id);
      modules = modules.filter(m => m.id !== id);
      addLog('Модуль выгружен', true);
    } catch (e: any) {
      addLog(`Ошибка выгрузки: ${e}`, false);
    }
  }

  async function handleImport() {
    if (!importFile || !importModuleId || !importEntityTypeId || !importFn) return;
    importing = true;
    importResult = null;
    try {
      const arrayBuffer = await importFile.arrayBuffer();
      const bytes = Array.from(new Uint8Array(arrayBuffer));
      const raw = await api.pluginCall(importModuleId, importFn.name, {
        format: importFormat,
        file_data: bytes,
        entity_type_id: importEntityTypeId,
      });
      importResult = raw as ImportResult;
      addLog(`Импорт: ${importResult.created}/${importResult.total} объектов`, importResult.errors.length === 0);
    } catch (e: any) {
      addLog(`Ошибка импорта: ${e}`, false);
    } finally {
      importing = false;
    }
  }

  async function handleExport() {
    if (!exportModuleId || !exportEntityTypeId || !exportFn) return;
    exporting = true;
    exportResult = null;
    try {
      const raw = await api.pluginCall(exportModuleId, exportFn.name, {
        format: exportFormat,
        entity_type_id: exportEntityTypeId,
      });
      exportResult = raw as ExportResult;
      // Download the file
      const bytes = new Uint8Array(exportResult.data);
      const blob = new Blob([bytes], { type: exportResult.content_type });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = exportResult.filename;
      a.click();
      URL.revokeObjectURL(url);
      addLog(`Экспорт: ${exportResult.filename} (${bytes.length} bytes)`, true);
    } catch (e: any) {
      addLog(`Ошибка экспорта: ${e}`, false);
    } finally {
      exporting = false;
    }
  }

  function onFileDrop(e: DragEvent) {
    e.preventDefault();
    const file = e.dataTransfer?.files[0];
    if (file) importFile = file;
  }

  function onFileSelect(e: Event) {
    const input = e.target as HTMLInputElement;
    importFile = input.files?.[0] ?? null;
  }

  onMount(loadModules);
</script>

<div class="flex h-full">
  <!-- Left panel: modules -->
  <div class="w-56 border-r border-surface-300-700 flex flex-col">
    <div class="p-3 border-b border-surface-300-700">
      <h3 class="font-semibold text-sm mb-2">WASM-модули</h3>
      <button class="btn btn-sm preset-tonal w-full text-xs" onclick={handleLoadModule}>
        <i class="fa-solid fa-upload mr-1"></i>Загрузить .wasm
      </button>
    </div>
    <div class="flex-1 overflow-y-auto p-2 space-y-1">
      {#if modules.length === 0}
        <p class="text-xs text-surface-500 py-2">Нет загруженных модулей</p>
      {/if}
      {#each modules as m (m.id)}
        <div class="card p-2 text-xs space-y-1">
          <div class="flex items-center justify-between">
            <span class="font-medium">{m.name}</span>
            <span class="text-surface-500">v{m.version}</span>
          </div>
          {#if m.functions.length > 0}
            <div class="space-y-0.5">
              {#each m.functions as fn}
                <div class="flex items-center gap-1 text-surface-400">
                  <i class="fa-solid fa-cog text-[10px]"></i>
                  <span>{fn.label}</span>
                </div>
              {/each}
            </div>
          {/if}
          <button class="btn btn-xs preset-tonal-error text-xs" onclick={() => handleUnloadModule(m.id)}>
            <i class="fa-solid fa-trash mr-1"></i>Выгрузить
          </button>
        </div>
      {/each}
    </div>
  </div>

  <!-- Main area -->
  <div class="flex-1 flex flex-col overflow-hidden">
    <h2 class="h3 text-sm p-3 border-b border-surface-300-700">
      <i class="fa-solid fa-right-left mr-1"></i>Конвертация данных
    </h2>

    {#if error}
      <div class="alert preset-tonal-error mx-3 mt-2 text-sm">{error}</div>
    {/if}

    <div class="flex-1 overflow-y-auto p-3">
      {#if loading}
        <div class="text-center py-8 text-surface-500 text-sm"><i class="fa-solid fa-spinner fa-spin mr-1"></i>Загрузка...</div>
      {:else}
        <div class="grid grid-cols-2 gap-4 max-w-4xl">
          <!-- Import panel -->
          <div class="card p-4 space-y-3">
            <h3 class="font-semibold text-sm"><i class="fa-solid fa-file-import mr-1"></i>Импорт</h3>

            <label class="label">
              <span class="label-text text-xs">Модуль</span>
              <select class="select select-sm" bind:value={importModuleId}>
                <option value="">Выберите модуль…</option>
                {#each modules as m}
                  {@const hasImport = m.functions.some(f => f.name === 'import_data')}
                  {#if hasImport}
                    <option value={m.id}>{m.name}</option>
                  {/if}
                {/each}
              </select>
            </label>

            {#if importFn}
              <label class="label">
                <span class="label-text text-xs">Описание</span>
                <p class="text-xs text-surface-500">{importFn.description}</p>
              </label>
            {/if}

            <label class="label">
              <span class="label-text text-xs">Формат</span>
              <select class="select select-sm" bind:value={importFormat}>
                {#each importFormats as f}
                  <option value={f}>{f.toUpperCase()}</option>
                {/each}
              </select>
            </label>

            <label class="label">
              <span class="label-text text-xs">Тип объекта</span>
              <select class="select select-sm" bind:value={importEntityTypeId}>
                <option value="">Выберите тип…</option>
                {#each entityTypes as et}
                  <option value={et._id}>{et.name}</option>
                {/each}
              </select>
            </label>

            <div
              class="border-2 border-dashed border-surface-300-600 rounded p-4 text-center text-xs text-surface-500 cursor-pointer hover:border-primary-500 transition-colors"
              ondrop={onFileDrop}
              ondragover={(e) => e.preventDefault()}
              onclick={() => document.getElementById('file-input')?.click()}
            >
              {#if importFile}
                <p class="text-surface-200">{importFile.name} ({importFile.size} bytes)</p>
              {:else}
                <p>Перетащите файл или нажмите для выбора</p>
              {/if}
            </div>
            <input id="file-input" type="file" class="hidden" accept=".csv,.json,.yaml,.xml" onchange={onFileSelect} />

            <button
              class="btn btn-sm preset-filled-primary w-full text-xs"
              disabled={importing || !importModuleId || !importEntityTypeId || !importFile || !importFn}
              onclick={handleImport}
            >
              {importing ? 'Импорт...' : 'Импортировать'}
            </button>

            {#if importResult}
              <div class="alert text-xs" class:preset-tonal-success={importResult.errors.length === 0} class:preset-tonal-error={importResult.errors.length > 0}>
                Создано: {importResult.created}/{importResult.total}
                {#if importResult.errors.length > 0}
                  <br>Ошибки: {importResult.errors.slice(0, 3).join('; ')}
                {/if}
              </div>
            {/if}
          </div>

          <!-- Export panel -->
          <div class="card p-4 space-y-3">
            <h3 class="font-semibold text-sm"><i class="fa-solid fa-file-export mr-1"></i>Экспорт</h3>

            <label class="label">
              <span class="label-text text-xs">Модуль</span>
              <select class="select select-sm" bind:value={exportModuleId}>
                <option value="">Выберите модуль…</option>
                {#each modules as m}
                  {@const hasExport = m.functions.some(f => f.name === 'export_data')}
                  {#if hasExport}
                    <option value={m.id}>{m.name}</option>
                  {/if}
                {/each}
              </select>
            </label>

            {#if exportFn}
              <label class="label">
                <span class="label-text text-xs">Описание</span>
                <p class="text-xs text-surface-500">{exportFn.description}</p>
              </label>
            {/if}

            <label class="label">
              <span class="label-text text-xs">Формат</span>
              <select class="select select-sm" bind:value={exportFormat}>
                {#each exportFormats as f}
                  <option value={f}>{f.toUpperCase()}</option>
                {/each}
              </select>
            </label>

            <label class="label">
              <span class="label-text text-xs">Тип объекта</span>
              <select class="select select-sm" bind:value={exportEntityTypeId}>
                <option value="">Выберите тип…</option>
                {#each entityTypes as et}
                  <option value={et._id}>{et.name}</option>
                {/each}
              </select>
            </label>

            <button
              class="btn btn-sm preset-filled-primary w-full text-xs"
              disabled={exporting || !exportModuleId || !exportEntityTypeId || !exportFn}
              onclick={handleExport}
            >
              {exporting ? 'Экспорт...' : 'Экспортировать'}
            </button>

            {#if exportResult}
              <div class="alert preset-tonal-success text-xs">
                Файл: {exportResult.filename} ({exportResult.data.length} bytes)
              </div>
            {/if}
          </div>
        </div>
      {/if}
    </div>

    <!-- Log -->
    {#if log.length > 0}
      <div class="border-t border-surface-300-700 max-h-40 overflow-y-auto p-2">
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
