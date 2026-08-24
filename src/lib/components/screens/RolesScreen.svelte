// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { api, type Role } from '$lib/services/api';
  import { auth } from '$lib/stores/auth';
  import { confirmDialog } from '$lib/components/ui/dialog';
  import { toastSuccess, toastError, errText } from '$lib/components/ui/toast';
  import PageToolbar from '$lib/components/ui/PageToolbar.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';

  let roles = $state<Role[]>([]);
  let loading = $state(true);

  let showForm = $state(false);
  let form = $state({ code: '', name: '', description: '' });
  let formError = $state('');

  async function load() {
    const companyId = get(auth)?.companyId;
    if (!companyId) { loading = false; return; }
    loading = true;
    try { roles = await api.listRoles(companyId); }
    catch (e) { toastError(errText(e)); }
    finally { loading = false; }
  }
  onMount(load);

  function openForm() {
    form = { code: '', name: '', description: '' };
    formError = '';
    showForm = true;
  }

  async function save() {
    formError = '';
    const companyId = get(auth)?.companyId;
    if (!companyId) { formError = 'Не выбрана компания'; return; }
    try {
      await api.createRole({ company_id: companyId, code: form.code, name: form.name, description: form.description || undefined });
      showForm = false;
      toastSuccess('Роль создана');
      await load();
    } catch (e) { formError = errText(e); }
  }

  async function remove(id: string) {
    if (!(await confirmDialog({ title: 'Удалить роль?', danger: true }))) return;
    try { await api.deleteRole(id); toastSuccess('Роль удалена'); await load(); }
    catch (e) { toastError(errText(e)); }
  }
</script>

<div class="space-y-3">
  <PageToolbar title="Роли" icon="fa-solid fa-user-shield">
    <button onclick={openForm} class="btn btn-sm preset-filled-primary-500"><i class="fa-solid fa-plus"></i> Добавить</button>
  </PageToolbar>

  {#if loading}
    <Spinner />
  {:else}
    {#if showForm}
      <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-4">
        <h3 class="mb-3 text-sm font-semibold text-surface-900-100">Новая роль</h3>
        <form onsubmit={(e) => { e.preventDefault(); save(); }} class="space-y-3">
          <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
            <label class="block text-xs font-medium text-surface-700-300">Код *
              <input bind:value={form.code} class="input mt-1 w-full" required /></label>
            <label class="block text-xs font-medium text-surface-700-300">Название *
              <input bind:value={form.name} class="input mt-1 w-full" required /></label>
            <label class="block text-xs font-medium text-surface-700-300">Описание
              <input bind:value={form.description} class="input mt-1 w-full" /></label>
          </div>
          {#if formError}<div class="text-sm text-error-600" role="alert">{formError}</div>{/if}
          <div class="flex gap-2">
            <button type="submit" class="btn btn-sm preset-filled-primary-500">Создать</button>
            <button type="button" onclick={() => (showForm = false)} class="btn btn-sm btn-outline">Отмена</button>
          </div>
        </form>
      </div>
    {/if}

    <div class="overflow-x-auto rounded-lg border border-surface-300-700">
      <table class="table table-dense w-full text-left">
        <thead><tr><th>Код</th><th>Название</th><th>Описание</th><th class="text-right">Действия</th></tr></thead>
        <tbody>
          {#each roles as role (role._id)}
            <tr>
              <td class="font-mono text-xs">{role.code}</td>
              <td>{role.name}</td>
              <td class="text-surface-600-400">{role.description ?? '—'}</td>
              <td class="text-right">
                <button onclick={() => remove(role._id)} class="rounded p-1.5 text-error-600 hover:bg-error-500/10" title="Удалить" aria-label={`Удалить роль ${role.name}`}>
                  <i class="fa-solid fa-trash text-xs"></i>
                </button>
              </td>
            </tr>
          {:else}
            <tr><td colspan="4" class="py-6 text-center text-surface-400">Нет ролей</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
