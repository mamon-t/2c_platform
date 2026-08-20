<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Event, type EventPage, EVENT_TYPE_META } from '$lib/services/api';

  let events: Event[] = $state([]);
  let totalCount = $state(0);
  let hasMore = $state(false);
  let nextCursor = $state<string | null>(null);
  let loading = $state(true);
  let error = $state('');

  let filterType = $state('');
  let filterStream = $state('');
  let expandedId = $state<string | null>(null);

  async function loadEvents(after?: string) {
    loading = true;
    error = '';
    try {
      const page: EventPage = await api.listEvents({
        event_type: filterType || undefined,
        stream_id: filterStream || undefined,
        limit: 50,
        after,
      });
      events = page.events;
      totalCount = page.total_count;
      hasMore = page.has_more;
      nextCursor = page.next_cursor;
    } catch (e: any) {
      error = e?.toString() || 'Ошибка загрузки';
    } finally {
      loading = false;
    }
  }

  onMount(() => loadEvents());

  function eventMeta(eventType: string) {
    return EVENT_TYPE_META[eventType] ?? { label: eventType, icon: 'fa-solid fa-circle text-surface-400' };
  }

  function formatTime(iso: string) {
    return new Date(iso).toLocaleString('ru-RU', {
      day: '2-digit', month: '2-digit', year: 'numeric',
      hour: '2-digit', minute: '2-digit', second: '2-digit',
    });
  }

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }
</script>

<div class="p-4 space-y-4">
  <div class="flex items-center gap-3 mb-4">
    <h2 class="h2"><i class="fa-solid fa-bolt text-warning-500 mr-2"></i>События</h2>
    <span class="badge preset-tonal-surface">{totalCount}</span>
  </div>

  <!-- Filters -->
  <div class="flex flex-wrap gap-3 items-end">
    <label class="label">
      <span class="label-text">Тип события</span>
      <input class="input" bind:value={filterType} placeholder="object.posted" />
    </label>
    <label class="label">
      <span class="label-text">Object ID</span>
      <input class="input" bind:value={filterStream} placeholder="UUID объекта" />
    </label>
    <button class="btn preset-filled-primary" onclick={() => loadEvents()}>Применить</button>
    <button class="btn preset-tonal" onclick={() => { filterType = ''; filterStream = ''; loadEvents(); }}>Сбросить</button>
  </div>

  {#if error}
    <div class="alert preset-tonal-error">{error}</div>
  {/if}

  {#if loading}
    <div class="text-center py-8 text-surface-500"><i class="fa-solid fa-spinner fa-spin mr-2"></i>Загрузка...</div>
  {:else if events.length === 0}
    <div class="text-center py-8 text-surface-500">Событий пока нет</div>
  {:else}
    <div class="overflow-x-auto">
      <table class="table-hover table">
        <thead>
          <tr>
            <th></th>
            <th>Дата</th>
            <th>Тип</th>
            <th>Поток</th>
            <th>v</th>
            <th>Исполнитель</th>
          </tr>
        </thead>
        <tbody>
          {#each events as ev (ev._id)}
            <tr class="cursor-pointer" onclick={() => toggleExpand(ev._id)}>
              <td><i class={eventMeta(ev.event_type).icon}></i></td>
              <td class="text-sm">{formatTime(ev.occurred_at)}</td>
              <td><span class="badge preset-tonal">{eventMeta(ev.event_type).label}</span></td>
              <td class="text-sm">{ev.stream_type}:{ev.stream_id.slice(0, 8)}…</td>
              <td class="text-sm">{ev.version}</td>
              <td class="text-sm">{ev.metadata.login}</td>
            </tr>
            {#if expandedId === ev._id}
              <tr>
                <td colspan="6" class="!p-4">
                  <div class="card p-4 space-y-2 text-sm bg-surface-50 dark:bg-surface-900">
                    <div><strong>ID:</strong> <code>{ev._id}</code></div>
                    <div><strong>Stream:</strong> {ev.stream_type} / {ev.stream_id}</div>
                    <div><strong>Event type:</strong> {ev.event_type}</div>
                    <div><strong>Version:</strong> {ev.version}</div>
                    <div><strong>Исполнитель:</strong> {ev.metadata.login} ({ev.metadata.full_name ?? '—'})</div>
                    {#if ev.correlation_id}
                      <div><strong>Correlation:</strong> <code>{ev.correlation_id}</code></div>
                    {/if}
                    {#if Object.keys(ev.payload).length > 0}
                      <div><strong>Payload:</strong></div>
                      <pre class="text-xs overflow-x-auto p-2 bg-surface-100 dark:bg-surface-800 rounded">{JSON.stringify(ev.payload, null, 2)}</pre>
                    {/if}
                  </div>
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    </div>

    {#if hasMore}
      <div class="text-center">
        <button class="btn preset-tonal" onclick={() => loadEvents(nextCursor ?? undefined)}>
          Загрузить ещё
        </button>
      </div>
    {/if}
  {/if}
</div>
