// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { api,
    type MessagingRoomPreviewTS, type MessagingMessageTS,
  } from '$lib/services/api';
  import { auth } from '$lib/stores/auth';

  let loading = $state(true);
  let error = $state('');
  let rooms = $state<MessagingRoomPreviewTS[]>([]);
  let selectedRoom = $state<MessagingRoomPreviewTS | null>(null);
  let messages = $state<MessagingMessageTS[]>([]);
  let newMessage = $state('');
  let chatContainer = $state<HTMLDivElement | null>(null);

  // Создание группового чата
  let showNewChat = $state(false);
  let newChatTitle = $state('');
  let users = $state<{ _id: string; display_name: string }[]>([]);
  let selectedUsers = $state<Set<string>>(new Set());

  const myId = (): string => $auth?.userId ?? '';

  function roomTitle(room: MessagingRoomPreviewTS): string {
    if (room.room.title) return room.room.title;
    if (room.room.room_type === 'direct') {
      const other = room.room.members.find((m) => m !== myId());
      return other ? `Диалог` : 'Диалог';
    }
    return room.room.room_type === 'document' ? 'Обсуждение документа' : 'Групповой чат';
  }

  async function load() {
    loading = true; error = '';
    try {
      rooms = await api.messagingRoomsList();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка';
    } finally { loading = false; }
  }
  onMount(load);

  async function openRoom(room: MessagingRoomPreviewTS) {
    selectedRoom = room;
    try {
      messages = await api.messagingMessagesList(room.room._id, 200);
      if (messages.length > 0) {
        await api.messagingReadsUpdate(room.room._id, messages[messages.length - 1]._id);
        room.unread_count = 0;
        rooms = rooms; // триггер обновления
      }
      await tick();
      scrollChat();
    } catch (e: any) { error = typeof e === 'string' ? e : e?.message ?? ''; }
  }

  function scrollChat() {
    if (chatContainer) chatContainer.scrollTop = chatContainer.scrollHeight;
  }

  async function sendMessage() {
    if (!selectedRoom || !newMessage.trim()) return;
    try {
      await api.messagingMessagesSend(selectedRoom.room._id, newMessage.trim());
      newMessage = '';
      const msgs = await api.messagingMessagesList(selectedRoom.room._id, 200);
      messages = msgs;
      await tick(); scrollChat();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? '';
    }
  }

  function openNewChatModal() {
    showNewChat = true;
    newChatTitle = ''; selectedUsers = new Set();
    api.listUsers().then(u => users = u.map(x => ({ _id: x._id, display_name: x.display_name }))).catch(() => {});
  }

  async function createGroupChat() {
    if (!newChatTitle.trim() || selectedUsers.size < 2) return;
    try {
      await api.messagingRoomsCreate(newChatTitle.trim(), [...selectedUsers]);
      showNewChat = false;
      await load();
    } catch (e: any) { error = typeof e === 'string' ? e : e?.message ?? ''; }
  }

  function toggleUser(id: string) {
    if (selectedUsers.has(id)) selectedUsers.delete(id); else selectedUsers.add(id);
    selectedUsers = new Set(selectedUsers);
  }

  function fmtTime(iso?: string): string {
    if (!iso) return '';
    try { return new Date(iso).toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' }); }
    catch { return ''; }
  }
</script>

<div class="flex h-full">
  <!-- Список комнат -->
  <div class="w-80 border-r border-surface-300-700 p-3 overflow-y-auto">
    <div class="flex items-center justify-between mb-3">
      <h3 class="text-sm font-semibold">Чаты</h3>
      <button class="btn btn-sm btn-primary" onclick={openNewChatModal} aria-label="Создать групповой чат">
        <i class="fa-solid fa-plus"></i>
      </button>
    </div>
    {#if error}
      <div class="alert preset-tonal-error text-xs mb-2" role="alert">{error}</div>
    {/if}
    <div class="space-y-1">
      {#each rooms as r (r.room._id)}
        <button class="w-full text-left p-2 rounded-lg transition-colors
          {selectedRoom?.room._id === r.room._id ? 'bg-primary-500/10 border-l-4 border-primary-500' : 'hover:bg-surface-100 dark:hover:bg-surface-800'}"
          onclick={() => openRoom(r)}>
          <div class="flex items-center justify-between">
            <span class="text-sm font-medium truncate">{roomTitle(r)}</span>
            {#if r.unread_count > 0}
              <span class="badge bg-error-500 text-white text-xs shrink-0">{r.unread_count}</span>
            {/if}
          </div>
          {#if r.last_message}
            <div class="text-xs text-surface-400 truncate mt-0.5">{r.last_message.content}</div>
          {/if}
        </button>
      {:else}
        <p class="text-sm text-surface-400 p-2">Чатов нет</p>
      {/each}
    </div>
  </div>

  <!-- Чат -->
  <div class="flex-1 flex flex-col min-w-0">
    {#if selectedRoom}
      <div class="border-b border-surface-300-700 p-3">
        <h3 class="font-medium">{roomTitle(selectedRoom)}</h3>
        <div class="text-xs text-surface-400">{selectedRoom.room.members.length} участник(ов)</div>
      </div>

      <div class="flex-1 overflow-y-auto p-4 space-y-3" bind:this={chatContainer}>
        {#each messages as m (m._id)}
          <div class="flex {m.author_id === myId() ? 'justify-end' : 'justify-start'}">
            <div class="max-w-md rounded-xl px-3 py-2 {m.author_id === myId()
              ? 'bg-primary-500/15 text-primary-900-50'
              : 'bg-surface-100 dark:bg-surface-800'}"
              class:opacity-50={m.is_deleted}>
              <div class="text-xs text-surface-400 mb-0.5">
                {m.author_id.slice(0,8)}… · {fmtTime(m.created_at)}
              </div>
              <div class="text-sm whitespace-pre-wrap break-words">{m.is_deleted ? '(удалено)' : m.content}</div>
            </div>
          </div>
        {:else}
          <div class="text-center text-surface-400 py-8">Сообщений нет — напишите первое</div>
        {/each}
      </div>

      <form class="border-t border-surface-300-700 p-3 flex gap-2"
        onsubmit={(e) => { e.preventDefault(); sendMessage(); }}>
        <input class="input flex-1" placeholder="Сообщение…"
          bind:value={newMessage} />
        <button class="btn btn-primary" type="submit" disabled={!newMessage.trim()}>
          <i class="fa-solid fa-paper-plane"></i>
        </button>
      </form>
    {:else}
      <div class="flex-1 grid place-items-center text-surface-300">
        <div class="text-center"><i class="fa-solid fa-comments text-4xl mb-2"></i><p>Выберите чат</p></div>
      </div>
    {/if}
  </div>
</div>

<!-- Модалка создания группового чата -->
{#if showNewChat}
  <div class="fixed inset-0 bg-black/50 z-50 grid place-items-center" role="presentation" onclick={() => showNewChat = false}>
    <div class="card p-5 w-96 space-y-3 bg-surface-50-950" onclick={(e) => e.stopPropagation()} role="dialog">
      <h3 class="font-semibold">Новый групповой чат</h3>
      <input class="input" placeholder="Название чата" bind:value={newChatTitle} />
      <div class="space-y-1 max-h-48 overflow-y-auto">
        {#each users as u (u._id)}
          <button class="w-full text-left p-2 rounded hover:bg-surface-100 dark:hover:bg-surface-800 text-sm
            {selectedUsers.has(u._id) ? 'bg-primary-50 dark:bg-primary-900/20' : ''}"
            onclick={() => toggleUser(u._id)}>
            {u.display_name || u._id.slice(0, 8)}
          </button>
        {/each}
      </div>
      <div class="flex justify-end gap-2">
        <button class="btn btn-outline" onclick={() => showNewChat = false}>Отмена</button>
        <button class="btn btn-primary" onclick={createGroupChat} disabled={selectedUsers.size < 2}>Создать</button>
      </div>
    </div>
  </div>
{/if}
