<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type NotificationItemTS } from '$lib/services/api';
  import { isAuthenticated } from '$lib/stores/auth';

  let unreadCount = $state(0);
  let notifications = $state<NotificationItemTS[]>([]);
  let open = $state(false);

  async function load() {
    if (!$isAuthenticated) return;
    try {
      unreadCount = await api.notificationsCountUnread();
      if (open) notifications = await api.notificationsList(30);
    } catch { /* тихо */ }
  }

  function toggle() {
    open = !open;
    if (open) load();
  }

  async function markAllRead() {
    try { await api.notificationsMarkRead(); await load(); } catch { /* тихо */ }
  }

  function sevColor(sev: string): string {
    return sev === 'critical' ? 'text-error-600' : sev === 'warning' ? 'text-warning-600' : 'text-primary-500';
  }
  function sevIcon(sev: string): string {
    return sev === 'critical' ? 'fa-solid fa-circle-exclamation'
         : sev === 'warning' ? 'fa-solid fa-triangle-exclamation'
         : 'fa-solid fa-circle-info';
  }
  function fmtTime(iso: string): string {
    try { return new Date(iso).toLocaleString('ru-RU', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' }); }
    catch { return ''; }
  }

  onMount(() => { load(); const t = setInterval(load, 30_000); return () => clearInterval(t); });
</script>

{#if $isAuthenticated}
  <div class="relative">
    <button
      onclick={toggle}
      class="relative rounded-lg p-2 text-surface-500-500 hover:bg-surface-200-800"
      title="Уведомления"
      aria-label="Уведомления{unreadCount > 0 ? ` (${unreadCount} непрочитанных)` : ''}"
    >
      <i class="fa-{unreadCount > 0 ? 'solid' : 'regular'} fa-bell"></i>
      {#if unreadCount > 0}
        <span class="absolute -top-1 -right-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-error-500 px-1 text-[10px] font-bold text-white">
          {unreadCount > 99 ? '99+' : unreadCount}
        </span>
      {/if}
    </button>

    {#if open}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="fixed inset-0 z-40" onclick={() => (open = false)} role="presentation"></div>
      <div
        class="fixed right-4 top-14 z-50 max-h-[70vh] w-96 overflow-hidden rounded-xl border border-surface-300-700 bg-surface-50-950 shadow-xl"
        role="dialog"
        aria-label="Уведомления"
      >
        <div class="flex items-center justify-between border-b border-surface-300-700 p-3">
          <h3 class="flex items-center gap-2 text-sm font-semibold"><i class="fa-solid fa-bell"></i> Уведомления</h3>
          {#if notifications.some((n) => n.status !== 'read')}
            <button class="text-xs text-primary-600 hover:underline" onclick={markAllRead}>Прочитать все</button>
          {/if}
        </div>
        <div class="max-h-80 divide-y divide-surface-200-700 overflow-y-auto">
          {#each notifications as n (n._id)}
            <div class="p-3 {n.status !== 'read' ? 'bg-primary-50 dark:bg-primary-900/10' : ''}">
              <div class="flex items-start gap-2">
                <i class="{sevIcon(n.severity)} mt-0.5 {sevColor(n.severity)}" aria-hidden="true"></i>
                <div class="min-w-0">
                  <div class="truncate text-sm font-medium">{n.title}</div>
                  {#if n.body}<div class="truncate text-xs text-surface-500">{n.body}</div>{/if}
                  <div class="text-[10px] text-surface-400">{fmtTime(n.created_at)}</div>
                </div>
              </div>
            </div>
          {:else}
            <div class="p-6 text-center text-sm text-surface-400">Уведомлений нет</div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
{/if}
