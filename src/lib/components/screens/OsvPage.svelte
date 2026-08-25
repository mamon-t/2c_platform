<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type LedgerOsvRowTS } from '$lib/services/api';

  let loading = $state(true);
  let error = $state('');
  let osvRows = $state<LedgerOsvRowTS[]>([]);

  let periodFrom = $state('');
  let periodTo = $state('');

  async function load() {
    loading = true; error = '';
    try {
      osvRows = (await api.ledgerOsv(periodFrom || undefined, periodTo || undefined)).rows;
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка';
    } finally { loading = false; }
  }

  onMount(load);

  function fmtAmount(v: number): string {
    return new Intl.NumberFormat('ru-RU', { minimumFractionDigits: 2 }).format(v / 100);
  }

  function typeLabel(t: string): string {
    const m: Record<string, string> = { asset: 'Актив', liability: 'Пассив', equity: 'Капитал', revenue: 'Доход', expense: 'Расход', off_balance: 'Забаланс.' };
    return m[t] ?? t;
  }
</script>

<div class="container mx-auto p-4 space-y-4">
  <h2 class="h4 flex items-center gap-2">
    <i class="fa-solid fa-scale-unbalanced"></i> Оборотно-сальдовая ведомость
  </h2>

  {#if error}
    <div class="alert preset-tonal-error text-sm">{error}</div>
  {/if}

  <div class="card p-3 flex gap-3 items-end flex-wrap">
    <div>
      <label class="label text-xs">С периода</label>
      <input class="input input-sm" type="month" bind:value={periodFrom} />
    </div>
    <div>
      <label class="label text-xs">По период</label>
      <input class="input input-sm" type="month" bind:value={periodTo} />
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
            <th>Код</th>
            <th>Название</th>
            <th class="text-center">Тип</th>
            <th class="text-right">Вх. сальдо</th>
            <th class="text-right">Оборот Дт</th>
            <th class="text-right">Оборот Кт</th>
            <th class="text-right">Исх. сальдо</th>
          </tr>
        </thead>
        <tbody>
          {#each osvRows as row}
            {@const closing = row.closing_balance}
            <tr>
              <td class="font-mono font-medium">{row.code}</td>
              <td>{row.name}</td>
              <td class="text-center text-surface-400">{typeLabel(row.type)}</td>
              <td class="text-right">{fmtAmount(row.opening_balance)}</td>
              <td class="text-right">{fmtAmount(row.debit_turnover)}</td>
              <td class="text-right">{fmtAmount(row.credit_turnover)}</td>
              <td class="text-right font-bold {closing < 0 ? 'text-red-500' : ''}">
                {fmtAmount(Math.abs(closing))}
                {#if closing !== 0}
                  <span class="text-surface-400 ml-1">{closing > 0 ? 'Дт' : 'Кт'}</span>
                {/if}
              </td>
            </tr>
          {:else}
            <tr>
              <td colspan="7" class="text-center text-surface-400 py-4">Нет данных за период</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
