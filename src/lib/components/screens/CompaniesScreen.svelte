<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Company } from '$lib/services/api';
  import { confirmDialog } from '$lib/components/ui/dialog';
  import { toastSuccess, toastError, errText } from '$lib/components/ui/toast';
  import PageToolbar from '$lib/components/ui/PageToolbar.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';

  let companies = $state<Company[]>([]);
  let loading = $state(true);

  let showForm = $state(false);
  let editingId = $state<string | null>(null);
  let form = $state({ code: '', name: '', inn: '' });
  let formError = $state('');

  async function load() {
    loading = true;
    try { companies = await api.listCompanies(); }
    catch (e) { toastError(errText(e)); }
    finally { loading = false; }
  }
  onMount(load);

  function openForm(company?: Company) {
    editingId = company?._id ?? null;
    form = { code: company?.code ?? '', name: company?.name ?? '', inn: company?.inn ?? '' };
    formError = '';
    showForm = true;
  }

  async function save() {
    formError = '';
    try {
      if (editingId) await api.updateCompany(editingId, { name: form.name, inn: form.inn || undefined });
      else await api.createCompany({ code: form.code, name: form.name, inn: form.inn || undefined });
      showForm = false;
      toastSuccess(editingId ? 'Компания обновлена' : 'Компания создана');
      await load();
    } catch (e) { formError = errText(e); }
  }

  async function remove(id: string) {
    if (!(await confirmDialog({ title: 'Удалить компанию?', danger: true }))) return;
    try { await api.deleteCompany(id); toastSuccess('Компания удалена'); await load(); }
    catch (e) { toastError(errText(e)); }
  }
</script>

<div class="space-y-3">
  <PageToolbar title="Компании" icon="fa-solid fa-building">
    <button onclick={() => openForm()} class="btn btn-sm preset-filled-primary-500"><i class="fa-solid fa-plus"></i> Добавить</button>
  </PageToolbar>

  {#if loading}
    <Spinner />
  {:else}
    {#if showForm}
      <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-4">
        <h3 class="mb-3 text-sm font-semibold text-surface-900-100">{editingId ? 'Редактировать' : 'Новая компания'}</h3>
        <form onsubmit={(e) => { e.preventDefault(); save(); }} class="space-y-3">
          <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
            <label class="block text-xs font-medium text-surface-700-300">Код *
              <input bind:value={form.code} class="input mt-1 w-full" required disabled={!!editingId} /></label>
            <label class="block text-xs font-medium text-surface-700-300">Название *
              <input bind:value={form.name} class="input mt-1 w-full" required /></label>
            <label class="block text-xs font-medium text-surface-700-300">ИНН
              <input bind:value={form.inn} class="input mt-1 w-full" /></label>
          </div>
          {#if formError}<div class="text-sm text-error-600" role="alert">{formError}</div>{/if}
          <div class="flex gap-2">
            <button type="submit" class="btn btn-sm preset-filled-primary-500">Сохранить</button>
            <button type="button" onclick={() => (showForm = false)} class="btn btn-sm btn-outline">Отмена</button>
          </div>
        </form>
      </div>
    {/if}

    <div class="overflow-x-auto rounded-lg border border-surface-300-700">
      <table class="table table-dense w-full text-left">
        <thead><tr><th>Код</th><th>Название</th><th>ИНН</th><th>Статус</th><th class="text-right">Действия</th></tr></thead>
        <tbody>
          {#each companies as company (company._id)}
            <tr>
              <td class="font-mono text-xs">{company.code}</td>
              <td>{company.name}</td>
              <td class="text-surface-600-400">{company.inn ?? '—'}</td>
              <td><span class="badge {company.active ? 'preset-tonal-success' : 'preset-tonal-error'}">{company.active ? 'Активна' : 'Неактивна'}</span></td>
              <td class="text-right">
                <button onclick={() => openForm(company)} class="mr-1 rounded p-1.5 text-primary-600 hover:bg-primary-500/10" title="Редактировать" aria-label={`Редактировать ${company.name}`}>
                  <i class="fa-solid fa-pen text-xs"></i>
                </button>
                <button onclick={() => remove(company._id)} class="rounded p-1.5 text-error-600 hover:bg-error-500/10" title="Удалить" aria-label={`Удалить ${company.name}`}>
                  <i class="fa-solid fa-trash text-xs"></i>
                </button>
              </td>
            </tr>
          {:else}
            <tr><td colspan="5" class="py-6 text-center text-surface-400">Нет компаний</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
