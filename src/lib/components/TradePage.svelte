<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type LedgerJournalEntryTS } from '$lib/services/api';
  import { auth, hasPermission } from '$lib/stores/auth';
  import { toastSuccess } from '$lib/components/ui/toast';

  let loading = $state(true);
  let error = $state('');
  let journalEntries = $state<LedgerJournalEntryTS[]>([]);

  let periodFrom = $state('');
  let periodTo = $state('');
  let journalAccount = $state('');

  const canManage = () => $auth && hasPermission($auth.permissions, 'settings', 'manage');
  let seeded = $state(false);

  async function load() {
    loading = true; error = '';
    try {
      journalEntries = await api.ledgerJournal({
        dateFrom: periodFrom || undefined,
        dateTo: periodTo || undefined,
        accountCode: journalAccount || undefined,
      });
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка';
    } finally { loading = false; }
  }

  onMount(() => {
    if (canManage()) {
      api.listEntityTypes().then((types) => {
        seeded = types.some((t) => t.code.startsWith('TRADE_'));
        load();
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
</script>

<div class="container mx-auto p-4 space-y-4">
  <header class="flex items-center justify-between gap-3 flex-wrap">
    <h2 class="h4 flex items-center gap-2">
      <i class="fa-solid fa-cart-shopping"></i> Журнал проводок
    </h2>
    {#if !seeded && canManage()}
      <button class="btn btn-sm btn-outline" onclick={seedTrade} disabled={seeding}>
        {#if seeding}<i class="fa-solid fa-spinner fa-spin"></i>{:else}<i class="fa-solid fa-seedling"></i>{/if}
        Создать метаданные торговли
      </button>
    {/if}
  </header>

  {#if error}
    <div class="alert preset-tonal-error text-sm">{error}</div>
  {/if}

  <div class="card p-3 flex gap-3 items-end flex-wrap">
    <div>
      <label class="label text-xs">С даты</label>
      <input class="input input-sm" type="date" bind:value={periodFrom} />
    </div>
    <div>
      <label class="label text-xs">По дату</label>
      <input class="input input-sm" type="date" bind:value={periodTo} />
    </div>
    <div>
      <label class="label text-xs">Счёт</label>
      <input class="input input-sm w-24" bind:value={journalAccount} placeholder="41,60…" />
    </div>
    <button class="btn btn-sm btn-primary" onclick={load}>
      <i class="fa-solid fa-magnifying-glass"></i>
    </button>
  </div>

  {#if loading}
    <div class="p-8 text-center text-surface-500">
      <i class="fa-solid fa-spinner fa-spin"></i>
    </div>
  {:else}
    <div class="overflow-x-auto">
      <table class="table table-sm text-xs">
        <thead>
          <tr>
            <th>Дата</th>
            <th>Документ</th>
            <th>Дт</th>
            <th>Кт</th>
            <th class="text-right">Сумма</th>
            <th>Описание</th>
          </tr>
        </thead>
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
            <tr>
              <td colspan="6" class="text-center text-surface-400 py-4">Проводок нет</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
