<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type LedgerOpeningBalanceTS } from '$lib/services/api';
  import { toastSuccess, toastError } from '$lib/components/ui/toast';

  let loading = $state(true);
  let saving = $state(false);
  let error = $state('');

  let periods = $state<Array<{ period_key: string; year: number; month: number; opened: boolean; closed: boolean }>>([]);
  let selectedPeriod = $state('');
  let balances = $state<Array<{ account_id: string; account_code: string; account_type: string; account_name: string; opening_balance: number; debit_turnover: number; credit_turnover: number }>>([]);
  let edited = $state<Record<string, number>>({});

  function currentPeriodKey(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`;
  }

  function periodLabel(pk: string): string {
    const [y, m] = pk.split('-');
    const months = ['Январь','Февраль','Март','Апрель','Май','Июнь','Июль','Август','Сентябрь','Октябрь','Ноябрь','Декабрь'];
    return `${months[parseInt(m, 10) - 1]} ${y}`;
  }

  function formatMoney(v: number): string {
    return new Intl.NumberFormat('ru-RU', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(v / 100);
  }

  function accountTypeLabel(t: string): string {
    const map: Record<string, string> = { asset: 'А', liability: 'П', equity: 'К', revenue: 'Д', expense: 'Р', off_balance: 'З' };
    return map[t] ?? t;
  }

  async function loadPeriods() {
    try {
      periods = (await api.ledgerPeriodsList()) as any[];
      if (!selectedPeriod) {
        const opened = periods.filter(p => p.opened && !p.closed);
        selectedPeriod = opened.length > 0 ? opened[0].period_key : currentPeriodKey();
      }
    } catch (e: any) {
      error = String(e);
    }
  }

  async function loadBalances() {
    if (!selectedPeriod) return;
    loading = true; error = '';
    try {
      const [accounts, existing] = await Promise.all([
        api.ledgerAccountsList(),
        api.ledgerGetOpeningBalances(selectedPeriod),
      ]);
      const accList = accounts as Array<{ _id: string; code: string; name: string; account_type: string; is_active: boolean }>;
      const existingMap = new Map(existing.map(b => [b.account_code, b]));

      balances = accList.filter(a => a.is_active).map(a => ({
        account_id: a._id,
        account_code: a.code,
        account_type: a.account_type,
        account_name: a.name,
        opening_balance: existingMap.get(a.code)?.opening_balance ?? 0,
        debit_turnover: existingMap.get(a.code)?.debit_turnover ?? 0,
        credit_turnover: existingMap.get(a.code)?.credit_turnover ?? 0,
      }));
      edited = {};
    } catch (e: any) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function onEdit(code: string, raw: string) {
    // Пользователь вводит рубли, храним копейки
    const kopecks = Math.round(parseFloat(raw.replace(',', '.').replace(/\s/g, '')) * 100) || 0;
    edited = { ...edited, [code]: kopecks };
  }

  async function handleSave() {
    if (Object.keys(edited).length === 0) return;
    saving = true;
    try {
      const items = Object.entries(edited).map(([code, balance]) => ({
        account_code: code,
        opening_balance: balance,
      }));
      await api.ledgerSaveOpeningBalances(selectedPeriod, items);
      toastSuccess(`Сохранено: ${items.length} счетов`);
      edited = {};
      await loadBalances();
    } catch (e: any) {
      toastError(String(e));
    } finally {
      saving = false;
    }
  }

  onMount(async () => {
    try {
      await loadPeriods();
      await loadBalances();
    } catch {
      // loadPeriods/loadBalances handle their own errors
    } finally {
      loading = false;
    }
  });
</script>

<div class="flex flex-col h-full">
  <div class="flex items-center gap-3 p-3 border-b border-surface-300-700">
    <h2 class="h3 text-sm">
      <i class="fa-solid fa-scale-unbalanced mr-1"></i>Входящие сальдо
    </h2>
    <select class="select select-sm w-56" bind:value={selectedPeriod} onchange={() => loadBalances()}>
      {#each periods as p}
        <option value={p.period_key} disabled={p.closed}>
          {periodLabel(p.period_key)}{p.closed ? ' (закрыт)' : ''}
        </option>
      {/each}
    </select>
    {#if Object.keys(edited).length > 0}
      <button class="btn btn-sm preset-filled-primary text-xs" disabled={saving} onclick={handleSave}>
        <i class="fa-solid fa-floppy-disk mr-1"></i>Сохранить ({Object.keys(edited).length})
      </button>
    {/if}
  </div>

  {#if error}
    <div class="alert preset-tonal-error mx-3 mt-2 text-sm">{error}</div>
  {/if}

  <div class="flex-1 overflow-y-auto">
    {#if loading}
      <div class="text-center py-8 text-surface-500 text-sm">
        <i class="fa-solid fa-spinner fa-spin mr-1"></i>Загрузка...
      </div>
    {:else}
      <table class="table table-sm text-xs">
        <thead>
          <tr>
            <th class="w-16">Код</th>
            <th>Название</th>
            <th class="w-8 text-center">Тип</th>
            <th class="w-44 text-right">Входящее сальдо</th>
            <th class="w-36 text-right text-surface-400">Оборот Дт</th>
            <th class="w-36 text-right text-surface-400">Оборот Кт</th>
          </tr>
        </thead>
        <tbody>
          {#each balances as b (b.account_code)}
            {@const isEdited = b.account_code in edited}
            <tr class={isEdited ? 'preset-tonal-primary' : ''}>
              <td class="font-mono font-medium">{b.account_code}</td>
              <td>{b.account_name}</td>
              <td class="text-center text-surface-400">{accountTypeLabel(b.account_type)}</td>
              <td class="text-right">
                <input
                  type="text"
                  class="input input-sm w-full text-right text-xs"
                  class:bg-primary-50-950={isEdited}
                  value={isEdited ? formatMoney(edited[b.account_code]) : formatMoney(b.opening_balance)}
                  onblur={(e) => onEdit(b.account_code, (e.target as HTMLInputElement).value)}
                  onkeydown={(e) => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); }}
                />
              </td>
              <td class="text-right text-surface-400">{formatMoney(b.debit_turnover)}</td>
              <td class="text-right text-surface-400">{formatMoney(b.credit_turnover)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>
