<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import type { DiagnosticsReport } from '$lib/services/api';
  import { auth, hasPermission } from '$lib/stores/auth';
  import { confirmDialog } from '$lib/components/ui/dialog';
  import { toastSuccess, toastError, errText } from '$lib/components/ui/toast';
  import { seedDemoData } from '$lib/utils/demoData';

  interface Props {
    diagnostics: DiagnosticsReport | null;
    loading: boolean;
  }
  let { diagnostics, loading }: Props = $props();

  const canManage = $derived(!!$auth && hasPermission($auth.permissions, 'settings', 'manage'));

  // ── Демо-данные ──
  let demoBusy = $state(false);
  let demoStep = $state('');

  async function runDemo() {
    if (!(await confirmDialog({
      title: 'Загрузить демо-данные?',
      message: 'Будут созданы: пользователи (smirnova/petrov/maria/ivanov), номенклатура, контрагенты, проведённые документы и заявка на согласовании.',
      confirmLabel: 'Загрузить',
    }))) return;
    demoBusy = true;
    demoStep = '';
    try {
      const summary = await seedDemoData((p) => { demoStep = `${p.done}/${p.total} · ${p.step}`; });
      toastSuccess('Демо-данные «ЛесТорг» загружены');
      demoStep = summary;
    } catch (e) {
      toastError(errText(e, 'Ошибка загрузки демо-данных'));
      demoStep = '';
    } finally {
      demoBusy = false;
    }
  }
</script>

{#if loading}
  <div class="flex items-center justify-center p-12"><div class="h-8 w-8 animate-spin rounded-full border-4 border-primary-500 border-t-transparent"></div></div>
{:else if diagnostics}
  <div class="space-y-6">
    <div class="flex items-center justify-between gap-3">
      <h2 class="text-2xl font-bold text-surface-900-100">Главная</h2>
      {#if canManage}
        <button class="btn btn-sm preset-tonal" onclick={runDemo} disabled={demoBusy}>
          {#if demoBusy}<i class="fa-solid fa-circle-notch fa-spin"></i>{:else}<i class="fa-solid fa-wand-magic-sparkles"></i>{/if}
          Демо-данные
        </button>
      {/if}
    </div>

    {#if demoBusy && demoStep}
      <div class="rounded-lg border border-surface-300-700 bg-surface-50-950 p-3 text-sm text-surface-600-400">
        <i class="fa-solid fa-spinner fa-spin mr-1"></i>{demoStep}
      </div>
    {:else if demoStep && !demoBusy}
      <div class="whitespace-pre-wrap rounded-lg border border-success-500/30 bg-success-500/5 p-3 text-sm text-surface-700-300">{demoStep}</div>
    {/if}

    <div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
      <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
        <div class="text-sm font-medium text-surface-500-500">Версия</div>
        <div class="mt-1 text-2xl font-bold text-surface-900-100">{diagnostics.app_version}</div>
      </div>
      <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
        <div class="text-sm font-medium text-surface-500-500">MongoDB</div>
        <div class="mt-1 flex items-center gap-2">
          <span class="text-2xl font-bold text-surface-900-100">{diagnostics.mongodb.connected ? 'Подключено' : 'Отключено'}</span>
          <span class="rounded-full px-2 py-0.5 text-xs font-medium text-white {diagnostics.mongodb.ok ? 'bg-success-500' : 'bg-error-500'}">{diagnostics.mongodb.ok ? 'OK' : 'ERR'}</span>
        </div>
        {#if diagnostics.mongodb.version}<div class="mt-1 text-xs text-surface-500-500">v{diagnostics.mongodb.version}</div>{/if}
        {#if diagnostics.mongodb.replica_set}<div class="mt-1 text-xs text-surface-500-500">RS: {diagnostics.mongodb.replica_set}</div>{/if}
      </div>
      <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
        <div class="text-sm font-medium text-surface-500-500">Модули</div>
        <div class="mt-1 text-2xl font-bold text-surface-900-100">{diagnostics.modules.length}</div>
        <div class="mt-2 space-y-1">
          {#each diagnostics.modules as mod}
            <div class="flex items-center gap-2 text-xs"><span class="h-2 w-2 rounded-full bg-success-500"></span><span class="text-surface-700-300">{mod.name} v{mod.version}</span></div>
          {/each}
        </div>
      </div>
    </div>
  </div>
{/if}
