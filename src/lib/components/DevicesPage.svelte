<script lang="ts">
  import { onMount } from 'svelte';
  import { api,
    type DeviceListItemTS, type DeviceConfigInputTS, type DeviceConfigTS,
    type DeviceEventTS, type PortDtoTS, type ConnectionKindTS } from '$lib/services/api';
  import { barcodeField } from '$lib/utils/barcode';

  let loading = $state(true);
  let error = $state('');
  let notice = $state('');
  let devices = $state<DeviceListItemTS[]>([]);
  let ports = $state<PortDtoTS[]>([]);
  let journal = $state<{ ev: DeviceEventTS; at: string }[]>([]);

  // Форма устройства
  let showForm = $state(false);
  let editingId = $state<string | null>(null);
  let saving = $state(false);
  let form = $state(formDefault());

  function formDefault() {
    return {
      name: '',
      kind: 'barcode_scanner' as DeviceConfigInputTS['kind'],
      connType: 'keyboard_wedge' as 'keyboard_wedge' | 'serial',
      port: '',
      baud: 9600,
      is_active: true,
      scan_handler: '',
    };
  }

  const KIND_META: Record<string, { label: string; icon: string; soon?: boolean }> = {
    barcode_scanner: { label: 'Сканер штрихкодов', icon: 'fa-solid fa-barcode' },
    scale: { label: 'Весы', icon: 'fa-solid fa-weight-scale' },
    fiscal_printer: { label: 'ККМ (v0.3)', icon: 'fa-solid fa-receipt', soon: true },
    label_printer: { label: 'Принтер этикеток', icon: 'fa-solid fa-print', soon: true },
  };

  function connLabel(c: ConnectionKindTS): string {
    if (c.kind === 'keyboard_wedge') return 'Keyboard wedge (USB-клавиатура)';
    if (c.kind === 'serial') return `COM: ${c.port} @ ${c.baud}`;
    return `TCP: ${c.host}:${c.port}`;
  }

  async function load() {
    loading = true;
    error = '';
    try {
      [devices, ports] = await Promise.all([api.devicesList(), api.devicesListPorts()]);
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка загрузки';
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    load();
    let unlisten: (() => void) | null = null;
    (async () => {
      try {
        const mod = await import('@tauri-apps/api/event');
        unlisten = await mod.listen<{ device_id: string; event: DeviceEventTS }>('device-event', (e) => {
          journal = [{ ev: e.payload.event, at: new Date().toLocaleTimeString('ru-RU') }, ...journal].slice(0, 50);
        });
      } catch { /* не tauri-окружение */ }
    })();
    return () => unlisten?.();
  });

  function openForm(d?: DeviceConfigTS) {
    editingId = d?.id ?? null;
    if (d) {
      form = {
        name: d.name,
        kind: d.kind,
        connType: d.connection.kind === 'serial' ? 'serial' : 'keyboard_wedge',
        port: d.connection.kind === 'serial' ? d.connection.port : '',
        baud: d.connection.kind === 'serial' ? d.connection.baud : 9600,
        is_active: d.is_active,
        scan_handler: typeof d.settings?.scan_handler === 'string' ? d.settings.scan_handler as string : '',
      };
    } else {
      form = formDefault();
    }
    showForm = true;
  }

  async function save() {
    if (!form.name.trim()) { error = 'Укажите название'; return; }
    if (form.connType === 'serial' && !form.port) { error = 'Выберите порт'; return; }
    saving = true; error = '';
    try {
      const settings: Record<string, unknown> = {};
      if (form.scan_handler.trim()) settings.scan_handler = form.scan_handler;

      const connection: ConnectionKindTS = form.connType === 'serial'
        ? { kind: 'serial', port: form.port, baud: Number(form.baud) || 9600 }
        : { kind: 'keyboard_wedge' };

      await api.devicesSave(editingId, {
        kind: form.kind,
        name: form.name.trim(),
        connection,
        settings,
        is_active: form.is_active,
      });
      showForm = false;
      notice = editingId ? 'Сохранено' : 'Устройство добавлено';
      await load();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка сохранения';
    } finally {
      saving = false;
    }
  }

  async function remove(id: string) {
    if (!confirm('Удалить устройство?')) return;
    try {
      await api.devicesDelete(id);
      notice = 'Удалено';
      await load();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка удаления';
    }
  }

  async function toggleConnect(d: DeviceListItemTS) {
    error = ''; notice = '';
    try {
      if (d.connected) {
        await api.devicesDisconnect(d.id);
        notice = 'Отключено';
      } else {
        await api.devicesConnect(d.id);
        notice = `${d.name}: подключено`;
      }
      await load();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка подключения';
      await load();
    }
  }

  async function test(d: DeviceConfigTS) {
    error = ''; notice = '';
    try {
      notice = await api.devicesTest(d.id);
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка теста';
    }
  }

  // Wedge-тестовое поле
  let lastWedgeCode = $state('');
  async function onWedgeCode(code: string) {
    lastWedgeCode = code;
    try {
      await api.devicesWedgeScan(code);
      notice = `Скан зафиксирован: ${code}`;
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка отправки скана';
    }
  }

  function eventIcon(ev: DeviceEventTS): string {
    switch (ev.type) {
      case 'scanned': return 'fa-solid fa-barcode text-success-500';
      case 'weighed': return 'fa-solid fa-weight-scale text-primary-500';
      case 'connected': return 'fa-solid fa-plug-circle-check text-success-500';
      case 'disconnected': return 'fa-solid fa-plug-circle-xmark text-surface-400';
      default: return 'fa-solid fa-triangle-exclamation text-error-500';
    }
  }

  function eventText(ev: DeviceEventTS): string {
    switch (ev.type) {
      case 'scanned': return `код ${ev.code}`;
      case 'weighed': return `${((ev.grams ?? 0) / 1000).toFixed(3)} кг${ev.stable ? '' : ' (нестабильно)'}`;
      case 'connected': return 'подключено';
      case 'disconnected': return 'отключено';
      default: return ev.message ?? 'ошибка';
    }
  }
