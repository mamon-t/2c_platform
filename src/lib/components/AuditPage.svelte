<script lang="ts">
  import { api } from '$lib/services/api';
  import type { AuditEntry, AuditPage, AuditLogFilters } from '$lib/services/api';
  import { AUDIT_ACTION_META } from '$lib/services/api';

  let page = $state<AuditPage | null>(null);
  let loading = $state(true);
  let selectedEntry = $state<AuditEntry | null>(null);
  let filters = $state<AuditLogFilters>({ limit: 50 });

  function actionMeta(action: string) {
    return AUDIT_ACTION_META[action] ?? { label: action, icon: 'fa-solid fa-circle-info text-surface-500', target_type: 'unknown' };
  }

  const fieldLabel = (k: string) => ({
    status: 'Статус', password: 'Пароль', locale: 'Локаль', timezone: 'Часовой пояс',
    must_change_password: 'Смена пароля при входе', last_name: 'Фамилия', first_name: 'Имя',
    middle_name: 'Отчество', display_name: 'Отображаемое имя', value: 'Значение',
    is_primary: 'Основной', is_verified: 'Верифицирован', channel_type: 'Тип канала',
    role_id: 'Роль', position: 'Должность', department: 'Отдел', employee_number: 'Табельный',
    is_active: 'Активен', name: 'Название', description: 'Описание',
    permission_policy_ids: 'Политики доступа',
  }[k] ?? k);

  const statusLabel = (s: string) => ({
    active: 'Активен', disabled: 'Заблокирован', archived: 'В архиве',
  }[s] ?? s);

  function fmtDate(d: string): string {
    try { return new Date(d).toLocaleString('ru-RU', { day: '2-digit', month: '2-digit', year: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' }); } catch { return d; }
  }

  function changesOld(val: unknown): string {
    if (val && typeof val === 'object' && 'old' in val) return String((val as any).old ?? '');
    return String(val);
  }

  function changesNew(val: unknown): string {
    if (val && typeof val === 'object' && 'new' in val) return String((val as any).new ?? '');
    return String(val);
  }

  function hasOldNew(val: unknown): boolean {
    return val !== null && typeof val === 'object' && 'old' in val;
  }

  function formatFieldValue(field: string, value: string): string {
    if (field === 'status') return statusLabel(value);
    if (field === 'password') return '••••••••';
    return value || '—';
  }

  async function loadPage(newFilters?: AuditLogFilters) {
    loading = true;
    try {
      if (newFilters) filters = newFilters;
      page = await api.listAuditLogs(filters);
    } catch {} finally { loading = false; }
  }

  function loadNext() {
    if (page?.next_cursor) {
      loadPage({ ...filters, before: page.next_cursor });
    }
  }

  function loadPrev() {
    if (page?.prev_cursor) {
      loadPage({ ...filters, after: page.prev_cursor, before: undefined });
    }
  }

  function resetFilters() {
    filters = { limit: 50 };
    loadPage();
  }

  $effect(() => { loadPage(); });
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h2 class="text-2xl font-bold text-surface-900-100">Журнал аудита</h2>
    {#if page}
      <span class="text-xs text-surface-500-500">Всего: {page.total_count}</span>
    {/if}
  </div>

  <div class="flex items-center gap-2 text-xs text-surface-500-500">
    <button class="rounded px-2 py-1 hover:bg-surface-200-800" onclick={resetFilters}>
      <i class="fa-solid fa-rotate-right mr-1"></i>Сбросить
    </button>
  </div>

  {#if loading}
    <div class="flex items-center justify-center p-12"><div class="h-8 w-8 animate-spin rounded-full border-4 border-primary-500 border-t-transparent"></div></div>
  {:else if page && page.entries.length > 0}
    <div class="overflow-x-auto rounded-xl border border-surface-300-700">
      <table class="w-full text-left text-sm">
        <thead class="border-b border-surface-300-700 bg-surface-100-900 text-xs font-medium uppercase text-surface-500-500">
          <tr>
            <th class="px-4 py-3 w-8"></th>
            <th class="px-4 py-3">Дата</th>
            <th class="px-4 py-3">Кто</th>
            <th class="px-4 py-3">Действие</th>
            <th class="px-4 py-3">Объект</th>
            <th class="px-4 py-3">Детали</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-surface-300-700">
          {#each page.entries as entry (entry._id)}
            <tr
              class="cursor-pointer transition-colors hover:bg-surface-100-900/50"
              onclick={() => selectedEntry = entry}
            >
              <td class="px-4 py-3"><i class="{actionMeta(entry.action).icon} text-sm"></i></td>
              <td class="px-4 py-3 text-xs text-surface-600-400 whitespace-nowrap">{fmtDate(entry.occurred_at)}</td>
              <td class="px-4 py-3 text-surface-900-100 font-medium">{entry.user_login ?? entry.user_id.slice(0, 8)}</td>
              <td class="px-4 py-3 text-surface-900-100">{actionMeta(entry.action).label}</td>
              <td class="px-4 py-3 text-surface-700-300">{entry.target_login ?? entry.target_id?.slice(0, 8) ?? '—'}</td>
              <td class="px-4 py-3 text-xs text-surface-500-500">
                {#if entry.changes && Object.keys(entry.changes).length > 0}
                  {Object.keys(entry.changes).map(fieldLabel).join(', ')}
                {:else}
                  —
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    {#if page.has_more || page.prev_cursor}
      <div class="flex items-center justify-between text-xs text-surface-500-500">
        <button
          class="rounded px-3 py-1 hover:bg-surface-200-800 disabled:opacity-30"
          disabled={!page.prev_cursor}
          onclick={loadPrev}
        >
          <i class="fa-solid fa-chevron-left mr-1"></i>Назад
        </button>
        <span>Показано {page.entries.length} из {page.total_count}</span>
        <button
          class="rounded px-3 py-1 hover:bg-surface-200-800 disabled:opacity-30"
          disabled={!page.has_more}
          onclick={loadNext}
        >
          Далее<i class="fa-solid fa-chevron-right ml-1"></i>
        </button>
      </div>
    {/if}
  {:else}
    <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">Журнал пуст</div>
  {/if}
</div>

{#if selectedEntry}
  <dialog class="modal-backdrop fixed inset-0 z-50 flex items-center justify-center bg-black/50" onclick={() => selectedEntry = null}>
    <div
      class="w-full max-w-lg rounded-2xl border border-surface-300-700 bg-surface-50-950 p-6 shadow-2xl"
      onclick={(e) => e.stopPropagation()}
    >
      <div class="mb-4 flex items-center justify-between">
        <h3 class="text-lg font-bold text-surface-900-100">Детали записи</h3>
        <button onclick={() => selectedEntry = null} class="rounded p-1 text-surface-500-500 hover:bg-surface-200-800">
          <i class="fa-solid fa-xmark text-lg"></i>
        </button>
      </div>

      <div class="space-y-3 text-sm">
        <div class="flex items-center gap-2">
          <i class="{actionMeta(selectedEntry.action).icon} text-lg"></i>
          <span class="font-semibold text-surface-900-100">{actionMeta(selectedEntry.action).label}</span>
        </div>

        <div class="grid grid-cols-2 gap-3 rounded-lg border border-surface-300-700 bg-surface-100-900 p-3">
          <div>
            <span class="text-xs text-surface-500-500">Дата</span>
            <div class="text-surface-900-100">{fmtDate(selectedEntry.occurred_at)}</div>
          </div>
          <div>
            <span class="text-xs text-surface-500-500">Кто</span>
            <div class="text-surface-900-100 font-medium">{selectedEntry.user_login ?? selectedEntry.user_id.slice(0, 8)}</div>
          </div>
          <div>
            <span class="text-xs text-surface-500-500">Тип объекта</span>
            <div class="text-surface-900-100">{selectedEntry.target_type}</div>
          </div>
          <div>
            <span class="text-xs text-surface-500-500">ID объекта</span>
            <div class="font-mono text-xs text-surface-600-400">{selectedEntry.target_id ?? '—'}</div>
          </div>
          {#if selectedEntry.entity_type}
            <div>
              <span class="text-xs text-surface-500-500">Тип сущности</span>
              <div class="text-surface-900-100">{selectedEntry.entity_type}</div>
            </div>
          {/if}
          {#if selectedEntry.object_id}
            <div>
              <span class="text-xs text-surface-500-500">ID сущности</span>
              <div class="font-mono text-xs text-surface-600-400">{selectedEntry.object_id}</div>
            </div>
          {/if}
        </div>

        {#if selectedEntry.changes && Object.keys(selectedEntry.changes).length > 0}
          <div class="rounded-lg border border-surface-300-700 p-3 space-y-2">
            <span class="text-xs font-medium text-surface-500-500 uppercase">Изменения</span>
            {#each Object.entries(selectedEntry.changes) as [field, value]}
              <div class="flex items-center gap-3 rounded bg-surface-100-900 px-3 py-2">
                <span class="w-32 text-xs font-medium text-surface-700-300">{fieldLabel(field)}</span>
                {#if hasOldNew(value)}
                  <div class="flex flex-1 items-center gap-2 text-sm">
                    <span class="rounded bg-error-500/10 px-2 py-0.5 text-error-600 line-through">{formatFieldValue(field, changesOld(value))}</span>
                    <i class="fa-solid fa-arrow-right text-xs text-surface-400-600"></i>
                    <span class="rounded bg-success-500/10 px-2 py-0.5 text-success-600">{formatFieldValue(field, changesNew(value))}</span>
                  </div>
                {:else}
                  <span class="text-sm text-surface-900-100">{String(value)}</span>
                {/if}
              </div>
            {/each}
          </div>
        {:else}
          <div class="rounded-lg border border-surface-300-700 bg-surface-100-900 p-3 text-center text-surface-500-500 text-xs">
            Без изменений данных
          </div>
        {/if}
      </div>
    </div>
  </dialog>
{/if}
