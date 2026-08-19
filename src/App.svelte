<script lang="ts">
  import { theme } from '$lib/stores/theme';
  import { navItems, activeNav } from '$lib/stores/navigation';
  import { onMount } from 'svelte';
  import { api } from '$lib/services/api';
  import type { DiagnosticsReport } from '$lib/services/api';

  let sidebarCollapsed = $state(false);
  let diagnostics = $state<DiagnosticsReport | null>(null);
  let loading = $state(true);
  let currentNav = $state('dashboard');

  function toggleSidebar() {
    sidebarCollapsed = !sidebarCollapsed;
  }

  function setNav(code: string) {
    currentNav = code;
    activeNav.set(code);
  }

  onMount(async () => {
    theme.init();
    try {
      diagnostics = await api.getDiagnostics();
    } catch (e) {
      console.error('Ошибка диагностики:', e);
    } finally {
      loading = false;
    }
  });

  const iconMap: Record<string, string> = {
    grid: '⊞',
    'file-text': '📄',
    book: '📚',
    'bar-chart': '📊',
    code: '⟨⟩',
    settings: '⚙',
  };
</script>

<div class="flex h-screen overflow-hidden bg-surface-50-950">
  <aside
    class="flex flex-col border-r border-surface-300-700 bg-surface-100-900 transition-all duration-300"
    class:w-64={!sidebarCollapsed}
    class:w-16={sidebarCollapsed}
  >
    <div class="flex items-center gap-2 border-b border-surface-300-700 p-4">
      <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary-500 text-sm font-bold text-white">
        2C
      </div>
      {#if !sidebarCollapsed}
        <span class="font-semibold text-surface-900-100">Платформа</span>
      {/if}
      <button
        onclick={toggleSidebar}
        class="ml-auto rounded p-1 text-surface-500-500 hover:bg-surface-200-800"
        title={sidebarCollapsed ? 'Развернуть' : 'Свернуть'}
      >
        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          {#if sidebarCollapsed}
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          {:else}
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
          {/if}
        </svg>
      </button>
    </div>

    <nav class="flex-1 overflow-y-auto p-2">
      {#each $navItems as item}
        <button
          onclick={() => setNav(item.code)}
          class="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors
            {currentNav === item.code
              ? 'bg-primary-500/10 font-medium text-primary-600'
              : 'text-surface-600-400 hover:bg-surface-200-800'}"
        >
          <span class="text-lg">{iconMap[item.icon] ?? '•'}</span>
          {#if !sidebarCollapsed}
            <span>{item.label}</span>
          {/if}
        </button>
      {/each}
    </nav>

    <div class="border-t border-surface-300-700 p-3">
      <div class="flex items-center gap-2">
        <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary-500 text-xs font-bold text-white">
          А
        </div>
        {#if !sidebarCollapsed}
          <div class="flex flex-col">
            <span class="text-sm font-medium text-surface-900-100">Администратор</span>
            <span class="text-xs text-surface-500-500">admin</span>
          </div>
        {/if}
      </div>
    </div>
  </aside>

  <main class="flex-1 overflow-y-auto">
    <header class="flex items-center justify-between border-b border-surface-300-700 bg-surface-50-950 px-6 py-4">
      <h1 class="text-lg font-semibold text-surface-900-100">
        {$navItems.find((n: any) => n.code === currentNav)?.label ?? 'Главная'}
      </h1>
      <div class="flex items-center gap-4">
        <button
          onclick={() => theme.set($theme === 'dark' ? 'light' : 'dark')}
          class="rounded-lg p-2 text-surface-500-500 hover:bg-surface-200-800"
          title="Сменить тему"
        >
          {$theme === 'dark' ? '☀' : '☾'}
        </button>
      </div>
    </header>

    <div class="p-6">
      {#if currentNav === 'dashboard'}
        {#if loading}
          <div class="flex items-center justify-center p-12">
            <div class="h-8 w-8 animate-spin rounded-full border-4 border-primary-500 border-t-transparent"></div>
          </div>
        {:else if diagnostics}
          <div class="space-y-6">
            <div>
              <h2 class="text-2xl font-bold text-surface-900-100">Главная</h2>
              <p class="mt-1 text-surface-500-500">Добро пожаловать в 2C Platform v0.1</p>
            </div>

            <div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
              <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
                <div class="text-sm font-medium text-surface-500-500">Версия</div>
                <div class="mt-1 text-2xl font-bold text-surface-900-100">{diagnostics.app_version}</div>
              </div>

              <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
                <div class="text-sm font-medium text-surface-500-500">MongoDB</div>
                <div class="mt-1 flex items-center gap-2">
                  <span class="text-2xl font-bold text-surface-900-100">
                    {diagnostics.mongodb.connected ? 'Подключено' : 'Не подключено'}
                  </span>
                  <span
                    class="rounded-full px-2 py-0.5 text-xs font-medium text-white"
                    class:bg-success-500={diagnostics.mongodb.ok}
                    class:bg-error-500={!diagnostics.mongodb.ok}
                  >
                    {diagnostics.mongodb.ok ? 'OK' : 'ERROR'}
                  </span>
                </div>
                {#if diagnostics.mongodb.version}
                  <div class="mt-1 text-xs text-surface-500-500">v{diagnostics.mongodb.version}</div>
                {/if}
                {#if diagnostics.mongodb.replica_set}
                  <div class="mt-1 text-xs text-surface-500-500">RS: {diagnostics.mongodb.replica_set}</div>
                {/if}
              </div>

              <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
                <div class="text-sm font-medium text-surface-500-500">Модули</div>
                <div class="mt-1 text-2xl font-bold text-surface-900-100">{diagnostics.modules.length}</div>
                <div class="mt-2 space-y-1">
                  {#each diagnostics.modules as mod}
                    <div class="flex items-center gap-2 text-xs">
                      <span class="h-2 w-2 rounded-full bg-success-500"></span>
                      <span class="text-surface-700-300">{mod.name} v{mod.version}</span>
                    </div>
                  {/each}
                </div>
              </div>
            </div>

            <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6">
              <h3 class="text-lg font-semibold text-surface-900-100">Этапы v0.1</h3>
              <ul class="mt-3 space-y-2 text-sm text-surface-600-400">
                <li class="text-success-600">✓ Каркас проекта</li>
                <li>2. Подключение к MongoDB и диагностика</li>
                <li>3. Компании, пользователи, роли</li>
                <li>4. Метаданные</li>
                <li>5. Объекты и CRUD</li>
                <li>6. События и версии</li>
                <li>7. Права</li>
                <li>8. Динамический UI</li>
                <li>9. Rhai-скрипты</li>
                <li>10. Управленческий учёт</li>
                <li>11. CSV и печать</li>
                <li>12. Уведомления inapp + e-mail</li>
                <li>13. Криптоподпись Linux-first</li>
                <li>14. Диагностика и логи</li>
                <li>15. Тесты и документация</li>
              </ul>
            </div>
          </div>
        {:else}
          <div class="rounded-xl border border-error-500/50 bg-error-500/10 p-5">
            <p class="text-error-700-300">Не удалось получить диагностику системы</p>
          </div>
        {/if}

      {:else if currentNav === 'documents'}
        <div class="space-y-4">
          <h2 class="text-2xl font-bold text-surface-900-100">Документы</h2>
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">
            Документы будут доступны после настройки метаданных (этап 5)
          </div>
        </div>

      {:else if currentNav === 'catalogs'}
        <div class="space-y-4">
          <h2 class="text-2xl font-bold text-surface-900-100">Справочники</h2>
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">
            Справочники будут доступны после настройки метаданных (этап 4)
          </div>
        </div>

      {:else if currentNav === 'reports'}
        <div class="space-y-4">
          <h2 class="text-2xl font-bold text-surface-900-100">Отчёты</h2>
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">
            ОСВ, журнал проводок, карточка счёта, баланс — будут доступны в этапе 10
          </div>
        </div>

      {:else if currentNav === 'scripts'}
        <div class="space-y-4">
          <h2 class="text-2xl font-bold text-surface-900-100">Скрипты Rhai</h2>
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">
            Редактор скриптов будет доступен в этапе 9
          </div>
        </div>

      {:else if currentNav === 'settings'}
        <div class="space-y-4">
          <h2 class="text-2xl font-bold text-surface-900-100">Настройки</h2>
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">
            Настройки системы будут доступны после этапа 3 (компании, пользователи, роли)
          </div>
        </div>
      {/if}
    </div>
  </main>
</div>
