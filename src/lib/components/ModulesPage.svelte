// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import { api } from '$lib/services/api';
  import type { InstalledModule } from '$lib/services/api';
  import { hasPermission, auth } from '$lib/stores/auth';

  let modules = $state<InstalledModule[]>([]);
  let loading = $state(true);
  let installing = $state(false);
  let error = $state('');
  let success = $state('');

  let expandedId = $state<string | null>(null);
  let fileInput = $state<HTMLInputElement>();
  let fileBytes = $state<number[] | null>(null);
  let fileName = $state('');

  const perms = $derived($auth?.permissions ?? []);
  const canRead = $derived(hasPermission(perms, 'plugins', 'read'));
  const canWrite = $derived(hasPermission(perms, 'plugins', 'manage'));

  const CAPABILITY_LABELS: Record<string, string> = {
    'objects.create': 'Создание объектов',
    'objects.read': 'Чтение объектов',
    'objects.update': 'Обновление объектов',
    'objects.delete': 'Удаление объектов',
    'metadata.read': 'Чтение метаданных',
    'events.emit': 'Эмиссия событий',
    'numbering.next': 'Нумерация',
    'logging': 'Логирование',
    'notifications': 'Уведомления',
  };

  const STATUS_META: Record<string, { label: string; cls: string; icon: string }> = {
    enabled:  { label: 'Включён', cls: 'bg-success-500/20 text-success-700', icon: 'fa-solid fa-check-circle' },
    disabled: { label: 'Отключён', cls: 'bg-warning-500/20 text-warning-700', icon: 'fa-solid fa-pause-circle' },
    installed:{ label: 'Установлен', cls: 'bg-surface-400/20 text-surface-600', icon: 'fa-solid fa-circle-info' },
  };

  function fmtDate(d: string | null): string {
    if (!d) return '—';
    try { return new Date(d).toLocaleString('ru-RU', { day: '2-digit', month: '2-digit', year: '2-digit', hour: '2-digit', minute: '2-digit' }); } catch { return d; }
  }

  async function loadModules() {
    loading = true;
    error = '';
    try { modules = await api.modulesList(); } catch (e: any) { error = typeof e === 'string' ? e : e?.message ?? 'Ошибка загрузки'; }
    finally { loading = false; }
  }

  function handleFileSelect() {
    fileInput?.click();
  }

  async function handleFileChange(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    fileName = file.name;
    const buf = await file.arrayBuffer();
    fileBytes = Array.from(new Uint8Array(buf));
    input.value = '';
  }

  async function handleInstall() {
    if (!fileBytes) return;
    installing = true;
    error = '';
    success = '';
    try {
      const result = await api.modulesInstall(fileBytes);
      success = `Модуль «${result.name}» v${result.version} установлен`;
      fileBytes = null;
      fileName = '';
      await loadModules();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка установки';
    } finally { installing = false; }
  }

  async function handleEnable(id: string) {
    error = '';
    try { await api.modulesEnable(id); await loadModules(); } catch (e: any) { error = typeof e === 'string' ? e : e?.message ?? 'Ошибка'; }
  }

  async function handleDisable(id: string) {
    error = '';
    try { await api.modulesDisable(id); await loadModules(); } catch (e: any) { error = typeof e === 'string' ? e : e?.message ?? 'Ошибка'; }
  }

  async function handleUninstall(id: string, name: string) {
    if (!(await confirmDialog({ title: `Удалить модуль «${name}»?`, message: 'Все настройки будут потеряны.', danger: true }))) return;
    error = '';
    try { await api.modulesUninstall(id); await loadModules(); } catch (e: any) { error = typeof e === 'string' ? e : e?.message ?? 'Ошибка'; }
  }

  const inputCls = 'w-full rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 placeholder:text-surface-400-600 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500';
  const btnPrimary = 'rounded-lg bg-primary-500 px-4 py-2 text-sm font-medium text-white hover:bg-primary-600 disabled:opacity-50';
  const btnSecondary = 'rounded-lg border border-surface-300-700 px-4 py-2 text-sm text-surface-700-300 hover:bg-surface-200-800';
  const btnDanger = 'rounded-lg bg-error-500 px-3 py-1 text-xs font-medium text-white hover:bg-error-600';

  $effect(() => { if (canRead) loadModules(); });
  import { confirmDialog } from '$lib/components/ui/dialog';
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <div>
      <h2 class="text-2xl font-bold text-surface-900-100">Прикладные модули</h2>
      <p class="mt-1 text-sm text-surface-500-500">WASM-плагины для расширения функциональности платформы</p>
    </div>
  </div>

  {#if error}
    <div class="rounded-lg bg-error-500/10 p-3 text-sm text-error-600 flex items-center gap-2">
      <i class="fa-solid fa-circle-exclamation"></i>{error}
      <button onclick={() => error = ''} class="ml-auto text-error-400 hover:text-error-600"><i class="fa-solid fa-xmark"></i></button>
    </div>
  {/if}
  {#if success}
    <div class="rounded-lg bg-success-500/10 p-3 text-sm text-success-700 flex items-center gap-2">
      <i class="fa-solid fa-circle-check"></i>{success}
      <button onclick={() => success = ''} class="ml-auto text-success-400 hover:text-success-600"><i class="fa-solid fa-xmark"></i></button>
    </div>
  {/if}

  <!-- Upload section -->
  {#if canWrite}
    <div class="rounded-xl border border-dashed border-surface-300-700 bg-surface-50-950 p-6">
      <h3 class="mb-3 font-semibold text-surface-900-100">Установить модуль</h3>
      <p class="mb-4 text-sm text-surface-500-500">Загрузите WASM-файл модуля. Модуль автоматически определит свои функции и capabilities через <code class="rounded bg-surface-200-800 px-1 font-mono text-xs">get_info()</code>.</p>
      <input bind:this={fileInput} type="file" accept=".wasm" class="hidden" onchange={handleFileChange} />
      <div class="flex items-center gap-3">
        <button onclick={handleFileSelect} class={btnSecondary}>
          <i class="fa-solid fa-upload mr-2"></i>{fileName || 'Выбрать файл'}
        </button>
        {#if fileBytes}
          <span class="text-sm text-surface-500-500">{fileName} ({(fileBytes.length / 1024).toFixed(1)} КБ)</span>
          <button onclick={handleInstall} disabled={installing} class={btnPrimary}>
            {installing ? 'Установка...' : 'Установить'}
          </button>
        {/if}
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="flex items-center justify-center p-12">
      <div class="h-8 w-8 animate-spin rounded-full border-4 border-primary-500 border-t-transparent"></div>
    </div>
  {:else if modules.length === 0}
    <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-12 text-center">
      <i class="fa-solid fa-puzzle-piece text-4xl text-surface-400-600 mb-3"></i>
      <p class="text-surface-500-500">Нет установленных модулей</p>
      {#if canWrite}
        <p class="mt-1 text-sm text-surface-400-600">Загрузите WASM-файл для установки первого модуля</p>
      {/if}
    </div>
  {:else}
    <!-- Stats -->
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-3">
      <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-4">
        <div class="text-sm font-medium text-surface-500-500">Всего</div>
        <div class="mt-1 text-2xl font-bold text-surface-900-100">{modules.length}</div>
      </div>
      <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-4">
        <div class="text-sm font-medium text-surface-500-500">Активных</div>
        <div class="mt-1 text-2xl font-bold text-success-500">{modules.filter(m => m.status === 'enabled').length}</div>
      </div>
      <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-4">
        <div class="text-sm font-medium text-surface-500-500">Отключённых</div>
        <div class="mt-1 text-2xl font-bold text-warning-500">{modules.filter(m => m.status === 'disabled').length}</div>
      </div>
    </div>

    <!-- Module cards -->
    <div class="space-y-3">
      {#each modules as mod (mod._id)}
        {@const st = STATUS_META[mod.status] ?? STATUS_META.installed}
        {@const expanded = expandedId === mod._id}
        <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 overflow-hidden">
          <!-- Header -->
          <div class="flex items-center gap-4 p-4">
            <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary-500/10 text-primary-500">
              <i class="fa-solid fa-puzzle-piece text-lg"></i>
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2">
                <h3 class="font-semibold text-surface-900-100">{mod.name}</h3>
                <span class="text-xs text-surface-400-600">v{mod.version}</span>
                <span class="rounded-full px-2 py-0.5 text-xs font-medium {st.cls}">
                  <i class="{st.icon} mr-1"></i>{st.label}
                </span>
              </div>
              <div class="mt-0.5 text-xs text-surface-500-500">
                <span class="font-mono">{mod.code}</span> · {mod.author} · API v{mod.api_version}
              </div>
              {#if mod.description}
                <p class="mt-1 text-sm text-surface-600-400 line-clamp-1">{mod.description}</p>
              {/if}
            </div>
            <div class="flex items-center gap-2 shrink-0">
              {#if canWrite}
                {#if mod.status === 'enabled'}
                  <button onclick={() => handleDisable(mod._id)} class="rounded-lg border border-warning-500/30 px-3 py-1 text-xs font-medium text-warning-600 hover:bg-warning-500/10" title="Отключить">
                    <i class="fa-solid fa-pause mr-1"></i>Отключить
                  </button>
                {:else}
                  <button onclick={() => handleEnable(mod._id)} class="rounded-lg border border-success-500/30 px-3 py-1 text-xs font-medium text-success-700 hover:bg-success-500/10" title="Включить">
                    <i class="fa-solid fa-play mr-1"></i>Включить
                  </button>
                {/if}
                <button onclick={() => handleUninstall(mod._id, mod.name)} class="rounded-lg px-3 py-1 text-xs font-medium text-error-500 hover:bg-error-500/10" title="Удалить">
                  <i class="fa-solid fa-trash"></i>
                </button>
              {/if}
              <button onclick={() => expandedId = expanded ? null : mod._id} class="rounded-lg p-2 text-surface-500-500 hover:bg-surface-200-800" title="Подробнее">
                <i class="fa-solid fa-chevron-down transition-transform {expanded ? 'rotate-180' : ''}"></i>
              </button>
            </div>
          </div>

          <!-- Expanded details -->
          {#if expanded}
            <div class="border-t border-surface-300-700 bg-surface-100-900/50 p-4 space-y-4">
              <!-- Info -->
              <div class="grid grid-cols-2 gap-3 text-sm md:grid-cols-4">
                <div><span class="text-surface-500-500">Установлен:</span> <span class="text-surface-900-100">{fmtDate(mod.installed_at)}</span></div>
                <div><span class="text-surface-500-500">Обновлён:</span> <span class="text-surface-900-100">{fmtDate(mod.updated_at)}</span></div>
                <div><span class="text-surface-500-500">API версия:</span> <span class="text-surface-900-100">{mod.api_version}</span></div>
                <div><span class="text-surface-500-500">Код:</span> <span class="font-mono text-surface-900-100">{mod.code}</span></div>
              </div>

              <!-- Capabilities -->
              <div>
                <h4 class="mb-2 text-sm font-medium text-surface-700-300">Capabilities (права доступа)</h4>
                <div class="flex flex-wrap gap-2">
                  {#each mod.capabilities as cap}
                    <span class="rounded-full bg-primary-500/10 px-2.5 py-0.5 text-xs font-medium text-primary-600">
                      <i class="fa-solid fa-key mr-1 text-[10px]"></i>{CAPABILITY_LABELS[cap] ?? cap}
                    </span>
                  {/each}
                </div>
              </div>

              <!-- Functions -->
              {#if mod.functions.length > 0}
                <div>
                  <h4 class="mb-2 text-sm font-medium text-surface-700-300">Функции ({mod.functions.length})</h4>
                  <div class="space-y-1">
                    {#each mod.functions as fn}
                      <div class="flex items-center gap-2 rounded-lg bg-surface-50-950 px-3 py-2 text-sm">
                        <span class="font-mono text-xs text-primary-500">{fn.name}</span>
                        <span class="text-surface-500-500">—</span>
                        <span class="text-surface-900-100">{fn.label}</span>
                        {#if fn.description}
                          <span class="text-xs text-surface-400-600 ml-2">({fn.description})</span>
                        {/if}
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
