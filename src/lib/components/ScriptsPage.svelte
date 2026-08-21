<script lang="ts">
  import { api } from '$lib/services/api';
  import { hasPermission, auth } from '$lib/stores/auth';

  let scriptSource = $state(`// Rhai-скрипт
// Доступные функции ядра (Core API):
//   object.get(field) — значение поля текущего объекта
//   object.set(field, value) — установить значение
//   log.info(msg) / log.warn(msg) / log.error(msg)
//   db.find(collection, query) — поиск в MongoDB
//   emit(event_type, payload) — создать событие
//   notify.user(user_id, message) — уведомление
//
// Контекст (ctx):
//   ctx.user, ctx.company, ctx.entity_type
//   ctx.object, ctx.changes, ctx.action

42 * 2`);

  let contextJson = $state('{}');
  let result = $state<unknown>(null);
  let resultJson = $state('');
  let error = $state('');
  let validating = $state(false);
  let executing = $state(false);
  let validationResult = $state<'ok' | 'error' | null>(null);
  let outputLog = $state<string[]>([]);

  function appendLog(level: string, msg: string) {
    const ts = new Date().toLocaleTimeString('ru-RU');
    outputLog = [...outputLog, `[${ts}] [${level}] ${msg}`];
  }

  async function handleValidate() {
    validating = true;
    error = '';
    validationResult = null;
    try {
      await api.validateRhaiScript(scriptSource);
      validationResult = 'ok';
      appendLog('info', 'Валидация прошла успешно');
    } catch (e: any) {
      validationResult = 'error';
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка валидации';
      appendLog('error', error);
    } finally { validating = false; }
  }

  async function handleExecute() {
    executing = true;
    error = '';
    result = null;
    resultJson = '';
    try {
      const res = await api.executeRhaiScript(scriptSource, contextJson);
      result = res;
      resultJson = JSON.stringify(res, null, 2);
      appendLog('info', `Результат: ${resultJson}`);
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка выполнения';
      appendLog('error', error);
    } finally { executing = false; }
  }

  function handleClearLog() { outputLog = []; }

  const canExecute = $derived(
    $auth && (hasPermission($auth.permissions, 'scripts', 'execute') || hasPermission($auth.permissions, 'scripts', 'read'))
  );
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h2 class="text-2xl font-bold text-surface-900-100">Скрипты Rhai</h2>
    <div class="flex gap-2">
      {#if canExecute}
        <button
          class="btn btn-sm preset-tonal-primary text-xs"
          disabled={validating}
          onclick={handleValidate}
        >
          {validating ? '...' : 'Валидировать'}
        </button>
        <button
          class="btn btn-sm preset-filled-primary text-xs"
          disabled={executing}
          onclick={handleExecute}
        >
          {executing ? '...' : 'Выполнить'}
        </button>
      {/if}
    </div>
  </div>

  {#if error}
    <div class="alert preset-tonal-error text-sm">{error}</div>
  {/if}

  {#if validationResult === 'ok'}
    <div class="alert preset-tonal-success text-sm">
      <i class="fa-solid fa-check mr-1"></i>Синтаксис валиден
    </div>
  {/if}

  <div class="grid grid-cols-1 lg:grid-cols-3 gap-4">
    <!-- Editor -->
    <div class="lg:col-span-2 space-y-3">
      <div class="card p-3">
        <div class="flex items-center justify-between mb-2">
          <span class="text-xs font-semibold text-surface-500 uppercase tracking-wider">Редактор</span>
          <span class="text-[10px] text-surface-400">{scriptSource.split('\n').length} строк</span>
        </div>
        <textarea
          class="w-full h-[400px] rounded-lg border border-surface-300-700 bg-surface-50-950 p-3 font-mono text-sm text-surface-900-100 focus:border-primary-500 focus:outline-none resize-y"
          spellcheck="false"
          bind:value={scriptSource}
        ></textarea>
      </div>
    </div>

    <!-- Sidebar: context + output -->
    <div class="space-y-3">
      <div class="card p-3">
        <span class="text-xs font-semibold text-surface-500 uppercase tracking-wider">Контекст (JSON)</span>
        <textarea
          class="w-full h-[120px] mt-2 rounded-lg border border-surface-300-700 bg-surface-50-950 p-2 font-mono text-xs text-surface-900-100 focus:border-primary-500 focus:outline-none resize-y"
          spellcheck="false"
          bind:value={contextJson}
        ></textarea>
      </div>

      {#if resultJson}
        <div class="card p-3">
          <span class="text-xs font-semibold text-surface-500 uppercase tracking-wider">Результат</span>
          <pre class="mt-2 max-h-[200px] overflow-auto rounded-lg bg-surface-100-900 p-2 text-xs text-surface-900-100">{resultJson}</pre>
        </div>
      {/if}

      <div class="card p-3">
        <div class="flex items-center justify-between mb-2">
          <span class="text-xs font-semibold text-surface-500 uppercase tracking-wider">Лог</span>
          {#if outputLog.length > 0}
            <button class="btn btn-xs text-[10px]" onclick={handleClearLog}>Очистить</button>
          {/if}
        </div>
        <div class="max-h-[200px] overflow-y-auto space-y-0.5">
          {#each outputLog as entry}
            <div class="font-mono text-[11px] text-surface-600-400">{entry}</div>
          {:else}
            <div class="text-[11px] text-surface-400 italic">Пока пусто</div>
          {/each}
        </div>
      </div>
    </div>
  </div>

  <!-- Reference -->
  <div class="card p-4">
    <h3 class="text-sm font-semibold text-surface-500 mb-2">Справочник по контексту</h3>
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4 text-xs text-surface-600-400">
      <div>
        <div class="font-medium text-surface-700-300 mb-1">Контекст</div>
        <ul class="space-y-0.5">
          <li><code class="text-primary-500">ctx.user</code> — текущий пользователь</li>
          <li><code class="text-primary-500">ctx.company</code> — текущая компания</li>
          <li><code class="text-primary-500">ctx.entity_type</code> — тип объекта</li>
          <li><code class="text-primary-500">ctx.object</code> — текущий объект</li>
          <li><code class="text-primary-500">ctx.changes</code> — изменения</li>
        </ul>
      </div>
      <div>
        <div class="font-medium text-surface-700-300 mb-1">API</div>
        <ul class="space-y-0.5">
          <li><code class="text-primary-500">db.find(coll, q)</code> — поиск</li>
          <li><code class="text-primary-500">emit(type, data)</code> — событие</li>
          <li><code class="text-primary-500">notify.user(id, msg)</code> — уведомление</li>
          <li><code class="text-primary-500">log.info/warn/error</code> — логирование</li>
        </ul>
      </div>
      <div>
        <div class="font-medium text-surface-700-300 mb-1">Ограничения</div>
        <ul class="space-y-0.5">
          <li>Таймаут: 10 сек</li>
          <li>Лимит операций: 10 000</li>
          <li>ФС/сеть: запрещены</li>
          <li>Типы: formula, validator, before/after_action, report, event_handler</li>
        </ul>
      </div>
    </div>
  </div>
</div>
