<script lang="ts">
  import { theme } from '$lib/stores/theme';
  import { allNavItems } from '$lib/stores/navigation';
  import { auth, isAuthenticated, hasPermission, type AuthUser } from '$lib/stores/auth';
  import { get } from 'svelte/store';
  import { api } from '$lib/services/api';
  import type { DiagnosticsReport, Company, User, Role, UserProfile, Person, UserContact, UserCertificate } from '$lib/services/api';
  import { onMount } from 'svelte';
  import AuditPage from '$lib/components/AuditPage.svelte';
  import SettingsPage from '$lib/components/SettingsPage.svelte';
  import EventsPage from '$lib/components/EventsPage.svelte';
  import MetadataPage from '$lib/components/MetadataPage.svelte';
  import ObjectsPage from '$lib/components/ObjectsPage.svelte';
  import ConvertPage from '$lib/components/ConvertPage.svelte';
  import PrintPage from '$lib/components/PrintPage.svelte';
  import NumberingPage from '$lib/components/NumberingPage.svelte';
  import ScriptsPage from '$lib/components/ScriptsPage.svelte';
  import ReportsPage from '$lib/components/ReportsPage.svelte';

  let sidebarCollapsed = $state(false);
  let diagnostics = $state<DiagnosticsReport | null>(null);
  let loading = $state(true);
  let currentNav = $state('dashboard');
  let connected = $state(false);
  let currentUser = $state<User | null>(null);
  let authUserData = $state<AuthUser | null>(null);

  let filteredNavItems = $derived(
    allNavItems.filter(item =>
      !item.requiredPermission ||
      !authUserData ||
      authUserData.permissions.length === 0 ||
      hasPermission(authUserData.permissions, item.requiredPermission.subsystem, item.requiredPermission.action)
    )
  );

  // Login
  let loginLogin = $state('');
  let loginPassword = $state('');
  let loginError = $state('');
  let loginLoading = $state(false);

  // Connection
  let dbUri = $state('mongodb://db_user:db_password@192.168.31.31:27017');
  let dbName = $state('2c_platform');
  let dbError = $state('');
  let dbLoading = $state(false);

  // CRUD data
  let companies = $state<Company[]>([]);
  let users = $state<User[]>([]);
  let roles = $state<Role[]>([]);

  // Company form
  let showCompanyForm = $state(false);
  let editingCompanyId = $state<string | null>(null);
  let companyForm = $state({ code: '', name: '', inn: '' });
  let companyError = $state('');

  // User form
  let showUserForm = $state(false);
  let userForm = $state({ login: '', password: '', last_name: '', first_name: '', middle_name: '', email: '', role_id: '', position: '', department: '' });
  let userError = $state('');
  let createUserRoles = $state<Role[]>([]);

  // Role form
  let showRoleForm = $state(false);
  let roleForm = $state({ code: '', name: '', description: '' });
  let roleError = $state('');

  // User detail view
  let showUserDetail = $state(false);
  let detailUser = $state<User | null>(null);
  let detailTab = $state('basic');
  let detailPerson = $state<Person | null>(null);
  let detailContacts = $state<UserContact[]>([]);
  let detailProfiles = $state<UserProfile[]>([]);
  let detailCerts = $state<UserCertificate[]>([]);
  let detailRoles = $state<Role[]>([]);
  let contactTypes = $state<Array<{code: string; name: string}>>([]);
  let editPerson = $state(false);
  let personForm = $state({ last_name: '', first_name: '', middle_name: '', display_name: '' });
  let contactForm = $state({ channel_type: 'email', value: '', is_primary: false, purposes: [] as string[], note: '' });
  let editingContactId = $state<string | null>(null);
  let editContactForm = $state({ value: '', is_primary: false, is_verified: false, purposes: [] as string[], note: '' });
  let profileForm = $state({ company_id: '', role_id: '', position: '', department: '' });
  let profileModalRoles = $state<Role[]>([]);
  let detailError = $state('');

  function toggleSidebar() { sidebarCollapsed = !sidebarCollapsed; }
  function setNav(code: string) { currentNav = code; }

  async function handleConnect() {
    dbLoading = true;
    dbError = '';
    try {
      await api.connectDb(dbUri, dbName);
      connected = true;
      diagnostics = await api.getDiagnostics();
    } catch (e: any) {
      dbError = typeof e === 'string' ? e : e?.message ?? 'Ошибка подключения';
    } finally { dbLoading = false; }
  }

  async function handleLogin() {
    loginLoading = true;
    loginError = '';
    try {
      const result = await api.authenticate(loginLogin, loginPassword);
      const lastCompanyId = auth.getLastCompanyId();
      let selectedCompanyId = result.companies[0]?.company_id ?? '';
      let selectedRoleId = result.companies[0]?.role_id ?? '';

      if (lastCompanyId && result.companies.some((c) => c.company_id === lastCompanyId)) {
        selectedCompanyId = lastCompanyId;
        const match = result.companies.find((c) => c.company_id === lastCompanyId);
        if (match) selectedRoleId = match.role_id;
      }

      let perms: AuthUser['permissions'] = [];
      try {
        const myPerms = await api.getMyPermissions();
        perms = myPerms.permissions.map(p => ({ subsystemCode: p.subsystem_code, actions: p.actions, recordScope: p.record_scope, deny: p.deny }));
      } catch {}

      auth.login({
        userId: result.user._id,
        companyId: selectedCompanyId,
        roleId: selectedRoleId,
        roleCode: result.role_code ?? 'SUPERADMIN',
        roleName: result.role_name ?? '',
        login: result.user.login,
        displayName: result.user.display_name,
        companies: result.companies.map((c) => ({
          companyId: c.company_id,
          companyName: c.company_name,
          companyCode: c.company_code,
          roleId: c.role_id,
          roleName: c.role_name,
        })),
        permissions: perms,
      });
      authUserData = get(auth);
      currentUser = result.user;
    } catch (e: any) {
      loginError = typeof e === 'string' ? e : e?.message ?? 'Ошибка авторизации';
    } finally { loginLoading = false; }
  }

  function handleLogout() { auth.logout(); currentUser = null; authUserData = null; }

  async function handleExit() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().destroy();
    } catch { window.close(); }
  }

  async function handleSwitchCompany(companyId: string) {
    try {
      const result = await api.switchCompany(companyId);
      let perms: AuthUser['permissions'] = [];
      try {
        const myPerms = await api.getMyPermissions();
        perms = myPerms.permissions.map(p => ({ subsystemCode: p.subsystem_code, actions: p.actions, recordScope: p.record_scope, deny: p.deny }));
        auth.switchCompany(companyId, myPerms.role_code, myPerms.role_name, perms);
      } catch {
        auth.switchCompany(companyId, result.role_code ?? 'SUPERADMIN', result.role_name ?? '', []);
      }
      authUserData = get(auth);
      currentUser = result.user;
    } catch (e: any) {
      console.error('Ошибка смены компании:', e);
    }
  }

  async function loadCompanies() { try { companies = await api.listCompanies(); } catch {} }
  async function loadUsers() { try { users = await api.listUsers(); } catch {} }
  async function loadRoles(cid?: string) {
    const companyId = cid ?? get(auth)?.companyId;
    if (companyId) { try { roles = await api.listRoles(companyId); } catch {} }
  }

  function openCompanyForm(company?: Company) {
    if (company) {
      editingCompanyId = company._id;
      companyForm = { code: company.code, name: company.name, inn: company.inn ?? '' };
    } else {
      editingCompanyId = null;
      companyForm = { code: '', name: '', inn: '' };
    }
    companyError = '';
    showCompanyForm = true;
  }

  async function saveCompany() {
    companyError = '';
    try {
      if (editingCompanyId) {
        await api.updateCompany(editingCompanyId, { name: companyForm.name, inn: companyForm.inn || undefined });
      } else {
        await api.createCompany({ code: companyForm.code, name: companyForm.name, inn: companyForm.inn || undefined });
      }
      showCompanyForm = false;
      await loadCompanies();
    } catch (e: any) { companyError = typeof e === 'string' ? e : e?.message ?? 'Ошибка сохранения'; }
  }

  async function deleteCompany(id: string) {
    if (!confirm('Удалить компанию?')) return;
    try { await api.deleteCompany(id); await loadCompanies(); } catch {}
  }

  async function openUserForm() {
    userForm = { login: '', password: '', last_name: '', first_name: '', middle_name: '', email: '', role_id: '', position: '', department: '' };
    userError = '';
    const companyId = get(auth)?.companyId;
    if (companyId) { try { createUserRoles = await api.listRoles(companyId); } catch {} }
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
      await loadUsers();
    } catch (e: any) { userError = typeof e === 'string' ? e : e?.message ?? 'Ошибка сохранения'; }
  }

  async function toggleUserStatus(user: User) {
    const newStatus = user.status === 'disabled' ? 'active' : 'disabled';
    const action = newStatus === 'disabled' ? 'Заблокировать' : 'Разблокировать';
    if (!confirm(`${action} пользователя ${user.login}?`)) return;
    try { await api.updateUser(user._id, { status: newStatus }); await loadUsers(); } catch {}
  }

  async function openUserDetail(user: User) {
    detailUser = user;
    detailTab = 'basic';
    detailPerson = null;
    detailContacts = [];
    detailProfiles = [];
    detailCerts = [];
    detailRoles = [];
    editPerson = false;
    detailError = '';
    personForm = { last_name: '', first_name: '', middle_name: '', display_name: '' };
    showUserDetail = true;

    try { await loadCompanies(); } catch {}
    try { contactTypes = await api.getContactTypes(); } catch { contactTypes = [{ code: 'email', name: 'Email' }, { code: 'phone', name: 'Телефон' }, { code: 'telegram', name: 'Telegram' }, { code: 'web', name: 'Веб' }]; }
    if (user.person_id) {
      try { detailPerson = await api.getPerson(user.person_id); personForm = { last_name: detailPerson.last_name, first_name: detailPerson.first_name, middle_name: detailPerson.middle_name ?? '', display_name: detailPerson.display_name }; } catch {}
    }
    try { detailContacts = await api.listUserContacts(user._id); } catch {}
    try { detailProfiles = await api.listUserProfiles(user._id); } catch {}
    try { detailCerts = await api.listUserCertificates(user._id); } catch {}
  }

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
    } catch (e: any) { detailError = typeof e === 'string' ? e : e?.message ?? 'Ошибка'; }
  }

  async function addContact() {
    detailError = '';
    if (!detailUser || !contactForm.value) return;
    try {
      await api.createContact({ user_id: detailUser._id, channel_type: contactForm.channel_type, value: contactForm.value, is_primary: contactForm.is_primary, purposes: contactForm.purposes, note: contactForm.note || undefined });
      detailContacts = await api.listUserContacts(detailUser._id);
      contactForm = { channel_type: 'email', value: '', is_primary: false, purposes: [], note: '' };
    } catch (e: any) { detailError = typeof e === 'string' ? e : e?.message ?? 'Ошибка'; }
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
    } catch (e: any) { detailError = typeof e === 'string' ? e : e?.message ?? 'Ошибка'; }
  }

  async function deleteContact(id: string) {
    if (!detailUser) return;
    try { await api.deleteContact(id); detailContacts = await api.listUserContacts(detailUser._id); } catch {}
  }

  async function loadProfileRoles(companyId: string) {
    if (!companyId) { profileModalRoles = []; return; }
    try { profileModalRoles = await api.listRoles(companyId); } catch { profileModalRoles = []; }
    profileForm.role_id = '';
  }

  async function addProfile() {
    detailError = '';
    if (!detailUser || !profileForm.company_id || !profileForm.role_id) { detailError = 'Выберите компанию и роль'; return; }
    try {
      await api.addUserProfile({ user_id: detailUser._id, company_id: profileForm.company_id, role_id: profileForm.role_id, position: profileForm.position || undefined, department: profileForm.department || undefined });
      detailProfiles = await api.listUserProfiles(detailUser._id);
      profileForm = { company_id: '', role_id: '', position: '', department: '' };
    } catch (e: any) { detailError = typeof e === 'string' ? e : e?.message ?? 'Ошибка'; }
  }

  async function removeProfile(id: string) {
    if (!detailUser) return;
    try { await api.removeUserProfile(id); detailProfiles = await api.listUserProfiles(detailUser._id); } catch {}
  }

  async function deactivateCert(id: string) {
    if (!detailUser) return;
    try { await api.deactivateCertificate(id); detailCerts = await api.listUserCertificates(detailUser._id); } catch {}
  }

  async function saveRole() {
    roleError = '';
    try {
      const companyId = get(auth)?.companyId;
      if (!companyId) return;
      await api.createRole({ company_id: companyId, ...roleForm, description: roleForm.description || undefined });
      showRoleForm = false;
      roleForm = { code: '', name: '', description: '' };
      await loadRoles(companyId);
    } catch (e: any) { roleError = typeof e === 'string' ? e : e?.message ?? 'Ошибка сохранения'; }
  }

  async function deleteRole(id: string) {
    if (!confirm('Удалить роль?')) return;
    try { await api.deleteRole(id); await loadRoles(); } catch {}
  }

  // Password reset
  let showPasswordReset = $state(false);
  let resetPasswordUserId = $state('');
  let resetPasswordLogin = $state('');
  let resetPasswordValue = $state('');
  let resetPasswordError = $state('');

  const statusLabel = (s: string) => ({ invited: 'Приглашён', active: 'Активен', disabled: 'Заблокирован', locked: 'Заблокирован', archived: 'В архиве' }[s] ?? s);
  const statusCls = (s: string) => s === 'active' ? 'bg-success-500/20 text-success-700' : s === 'invited' ? 'bg-warning-500/20 text-warning-700' : 'bg-error-500/20 text-error-700';
  function fmtDate(d: string | null): string {
    if (!d) return '—';
    try { return new Date(d).toLocaleString('ru-RU', { day: '2-digit', month: '2-digit', year: '2-digit', hour: '2-digit', minute: '2-digit' }); } catch { return d; }
  }
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
    } catch (e: any) { resetPasswordError = typeof e === 'string' ? e : e?.message ?? 'Ошибка'; }
  }

  onMount(async () => {
    theme.init();
    try {
      const me = await api.getMe();
      if (me) {
        auth.restore();
        authUserData = get(auth);
        currentUser = me;
        try {
          const myPerms = await api.getMyPermissions();
          if (authUserData) {
            authUserData.permissions = myPerms.permissions.map(p => ({ subsystemCode: p.subsystem_code, actions: p.actions, recordScope: p.record_scope, deny: p.deny }));
            authUserData.roleCode = myPerms.role_code;
            authUserData.roleName = myPerms.role_name;
          }
        } catch {}
        diagnostics = await api.getDiagnostics();
        connected = diagnostics?.mongodb.ok ?? false;
      } else {
        localStorage.removeItem('2c-user');
        localStorage.removeItem('2c-token');
        localStorage.removeItem('2c-company');
      }
    } catch {
      localStorage.removeItem('2c-user');
      localStorage.removeItem('2c-token');
      localStorage.removeItem('2c-company');
    } finally { loading = false; }
  });

  $effect(() => {
    if (currentUser && currentNav === 'companies') loadCompanies();
    if (currentUser && currentNav === 'users') loadUsers();
    if (currentUser && currentNav === 'roles') loadRoles();
  });

  const inputCls = 'w-full rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 placeholder:text-surface-400-600 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500';
  const btnPrimary = 'rounded-lg bg-primary-500 px-4 py-2 text-sm font-medium text-white hover:bg-primary-600 disabled:opacity-50';
  const btnDanger = 'rounded-lg bg-error-500 px-3 py-1 text-xs font-medium text-white hover:bg-error-600';
  const btnSecondary = 'rounded-lg border border-surface-300-700 px-4 py-2 text-sm text-surface-700-300 hover:bg-surface-200-800';
