<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type EntityType, type ObjectEntity, OBJECT_STATE_META, FIELD_KIND_META } from '$lib/services/api';

  let entityTypes = $state<EntityType[]>([]);
  let loading = $state(true);
  let error = $state('');
  let selectedType = $state<string>('');
  let stats = $state<Record<string, number>>({});
  let allObjects = $state<ObjectEntity[]>([]);
  let totalObjects = $state(0);

  type StateCount = { state: string; count: number; label: string; color: string };
  let stateBreakdown = $state<StateCount[]>([]);
  type TypeCount = { type: string; name: string; count: number };
  let typeBreakdown = $state<TypeCount[]>([]);
  type RecentObject = { id: string; number: string | null; state: string; typeName: string; updatedAt: string; data: Record<string, unknown> };
  let recentObjects = $state<RecentObject[]>([]);

  async function loadData() {
    loading = true;
    error = '';
    try {
      entityTypes = await api.listEntityTypes();
      const page = await api.listObjects({ limit: 1000 });
      allObjects = page.objects;
      totalObjects = page.total_count;

      const sc: Record<string, number> = {};
      const tc: Record<string, number> = {};
      for (const obj of allObjects) {
        sc[obj.state] = (sc[obj.state] ?? 0) + 1;
        tc[obj.entity_type_id] = (tc[obj.entity_type_id] ?? 0) + 1;
      }
      stats = sc;

      stateBreakdown = Object.entries(sc).map(([state, count]) => ({
        state,
        count,
        label: OBJECT_STATE_META[state as keyof typeof OBJECT_STATE_META]?.label ?? state,
        color: OBJECT_STATE_META[state as keyof typeof OBJECT_STATE_META]?.color === 'error' ? 'text-error-500'
          : OBJECT_STATE_META[state as keyof typeof OBJECT_STATE_META]?.color === 'success' ? 'text-success-500'
          : OBJECT_STATE_META[state as keyof typeof OBJECT_STATE_META]?.color === 'warning' ? 'text-warning-500'
          : 'text-surface-500',
      })).sort((a, b) => b.count - a.count);

      typeBreakdown = Object.entries(tc).map(([typeId, count]) => ({
        type: typeId,
        name: entityTypes.find(t => t._id === typeId)?.name ?? typeId,
        count,
      })).sort((a, b) => b.count - a.count);

      const sorted = [...allObjects].sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime());
      recentObjects = sorted.slice(0, 10).map(obj => ({
        id: obj._id,
        number: obj.number,
        state: obj.state,
        typeName: entityTypes.find(t => t._id === obj.entity_type_id)?.name ?? obj.entity_type_id,
        updatedAt: obj.updated_at,
        data: obj.data,
      }));
    } catch (e: any) {
      error = e?.toString() ?? 'Ошибка загрузки';
    } finally { loading = false; }
  }

  function fmtDate(iso: string) {
    try { return new Date(iso).toLocaleString('ru-RU', { day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit' }); }
    catch { return iso; }
  }

  function maxBar(): number {
    const counts = typeBreakdown.map(t => t.count);
    return counts.length > 0 ? Math.max(...counts) : 1;
  }

  onMount(loadData);
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h2 class="text-2xl font-bold text-surface-900-100">Отчёты</h2>
    <button class="btn btn-sm preset-tonal text-xs" onclick={loadData} disabled={loading}>
      <i class="fa-solid fa-rotate-right mr-1"></i>{loading ? '...' : 'Обновить'}
    </button>
  </div>

  {#if error}
    <div class="alert preset-tonal-error text-sm">{error}</div>
  {/if}

  {#if loading}
    <div class="text-center py-12 text-surface-500 text-sm"><i class="fa-solid fa-spinner fa-spin mr-1"></i>Загрузка...</div>
  {:else}
    <!-- Summary cards -->
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
      <div class="card p-5">
        <div class="text-sm font-medium text-surface-500">Всего объектов</div>
        <div class="mt-1 text-3xl font-bold text-surface-900-100">{totalObjects}</div>
      </div>
      <div class="card p-5">
        <div class="text-sm font-medium text-surface-500">Типов объектов</div>
        <div class="mt-1 text-3xl font-bold text-surface-900-100">{typeBreakdown.length}</div>
      </div>
      <div class="card p-5">
        <div class="text-sm font-medium text-surface-500">Черновиков</div>
        <div class="mt-1 text-3xl font-bold text-warning-500">{stats['draft'] ?? 0}</div>
      </div>
      <div class="card p-5">
        <div class="text-sm font-medium text-surface-500">Проведено</div>
        <div class="mt-1 text-3xl font-bold text-success-500">{stats['posted'] ?? 0}</div>
      </div>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- By state -->
      <div class="card p-5">
        <h3 class="text-sm font-semibold text-surface-500 uppercase tracking-wider mb-4">По состояниям</h3>
        {#if stateBreakdown.length === 0}
          <div class="text-sm text-surface-400">Нет данных</div>
        {:else}
          <div class="space-y-2">
            {#each stateBreakdown as sc}
              <div class="flex items-center gap-3">
                <span class="w-24 text-xs text-surface-600-400 shrink-0">{sc.label}</span>
                <div class="flex-1 bg-surface-100-900 rounded-full h-5 overflow-hidden">
                  <div class="h-full rounded-full bg-primary-500/30" style="width: {totalObjects > 0 ? (sc.count / totalObjects * 100) : 0}%"></div>
                </div>
                <span class="w-12 text-right text-xs font-medium {sc.color}">{sc.count}</span>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- By type -->
      <div class="card p-5">
        <h3 class="text-sm font-semibold text-surface-500 uppercase tracking-wider mb-4">По типам объектов</h3>
        {#if typeBreakdown.length === 0}
          <div class="text-sm text-surface-400">Нет данных</div>
        {:else}
          <div class="space-y-2">
            {#each typeBreakdown as tc}
              <div class="flex items-center gap-3">
                <span class="w-36 text-xs text-surface-600-400 truncate shrink-0" title={tc.name}>{tc.name}</span>
                <div class="flex-1 bg-surface-100-900 rounded-full h-5 overflow-hidden">
                  <div class="h-full rounded-full bg-primary-500/50" style="width: {(tc.count / maxBar()) * 100}%"></div>
                </div>
                <span class="w-12 text-right text-xs font-medium text-surface-900-100">{tc.count}</span>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <!-- Recent objects -->
    <div class="card p-5">
      <h3 class="text-sm font-semibold text-surface-500 uppercase tracking-wider mb-4">Последние объекты</h3>
      {#if recentObjects.length === 0}
        <div class="text-sm text-surface-400">Нет объектов</div>
      {:else}
        <div class="overflow-x-auto">
          <table class="w-full text-left text-xs">
            <thead class="border-b border-surface-300-700">
              <tr>
                <th class="pb-2 text-surface-500 font-medium">Номер</th>
                <th class="pb-2 text-surface-500 font-medium">Тип</th>
                <th class="pb-2 text-surface-500 font-medium">Состояние</th>
                <th class="pb-2 text-surface-500 font-medium">Название</th>
                <th class="pb-2 text-surface-500 font-medium text-right">Обновлено</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-surface-300-700/50">
              {#each recentObjects as obj}
                <tr class="hover:bg-surface-100-900/50">
                  <td class="py-1.5 font-mono text-surface-900-100">{obj.number ?? '—'}</td>
                  <td class="py-1.5 text-surface-600-400">{obj.typeName}</td>
                  <td class="py-1.5">
                    {@const meta = OBJECT_STATE_META[obj.state as keyof typeof OBJECT_STATE_META]}
                    <span class="rounded-full px-1.5 py-0.5 text-[10px] font-medium preset-tonal-{meta?.color ?? 'primary'}">
                      {meta?.label ?? obj.state}
                    </span>
                  </td>
                  <td class="py-1.5 text-surface-600-400 max-w-[200px] truncate">{obj.data?.name ?? obj.data?.title ?? '—'}</td>
                  <td class="py-1.5 text-surface-500 text-right">{fmtDate(obj.updatedAt)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  {/if}
</div>
