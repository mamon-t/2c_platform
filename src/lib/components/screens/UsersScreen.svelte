// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { api, type Company, type Role, type User, type Person, type UserContact, type UserProfile, type UserCertificate } from '$lib/services/api';
  import { auth } from '$lib/stores/auth';
  import { confirmDialog } from '$lib/components/ui/dialog';
  import { toastSuccess, toastError, errText } from '$lib/components/ui/toast';
  import PageToolbar from '$lib/components/ui/PageToolbar.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';

  // ── Список ──
  let users = $state<User[]>([]);
  let companies = $state<Company[]>([]);
  let loading = $state(true);

  let showUserForm = $state(false);
  let userForm = $state({ login: '', password: '', last_name: '', first_name: '', middle_name: '', email: '', role_id: '', position: '', department: '' });
  let userError = $state('');
  let createUserRoles = $state<Role[]>([]);

  async function loadUsers() {
    loading = true;
    try { users = await api.listUsers(); }
    catch (e) { toastError(errText(e)); }
    finally { loading = false; }
  }
  onMount(loadUsers);

  async function openUserForm() {
    userForm = { login: '', password: '', last_name: '', first_name: '', middle_name: '', email: '', role_id: '', position: '', department: '' };
    userError = '';
    const companyId = get(auth)?.companyId;
    if (companyId) { try { createUserRoles = await api.listRoles(companyId); } catch { /* тихо */ } }
    showUserForm = true;
  }

  async function saveUser() {
    userError = '';
    try {
      const companyId = get(auth)?.companyId;
      if (!companyId) { userError = 'Не выбрана компания'; return; }
      await api.createUser({
        login: userForm.login,
        password: userForm.password,
        last_name: userForm.last_name || undefined,
        first_name: userForm.first_name || undefined,
        middle_name: userForm.middle_name || undefined,
        display_name: [userForm.last_name, userForm.first_name, userForm.middle_name].filter(Boolean).join(' ') || undefined,
        email: userForm.email || undefined,
        company_id: companyId,
        role_id: userForm.role_id || undefined,
        position: userForm.position || undefined,
        department: userForm.department || undefined,
      });
      showUserForm = false;
      toastSuccess('Пользователь создан');
      await loadUsers();
    } catch (e) { userError = errText(e); }
  }

  async function toggleUserStatus(user: User) {
    const newStatus = user.status === 'disabled' ? 'active' : 'disabled';
    const action = newStatus === 'disabled' ? 'Разблокировать' : 'Заблокировать';
    if (!(await confirmDialog({ title: `${action} пользователя?`, message: user.login, danger: newStatus === 'disabled', confirmLabel: action }))) return;
    try { await api.updateUser(user._id, { status: newStatus }); await loadUsers(); } catch (e) { toastError(errText(e)); }
  }

  // ── Детальная карточка ──
  let showDetail = $state(false);
  let detailTab = $state('basic');
  let detailUser = $state<User | null>(null);
  let detailPerson = $state<Person | null>(null);
  let detailContacts = $state<UserContact[]>([]);
  let detailProfiles = $state<UserProfile[]>([]);
  let detailCerts = $state<UserCertificate[]>([]);
  let contactTypes = $state<Array<{ code: string; name: string }>>([]);
  let editPerson = $state(false);
  let personForm = $state({ last_name: '', first_name: '', middle_name: '', display_name: '' });
  let contactForm = $state({ channel_type: 'email', value: '', is_primary: false, purposes: [] as string[], note: '' });
  let editingContactId = $state<string | null>(null);
  let editContactForm = $state({ value: '', is_primary: false, is_verified: false, purposes: [] as string[], note: '' });
  let profileForm = $state({ company_id: '', role_id: '', position: '', department: '' });
  let profileRoles = $state<Role[]>([]);
  let detailError = $state('');

  async function openDetail(user: User) {
    detailUser = user;
    detailTab = 'basic';
    detailPerson = null;
    detailContacts = [];
    detailProfiles = [];
    detailCerts = [];
    editPerson = false;
    detailError = '';
    personForm = { last_name: '', first_name: '', middle_name: '', display_name: '' };
    showDetail = true;

    try { companies = await api.listCompanies(); } catch { /* тихо */ }
    try { contactTypes = await api.getContactTypes(); }
    catch { contactTypes = [{ code: 'email', name: 'Email' }, { code: 'phone', name: 'Телефон' }, { code: 'telegram', name: 'Telegram' }, { code: 'web', name: 'Веб' }]; }
    if (user.person_id) {
      try {
        detailPerson = await api.getPerson(user.person_id);
        personForm = { last_name: detailPerson.last_name, first_name: detailPerson.first_name, middle_name: detailPerson.middle_name ?? '', display_name: detailPerson.display_name };
      } catch { /* тихо */ }
    }
    try { detailContacts = await api.listUserContacts(user._id); } catch { /* тихо */ }
    try { detailProfiles = await api.listUserProfiles(user._id); } catch { /* тихо */ }
    try { detailCerts = await api.listUserCertificates(user._id); } catch { /* тихо */ }
  }

  function closeDetail() { showDetail = false; detailUser = null; loadUsers(); }

  async function savePerson() {
    detailError = '';
    if (!detailUser?.person_id) return;
    try {
      await api.updatePerson(detailUser.person_id, {
        last_name: personForm.last_name || undefined,
        first_name: personForm.first_name || undefined,
        middle_name: personForm.middle_name || undefined,
        display_name: personForm.display_name || undefined,
      });
      detailPerson = await api.getPerson(detailUser.person_id);
      editPerson = false;
    } catch (e) { detailError = errText(e); }
  }

  async function addContact() {
    detailError = '';
    if (!detailUser || !contactForm.value) return;
    try {
      await api.createContact({ user_id: detailUser._id, channel_type: contactForm.channel_type, value: contactForm.value, is_primary: contactForm.is_primary, purposes: contactForm.purposes, note: contactForm.note || undefined });
      detailContacts = await api.listUserContacts(detailUser._id);
      contactForm = { channel_type: 'email', value: '', is_primary: false, purposes: [], note: '' };
    } catch (e) { detailError = errText(e); }
  }

  function startEditContact(c: UserContact) {
    editingContactId = c._id;
    editContactForm = { value: c.value, is_primary: c.is_primary, is_verified: c.is_verified, purposes: [...c.purposes], note: c.note ?? '' };
  }

  function cancelEditContact() { editingContactId = null; }

  async function saveEditContact(id: string) {
    detailError = '';
    if (!detailUser) return;
    try {
      await api.updateContact(id, { value: editContactForm.value, is_primary: editContactForm.is_primary, is_verified: editContactForm.is_verified, purposes: editContactForm.purposes, note: editContactForm.note || undefined });
      detailContacts = await api.listUserContacts(detailUser._id);
      editingContactId = null;
    } catch (e) { detailError = errText(e); }
  }

  async function deleteContact(id: string) {
    if (!detailUser) return;
    try { await api.deleteContact(id); detailContacts = await api.listUserContacts(detailUser._id); } catch { /* тихо */ }
  }

  async function loadProfileRoles(companyId: string) {
    if (!companyId) { profileRoles = []; return; }
    try { profileRoles = await api.listRoles(companyId); } catch { profileRoles = []; }
    profileForm.role_id = '';
  }

  async function addProfile() {
    detailError = '';
    if (!detailUser || !profileForm.company_id || !profileForm.role_id) { detailError = 'Выберите компанию и роль'; return; }
    try {
      await api.addUserProfile({ user_id: detailUser._id, company_id: profileForm.company_id, role_id: profileForm.role_id, position: profileForm.position || undefined, department: profileForm.department || undefined });
      detailProfiles = await api.listUserProfiles(detailUser._id);
      profileForm = { company_id: '', role_id: '', position: '', department: '' };
    } catch (e) { detailError = errText(e); }
  }

  async function removeProfile(id: string) {
    if (!detailUser) return;
    try { await api.removeUserProfile(id); detailProfiles = await api.listUserProfiles(detailUser._id); } catch { /* тихо */ }
  }

  async function deactivateCert(id: string) {
    if (!detailUser) return;
    try { await api.deactivateCertificate(id); detailCerts = await api.listUserCertificates(detailUser._id); } catch { /* тихо */ }
  }

  // ── Сброс пароля ──
  let showPasswordReset = $state(false);
  let resetPasswordUserId = $state('');
  let resetPasswordLogin = $state('');
  let resetPasswordValue = $state('');
  let resetPasswordError = $state('');

  function openPasswordReset(userId: string, login: string) {
    resetPasswordUserId = userId;
    resetPasswordLogin = login;
    resetPasswordValue = '';
    resetPasswordError = '';
    showPasswordReset = true;
  }

  async function confirmPasswordReset() {
    resetPasswordError = '';
    if (!resetPasswordValue || resetPasswordValue.length < 4) { resetPasswordError = 'Минимум 4 символа'; return; }
    try {
      await api.updateUser(resetPasswordUserId, { new_password: resetPasswordValue, must_change_password: true });
      showPasswordReset = false;
      toastSuccess(`Пароль ${resetPasswordLogin} сброшен`);
    } catch (e) { resetPasswordError = errText(e); }
  }

  // ── Хелперы ──
  const statusLabel = (s: string) => ({ invited: 'Приглашён', active: 'Активен', disabled: 'Заблокирован', locked: 'Заблокирован', archived: 'В архиве' }[s] ?? s);
  const statusCls = (s: string) => s === 'active' ? 'preset-tonal-success' : s === 'invited' ? 'preset-tonal-warning' : 'preset-tonal-error';
  function fmtDate(d: string | null): string {
    if (!d) return '—';
    try { return new Date(d).toLocaleString('ru-RU', { day: '2-digit', month: '2-digit', year: '2-digit', hour: '2-digit', minute: '2-digit' }); } catch { return d; }
  }
