<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Company, type CompanyInput } from '$lib/services/api';
  import { confirmDialog } from '$lib/components/ui/dialog';
  import { toastSuccess, toastError, errText } from '$lib/components/ui/toast';
  import PageToolbar from '$lib/components/ui/PageToolbar.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';

  let companies = $state<Company[]>([]);
  let loading = $state(true);

  let showForm = $state(false);
  let editingId = $state<string | null>(null);
  let form = $state<CompanyInput>({});
  let formError = $state('');
  let formSection = $state<'main' | 'requisites' | 'bank' | 'signatories' | 'tax'>('main');

  async function load() {
    loading = true;
    try { companies = await api.listCompanies(); }
    catch (e) { toastError(errText(e)); }
    finally { loading = false; }
  }
  onMount(load);

  function defaultForm(): CompanyInput {
    return { code: '', name: '', inn: '', kpp: '', ogrn: '', okved: '', legal_address: '',
      postal_address: '', phone: '', email: '', website: '', bank_name: '', bank_bik: '',
      bank_account: '', bank_correspondent_account: '', director_name: '', director_position: '',
      accountant_name: '', tax_regime_usn: false };
  }

  function openForm(company?: Company) {
    editingId = company?._id ?? null;
    form = company ? { ...company } : defaultForm();
    formError = '';
    formSection = 'main';
    showForm = true;
  }

  async function save() {
    formError = '';
    if (!form.code?.trim()) { formError = 'Укажите код компании'; formSection = 'main'; return; }
    if (!form.name?.trim()) { formError = 'Укажите название'; formSection = 'main'; return; }
    try {
      if (editingId) await api.updateCompany(editingId, form);
      else await api.createCompany(form);
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

  const tabs = [
    { code: 'main', label: 'Основные', icon: 'fa-solid fa-building' },
    { code: 'requisites', label: 'Реквизиты', icon: 'fa-solid fa-file-invoice' },
    { code: 'bank', label: 'Банк', icon: 'fa-solid fa-landmark' },
    { code: 'signatories', label: 'Подписанты', icon: 'fa-solid fa-user-tie' },
    { code: 'tax', label: 'Налоги', icon: 'fa-solid fa-percent' },
  ] as const;
</script>

<div class="space-y-3">
  <PageToolbar title="Компании" icon="fa-solid fa-building">
    <button onclick={() => openForm()} class="btn btn-sm preset-filled-primary-500"><i class="fa-solid fa-plus"></i> Добавить</button>
  </PageToolbar>

  {#if loading}
    <Spinner />
  {:else}
    {#if showForm}
      <div class="rounded-xl border border-surface-300-700 bg-surface-50-950">
        <div class="flex items-center gap-2 border-b border-surface-300-700 px-4 pt-3">
          <h3 class="text-sm font-semibold text-surface-900-100">{editingId ? 'Редактировать компанию' : 'Новая компания'}</h3>
          <span class="text-xs text-surface-500-500 ml-2">{form.name || form.code || ''}</span>
        </div>
        <div class="flex gap-1 border-b border-surface-300-700 px-4 pt-1">
          {#each tabs as t (t.code)}
            <button class="flex items-center gap-1.5 rounded-t-lg px-3 py-1.5 text-xs font-medium transition-colors
              {formSection === t.code ? 'bg-surface-100-900 text-primary-600 border-x border-t border-surface-300-700 -mb-px'
              : 'text-surface-500-500 hover:text-surface-700-300'}"
              onclick={() => { formSection = t.code as typeof formSection; }}
            >
              <i class="{t.icon}"></i> {t.label}
            </button>
          {/each}
        </div>
        <form onsubmit={(e) => { e.preventDefault(); save(); }} class="p-4 space-y-3">
          {#if formSection === 'main'}
            <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
              <label class="block text-xs font-medium text-surface-700-300">Код *
                <input bind:value={form.code} class="input mt-1 w-full" required disabled={!!editingId} /></label>
              <label class="block text-xs font-medium text-surface-700-300">Название *
                <input bind:value={form.name} class="input mt-1 w-full" required /></label>
              <label class="block text-xs font-medium text-surface-700-300">ИНН
                <input bind:value={form.inn} class="input mt-1 w-full" maxlength="12" /></label>
            </div>
            <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
              <label class="block text-xs font-medium text-surface-700-300">Телефон
                <input bind:value={form.phone} class="input mt-1 w-full" placeholder="+7 (999) 123-45-67" /></label>
              <label class="block text-xs font-medium text-surface-700-300">Email
                <input bind:value={form.email} class="input mt-1 w-full" type="email" /></label>
              <label class="block text-xs font-medium text-surface-700-300">Сайт
                <input bind:value={form.website} class="input mt-1 w-full" placeholder="https://" /></label>
            </div>
            <label class="flex items-center gap-2 text-sm text-surface-700-300">
              <input type="checkbox" bind:checked={form.active} class="checkbox" /> Компания активна
            </label>
          {:else if formSection === 'requisites'}
            <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
              <label class="block text-xs font-medium text-surface-700-300">КПП
                <input bind:value={form.kpp} class="input mt-1 w-full" maxlength="9" /></label>
              <label class="block text-xs font-medium text-surface-700-300">ОГРН
                <input bind:value={form.ogrn} class="input mt-1 w-full" maxlength="15" /></label>
              <label class="block text-xs font-medium text-surface-700-300">ОКВЭД
                <input bind:value={form.okved} class="input mt-1 w-full" placeholder="xx.xx.xx" /></label>
            </div>
            <label class="block text-xs font-medium text-surface-700-300">Юридический адрес
              <input bind:value={form.legal_address} class="input mt-1 w-full" placeholder="г. Москва, ул. ..." /></label>
            <label class="block text-xs font-medium text-surface-700-300">Почтовый адрес
              <input bind:value={form.postal_address} class="input mt-1 w-full" placeholder="Совпадает с юридическим" /></label>
          {:else if formSection === 'bank'}
            <label class="block text-xs font-medium text-surface-700-300">Наименование банка
              <input bind:value={form.bank_name} class="input mt-1 w-full" placeholder="ПАО Сбербанк" /></label>
            <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
              <label class="block text-xs font-medium text-surface-700-300">БИК
                <input bind:value={form.bank_bik} class="input mt-1 w-full" maxlength="9" /></label>
              <label class="block text-xs font-medium text-surface-700-300">Расчётный счёт
                <input bind:value={form.bank_account} class="input mt-1 w-full" maxlength="20" /></label>
              <label class="block text-xs font-medium text-surface-700-300">Корр. счёт
                <input bind:value={form.bank_correspondent_account} class="input mt-1 w-full" maxlength="20" /></label>
            </div>
          {:else if formSection === 'signatories'}
            <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
              <label class="block text-xs font-medium text-surface-700-300">Должность руководителя
                <input bind:value={form.director_position} class="input mt-1 w-full" placeholder="Генеральный директор" /></label>
              <label class="block text-xs font-medium text-surface-700-300">ФИО руководителя
                <input bind:value={form.director_name} class="input mt-1 w-full" placeholder="Иванов Иван Иванович" /></label>
            </div>
            <label class="block text-xs font-medium text-surface-700-300">Главный бухгалтер
              <input bind:value={form.accountant_name} class="input mt-1 w-full" placeholder="ФИО" /></label>
          {:else if formSection === 'tax'}
            <label class="flex items-center gap-3 rounded-lg border border-surface-300-700 p-3 text-sm text-surface-700-300">
              <input type="checkbox" bind:checked={form.tax_regime_usn} class="checkbox" />
              <div>
                <span class="font-medium">Упрощённая система налогообложения (УСН)</span>
                <p class="text-xs text-surface-500-500">Отметьте, если компания применяет УСН. Если не отмечено — ОСН.</p>
              </div>
            </label>
          {/if}

          {#if formError}<div class="rounded-lg bg-error-500/10 p-2 text-sm text-error-600" role="alert">{formError}</div>{/if}
          <div class="flex gap-2 pt-1">
            <button type="submit" class="btn btn-sm preset-filled-primary-500">Сохранить</button>
            <button type="button" onclick={() => (showForm = false)} class="btn btn-sm btn-outline">Отмена</button>
          </div>
        </form>
      </div>
    {/if}

    <div class="overflow-x-auto rounded-lg border border-surface-300-700">
      <table class="table table-dense w-full text-left">
        <thead><tr><th>Код</th><th>Название</th><th>ИНН</th><th>Режим</th><th>Статус</th><th class="text-right">Действия</th></tr></thead>
        <tbody>
          {#each companies as company (company._id)}
            <tr>
              <td class="font-mono text-xs">{company.code}</td>
              <td>{company.name}</td>
              <td class="text-surface-600-400">{company.inn ?? '—'}</td>
              <td><span class="text-xs font-medium {company.tax_regime_usn ? 'text-success-600' : 'text-surface-500-500'}">
                {company.tax_regime_usn ? 'УСН' : 'ОСН'}
              </span></td>
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
            <tr><td colspan="6" class="py-6 text-center text-surface-400">Нет компаний</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