</script>

<div class="container mx-auto p-4 space-y-4">
  <header class="flex items-center justify-between">
    <h2 class="h4 flex items-center gap-2"><i class="fa-solid fa-plug"></i> Оборудование</h2>
    <button class="btn btn-sm btn-primary" onclick={() => openForm()}>
      <i class="fa-solid fa-plus"></i> Добавить устройство
    </button>
  </header>

  {#if error}<div class="alert alert-error whitespace-pre-line">{error}</div>{/if}
  {#if notice}<div class="alert alert-success">{notice}</div>{/if}

  <div class="grid gap-3 md:grid-cols-2">
    {#each devices as d (d.id)}
      <div class="card p-4 space-y-2">
        <div class="flex items-start justify-between gap-2">
          <div class="flex items-center gap-3 min-w-0">
            <i class="{KIND_META[d.kind]?.icon} text-xl"></i>
            <div class="min-w-0">
              <div class="font-medium truncate">{d.name}</div>
              <div class="text-xs text-surface-500">{KIND_META[d.kind]?.label} · {connLabel(d.connection)}</div>
            </div>
          </div>
          <span class="badge shrink-0 {d.connected ? 'bg-success-500 text-white' : d.is_active ? 'bg-surface-300' : 'bg-warning-500 text-white'}">
            {d.connected ? 'подключено' : d.is_active ? 'не подключено' : 'выключено'}
          </span>
        </div>

        <div class="flex flex-wrap gap-2 pt-1">
          {#if d.connection.kind !== 'keyboard_wedge'}
            <button class="btn btn-sm {d.connected ? 'btn-outline' : 'variant-filled-success'}"
              onclick={() => toggleConnect(d)}>
              <i class="fa-solid {d.connected ? 'fa-link-slash' : 'fa-plug'}"></i>
              {d.connected ? 'Отключить' : 'Подключить'}
            </button>
          {/if}
          <button class="btn btn-sm btn-outline" onclick={() => test(d)}>
            <i class="fa-solid fa-vial"></i> Тест
          </button>
          <button class="btn btn-sm btn-outline" onclick={() => openForm(d)}>
            <i class="fa-solid fa-pen"></i>
          </button>
          <button class="btn btn-sm btn-outline text-error-600" onclick={() => remove(d.id)}>
            <i class="fa-solid fa-trash"></i>
          </button>
        </div>
      </div>
    {:else}
      <div class="card p-6 text-center text-surface-400 md:col-span-2">
        Устройств нет. Добавьте сканер или весы.
      </div>
    {/each}
  </div>

  <!-- Тест wedge -->
  <div class="card p-4 space-y-2">
    <h3 class="text-sm font-semibold">Проверка keyboard-wedge</h3>
    <p class="text-xs text-surface-500">
      Фокус в поле → отсканируйте штрихкод. Слушатель активен только здесь.
    </p>
    <input class="input max-w-md" placeholder="Отсканируйте или введите код + Enter"
      use:barcodeField={{ onCode: onWedgeCode }} />
    {#if lastWedgeCode}
      <div class="text-xs text-success-600"><i class="fa-solid fa-check"></i> последний код: {lastWedgeCode}</div>
    {/if}
  </div>

  <!-- Живой журнал -->
  <div class="card p-4">
    <h3 class="text-sm font-semibold mb-2"><i class="fa-solid fa-wave-square"></i> Журнал событий устройств</h3>
    <div class="max-h-64 overflow-y-auto space-y-1">
      {#each journal as j, i (j.at + String(i))}
        <div class="flex items-center gap-2 text-sm">
          <span class="text-xs text-surface-400 w-20">{j.at}</span>
          <i class="{eventIcon(j.ev)} w-5 text-center"></i>
          <span class="truncate">{j.ev.device_id}: {eventText(j.ev)}</span>
        </div>
      {:else}
        <div class="text-sm text-surface-400 py-2">Пока тихо…</div>
      {/each}
    </div>
  </div>
</div>

<!-- Модалка формы -->
{#if showForm}
  <div class="fixed inset-0 bg-black/50 grid place-items-center z-50 overflow-auto py-8" role="presentation">
    <div class="card p-5 w-[520px] space-y-3">
      <h3 class="h5">{editingId ? 'Изменить устройство' : 'Новое устройство'}</h3>

      <label class="label">Название *</label>
      <input class="input" bind:value={form.name} placeholder="Сканер на кассе 1" />

      <label class="label">Тип</label>
      <select class="select" bind:value={form.kind}>
        {#each Object.entries(KIND_META) as [k, meta]}
          <option value={k} disabled={meta.soon}>{meta.label}{meta.soon ? ' — скоро' : ''}</option>
        {/each}
      </select>

      <label class="label">Подключение</label>
      <select class="select" bind:value={form.connType}>
        <option value="keyboard_wedge">Keyboard wedge (воткнул — работает)</option>
        <option value="serial">Последовательный порт (COM)</option>
      </select>

      {#if form.connType === 'serial'}
        <div class="grid grid-cols-[1fr_120px] gap-3">
          <div>
            <label class="label">Порт</label>
            <select class="select" bind:value={form.port}>
              <option value="" disabled>{ports.length ? '— выберите порт —' : 'порты не найдены'}</option>
              {#each ports as p (p.path)}
                <option value={p.path}>{p.path}{p.description !== 'Последовательный порт' ? ` · ${p.description}` : ''}</option>
              {/each}
            </select>
          </div>
          <div>
            <label class="label">Baud</label>
            <select class="select" bind:value={form.baud}>
              {#each [4800, 9600, 19200, 38400, 57600, 115200] as b}
                <option value={b}>{b}</option>
              {/each}
            </select>
          </div>
        </div>
      {/if}

      <label class="flex items-center gap-2 text-sm">
        <input type="checkbox" class="checkbox" bind:checked={form.is_active} /> Активно
      </label>

      <details class="text-sm">
        <summary class="cursor-pointer text-surface-500">Дополнительно: Rhai-обработчик событий</summary>
        <textarea class="textarea mt-2 font-mono text-xs" rows="5" bind:value={form.scan_handler}
          placeholder={'// ctx.event = {type:"scanned", code:"..."}\nlog_info(ctx.event.code);'}></textarea>
      </details>

      <div class="flex justify-end gap-2 pt-2">
        <button class="btn btn-outline" onclick={() => (showForm = false)}>Отмена</button>
        <button class="btn btn-primary" disabled={saving} onclick={save}>
          {saving ? 'Сохранение…' : 'Сохранить'}
        </button>
      </div>
    </div>
  </div>
{/if}