</script>

{#if !connected && !loading}
<div class="flex h-screen items-center justify-center bg-surface-50-950">
  <div class="w-full max-w-md space-y-6 rounded-2xl border border-surface-300-700 bg-surface-50-950 p-8 shadow-xl">
    <div class="text-center">
      <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-primary-500 text-2xl font-bold text-white">2C</div>
      <h1 class="mt-4 text-xl font-bold text-surface-900-100">Подключение к базе данных</h1>
      <p class="mt-1 text-sm text-surface-500-500">Введите параметры подключения к MongoDB</p>
    </div>
    <form onsubmit={(e) => { e.preventDefault(); handleConnect(); }} class="space-y-4">
      <label class="block text-sm font-medium text-surface-700-300">
        URI подключения
        <input bind:value={dbUri} class={inputCls + ' mt-1'} placeholder="mongodb://user:pass@host:port" />
      </label>
      <label class="block text-sm font-medium text-surface-700-300">
        Имя базы данных
        <input bind:value={dbName} class={inputCls + ' mt-1'} placeholder="2c_platform" />
      </label>
      {#if dbError}<div class="rounded-lg bg-error-500/10 p-3 text-sm text-error-600">{dbError}</div>{/if}
      <button type="submit" disabled={dbLoading} class={btnPrimary + ' w-full'}>{dbLoading ? 'Подключение...' : 'Подключиться'}</button>
    </form>
  </div>
</div>

{:else if connected && !$isAuthenticated && !loading}
<div class="flex h-screen items-center justify-center bg-surface-50-950">
  <div class="w-full max-w-md space-y-6 rounded-2xl border border-surface-300-700 bg-surface-50-950 p-8 shadow-xl">
    <div class="text-center">
      <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-primary-500 text-2xl font-bold text-white">2C</div>
      <h1 class="mt-4 text-xl font-bold text-surface-900-100">Вход в систему</h1>
      <p class="mt-1 text-sm text-surface-500-500">При первом входе: <strong>admin</strong> / <strong>admin</strong></p>
    </div>
    <form onsubmit={(e) => { e.preventDefault(); handleLogin(); }} class="space-y-4">
      <label class="block text-sm font-medium text-surface-700-300">
        Логин
        <input bind:value={loginLogin} class={inputCls + ' mt-1'} placeholder="admin" autofocus />
      </label>
      <label class="block text-sm font-medium text-surface-700-300">
        Пароль
        <input bind:value={loginPassword} type="password" class={inputCls + ' mt-1'} placeholder="••••••••" />
      </label>
      {#if loginError}<div class="rounded-lg bg-error-500/10 p-3 text-sm text-error-600">{loginError}</div>{/if}
      <button type="submit" disabled={loginLoading} class={btnPrimary + ' w-full'}>{loginLoading ? 'Вход...' : 'Войти'}</button>
    </form>
  </div>
</div>

{:else}
<div class="flex h-screen overflow-hidden bg-surface-50-950">
  <main class="flex-1 overflow-y-auto">
    <header class="flex items-center justify-between border-b border-surface-300-700 bg-surface-50-950 px-6 py-4">
      <h1 class="text-lg font-semibold text-surface-900-100">{filteredNavItems.find((n: any) => n.code === currentNav)?.label ?? 'Главная'}</h1>
      <div class="flex items-center gap-4">
        <span class="rounded-full px-2 py-0.5 text-xs font-medium {diagnostics?.mongodb.ok ? 'bg-success-500/20 text-success-700' : 'bg-error-500/20 text-error-700'}">
          DB {diagnostics?.mongodb.ok ? 'OK' : 'ERR'}
        </span>
        <button onclick={() => theme.set($theme === 'dark' ? 'light' : 'dark')} class="rounded-lg p-2 text-surface-500-500 hover:bg-surface-200-800" title="Сменить тему">
          <i class="fa-solid {$theme === 'dark' ? 'fa-sun' : 'fa-moon'}"></i>
        </button>
      </div>
    </header>

    <div class="p-6">
      {#if currentNav === 'dashboard'}
        {#if loading}
          <div class="flex items-center justify-center p-12"><div class="h-8 w-8 animate-spin rounded-full border-4 border-primary-500 border-t-transparent"></div></div>
        {:else if diagnostics}
          <div class="space-y-6">
            <h2 class="text-2xl font-bold text-surface-900-100">Главная</h2>
            <div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
              <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
                <div class="text-sm font-medium text-surface-500-500">Версия</div>
                <div class="mt-1 text-2xl font-bold text-surface-900-100">{diagnostics.app_version}</div>
              </div>
              <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
                <div class="text-sm font-medium text-surface-500-500">MongoDB</div>
                <div class="mt-1 flex items-center gap-2">
                  <span class="text-2xl font-bold text-surface-900-100">{diagnostics.mongodb.connected ? 'Подключено' : 'Отключено'}</span>
                  <span class="rounded-full px-2 py-0.5 text-xs font-medium text-white {diagnostics.mongodb.ok ? 'bg-success-500' : 'bg-error-500'}">{diagnostics.mongodb.ok ? 'OK' : 'ERR'}</span>
                </div>
                {#if diagnostics.mongodb.version}<div class="mt-1 text-xs text-surface-500-500">v{diagnostics.mongodb.version}</div>{/if}
                {#if diagnostics.mongodb.replica_set}<div class="mt-1 text-xs text-surface-500-500">RS: {diagnostics.mongodb.replica_set}</div>{/if}
              </div>
              <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
                <div class="text-sm font-medium text-surface-500-500">Модули</div>
                <div class="mt-1 text-2xl font-bold text-surface-900-100">{diagnostics.modules.length}</div>
                <div class="mt-2 space-y-1">
                  {#each diagnostics.modules as mod}
                    <div class="flex items-center gap-2 text-xs"><span class="h-2 w-2 rounded-full bg-success-500"></span><span class="text-surface-700-300">{mod.name} v{mod.version}</span></div>
                  {/each}
                </div>
              </div>
            </div>
          </div>
        {/if}

      {:else if currentNav === 'companies'}
        <div class="space-y-4">
          <div class="flex items-center justify-between">
            <h2 class="text-2xl font-bold text-surface-900-100">Компании</h2>
            <button onclick={() => openCompanyForm()} class={btnPrimary}>+ Добавить</button>
          </div>
          {#if showCompanyForm}
            <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
              <h3 class="mb-3 font-semibold text-surface-900-100">{editingCompanyId ? 'Редактировать' : 'Новая компания'}</h3>
              <form onsubmit={(e) => { e.preventDefault(); saveCompany(); }} class="space-y-3">
                <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
                  <label class="block text-sm text-surface-700-300">Код *<input bind:value={companyForm.code} class={inputCls + ' mt-1'} required disabled={!!editingCompanyId} /></label>
                  <label class="block text-sm text-surface-700-300">Название *<input bind:value={companyForm.name} class={inputCls + ' mt-1'} required /></label>
                  <label class="block text-sm text-surface-700-300">ИНН<input bind:value={companyForm.inn} class={inputCls + ' mt-1'} /></label>
                </div>
                {#if companyError}<div class="text-sm text-error-600">{companyError}</div>{/if}
                <div class="flex gap-2"><button type="submit" class={btnPrimary}>Сохранить</button><button type="button" onclick={() => { showCompanyForm = false; }} class={btnSecondary}>Отмена</button></div>
              </form>
            </div>
          {/if}
          <div class="overflow-x-auto rounded-xl border border-surface-300-700">
            <table class="w-full text-left text-sm">
              <thead class="border-b border-surface-300-700 bg-surface-100-900 text-xs font-medium uppercase text-surface-500-500">
                <tr><th class="px-4 py-3">Код</th><th class="px-4 py-3">Название</th><th class="px-4 py-3">ИНН</th><th class="px-4 py-3">Статус</th><th class="px-4 py-3 text-right">Действия</th></tr>
              </thead>
              <tbody class="divide-y divide-surface-300-700">
                {#each companies as company (company._id)}
                  <tr class="hover:bg-surface-100-900/50">
                    <td class="px-4 py-3 font-mono text-xs text-surface-900-100">{company.code}</td>
                    <td class="px-4 py-3 text-surface-900-100">{company.name}</td>
                    <td class="px-4 py-3 text-surface-600-400">{company.inn ?? '—'}</td>
                    <td class="px-4 py-3"><span class="rounded-full px-2 py-0.5 text-xs font-medium {company.active ? 'bg-success-500/20 text-success-700' : 'bg-error-500/20 text-error-700'}">{company.active ? 'Активна' : 'Неактивна'}</span></td>
                    <td class="px-4 py-3 text-right">
                      <button onclick={() => openCompanyForm(company)} class="mr-2 text-primary-500 hover:underline">Ред.</button>
                      <button onclick={() => deleteCompany(company._id)} class={btnDanger}>Удалить</button>
                    </td>
                  </tr>
                {:else}
                  <tr><td colspan="5" class="px-4 py-8 text-center text-surface-500-500">Нет компаний</td></tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>

      {:else if currentNav === 'users' && !showUserDetail}
        <div class="space-y-4">
          <div class="flex items-center justify-between">
            <h2 class="text-2xl font-bold text-surface-900-100">Пользователи</h2>
            <button onclick={openUserForm} class={btnPrimary}>+ Добавить</button>
          </div>
          {#if showUserForm}
            <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
              <h3 class="mb-3 font-semibold text-surface-900-100">Новый пользователь</h3>
              <form onsubmit={(e) => { e.preventDefault(); saveUser(); }} class="space-y-3">
                <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
                  <label class="block text-sm text-surface-700-300">Логин *<input bind:value={userForm.login} class={inputCls + ' mt-1'} required /></label>
                  <label class="block text-sm text-surface-700-300">Пароль *<input bind:value={userForm.password} type="password" class={inputCls + ' mt-1'} required /></label>
                  <label class="block text-sm text-surface-700-300">Роль<select bind:value={userForm.role_id} class={inputCls + ' mt-1'}><option value="">Без роли</option>{#each createUserRoles as role}<option value={role._id}>{role.name}</option>{/each}</select></label>
                  <label class="block text-sm text-surface-700-300">Фамилия<input bind:value={userForm.last_name} class={inputCls + ' mt-1'} /></label>
                  <label class="block text-sm text-surface-700-300">Имя<input bind:value={userForm.first_name} class={inputCls + ' mt-1'} /></label>
                  <label class="block text-sm text-surface-700-300">Отчество<input bind:value={userForm.middle_name} class={inputCls + ' mt-1'} /></label>
                  <label class="block text-sm text-surface-700-300">Email<input bind:value={userForm.email} type="email" class={inputCls + ' mt-1'} /></label>
                  <label class="block text-sm text-surface-700-300">Должность<input bind:value={userForm.position} class={inputCls + ' mt-1'} /></label>
                  <label class="block text-sm text-surface-700-300">Отдел<input bind:value={userForm.department} class={inputCls + ' mt-1'} /></label>
                </div>
                {#if userError}<div class="text-sm text-error-600">{userError}</div>{/if}
                <div class="flex gap-2"><button type="submit" class={btnPrimary}>Создать</button><button type="button" onclick={() => { showUserForm = false; }} class={btnSecondary}>Отмена</button></div>
              </form>
            </div>
          {/if}
          <div class="overflow-x-auto rounded-xl border border-surface-300-700">
            <table class="w-full text-left text-sm">
              <thead class="border-b border-surface-300-700 bg-surface-100-900 text-xs font-medium uppercase text-surface-500-500">
                <tr><th class="px-4 py-3">Логин</th><th class="px-4 py-3">Имя</th><th class="px-4 py-3">Статус</th><th class="px-4 py-3">Последний вход</th><th class="px-4 py-3 text-right">Действия</th></tr>
              </thead>
              <tbody class="divide-y divide-surface-300-700">
                {#each users as user (user._id)}
                  <tr class="cursor-pointer hover:bg-surface-100-900/50" onclick={() => openUserDetail(user)}>
                    <td class="px-4 py-3 font-mono text-xs text-surface-900-100">{user.login}</td>
                    <td class="px-4 py-3 text-surface-900-100">{user.display_name}</td>
                    <td class="px-4 py-3"><span class="rounded-full px-2 py-0.5 text-xs font-medium {statusCls(user.status)}">{statusLabel(user.status)}</span></td>
                    <td class="px-4 py-3 text-xs text-surface-600-400">{fmtDate(user.last_login_at)}</td>
                    <td class="px-4 py-3 text-right">
                      <button onclick={(e) => { e.stopPropagation(); toggleUserStatus(user); }}
                        class="rounded-lg px-3 py-1 text-xs font-medium {user.status === 'disabled' ? 'bg-success-500/20 text-success-700 hover:bg-success-500/30' : 'bg-surface-300-700 text-surface-600-400 hover:bg-surface-400-600'}">
                        {user.status === 'disabled' ? 'Разблокировать' : 'Заблокировать'}
                      </button>
                    </td>
                  </tr>
                {:else}
                  <tr><td colspan="5" class="px-4 py-8 text-center text-surface-500-500">Нет пользователей</td></tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>

      {:else if currentNav === 'users' && showUserDetail && detailUser}
        <div class="space-y-4">
          <div class="flex items-center gap-3">
            <button onclick={() => { showUserDetail = false; loadUsers(); }} class="text-surface-500-500 hover:text-surface-700-300"><i class="fa-solid fa-arrow-left"></i></button>
            <h2 class="text-2xl font-bold text-surface-900-100">{detailUser.display_name}</h2>
            <span class="rounded-full px-2 py-0.5 text-xs font-medium {statusCls(detailUser.status)}">{statusLabel(detailUser.status)}</span>
          </div>

          <div class="flex gap-1 rounded-lg border border-surface-300-700 bg-surface-100-900 p-1">
            {#each [{ code: 'basic', label: 'Основное', icon: 'fa-user' }, { code: 'contacts', label: 'Контакты', icon: 'fa-envelope' }, { code: 'profiles', label: 'Компании', icon: 'fa-building' }, { code: 'certs', label: 'Сертификаты', icon: 'fa-certificate' }] as tab}
              <button onclick={() => { detailTab = tab.code; }} class="flex items-center gap-2 rounded-md px-3 py-1.5 text-sm transition-colors {detailTab === tab.code ? 'bg-surface-50-950 font-medium text-primary-600 shadow-sm' : 'text-surface-600-400 hover:text-surface-900-100'}">
                <i class="fa-solid {tab.icon} text-xs"></i>{tab.label}
              </button>
            {/each}
          </div>

          {#if detailError}<div class="rounded-lg bg-error-500/10 p-3 text-sm text-error-600">{detailError}</div>{/if}

          {#if detailTab === 'basic'}
            <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5 space-y-4">
              <div class="flex items-center justify-between">
                <h3 class="font-semibold text-surface-900-100">Личные данные</h3>
                <div class="flex gap-2">
                  {#if detailUser.person_id}
                    <button onclick={() => { editPerson = !editPerson; }} class="text-sm text-primary-500 hover:underline">{editPerson ? 'Отмена' : 'Редактировать'}</button>
                  {/if}
                  <button onclick={() => openPasswordReset(detailUser!._id, detailUser!.login)} class="text-sm text-warning-600 hover:underline">Сбросить пароль</button>
                </div>
              </div>

              <div class="grid grid-cols-1 gap-3 md:grid-cols-3 text-sm border-b border-surface-300-700 pb-4">
                <div><span class="text-surface-500-500">Логин:</span> <span class="text-surface-900-100 font-mono">{detailUser.login}</span></div>
                <div><span class="text-surface-500-500">Статус:</span> <span class="rounded-full px-2 py-0.5 text-xs font-medium {statusCls(detailUser.status)}">{statusLabel(detailUser.status)}</span></div>
                <div><span class="text-surface-500-500">Последний вход:</span> <span class="text-surface-900-100">{fmtDate(detailUser.last_login_at)}</span></div>
              </div>
              {#if editPerson}
                <form onsubmit={(e) => { e.preventDefault(); savePerson(); }} class="space-y-3">
                  <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
                    <label class="block text-sm text-surface-700-300">Фамилия<input bind:value={personForm.last_name} class={inputCls + ' mt-1'} /></label>
                    <label class="block text-sm text-surface-700-300">Имя<input bind:value={personForm.first_name} class={inputCls + ' mt-1'} /></label>
                    <label class="block text-sm text-surface-700-300">Отчество<input bind:value={personForm.middle_name} class={inputCls + ' mt-1'} /></label>
                  </div>
                  <label class="block text-sm text-surface-700-300">Отображаемое имя<input bind:value={personForm.display_name} class={inputCls + ' mt-1'} /></label>
                  <button type="submit" class={btnPrimary}>Сохранить</button>
                </form>
              {:else if detailPerson}
                <div class="grid grid-cols-1 gap-3 md:grid-cols-3 text-sm">
                  <div><span class="text-surface-500-500">Фамилия:</span> <span class="text-surface-900-100">{detailPerson.last_name || '—'}</span></div>
                  <div><span class="text-surface-500-500">Имя:</span> <span class="text-surface-900-100">{detailPerson.first_name || '—'}</span></div>
                  <div><span class="text-surface-500-500">Отчество:</span> <span class="text-surface-900-100">{detailPerson.middle_name || '—'}</span></div>
                </div>
              {:else}
                <p class="text-sm text-surface-500-500">Нет данных</p>
              {/if}
            </div>

          {:else if detailTab === 'contacts'}
            <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5 space-y-4">
              <h3 class="font-semibold text-surface-900-100">Контакты</h3>
              {#if detailContacts.length > 0}
                <div class="space-y-2">
                  {#each detailContacts as c}
                    {#if editingContactId === c._id}
                      <div class="rounded-lg border border-primary-500/50 bg-surface-100-900 p-3 space-y-2">
                        <div class="flex gap-2">
                          <input bind:value={editContactForm.value} class={inputCls} placeholder="Значение" />
                        </div>
                        <div class="flex flex-wrap items-center gap-3">
                          <label class="flex items-center gap-1 text-xs text-surface-700-300">
                            <input type="checkbox" bind:checked={editContactForm.is_primary} class="rounded" /> Основной
                          </label>
                          <label class="flex items-center gap-1 text-xs text-surface-700-300">
                            <input type="checkbox" bind:checked={editContactForm.is_verified} class="rounded" /> Подтверждён
                          </label>
                          <label class="flex items-center gap-1 text-xs text-surface-700-300">
                            <input type="checkbox" checked={editContactForm.purposes.includes('login')} onchange={(e) => { const v = (e.target as HTMLInputElement).checked; editContactForm.purposes = v ? [...editContactForm.purposes, 'login'] : editContactForm.purposes.filter(p => p !== 'login'); }} class="rounded" /> Вход
                          </label>
                          <label class="flex items-center gap-1 text-xs text-surface-700-300">
                            <input type="checkbox" checked={editContactForm.purposes.includes('notifications')} onchange={(e) => { const v = (e.target as HTMLInputElement).checked; editContactForm.purposes = v ? [...editContactForm.purposes, 'notifications'] : editContactForm.purposes.filter(p => p !== 'notifications'); }} class="rounded" /> Уведомления
                          </label>
                          <label class="flex items-center gap-1 text-xs text-surface-700-300">
                            <input type="checkbox" checked={editContactForm.purposes.includes('personal')} onchange={(e) => { const v = (e.target as HTMLInputElement).checked; editContactForm.purposes = v ? [...editContactForm.purposes, 'personal'] : editContactForm.purposes.filter(p => p !== 'personal'); }} class="rounded" /> Личный
                          </label>
                          <label class="flex items-center gap-1 text-xs text-surface-700-300">
                            <input type="checkbox" checked={editContactForm.purposes.includes('work')} onchange={(e) => { const v = (e.target as HTMLInputElement).checked; editContactForm.purposes = v ? [...editContactForm.purposes, 'work'] : editContactForm.purposes.filter(p => p !== 'work'); }} class="rounded" /> Рабочий
                          </label>
                        </div>
                        <input bind:value={editContactForm.note} class={inputCls} placeholder="Заметка (напр. &quot;не звонить после 20:00&quot;)" />
                        <div class="flex gap-2">
                          <button onclick={() => saveEditContact(c._id)} class={btnPrimary}>Сохранить</button>
                          <button onclick={cancelEditContact} class={btnSecondary}>Отмена</button>
                        </div>
                      </div>
                    {:else}
                      <div class="flex items-start justify-between rounded-lg border border-surface-300-700 bg-surface-100-900 px-3 py-2">
                        <div class="space-y-1">
                          <div class="flex items-center gap-2">
                            <span class="text-xs font-medium text-surface-500-500 uppercase">{c.channel_type}</span>
                            <span class="text-sm text-surface-900-100">{c.value}</span>
                            {#if c.is_primary}<span class="rounded bg-primary-500/20 px-1 text-xs text-primary-600">Основной</span>{/if}
                            {#if c.is_verified}<span class="rounded bg-success-500/20 px-1 text-xs text-success-700">Подтверждён</span>{/if}
                          </div>
                          {#if c.purposes.length > 0}
                            <div class="flex gap-1">
                              {#each c.purposes as p}<span class="rounded bg-surface-200-800 px-1 text-[10px] text-surface-600-400">{p}</span>{/each}
                            </div>
                          {/if}
                          {#if c.note}<p class="text-xs text-surface-500-500 italic">{c.note}</p>{/if}
                        </div>
                        <div class="flex gap-1 shrink-0 ml-2">
                          <button onclick={() => startEditContact(c)} class="rounded p-1 text-surface-500-500 hover:bg-surface-200-800" title="Редактировать"><i class="fa-solid fa-pen text-xs"></i></button>
                          <button onclick={() => deleteContact(c._id)} class="rounded p-1 text-error-500 hover:bg-error-500/10" title="Удалить"><i class="fa-solid fa-trash text-xs"></i></button>
                        </div>
                      </div>
                    {/if}
                  {/each}
                </div>
              {/if}
              <form onsubmit={(e) => { e.preventDefault(); addContact(); }} class="space-y-2 rounded-lg border border-dashed border-surface-300-700 p-3">
                <div class="flex gap-2">
                  <select bind:value={contactForm.channel_type} class="w-32 shrink-0 rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 focus:border-primary-500 focus:outline-none">
                    {#each contactTypes as ct}<option value={ct.code}>{ct.name}</option>{/each}
                  </select>
                  <input bind:value={contactForm.value} class={inputCls} placeholder="Значение" />
                </div>
                <div class="flex flex-wrap items-center gap-3">
                  <label class="flex items-center gap-1 text-xs text-surface-700-300">
                    <input type="checkbox" bind:checked={contactForm.is_primary} class="rounded" /> Основной
                  </label>
                  <label class="flex items-center gap-1 text-xs text-surface-700-300">
                    <input type="checkbox" checked={contactForm.purposes.includes('login')} onchange={(e) => { const v = (e.target as HTMLInputElement).checked; contactForm.purposes = v ? [...contactForm.purposes, 'login'] : contactForm.purposes.filter(p => p !== 'login'); }} class="rounded" /> Вход
                  </label>
                  <label class="flex items-center gap-1 text-xs text-surface-700-300">
                    <input type="checkbox" checked={contactForm.purposes.includes('notifications')} onchange={(e) => { const v = (e.target as HTMLInputElement).checked; contactForm.purposes = v ? [...contactForm.purposes, 'notifications'] : contactForm.purposes.filter(p => p !== 'notifications'); }} class="rounded" /> Уведомления
                  </label>
                  <label class="flex items-center gap-1 text-xs text-surface-700-300">
                    <input type="checkbox" checked={contactForm.purposes.includes('personal')} onchange={(e) => { const v = (e.target as HTMLInputElement).checked; contactForm.purposes = v ? [...contactForm.purposes, 'personal'] : contactForm.purposes.filter(p => p !== 'personal'); }} class="rounded" /> Личный
                  </label>
                  <label class="flex items-center gap-1 text-xs text-surface-700-300">
                    <input type="checkbox" checked={contactForm.purposes.includes('work')} onchange={(e) => { const v = (e.target as HTMLInputElement).checked; contactForm.purposes = v ? [...contactForm.purposes, 'work'] : contactForm.purposes.filter(p => p !== 'work'); }} class="rounded" /> Рабочий
                  </label>
                </div>
                <input bind:value={contactForm.note} class={inputCls} placeholder="Заметка (напр. &quot;не звонить после 20:00&quot;)" />
                <div>
                  <button type="submit" class={btnPrimary}>Добавить</button>
                </div>
              </form>
            </div>

          {:else if detailTab === 'profiles'}
            <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5 space-y-4">
              <h3 class="font-semibold text-surface-900-100">Рабочие профили</h3>
              {#if detailProfiles.length > 0}
                <div class="space-y-2">
                  {#each detailProfiles as p}
                    <div class="flex items-center justify-between rounded-lg border border-surface-300-700 bg-surface-100-900 px-3 py-2">
                      <div>
                        <span class="text-sm font-medium text-surface-900-100">{p.company_name}</span>
                        <span class="ml-2 text-xs text-surface-500-500">({p.role_name})</span>
                        {#if p.position}<span class="ml-2 text-xs text-surface-500-500">· {p.position}</span>{/if}
                      </div>
                      <button onclick={() => removeProfile(p._id)} class="rounded p-1 text-error-500 hover:bg-error-500/10"><i class="fa-solid fa-trash text-xs"></i></button>
                    </div>
                  {/each}
                </div>
              {/if}
              <form onsubmit={(e) => { e.preventDefault(); addProfile(); }} class="space-y-2">
                <div class="flex gap-2">
                  <select bind:value={profileForm.company_id} onchange={(e) => loadProfileRoles(e.currentTarget.value)} class={inputCls}>
                    <option value="">Компания</option>
                    {#each companies as c}<option value={c._id}>{c.name}</option>{/each}
                  </select>
                  <select bind:value={profileForm.role_id} class={inputCls}>
                    <option value="">Роль</option>
                    {#each profileModalRoles as r}<option value={r._id}>{r.name}</option>{/each}
                  </select>
                </div>
                <div class="flex gap-2">
                  <input bind:value={profileForm.position} class={inputCls} placeholder="Должность" />
                  <input bind:value={profileForm.department} class={inputCls} placeholder="Отдел" />
                  <button type="submit" class={btnPrimary}>Добавить</button>
                </div>
              </form>
            </div>

          {:else if detailTab === 'certs'}
            <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5 space-y-4">
              <h3 class="font-semibold text-surface-900-100">Сертификаты</h3>
              {#if detailCerts.length > 0}
                <div class="space-y-2">
                  {#each detailCerts as cert}
                    <div class="flex items-center justify-between rounded-lg border border-surface-300-700 bg-surface-100-900 px-3 py-2">
                      <div>
                        <span class="text-sm font-medium text-surface-900-100">{cert.subject}</span>
                        <div class="text-xs text-surface-500-500">{cert.issuer} · {cert.serial_number}</div>
                      </div>
                      {#if cert.is_active}
                        <button onclick={() => deactivateCert(cert._id)} class="rounded p-1 text-error-500 hover:bg-error-500/10"><i class="fa-solid fa-ban text-xs"></i></button>
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

      {:else if currentNav === 'roles'}
        <div class="space-y-4">
          <div class="flex items-center justify-between">
            <h2 class="text-2xl font-bold text-surface-900-100">Роли</h2>
            <button onclick={() => { showRoleForm = !showRoleForm; roleError = ''; }} class={btnPrimary}>+ Добавить</button>
          </div>
          {#if showRoleForm}
            <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
              <h3 class="mb-3 font-semibold text-surface-900-100">Новая роль</h3>
              <form onsubmit={(e) => { e.preventDefault(); saveRole(); }} class="space-y-3">
                <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
                  <label class="block text-sm text-surface-700-300">Код *<input bind:value={roleForm.code} class={inputCls + ' mt-1'} required /></label>
                  <label class="block text-sm text-surface-700-300">Название *<input bind:value={roleForm.name} class={inputCls + ' mt-1'} required /></label>
                  <label class="block text-sm text-surface-700-300">Описание<input bind:value={roleForm.description} class={inputCls + ' mt-1'} /></label>
                </div>
                {#if roleError}<div class="text-sm text-error-600">{roleError}</div>{/if}
                <div class="flex gap-2"><button type="submit" class={btnPrimary}>Создать</button><button type="button" onclick={() => { showRoleForm = false; }} class={btnSecondary}>Отмена</button></div>
              </form>
            </div>
          {/if}
          <div class="overflow-x-auto rounded-xl border border-surface-300-700">
            <table class="w-full text-left text-sm">
              <thead class="border-b border-surface-300-700 bg-surface-100-900 text-xs font-medium uppercase text-surface-500-500">
                <tr><th class="px-4 py-3">Код</th><th class="px-4 py-3">Название</th><th class="px-4 py-3">Описание</th><th class="px-4 py-3 text-right">Действия</th></tr>
              </thead>
              <tbody class="divide-y divide-surface-300-700">
                {#each roles as role (role._id)}
                  <tr class="hover:bg-surface-100-900/50">
                    <td class="px-4 py-3 font-mono text-xs text-surface-900-100">{role.code}</td>
                    <td class="px-4 py-3 text-surface-900-100">{role.name}</td>
                    <td class="px-4 py-3 text-surface-600-400">{role.description ?? '—'}</td>
                    <td class="px-4 py-3 text-right"><button onclick={() => deleteRole(role._id)} class={btnDanger}>Удалить</button></td>
                  </tr>
                {:else}
                  <tr><td colspan="4" class="px-4 py-8 text-center text-surface-500-500">Нет ролей</td></tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>

      {:else if currentNav === 'objects' || currentNav === 'documents' || currentNav === 'catalogs'}
        <ObjectsPage />

      {:else if currentNav === 'events'}
        <EventsPage />

      {:else if currentNav === 'metadata'}
        <MetadataPage />

      {:else if currentNav === 'audit'}
        <AuditPage />

      {:else if currentNav === 'convert'}
        <ConvertPage />

      {:else if currentNav === 'print'}
        <PrintPage />

      {:else if currentNav === 'numbering'}
        <NumberingPage />

      {:else if currentNav === 'scripts'}
        <ScriptsPage />

      {:else if currentNav === 'reports'}
        <ReportsPage />

      {:else if currentNav === 'settings'}
        <SettingsPage />

      {:else}
        <div class="space-y-4">
          <h2 class="text-2xl font-bold text-surface-900-100">{filteredNavItems.find((n: any) => n.code === currentNav)?.label ?? ''}</h2>
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">Раздел в разработке</div>
        </div>
      {/if}
    </div>
  </main>

  <aside class="flex flex-col border-l border-surface-300-700 bg-surface-100-900 transition-all duration-300" class:w-64={!sidebarCollapsed} class:w-16={sidebarCollapsed}>
    <div class="flex items-center gap-2 border-b border-surface-300-700 p-4">
      {#if !sidebarCollapsed}<span class="font-semibold text-surface-900-100">Платформа</span>{/if}
      <button onclick={toggleSidebar} class="ml-auto rounded p-1 text-surface-500-500 hover:bg-surface-200-800">
        <i class="fa-solid {sidebarCollapsed ? 'fa-chevron-left' : 'fa-chevron-right'} text-sm"></i>
      </button>
    </div>
    <nav class="flex-1 overflow-y-auto p-2">
      {#each filteredNavItems as item}
        <button onclick={() => setNav(item.code)} class="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors {currentNav === item.code ? 'bg-primary-500/10 font-medium text-primary-600' : 'text-surface-600-400 hover:bg-surface-200-800'}">
          <i class="{item.icon} text-lg w-5 text-center"></i>
          {#if !sidebarCollapsed}<span>{item.label}</span>{/if}
        </button>
      {/each}
    </nav>
    {#if currentUser && (get(auth)?.companies?.length ?? 0) > 1 && !sidebarCollapsed}
      <div class="border-t border-surface-300-700 p-3">
        <label class="text-xs text-surface-500-500">Компания</label>
        <select class="mt-1 w-full rounded-lg border border-surface-300-700 bg-surface-50-950 px-2 py-1.5 text-sm text-surface-900-100 focus:border-primary-500 focus:outline-none" onchange={(e) => handleSwitchCompany(e.currentTarget.value)}>
          {#each (get(auth)?.companies ?? []) as uc}
            <option value={uc.companyId} selected={uc.companyId === currentUser?._id}>{uc.companyName}</option>
          {/each}
        </select>
      </div>
    {/if}
    {#if currentUser}
      <div class="border-t border-surface-300-700 p-3">
        <div class="flex items-center gap-2">
          <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary-500 text-xs font-bold text-white">{currentUser.display_name?.[0] ?? 'U'}</div>
          {#if !sidebarCollapsed}
            <div class="flex flex-1 flex-col">
              <span class="text-sm font-medium text-surface-900-100">{currentUser.display_name}</span>
              <span class="text-xs text-surface-500-500">{currentUser.login}</span>
            </div>
            <button onclick={handleLogout} class="rounded p-1 text-surface-500-500 hover:bg-surface-200-800" title="Выйти"><i class="fa-solid fa-right-from-bracket text-sm"></i></button>
            <button onclick={handleExit} class="rounded p-1 text-surface-500-500 hover:bg-surface-200-800" title="Закрыть приложение"><i class="fa-solid fa-power-off text-sm"></i></button>
          {/if}
        </div>
      </div>
    {/if}
  </aside>
</div>

{#if showPasswordReset}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onclick={() => { showPasswordReset = false; }}>
    <div class="w-full max-w-sm rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 shadow-xl" onclick={(e) => e.stopPropagation()}>
      <h3 class="mb-4 font-semibold text-surface-900-100">Сброс пароля: {resetPasswordLogin}</h3>
      <label class="block text-sm text-surface-700-300">
        Новый пароль
        <input bind:value={resetPasswordValue} type="password" class={inputCls + ' mt-1'} placeholder="Минимум 4 символа" autofocus />
      </label>
      <p class="mt-1 text-xs text-surface-500-500">Пользователь будет вынужден сменить пароль при следующем входе.</p>
      {#if resetPasswordError}<div class="mt-2 text-sm text-error-600">{resetPasswordError}</div>{/if}
      <div class="mt-4 flex gap-2">
        <button onclick={confirmPasswordReset} class={btnPrimary}>Установить</button>
        <button onclick={() => { showPasswordReset = false; }} class={btnSecondary}>Отмена</button>
      </div>
    </div>
  </div>
{/if}
{/if}
