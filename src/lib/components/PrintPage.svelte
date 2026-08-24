// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api,
    type PrintTemplate,
    type CreatePrintTemplateInput,
    type UpdatePrintTemplateInput,
    type EntityType,
    type PaperFormat,
    type Orientation,
  } from '$lib/services/api';

  let entityTypes: EntityType[] = $state([]);
  let templates: PrintTemplate[] = $state([]);
  let loading = $state(true);
  let error = $state('');
  let filterEntityType = $state('');

  let showForm = $state(false);
  let editingId = $state<string | null>(null);
  let form = $state({
    code: '',
    name: '',
    entity_type: '',
    form_code: '',
    template_body: '',
    css_styles: '',
    paper_format: 'a4' as PaperFormat,
    orientation: 'portrait' as Orientation,
    is_default: false,
    before_print_script: '',
  });
  let formError = $state('');

  let showPreview = $state(false);
  let previewHtml = $state('');
  let previewLoading = $state(false);

  const inputCls = 'w-full rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 placeholder:text-surface-400-600 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500';
  const btnPrimary = 'rounded-lg bg-primary-500 px-4 py-2 text-sm font-medium text-white hover:bg-primary-600 disabled:opacity-50';
  const btnSecondary = 'rounded-lg border border-surface-300-700 px-4 py-2 text-sm text-surface-700-300 hover:bg-surface-200-800';
  const btnDanger = 'rounded-lg bg-error-500 px-3 py-1 text-xs font-medium text-white hover:bg-error-600';

  onMount(async () => {
    try {
      entityTypes = await api.listEntityTypes();
    } catch {}
    await loadTemplates();
  });

  async function loadTemplates() {
    loading = true;
    error = '';
    try {
      templates = await api.printListTemplates(filterEntityType || 'document');
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка загрузки';
    } finally {
      loading = false;
    }
  }

  function openCreateForm() {
    editingId = null;
    form = {
      code: '',
      name: '',
      entity_type: filterEntityType || 'document',
      form_code: 'document_print',
      template_body: '<div class="header">\n  <h2>{{object.number}}</h2>\n</div>\n<table>\n  <tbody>\n    {{#each object.data}}\n    <tr><td>{{@key}}</td><td>{{this}}</td></tr>\n    {{/each}}\n  </tbody>\n</table>',
      css_styles: '',
      paper_format: 'a4',
      orientation: 'portrait',
      is_default: false,
      before_print_script: '',
    };
    formError = '';
    showForm = true;
  }

  function openEditForm(tmpl: PrintTemplate) {
    editingId = tmpl._id;
    form = {
      code: tmpl.code,
      name: tmpl.name,
      entity_type: tmpl.entity_type,
      form_code: tmpl.form_code,
      template_body: tmpl.template_body,
      css_styles: tmpl.css_styles,
      paper_format: tmpl.paper_format,
      orientation: tmpl.orientation,
      is_default: tmpl.is_default,
      before_print_script: tmpl.before_print_script ?? '',
    };
    formError = '';
    showForm = true;
  }

  async function saveTemplate() {
    formError = '';
    if (!form.code || !form.name || !form.entity_type || !form.form_code) {
      formError = 'Заполните код, название, тип сущности и код формы';
      return;
    }
    try {
      if (editingId) {
        const input: UpdatePrintTemplateInput = {
          name: form.name,
          template_body: form.template_body,
          css_styles: form.css_styles,
          paper_format: form.paper_format,
          orientation: form.orientation,
          is_default: form.is_default,
          before_print_script: form.before_print_script || undefined,
        };
        await api.printUpdateTemplate(editingId, input);
      } else {
        const input: CreatePrintTemplateInput = {
          code: form.code,
          name: form.name,
          entity_type: form.entity_type,
          form_code: form.form_code,
          template_body: form.template_body,
          css_styles: form.css_styles || undefined,
          paper_format: form.paper_format,
          orientation: form.orientation,
          is_default: form.is_default,
          before_print_script: form.before_print_script || undefined,
        };
        await api.printCreateTemplate(input);
      }
      showForm = false;
      await loadTemplates();
    } catch (e: any) {
      formError = typeof e === 'string' ? e : e?.message ?? 'Ошибка сохранения';
    }
  }

  async function deleteTemplate(id: string) {
    if (!confirm('Удалить шаблон?')) return;
    try {
      await api.printDeleteTemplate(id);
      await loadTemplates();
    } catch {}
  }

  async function previewTemplate(tmpl: PrintTemplate) {
    previewLoading = true;
    error = '';
    try {
      const html = await api.printRender(tmpl._id, '00000000-0000-0000-0000-000000000000');
      previewHtml = html;
      showPreview = true;
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка предпросмотра: нет объекта для печати';
    } finally {
      previewLoading = false;
    }
  }

  function downloadHtml(html: string, name: string) {
    const blob = new Blob([html], { type: 'text/html;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${name}.html`;
    a.click();
    URL.revokeObjectURL(url);
  }
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h2 class="text-2xl font-bold text-surface-900-100">Печатные формы</h2>
    <div class="flex items-center gap-3">
      <select bind:value={filterEntityType} onchange={() => loadTemplates()} class="w-48 rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 focus:border-primary-500 focus:outline-none">
        <option value="">Все типы</option>
        <option value="document">Документы</option>
        <option value="catalog">Справочники</option>
        <option value="report">Отчёты</option>
        <option value="register">Реестры</option>
      </select>
      <button onclick={openCreateForm} class={btnPrimary}>+ Создать шаблон</button>
    </div>
  </div>

  {#if error}<div class="rounded-lg bg-error-500/10 p-3 text-sm text-error-600">{error}</div>{/if}

  {#if showForm}
    <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5 space-y-4">
      <h3 class="font-semibold text-surface-900-100">{editingId ? 'Редактировать шаблон' : 'Новый шаблон'}</h3>
      <div class="grid grid-cols-1 gap-3 md:grid-cols-4">
        <label class="block text-sm text-surface-700-300">Код *
          <input bind:value={form.code} class={inputCls + ' mt-1'} required disabled={!!editingId} placeholder="document_standard" />
        </label>
        <label class="block text-sm text-surface-700-300">Название *
          <input bind:value={form.name} class={inputCls + ' mt-1'} required placeholder="Документ операции" />
        </label>
        <label class="block text-sm text-surface-700-300">Тип сущности *
          <select bind:value={form.entity_type} class={inputCls + ' mt-1'}>
            <option value="document">Документ</option>
            <option value="catalog">Справочник</option>
            <option value="report">Отчёт</option>
            <option value="register">Реестр</option>
          </select>
        </label>
        <label class="block text-sm text-surface-700-300">Код формы *
          <input bind:value={form.form_code} class={inputCls + ' mt-1'} placeholder="document_print" />
        </label>
      </div>
      <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
        <label class="block text-sm text-surface-700-300">Формат бумаги
          <select bind:value={form.paper_format} class={inputCls + ' mt-1'}>
            <option value="a4">A4</option>
            <option value="a5">A5</option>
            <option value="letter">Letter</option>
          </select>
        </label>
        <label class="block text-sm text-surface-700-300">Ориентация
          <select bind:value={form.orientation} class={inputCls + ' mt-1'}>
            <option value="portrait">Книжная</option>
            <option value="landscape">Альбомная</option>
          </select>
        </label>
        <label class="flex items-center gap-2 text-sm text-surface-700-300 pt-6">
          <input type="checkbox" bind:checked={form.is_default} class="rounded" />
          Шаблон по умолчанию
        </label>
      </div>

      <label class="block text-sm text-surface-700-300">Handlebars шаблон
        <textarea bind:value={form.template_body} class={inputCls + ' mt-1 font-mono'} rows="14" placeholder="HTML + Handlebars"></textarea>
      </label>

      <label class="block text-sm text-surface-700-300">Пользовательский CSS
        <textarea bind:value={form.css_styles} class={inputCls + ' mt-1 font-mono'} rows="4" placeholder="Дополнительные стили"></textarea>
      </label>

      <label class="block text-sm text-surface-700-300">Скрипт beforePrint (Rhai)
        <textarea bind:value={form.before_print_script} class={inputCls + ' mt-1 font-mono'} rows="4" placeholder="// Вычисляемые поля&#10;let result = #{'{}'};&#10;result[&quot;total&quot;] = 42;&#10;result"></textarea>
      </label>

      {#if formError}<div class="text-sm text-error-600">{formError}</div>{/if}
      <div class="flex gap-2">
        <button onclick={saveTemplate} class={btnPrimary}>{editingId ? 'Обновить' : 'Создать'}</button>
        <button onclick={() => { showForm = false; }} class={btnSecondary}>Отмена</button>
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="flex items-center justify-center p-12">
      <div class="h-8 w-8 animate-spin rounded-full border-4 border-primary-500 border-t-transparent"></div>
    </div>
  {:else}
    <div class="overflow-x-auto rounded-xl border border-surface-300-700">
      <table class="w-full text-left text-sm">
        <thead class="border-b border-surface-300-700 bg-surface-100-900 text-xs font-medium uppercase text-surface-500-500">
          <tr>
            <th class="px-4 py-3">Код</th>
            <th class="px-4 py-3">Название</th>
            <th class="px-4 py-3">Тип</th>
            <th class="px-4 py-3">Форма</th>
            <th class="px-4 py-3">Бумага</th>
            <th class="px-4 py-3">По умолч.</th>
            <th class="px-4 py-3">Версия</th>
            <th class="px-4 py-3 text-right">Действия</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-surface-300-700">
          {#each templates as t (t._id)}
            <tr class="hover:bg-surface-100-900/50">
              <td class="px-4 py-3 font-mono text-xs text-surface-900-100">{t.code}</td>
              <td class="px-4 py-3 text-surface-900-100">{t.name}</td>
              <td class="px-4 py-3 text-surface-600-400">{t.entity_type}</td>
              <td class="px-4 py-3 text-surface-600-400">{t.form_code}</td>
              <td class="px-4 py-3 text-surface-600-400">{t.paper_format} / {t.orientation}</td>
              <td class="px-4 py-3">
                {#if t.is_default}
                  <span class="rounded bg-primary-500/20 px-1.5 py-0.5 text-xs text-primary-600">По умолчанию</span>
                {:else}
                  <span class="text-surface-500-500">—</span>
                {/if}
              </td>
              <td class="px-4 py-3 text-surface-600-400">v{t.version}</td>
              <td class="px-4 py-3 text-right">
                <button onclick={() => openEditForm(t)} class="mr-2 text-primary-500 hover:underline text-xs">Ред.</button>
                <button onclick={() => previewTemplate(t)} disabled={previewLoading} class="mr-2 text-success-500 hover:underline text-xs">
                  {previewLoading ? '...' : 'Просмотр'}
                </button>
                <button onclick={() => deleteTemplate(t._id)} class={btnDanger}>Удалить</button>
              </td>
            </tr>
          {:else}
            <tr><td colspan="8" class="px-4 py-8 text-center text-surface-500-500">Нет шаблонов</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

{#if showPreview}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onclick={() => { showPreview = false; }}>
    <div class="flex h-[90vh] w-[90vw] flex-col rounded-xl border border-surface-300-700 bg-surface-50-950 shadow-xl" onclick={(e) => e.stopPropagation()}>
      <div class="flex items-center justify-between border-b border-surface-300-700 px-6 py-3">
        <h3 class="font-semibold text-surface-900-100">Предпросмотр печатной формы</h3>
        <div class="flex gap-2">
          <button onclick={() => downloadHtml(previewHtml, 'print_preview')} class={btnSecondary}>
            <i class="fa-solid fa-download mr-1"></i>Скачать HTML
          </button>
          <button onclick={() => { showPreview = false; }} class="rounded p-1 text-surface-500-500 hover:bg-surface-200-800">
            <i class="fa-solid fa-xmark text-lg"></i>
          </button>
        </div>
      </div>
      <div class="flex-1 overflow-auto bg-white p-4">
        <iframe srcdoc={previewHtml} class="h-full w-full border-0" title="Предпросмотр"></iframe>
      </div>
    </div>
  </div>
{/if}
