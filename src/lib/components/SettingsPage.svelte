<script lang="ts">
  import { api } from '$lib/services/api';
  import { onMount } from 'svelte';

  interface ContactType { code: string; name: string; }

  let contactTypes = $state<ContactType[]>([]);
  let loading = $state(true);
  let saving = $state(false);
  let message = $state('');
  let newType = $state({ code: '', name: '' });

  const inputCls = 'w-full rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 placeholder:text-surface-400-600 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500';
  const btnPrimary = 'rounded-lg bg-primary-500 px-4 py-2 text-sm font-medium text-white hover:bg-primary-600 disabled:opacity-50';

  onMount(async () => {
    try { contactTypes = await api.getContactTypes(); } catch {} finally { loading = false; }
  });

  async function saveTypes() {
    saving = true;
    message = '';
    try { await api.saveContactTypes(contactTypes); message = 'Сохранено'; } catch (e: any) { message = typeof e === 'string' ? e : 'Ошибка'; }
    finally { saving = false; }
  }

  function addType() {
    if (!newType.code || !newType.name) return;
    if (contactTypes.some(t => t.code === newType.code)) { message = 'Код уже существует'; return; }
    contactTypes = [...contactTypes, { code: newType.code.trim(), name: newType.name.trim() }];
    newType = { code: '', name: '' };
  }

  function removeType(code: string) {
    contactTypes = contactTypes.filter(t => t.code !== code);
  }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h2 class="text-2xl font-bold text-surface-900-100">Настройки</h2>
  </div>

  <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5 space-y-4">
    <h3 class="font-semibold text-surface-900-100">Типы контактов</h3>
    <p class="text-sm text-surface-500-500">Список типов контактов, доступных при добавлении и редактировании контактов пользователей.</p>

    {#if loading}
      <div class="flex items-center justify-center p-6"><div class="h-6 w-6 animate-spin rounded-full border-4 border-primary-500 border-t-transparent"></div></div>
    {:else}
      <div class="space-y-2">
        {#each contactTypes as t, i}
          <div class="flex items-center gap-3 rounded-lg border border-surface-300-700 bg-surface-100-900 px-3 py-2">
            <span class="w-32 font-mono text-xs text-surface-900-100">{t.code}</span>
            <input bind:value={contactTypes[i].name} class={inputCls + ' flex-1'} />
            <button onclick={() => removeType(t.code)} class="rounded p-1 text-error-500 hover:bg-error-500/10" title="Удалить">
              <i class="fa-solid fa-trash text-xs"></i>
            </button>
          </div>
        {/each}
      </div>

      <div class="flex gap-2">
        <input bind:value={newType.code} class="w-32 rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 placeholder:text-surface-400-600 focus:border-primary-500 focus:outline-none" placeholder="Код" />
        <input bind:value={newType.name} class={inputCls + ' flex-1'} placeholder="Название" />
        <button onclick={addType} class="rounded-lg border border-surface-300-700 px-3 py-2 text-sm text-surface-700-300 hover:bg-surface-200-800">+</button>
      </div>

      <div class="flex items-center gap-3">
        <button onclick={saveTypes} disabled={saving} class={btnPrimary}>{saving ? 'Сохранение...' : 'Сохранить'}</button>
        {#if message}<span class="text-sm {message === 'Сохранено' ? 'text-success-600' : 'text-error-600'}">{message}</span>{/if}
      </div>
    {/if}
  </div>
</div>
