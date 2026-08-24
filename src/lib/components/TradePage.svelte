// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type LedgerOsvRowTS, type LedgerJournalEntryTS } from '$lib/services/api';
  import { auth, hasPermission } from '$lib/stores/auth';

  let loading = $state(true);
  let error = $state('');
  let tab = $state<'osv' | 'journal'>('osv');

  let osvRows = $state<LedgerOsvRowTS[]>([]);
  let journalEntries = $state<LedgerJournalEntryTS[]>([]);

  let periodFrom = $state('');
  let periodTo = $state('');
  let journalAccount = $state('');

  const canManage = () => $auth && hasPermission($auth.permissions, 'settings', 'manage');
  let seeded = $state(false);

  async function load() {
    loading = true; error = '';
    try {
      if (tab === 'osv') {
        osvRows = (await api.ledgerOsv(periodFrom || undefined, periodTo || undefined)).rows;
      } else {
        journalEntries = await api.ledgerJournal({
          dateFrom: periodFrom || undefined,
          dateTo: periodTo || undefined,
          accountCode: journalAccount || undefined,
        });
      }
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка';
    } finally { loading = false; }
  }

  onMount(() => {
    if (canManage()) {
      // Кнопку seed показываем только если метаданных ещё нет
      api.listEntityTypes().then((types) => {
        seeded = types.some((t) => t.code.startsWith('TRADE_'));
        if (!seeded) load();
      }).catch(() => load());
    } else {
      load();
    }
  });

  function fmtAmount(v: number): string {
    return new Intl.NumberFormat('ru-RU', { minimumFractionDigits: 2 }).format(v / 100);
  }

  let seeding = $state(false);
  async function seedTrade() {
    seeding = true; error = '';
    try {
      toastSuccess(await api.tradeSeedMetadata());
      seeded = true;
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка seed';
    } finally {
      seeding = false;
    }
  }
  import { toastSuccess, toastError, errText } from '$lib/components/ui/toast';
</script>

<div class="container mx-auto p-4 space-y-4">
  <header class="flex items-center justify-between gap-3 flex-wrap">
    <h2 class="h4 flex items-center gap-2"><i class="fa-solid fa-cart-shopping"></i> Торговля</h2>
    {#if !seeded && canManage()}
      <button class="btn btn-sm btn-outline" onclick={seedTrade} disabled={seeding}>
        {#if seeding}<i class="fa-solid fa-spinner fa-spin"></i>{:else}<i class="fa-solid fa-seedling"></i>{/if}
        Создать метаданные торговли
      </button>
    {/if}
  </header>

  {#if error}<div class="alert alert-error">{error}</div>{/if}

  <!-- Фильтр периода -->
  <div class="card p-3 flex gap-3 items-end flex-wrap">
    <div><label class="label text-xs">С даты</label>
      <input class="input input-sm" type="date" bind:value={periodFrom} /></div>
    <div><label class="label text-xs">По дату</label>
      <input class="input input-sm" type="date" bind:value={periodTo} /></div>
    {#if tab === 'journal'}
      <div><label class="label text-xs">Счёт</label>
        <input class="input input-sm w-24" bind:value={journalAccount} placeholder="41,60…" /></div>
    {/if}
    <button class="btn btn-sm btn-primary" onclick={load}><i class="fa-solid fa-magnifying-glass"></i></button>
  </div>

  <div class="flex gap-1 border-b border-surface-200">
    {#each [['osv','ОСВ'],['journal','Журнал проводок']] as [k,label]}
      <button class="btn btn-sm {tab===k?'variant-filled-primary':'btn-transparent'} rounded-b-none"
        onclick={()=>{tab=k as typeof tab; load();}}>{label}</button>
    {/each}
  </div>

  {#if loading}
    <div class="p-8 text-center text-surface-500"><i class="fa-solid fa-spinner fa-spin"></i></div>
  {:else if tab === 'osv'}
    <table class="table table-sm">
      <thead><tr><th>Код</th><th>Название</th><th>Тип</th>
        <th class="text-right">Оборот Дт</th><th class="text-right">Оборот Кт</th>
        <th class="text-right">Сальдо</th></tr></thead>
      <tbody>
        {#each osvRows as row}
          <tr>
            <td class="font-mono font-medium">{row.code}</td>
            <td>{row.name}</td>
            <td class="text-xs text-surface-400">{row.type}</td>
            <td class="text-right">{fmtAmount(row.debit_turnover)}</td>
            <td class="text-right">{fmtAmount(row.credit_turnover)}</td>
            <td class="text-right font-bold {row.balance < 0 ? 'text-error-600' : ''}">
              {fmtAmount(Math.abs(row.balance))}
              {#if row.balance !== 0}<span class="text-xs text-surface-400 ml-1">
                {row.balance > 0 ? 'Дт' : 'Кт'}
              </span>{/if}
            </td>
          </tr>
        {:else}
          <tr><td colspan="6" class="text-center text-surface-400 py-4">Нет данных за период</td></tr>
        {/each}
      </tbody>
    </table>

  {:else if tab === 'journal'}
    <table class="table table-sm">
      <thead><tr><th>Дата</th><th>Документ</th><th>Дт</th><th>Кт</th>
        <th class="text-right">Сумма</th><th>Описание</th></tr></thead>
      <tbody>
        {#each journalEntries as e}
          <tr class="{e.is_reversal ? 'bg-warning-50 dark:bg-warning-900/20' : ''}">
            <td class="font-mono text-xs">{e.date}</td>
            <td class="font-mono text-xs truncate max-w-32">{e.doc_id?.slice(0,8) ?? '—'}</td>
            <td class="font-mono font-medium">{e.debit_code}</td>
            <td class="font-mono">{e.credit_code}</td>
            <td class="text-right font-medium">{fmtAmount(e.amount)}</td>
            <td class="text-xs text-surface-500 truncate max-w-48">{e.description ?? ''}</td>
          </tr>
        {:else}
          <tr><td colspan="6" class="text-center text-surface-400 py-4">Проводок нет</td></tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>