</script>

{#if !showDetail}
  <!-- ── Список пользователей ── -->
  <div class="space-y-3">
    <PageToolbar title="Пользователи" icon="fa-solid fa-users">
      <button onclick={openUserForm} class="btn btn-sm preset-filled-primary-500"><i class="fa-solid fa-plus"></i> Добавить</button>
    </PageToolbar>

    {#if loading}
      <Spinner />
    {:else}
      {#if showUserForm}
        <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-4">
          <h3 class="mb-3 text-sm font-semibold text-surface-900-100">Новый пользователь</h3>
          <form onsubmit={(e) => { e.preventDefault(); saveUser(); }} class="space-y-3">
            <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
              <label class="block text-xs font-medium text-surface-700-300">Логин *<input bind:value={userForm.login} class="input mt-1 w-full" required /></label>
              <label class="block text-xs font-medium text-surface-700-300">Пароль *<input bind:value={userForm.password} type="password" class="input mt-1 w-full" required /></label>
              <label class="block text-xs font-medium text-surface-700-300">Роль
                <select bind:value={userForm.role_id} class="select mt-1 w-full"><option value="">Без роли</option>{#each createUserRoles as role}<option value={role._id}>{role.name}</option>{/each}</select></label>
              <label class="block text-xs font-medium text-surface-700-300">Фамилия<input bind:value={userForm.last_name} class="input mt-1 w-full" /></label>
              <label class="block text-xs font-medium text-surface-700-300">Имя<input bind:value={userForm.first_name} class="input mt-1 w-full" /></label>
              <label class="block text-xs font-medium text-surface-700-300">Отчество<input bind:value={userForm.middle_name} class="input mt-1 w-full" /></label>
              <label class="block text-xs font-medium text-surface-700-300">Email<input bind:value={userForm.email} type="email" class="input mt-1 w-full" /></label>
              <label class="block text-xs font-medium text-surface-700-300">Должность<input bind:value={userForm.position} class="input mt-1 w-full" /></label>
              <label class="block text-xs font-medium text-surface-700-300">Отдел<input bind:value={userForm.department} class="input mt-1 w-full" /></label>
            </div>
            {#if userError}<div class="text-sm text-error-600" role="alert">{userError}</div>{/if}
            <div class="flex gap-2">
              <button type="submit" class="btn btn-sm preset-filled-primary-500">Создать</button>
              <button type="button" onclick={() => (showUserForm = false)} class="btn btn-sm btn-outline">Отмена</button>
            </div>
          </form>
        </div>
      {/if}

      <div class="overflow-x-auto rounded-lg border border-surface-300-700">
        <table class="table table-dense w-full text-left">
          <thead><tr><th>Логин</th><th>Имя</th><th>Статус</th><th>Последний вход</th><th class="text-right">Действия</th></tr></thead>
          <tbody>
            {#each users as user (user._id)}
              <tr class="cursor-pointer" onclick={() => openDetail(user)}
                onkeydown={(e) => { if (e.key === 'Enter') openDetail(user); }}>
                <td class="font-mono text-xs">{user.login}</td>
                <td>{user.display_name}</td>
                <td><span class="badge {statusCls(user.status)}">{statusLabel(user.status)}</span></td>
                <td class="text-xs text-surface-600-400">{fmtDate(user.last_login_at)}</td>
                <td class="text-right">
                  <button onclick={(e) => { e.stopPropagation(); toggleUserStatus(user); }}
                    class="btn btn-xs {user.status === 'disabled' ? 'preset-tonal-success' : 'btn-outline'}">
                    {user.status === 'disabled' ? 'Разблокировать' : 'Заблокировать'}
                  </button>
                </td>
              </tr>
            {:else}
              <tr><td colspan="5" class="py-6 text-center text-surface-400">Нет пользователей</td></tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
{:else if detailUser}
  <!-- ── Карточка пользователя ── -->
  <div class="space-y-3">
    <div class="flex items-center gap-3">
      <button onclick={closeDetail} class="rounded p-1.5 text-surface-500-500 hover:bg-surface-200-800" aria-label="Назад к списку">
        <i class="fa-solid fa-arrow-left"></i>
      </button>
      <h2 class="text-xl font-bold text-surface-900-100">{detailUser.display_name}</h2>
      <span class="badge {statusCls(detailUser.status)}">{statusLabel(detailUser.status)}</span>
    </div>

    <div class="flex gap-1 rounded-lg border border-surface-300-700 bg-surface-100-900 p-1">
      {#each [{ code: 'basic', label: 'Основное', icon: 'fa-user' }, { code: 'contacts', label: 'Контакты', icon: 'fa-envelope' }, { code: 'profiles', label: 'Компании', icon: 'fa-building' }, { code: 'certs', label: 'Сертификаты', icon: 'fa-certificate' }] as tab}
        <button onclick={() => (detailTab = tab.code)} aria-label={tab.label}
          class="flex items-center gap-2 rounded-md px-3 py-1.5 text-sm transition-colors {detailTab === tab.code ? 'bg-surface-50-950 font-medium text-primary-600 shadow-sm' : 'text-surface-600-400 hover:text-surface-900-100'}">
          <i class="fa-solid {tab.icon} text-xs"></i>{tab.label}
        </button>
      {/each}
    </div>

    {#if detailError}<div class="rounded-lg bg-error-500/10 p-3 text-sm text-error-600" role="alert">{detailError}</div>{/if}

    {#if detailTab === 'basic'}
      <div class="space-y-4 rounded-xl border border-surface-300-700 bg-surface-50-950 p-4">
        <div class="flex items-center justify-between">
          <h3 class="text-sm font-semibold text-surface-900-100">Личные данные</h3>
          <div class="flex gap-3">
            {#if detailUser.person_id}
              <button onclick={() => (editPerson = !editPerson)} class="text-sm text-primary-600 hover:underline">{editPerson ? 'Отмена' : 'Редактировать'}</button>
            {/if}
            <button onclick={() => openPasswordReset(detailUser!._id, detailUser!.login)} class="text-sm text-warning-600 hover:underline">Сбросить пароль</button>
          </div>
        </div>

        <div class="grid grid-cols-1 gap-3 border-b border-surface-300-700 pb-4 text-sm md:grid-cols-3">
          <div><span class="text-surface-500-500">Логин:</span> <span class="font-mono text-surface-900-100">{detailUser.login}</span></div>
          <div><span class="text-surface-500-500">Статус:</span> <span class="badge {statusCls(detailUser.status)}">{statusLabel(detailUser.status)}</span></div>
          <div><span class="text-surface-500-500">Последний вход:</span> <span class="text-surface-900-100">{fmtDate(detailUser.last_login_at)}</span></div>
        </div>
        {#if editPerson}
          <form onsubmit={(e) => { e.preventDefault(); savePerson(); }} class="space-y-3">
            <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
              <label class="block text-xs font-medium text-surface-700-300">Фамилия<input bind:value={personForm.last_name} class="input mt-1 w-full" /></label>
              <label class="block text-xs font-medium text-surface-700-300">Имя<input bind:value={personForm.first_name} class="input mt-1 w-full" /></label>
              <label class="block text-xs font-medium text-surface-700-300">Отчество<input bind:value={personForm.middle_name} class="input mt-1 w-full" /></label>
            </div>
            <label class="block text-xs font-medium text-surface-700-300">Отображаемое имя<input bind:value={personForm.display_name} class="input mt-1 w-full" /></label>
            <button type="submit" class="btn btn-sm preset-filled-primary-500">Сохранить</button>
          </form>
        {:else if detailPerson}
          <div class="grid grid-cols-1 gap-3 text-sm md:grid-cols-3">
            <div><span class="text-surface-500-500">Фамилия:</span> <span class="text-surface-900-100">{detailPerson.last_name || '—'}</span></div>
            <div><span class="text-surface-500-500">Имя:</span> <span class="text-surface-900-100">{detailPerson.first_name || '—'}</span></div>
            <div><span class="text-surface-500-500">Отчество:</span> <span class="text-surface-900-100">{detailPerson.middle_name || '—'}</span></div>
          </div>
        {:else}
          <p class="text-sm text-surface-500-500">Нет данных</p>
        {/if}
      </div>

    {:else if detailTab === 'contacts'}
      {@render contactsTab()}

    {:else if detailTab === 'profiles'}
      <div class="space-y-4 rounded-xl border border-surface-300-700 bg-surface-50-950 p-4">
        <h3 class="text-sm font-semibold text-surface-900-100">Рабочие профили</h3>
        {#if detailProfiles.length > 0}
          <div class="space-y-2">
            {#each detailProfiles as p (p._id)}
              <div class="flex items-center justify-between rounded-lg border border-surface-300-700 bg-surface-100-900 px-3 py-2">
                <div>
                  <span class="text-sm font-medium text-surface-900-100">{p.company_name}</span>
                  <span class="ml-2 text-xs text-surface-500-500">({p.role_name})</span>
                  {#if p.position}<span class="ml-2 text-xs text-surface-500-500">· {p.position}</span>{/if}
                </div>
                <button onclick={() => removeProfile(p._id)} class="rounded p-1.5 text-error-600 hover:bg-error-500/10" title="Удалить профиль" aria-label={`Удалить профиль ${p.company_name}`}>
                  <i class="fa-solid fa-trash text-xs"></i>
                </button>
              </div>
            {/each}
          </div>
        {/if}
        <form onsubmit={(e) => { e.preventDefault(); addProfile(); }} class="space-y-2">
          <div class="flex flex-wrap gap-2">
            <select bind:value={profileForm.company_id} onchange={(e) => loadProfileRoles(e.currentTarget.value)} class="select w-56">
              <option value="">Компания</option>
              {#each companies as c (c._id)}<option value={c._id}>{c.name}</option>{/each}
            </select>
            <select bind:value={profileForm.role_id} class="select w-48">
              <option value="">Роль</option>
              {#each profileRoles as r (r._id)}<option value={r._id}>{r.name}</option>{/each}
            </select>
          </div>
          <div class="flex flex-wrap gap-2">
            <input bind:value={profileForm.position} class="input w-48" placeholder="Должность" />
            <input bind:value={profileForm.department} class="input w-48" placeholder="Отдел" />
            <button type="submit" class="btn btn-sm preset-filled-primary-500">Добавить</button>
          </div>
        </form>
      </div>

    {:else if detailTab === 'certs'}
      <div class="space-y-4 rounded-xl border border-surface-300-700 bg-surface-50-950 p-4">
        <h3 class="text-sm font-semibold text-surface-900-100">Сертификаты</h3>
        {#if detailCerts.length > 0}
          <div class="space-y-2">
            {#each detailCerts as cert (cert._id)}
              <div class="flex items-center justify-between rounded-lg border border-surface-300-700 bg-surface-100-900 px-3 py-2">
                <div>
                  <span class="text-sm font-medium text-surface-900-100">{cert.subject}</span>
                  <div class="text-xs text-surface-500-500">{cert.issuer} · {cert.serial_number}</div>
                </div>
                {#if cert.is_active}
                  <button onclick={() => deactivateCert(cert._id)} class="rounded p-1.5 text-error-600 hover:bg-error-500/10" title="Деактивировать" aria-label={`Деактивировать сертификат ${cert.subject}`}>
                    <i class="fa-solid fa-ban text-xs"></i>
                  </button>
                {:else}
                  <span class="text-xs text-surface-500-500">Деактивирован</span>
                {/if}
              </div>
            {/each}
          </div>
        {:else}
          <p class="text-sm text-surface-500-500">Нет сертификатов</p>
        {/if}
      </div>
    {/if}
  </div>

  {#if showPasswordReset}
    <div class="fixed inset-0 z-[80] grid place-items-center bg-black/50" role="presentation">
      <div class="card w-96 max-w-[92vw] space-y-3 p-4" role="dialog" aria-modal="true" aria-label="Сброс пароля">
        <h3 class="text-sm font-semibold">Сброс пароля — {resetPasswordLogin}</h3>
        <label class="block text-xs font-medium text-surface-700-300">
          Новый пароль
          <input bind:value={resetPasswordValue} class="input mt-1 w-full" />
        </label>
        <p class="text-xs text-surface-500-500">Пользователь обязан сменить пароль при следующем входе.</p>
        {#if resetPasswordError}<div class="text-sm text-error-600" role="alert">{resetPasswordError}</div>{/if}
        <div class="flex justify-end gap-2">
          <button class="btn btn-sm btn-outline" onclick={() => (showPasswordReset = false)}>Отмена</button>
          <button class="btn btn-sm preset-filled-primary-500" onclick={confirmPasswordReset}>Сбросить</button>
        </div>
      </div>
    </div>
  {/if}
{/if}

{#snippet contactsTab()}
  <div class="space-y-4 rounded-xl border border-surface-300-700 bg-surface-50-950 p-4">
    <h3 class="text-sm font-semibold text-surface-900-100">Контакты</h3>
    {#if detailContacts.length > 0}
      <div class="space-y-2">
        {#each detailContacts as c (c._id)}
          {#if editingContactId === c._id}
            <div class="space-y-2 rounded-lg border border-primary-500/50 bg-surface-100-900 p-3">
              <input bind:value={editContactForm.value} class="input w-full" placeholder="Значение" aria-label="Значение контакта" />
              <div class="flex flex-wrap items-center gap-3">
                <label class="flex items-center gap-1 text-xs text-surface-700-300">
                  <input type="checkbox" bind:checked={editContactForm.is_primary} /> Основной
                </label>
                <label class="flex items-center gap-1 text-xs text-surface-700-300">
                  <input type="checkbox" bind:checked={editContactForm.is_verified} /> Подтверждён
                </label>
                {#each [['login', 'Вход'], ['notifications', 'Уведомления'], ['personal', 'Личный'], ['work', 'Рабочий']] as [purpose, label]}
                  <label class="flex items-center gap-1 text-xs text-surface-700-300">
                    <input type="checkbox" checked={editContactForm.purposes.includes(purpose)}
                      onchange={(e) => { const v = (e.target as HTMLInputElement).checked; editContactForm.purposes = v ? [...editContactForm.purposes, purpose] : editContactForm.purposes.filter((p) => p !== purpose); }} />
                    {label}
                  </label>
                {/each}
              </div>
              <input bind:value={editContactForm.note} class="input w-full" placeholder='Заметка (напр. "не звонить после 20:00")' aria-label="Заметка" />
              <div class="flex gap-2">
                <button onclick={() => saveEditContact(c._id)} class="btn btn-sm preset-filled-primary-500">Сохранить</button>
                <button onclick={cancelEditContact} class="btn btn-sm btn-outline">Отмена</button>
              </div>
            </div>
          {:else}
            <div class="flex items-start justify-between rounded-lg border border-surface-300-700 bg-surface-100-900 px-3 py-2">
              <div class="space-y-1">
                <div class="flex flex-wrap items-center gap-2">
                  <span class="text-xs font-medium uppercase text-surface-500-500">{c.channel_type}</span>
                  <span class="text-sm text-surface-900-100">{c.value}</span>
                  {#if c.is_primary}<span class="badge preset-tonal-primary text-[10px]">Основной</span>{/if}
                  {#if c.is_verified}<span class="badge preset-tonal-success text-[10px]">Подтверждён</span>{/if}
                </div>
                {#if c.purposes.length > 0}
                  <div class="flex gap-1">
                    {#each c.purposes as p}<span class="rounded bg-surface-200-800 px-1 text-[10px] text-surface-600-400">{p}</span>{/each}
                  </div>
                {/if}
                {#if c.note}<p class="text-xs italic text-surface-500-500">{c.note}</p>{/if}
              </div>
              <div class="ml-2 flex shrink-0 gap-1">
                <button onclick={() => startEditContact(c)} class="rounded p-1.5 text-surface-500-500 hover:bg-surface-200-800" title="Редактировать" aria-label={`Редактировать контакт ${c.value}`}>
                  <i class="fa-solid fa-pen text-xs"></i>
                </button>
                <button onclick={() => deleteContact(c._id)} class="rounded p-1.5 text-error-600 hover:bg-error-500/10" title="Удалить" aria-label={`Удалить контакт ${c.value}`}>
                  <i class="fa-solid fa-trash text-xs"></i>
                </button>
              </div>
            </div>
          {/if}
        {/each}
      </div>
    {/if}
    <form onsubmit={(e) => { e.preventDefault(); addContact(); }} class="space-y-2 rounded-lg border border-dashed border-surface-300-700 p-3">
      <div class="flex gap-2">
        <select bind:value={contactForm.channel_type} class="select w-36 shrink-0" aria-label="Тип контакта">
          {#each contactTypes as ct (ct.code)}<option value={ct.code}>{ct.name}</option>{/each}
        </select>
        <input bind:value={contactForm.value} class="input flex-1" placeholder="Значение" aria-label="Значение нового контакта" />
      </div>
      <div class="flex flex-wrap items-center gap-3">
        <label class="flex items-center gap-1 text-xs text-surface-700-300">
          <input type="checkbox" bind:checked={contactForm.is_primary} /> Основной
        </label>
        {#each [['login', 'Вход'], ['notifications', 'Уведомления'], ['personal', 'Личный'], ['work', 'Рабочий']] as [purpose, label]}
          <label class="flex items-center gap-1 text-xs text-surface-700-300">
            <input type="checkbox" checked={contactForm.purposes.includes(purpose)}
              onchange={(e) => { const v = (e.target as HTMLInputElement).checked; contactForm.purposes = v ? [...contactForm.purposes, purpose] : contactForm.purposes.filter((p) => p !== purpose); }} />
            {label}
          </label>
        {/each}
      </div>
      <input bind:value={contactForm.note} class="input w-full" placeholder='Заметка (напр. "не звонить после 20:00")' aria-label="Заметка" />
      <button type="submit" class="btn btn-sm preset-filled-primary-500">Добавить</button>
    </form>
  </div>
{/snippet}
