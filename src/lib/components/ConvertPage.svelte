<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api,
    type WasmModuleInfo,
    type EntityType,
    type ImportResult,
    type ExportResult,
  } from '$lib/services/api';

  let modules: WasmModuleInfo[] = $state([]);
  let entityTypes: EntityType[] = $state([]);
  let loading = $state(true);
  let error = $state('');
  let log: { time: string; msg: string; ok: boolean }[] = $state([]);

  // Import state
  let importModuleId = $state('');
  let importFormat = $state('csv');
  let importEntityTypeId = $state('');
  let importFile = $state<File | null>(null);
  let importResult = $state<ImportResult | null>(null);
  let importing = $state(false);

  // Export state
  let exportModuleId = $state('');
  let exportFormat = $state('csv');
  let exportEntityTypeId = $state('');
  let exporting = $state(false);
  let exportResult = $state<ExportResult | null>(null);

  const FORMAT_OPTIONS = ['csv', 'json', 'yaml', 'xml'];

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
      const path = await file.text(); // For now, user must provide path manually
      try {
        const info = await api.loadWasmModule(file.name);
        modules = [...modules, info];
        addLog(`Модуль загружен: ${info.name} v${info.version}`, true);
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
    if (!importFile || !importModuleId || !importEntityTypeId) return;
    importing = true;
    importResult = null;
    try {
      const arrayBuffer = await importFile.arrayBuffer();
      const bytes = Array.from(new Uint8Array(arrayBuffer));
      const result = await api.importObjectsViaWasm({
        module_id: importModuleId,
        file: bytes,
        filename: importFile.name,
        entity_type_id: importEntityTypeId,
        format: importFormat,
      });
      importResult = result;
      addLog(`Импорт: ${result.created}/${result.total} объектов`, result.errors.length === 0);
    } catch (e: any) {
      addLog(`Ошибка импорта: ${e}`, false);
    } finally {
      importing = false;
    }
  }

  async function handleExport() {
    if (!exportModuleId || !exportEntityTypeId) return;
    exporting = true;
    exportResult = null;
    try {
      const result = await api.exportObjectsViaWasm({
        module_id: exportModuleId,
        entity_type_id: exportEntityTypeId,
        format: exportFormat,
      });
      exportResult = result;
      // Download the file
      const bytes = new Uint8Array(result.data);
      const blob = new Blob([bytes], { type: result.content_type });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = result.filename;
      a.click();
      URL.revokeObjectURL(url);
      addLog(`Экспорт: ${result.filename} (${bytes.length} bytes)`, true);
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
          <div class="text-surface-500">{m.formats.join(', ')}</div>
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
                  <option value={m.id}>{m.name} ({m.formats.join(', ')})</option>
                {/each}
              </select>
            </label>

            <label class="label">
              <span class="label-text text-xs">Формат</span>
              <select class="select select-sm" bind:value={importFormat}>
                {#each FORMAT_OPTIONS as f}
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

            <!-- Drag and drop area -->
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
              disabled={importing || !importModuleId || !importEntityTypeId || !importFile}
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
                  <option value={m.id}>{m.name} ({m.formats.join(', ')})</option>
                {/each}
              </select>
            </label>

            <label class="label">
              <span class="label-text text-xs">Формат</span>
              <select class="select select-sm" bind:value={exportFormat}>
                {#each FORMAT_OPTIONS as f}
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
              disabled={exporting || !exportModuleId || !exportEntityTypeId}
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
