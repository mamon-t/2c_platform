// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import { onMount } from 'svelte';
  import { api,
    type StockBalanceTS, type StockHandoverItemTS,
  } from '$lib/services/api';
  import { auth, hasPermission } from '$lib/stores/auth';

  let loading = $state(true);
  let error = $state('');
  let notice = $state('');
  let tab = $state<'balances' | 'handover' | 'overdue'>('balances');

  let balances = $state<StockBalanceTS[]>([]);
  let handover = $state<StockHandoverItemTS[]>([]);
  let overdue = $state<StockHandoverItemTS[]>([]);

  let filterLocation = $state('');

  const canManage = () => $auth && hasPermission($auth.permissions, 'settings', 'manage');
  let seeded = $state(false);

  async function load() {
    loading = true;
    error = '';
    try {
      balances = (await api.stockBalances(filterLocation || undefined)).balances;
      if ($auth && hasPermission($auth.permissions, 'stock', 'read')) {
        const h = await api.stockReportHandover();
        handover = h.items;
        overdue = (await api.stockReportOverdue()).items;
      }
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка загрузки';
    } finally {
      loading = false;
    }
  }

  onMount(() => { load(); });

  async function seed() {
    try {
      notice = await api.stockSeedMetadata();
      seeded = true;
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка seed';
    }
  }

  function fmtQty(q: number): string {
    return String(Math.round(q * 1000) / 1000);
  }

  function fmtMs(ms?: number): string {
    if (!ms) return '—';
    return new Date(ms).toLocaleString('ru-RU');
  }

  // Уникальные локации для фильтра
  let locations = $derived(
    [...new Set(balances.map(b => b.location_id))].sort()
  );
</script>

<div class="container mx-auto p-4 space-y-4">
  <header class="flex items-center justify-between gap-3 flex-wrap">
    <h2 class="h4 flex items-center gap-2"><i class="fa-solid fa-boxes-stacked"></i> Склад</h2>
    <div class="flex gap-2">
      <button class="btn btn-sm btn-outline" onclick={load}><i class="fa-solid fa-rotate"></i></button>
    </div>
  </header>

  {#if error}<div class="alert alert-error whitespace-pre-line">{error}</div>{/if}
  {#if notice}<div class="alert alert-success text-xs">{notice}</div>{/if}

  <!-- Первый запуск -->
  {#if !seeded && canManage() && balances.length === 0 && !loading}
    <div class="card p-4 text-sm space-y-2">
      <p>Похоже, справочники склада ещё не созданы. Создать метаданные
        (номенклатура, места учёта, документы перемещения/инвентаризации/выдачи)?
      </p>
      <button class="btn btn-sm btn-primary" onclick={seed}>
        <i class="fa-solid fa-seedling"></i> Создать метаданные склада
      </button>
      <p class="text-xs text-surface-400">Справочники и документы затем заполняются через разделы «Справочники» и «Документы».</p>
    </div>
  {/if}

  <div class="flex gap-1 border-b border-surface-200">
    {#each [['balances','Остатки'],['handover','Подотчёт'],['overdue','Просрочки']] as [k,label]}
      <button class="btn btn-sm {tab===k?'variant-filled-primary':'btn-transparent'} rounded-b-none"
        onclick={()=>{
          tab = k as typeof tab;
          if (k!=='balances') load();
        }}>{label}</button>
    {/each}
  </div>

  {#if loading}
    <div class="p-8 text-center text-surface-500"><i class="fa-solid fa-spinner fa-spin"></i> Загрузка…</div>
  {:else if tab === 'balances'}
    <div class="card p-3 flex gap-2 items-end flex-wrap">
      <div>
        <label class="label text-xs">Место учёта</label>
        <select class="select select-sm max-w-xs" bind:value={filterLocation}>
          <option value="">— все —</option>
          {#each locations as l}<option value={l}>{l}</option>{/each}
        </select>
      </div>
      <button class="btn btn-sm" onclick={load}>Применить</button>
    </div>

    <table class="table table-sm">
      <thead><tr><th>Место учёта</th><th>Номенклатура</th><th class="text-right">Остаток</th></tr></thead>
      <tbody>
        {#each balances.filter(b=>!filterLocation || b.location_id===filterLocation) as b}
          <tr>
            <td class="font-mono text-xs">{b.location_id}</td>
            <td class="font-mono text-xs">{b.nomenclature_id}</td>
            <td class="text-right font-medium {b.quantity < 0 ? 'text-error-600' : ''}">{fmtQty(b.quantity)}</td>
          </tr>
        {:else}
          <tr><td colspan="3" class="text-center text-surface-400 py-4">Остатков нет</td></tr>
        {/each}
      </tbody>
    </table>

  {:else if tab === 'handover'}
    <table class="table table-sm">
      <thead><tr>
        <th>У кого</th><th>Что</th><th>Кол-во</th><th>Ответственный</th><th>Вернуть до</th><th>Выдано</th>
      </tr></thead>
      <tbody>
        {#each handover as i}
          <tr>
            <td class="font-mono text-xs">{i.custodian_name ?? i.location_id}</td>
            <td class="font-mono text-xs">{i.nomenclature_id}</td>
            <td>{fmtQty(i.qty_on_hand)}</td>
            <td class="font-mono text-xs">{i.responsible_user_id || '—'}</td>
            <td class="{(!i.expected_return_date || i.expected_return_date >= new Date().toISOString().slice(0,10)) ? '' : 'text-error-600 font-medium'}">
              {i.expected_return_date || '—'}
            </td>
            <td class="text-xs">{fmtMs(i.issued_at_ms)}</td>
          </tr>
        {:else}
          <tr><td colspan="6" class="text-center text-surface-400 py-4">На руках ничего нет</td></tr>
        {/each}
      </tbody>
    </table>

  {:else if tab === 'overdue'}
    <div class="space-y-2">
      {#each overdue as i}
        <div class="alert alert-error flex items-center justify-between">
          <span><i class="fa-solid fa-triangle-exclamation"></i>
            {i.custodian_name ?? i.location_id}: {i.nomenclature_id} × {fmtQty(i.qty_on_hand)}</span>
          <span class="text-xs">срок истёк {i.expected_return_date}</span>
        </div>
      {:else}
        <div class="p-8 text-center text-success-600">
          <i class="fa-solid fa-circle-check"></i> Просроченных возвратов нет
        </div>
      {/each}
    </div>
  {/if}
</div>
