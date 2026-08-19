<script lang="ts">
  import { theme } from '$lib/stores/theme';
  import { navItems, activeNav } from '$lib/stores/navigation';
  import { auth, isAuthenticated } from '$lib/stores/auth';
  import { api } from '$lib/services/api';
  import type { DiagnosticsReport, Company, User, Role } from '$lib/services/api';
  import { onMount } from 'svelte';

  let sidebarCollapsed = $state(false);
  let diagnostics = $state<DiagnosticsReport | null>(null);
  let loading = $state(true);
  let currentNav = $state('dashboard');
  let connected = $state(false);
  let currentUser = $state<User | null>(null);

  // Login form
  let loginUsername = $state('');
  let loginPassword = $state('');
  let loginError = $state('');
  let loginLoading = $state(false);

  // Connection form
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
  let userForm = $state({ username: '', display_name: '', email: '', password: '', role_id: '' });
  let userError = $state('');

  // Role form
  let showRoleForm = $state(false);
  let roleForm = $state({ code: '', name: '', description: '' });
  let roleError = $state('');

  function toggleSidebar() { sidebarCollapsed = !sidebarCollapsed; }
  function setNav(code: string) { currentNav = code; activeNav.set(code); }

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
      const result = await api.authenticate(loginUsername, loginPassword);
      auth.login({
        userId: result.user._id,
        companyId: result.user.company_id,
        roleId: result.user.role_id,
        username: result.user.username,
        displayName: result.user.display_name,
      });
      currentUser = result.user;
    } catch (e: any) {
      loginError = typeof e === 'string' ? e : e?.message ?? 'Ошибка авторизации';
    } finally { loginLoading = false; }
  }

  function handleLogout() {
    auth.logout();
    currentUser = null;
  }

  async function loadCompanies() {
    try { companies = await api.listCompanies(); } catch {}
  }
  async function loadUsers() {
    if (currentUser) { try { users = await api.listUsers(currentUser.company_id); } catch {} }
  }
  async function loadRoles() {
    if (currentUser) { try { roles = await api.listRoles(currentUser.company_id); } catch {} }
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
    } catch (e: any) {
      companyError = typeof e === 'string' ? e : e?.message ?? 'Ошибка сохранения';
    }
  }

  async function deleteCompany(id: string) {
    if (!confirm('Удалить компанию?')) return;
    try { await api.deleteCompany(id); await loadCompanies(); } catch {}
  }

  async function saveUser() {
    userError = '';
    try {
      if (!currentUser) return;
      await api.createUser({
        company_id: currentUser.company_id,
        ...userForm,
        email: userForm.email || undefined,
      });
      showUserForm = false;
      userForm = { username: '', display_name: '', email: '', password: '', role_id: '' };
      await loadUsers();
    } catch (e: any) {
      userError = typeof e === 'string' ? e : e?.message ?? 'Ошибка сохранения';
    }
  }

  async function deleteUser(id: string) {
    if (!confirm('Удалить пользователя?')) return;
    try { await api.deleteUser(id); await loadUsers(); } catch {}
  }

  async function saveRole() {
    roleError = '';
    try {
      if (!currentUser) return;
      await api.createRole({ company_id: currentUser.company_id, ...roleForm, description: roleForm.description || undefined });
      showRoleForm = false;
      roleForm = { code: '', name: '', description: '' };
      await loadRoles();
    } catch (e: any) {
      roleError = typeof e === 'string' ? e : e?.message ?? 'Ошибка сохранения';
    }
  }

  async function deleteRole(id: string) {
    if (!confirm('Удалить роль?')) return;
    try { await api.deleteRole(id); await loadRoles(); } catch {}
  }

  onMount(async () => {
    theme.init();
    auth.restore();
    const stored = $isAuthenticated;
    if (!stored) { loading = false; return; }
    try {
      const me = await api.getMe();
      if (me) {
        currentUser = me;
        diagnostics = await api.getDiagnostics();
        connected = diagnostics?.mongodb.ok ?? false;
      }
    } catch {} finally { loading = false; }
  });

  $effect(() => {
    if (currentUser && currentNav === 'companies') loadCompanies();
    if (currentUser && currentNav === 'users') loadUsers();
    if (currentUser && currentNav === 'roles') loadRoles();
  });

  const iconMap: Record<string, string> = {
    grid: '⊞', building: '🏢', users: '👥', shield: '🛡',
    'file-text': '📄', book: '📚', 'bar-chart': '📊', code: '⟨⟩', settings: '⚙',
  };

  const inputCls = 'w-full rounded-lg border border-surface-300-700 bg-surface-50-950 px-3 py-2 text-sm text-surface-900-100 placeholder:text-surface-400-600 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500';
  const btnPrimary = 'rounded-lg bg-primary-500 px-4 py-2 text-sm font-medium text-white hover:bg-primary-600 disabled:opacity-50';
  const btnDanger = 'rounded-lg bg-error-500 px-3 py-1 text-xs font-medium text-white hover:bg-error-600';
  const btnSecondary = 'rounded-lg border border-surface-300-700 px-4 py-2 text-sm text-surface-700-300 hover:bg-surface-200-800';
</script>

<!-- Not connected → connection form -->
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
      {#if dbError}
        <div class="rounded-lg bg-error-500/10 p-3 text-sm text-error-600">{dbError}</div>
      {/if}
      <button type="submit" disabled={dbLoading} class={btnPrimary + ' w-full'}>
        {dbLoading ? 'Подключение...' : 'Подключиться'}
      </button>
    </form>
  </div>
</div>

<!-- Connected but not logged in → login -->
{:else if connected && !$isAuthenticated && !loading}
<div class="flex h-screen items-center justify-center bg-surface-50-950">
  <div class="w-full max-w-md space-y-6 rounded-2xl border border-surface-300-700 bg-surface-50-950 p-8 shadow-xl">
    <div class="text-center">
      <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-primary-500 text-2xl font-bold text-white">2C</div>
      <h1 class="mt-4 text-xl font-bold text-surface-900-100">Вход в систему</h1>
      <p class="mt-1 text-sm text-surface-500-500">
        {#if !currentUser}
          При первом входе введите <strong>admin</strong> / <strong>admin</strong> для создания системы.
        {/if}
      </p>
    </div>
    <form onsubmit={(e) => { e.preventDefault(); handleLogin(); }} class="space-y-4">
      <label class="block text-sm font-medium text-surface-700-300">
        Имя пользователя
        <input bind:value={loginUsername} class={inputCls + ' mt-1'} placeholder="admin" autofocus />
      </label>
      <label class="block text-sm font-medium text-surface-700-300">
        Пароль
        <input bind:value={loginPassword} type="password" class={inputCls + ' mt-1'} placeholder="••••••••" />
      </label>
      {#if loginError}
        <div class="rounded-lg bg-error-500/10 p-3 text-sm text-error-600">{loginError}</div>
      {/if}
      <button type="submit" disabled={loginLoading} class={btnPrimary + ' w-full'}>
        {loginLoading ? 'Вход...' : 'Войти'}
      </button>
    </form>
  </div>
</div>

<!-- Main app -->
{:else}
<div class="flex h-screen overflow-hidden bg-surface-50-950">
  <aside class="flex flex-col border-r border-surface-300-700 bg-surface-100-900 transition-all duration-300"
    class:w-64={!sidebarCollapsed} class:w-16={sidebarCollapsed}>
    <div class="flex items-center gap-2 border-b border-surface-300-700 p-4">
      <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary-500 text-sm font-bold text-white">2C</div>
      {#if !sidebarCollapsed}
        <span class="font-semibold text-surface-900-100">Платформа</span>
      {/if}
      <button onclick={toggleSidebar} class="ml-auto rounded p-1 text-surface-500-500 hover:bg-surface-200-800">
        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          {#if sidebarCollapsed}
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          {:else}
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
          {/if}
        </svg>
      </button>
    </div>
    <nav class="flex-1 overflow-y-auto p-2">
      {#each $navItems as item}
        <button onclick={() => setNav(item.code)}
          class="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors
            {currentNav === item.code ? 'bg-primary-500/10 font-medium text-primary-600' : 'text-surface-600-400 hover:bg-surface-200-800'}">
          <span class="text-lg">{iconMap[item.icon] ?? '•'}</span>
          {#if !sidebarCollapsed}<span>{item.label}</span>{/if}
        </button>
      {/each}
    </nav>
    <div class="border-t border-surface-300-700 p-3">
      <div class="flex items-center gap-2">
        <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary-500 text-xs font-bold text-white">
          {currentUser?.display_name?.[0] ?? 'U'}
        </div>
        {#if !sidebarCollapsed}
          <div class="flex flex-1 flex-col">
            <span class="text-sm font-medium text-surface-900-100">{currentUser?.display_name ?? 'Пользователь'}</span>
            <span class="text-xs text-surface-500-500">{currentUser?.username ?? ''}</span>
          </div>
          <button onclick={handleLogout} class="rounded p-1 text-surface-500-500 hover:bg-surface-200-800" title="Выйти">
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
            </svg>
          </button>
        {/if}
      </div>
    </div>
  </aside>

  <main class="flex-1 overflow-y-auto">
    <header class="flex items-center justify-between border-b border-surface-300-700 bg-surface-50-950 px-6 py-4">
      <h1 class="text-lg font-semibold text-surface-900-100">
        {$navItems.find((n: any) => n.code === currentNav)?.label ?? 'Главная'}
      </h1>
      <div class="flex items-center gap-4">
        <span class="rounded-full px-2 py-0.5 text-xs font-medium {diagnostics?.mongodb.ok ? 'bg-success-500/20 text-success-700' : 'bg-error-500/20 text-error-700'}">
          DB {diagnostics?.mongodb.ok ? 'OK' : 'ERR'}
        </span>
        <button onclick={() => theme.set($theme === 'dark' ? 'light' : 'dark')}
          class="rounded-lg p-2 text-surface-500-500 hover:bg-surface-200-800" title="Сменить тему">
          {$theme === 'dark' ? '☀' : '☾'}
        </button>
      </div>
    </header>

    <div class="p-6">
      {#if currentNav === 'dashboard'}
        {#if loading}
          <div class="flex items-center justify-center p-12">
            <div class="h-8 w-8 animate-spin rounded-full border-4 border-primary-500 border-t-transparent"></div>
          </div>
        {:else if diagnostics}
          <div class="space-y-6">
            <div>
              <h2 class="text-2xl font-bold text-surface-900-100">Главная</h2>
              <p class="mt-1 text-surface-500-500">Добро пожаловать в 2C Platform v0.1</p>
            </div>
            <div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
              <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
                <div class="text-sm font-medium text-surface-500-500">Версия</div>
                <div class="mt-1 text-2xl font-bold text-surface-900-100">{diagnostics.app_version}</div>
              </div>
              <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
                <div class="text-sm font-medium text-surface-500-500">MongoDB</div>
                <div class="mt-1 flex items-center gap-2">
                  <span class="text-2xl font-bold text-surface-900-100">{diagnostics.mongodb.connected ? 'Подключено' : 'Отключено'}</span>
                  <span class="rounded-full px-2 py-0.5 text-xs font-medium text-white {diagnostics.mongodb.ok ? 'bg-success-500' : 'bg-error-500'}">
                    {diagnostics.mongodb.ok ? 'OK' : 'ERROR'}
                  </span>
                </div>
                {#if diagnostics.mongodb.version}
                  <div class="mt-1 text-xs text-surface-500-500">v{diagnostics.mongodb.version}</div>
                {/if}
                {#if diagnostics.mongodb.replica_set}
                  <div class="mt-1 text-xs text-surface-500-500">RS: {diagnostics.mongodb.replica_set}</div>
                {/if}
              </div>
              <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
                <div class="text-sm font-medium text-surface-500-500">Модули</div>
                <div class="mt-1 text-2xl font-bold text-surface-900-100">{diagnostics.modules.length}</div>
                <div class="mt-2 space-y-1">
                  {#each diagnostics.modules as mod}
                    <div class="flex items-center gap-2 text-xs">
                      <span class="h-2 w-2 rounded-full bg-success-500"></span>
                      <span class="text-surface-700-300">{mod.name} v{mod.version}</span>
                    </div>
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
                  <label class="block text-sm text-surface-700-300">
                    Код *
                    <input bind:value={companyForm.code} class={inputCls + ' mt-1'} required disabled={!!editingCompanyId} />
                  </label>
                  <label class="block text-sm text-surface-700-300">
                    Название *
                    <input bind:value={companyForm.name} class={inputCls + ' mt-1'} required />
                  </label>
                  <label class="block text-sm text-surface-700-300">
                    ИНН
                    <input bind:value={companyForm.inn} class={inputCls + ' mt-1'} />
                  </label>
                </div>
                {#if companyError}<div class="text-sm text-error-600">{companyError}</div>{/if}
                <div class="flex gap-2">
                  <button type="submit" class={btnPrimary}>Сохранить</button>
                  <button type="button" onclick={() => { showCompanyForm = false; }} class={btnSecondary}>Отмена</button>
                </div>
              </form>
            </div>
          {/if}

          <div class="overflow-x-auto rounded-xl border border-surface-300-700">
            <table class="w-full text-left text-sm">
              <thead class="border-b border-surface-300-700 bg-surface-100-900 text-xs font-medium uppercase text-surface-500-500">
                <tr>
                  <th class="px-4 py-3">Код</th>
                  <th class="px-4 py-3">Название</th>
                  <th class="px-4 py-3">ИНН</th>
                  <th class="px-4 py-3">Статус</th>
                  <th class="px-4 py-3 text-right">Действия</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-surface-300-700">
                {#each companies as company (company._id)}
                  <tr class="hover:bg-surface-100-900/50">
                    <td class="px-4 py-3 font-mono text-xs text-surface-900-100">{company.code}</td>
                    <td class="px-4 py-3 text-surface-900-100">{company.name}</td>
                    <td class="px-4 py-3 text-surface-600-400">{company.inn ?? '—'}</td>
                    <td class="px-4 py-3">
                      <span class="rounded-full px-2 py-0.5 text-xs font-medium {company.active ? 'bg-success-500/20 text-success-700' : 'bg-error-500/20 text-error-700'}">
                        {company.active ? 'Активна' : 'Неактивна'}
                      </span>
                    </td>
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

      {:else if currentNav === 'users'}
        <div class="space-y-4">
          <div class="flex items-center justify-between">
            <h2 class="text-2xl font-bold text-surface-900-100">Пользователи</h2>
            <button onclick={() => { showUserForm = !showUserForm; userError = ''; }} class={btnPrimary}>+ Добавить</button>
          </div>

          {#if showUserForm}
            <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-5">
              <h3 class="mb-3 font-semibold text-surface-900-100">Новый пользователь</h3>
              <form onsubmit={(e) => { e.preventDefault(); saveUser(); }} class="space-y-3">
                <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
                  <label class="block text-sm text-surface-700-300">
                    Логин *
                    <input bind:value={userForm.username} class={inputCls + ' mt-1'} required />
                  </label>
                  <label class="block text-sm text-surface-700-300">
                    Отображаемое имя *
                    <input bind:value={userForm.display_name} class={inputCls + ' mt-1'} required />
                  </label>
                  <label class="block text-sm text-surface-700-300">
                    Email
                    <input bind:value={userForm.email} type="email" class={inputCls + ' mt-1'} />
                  </label>
                  <label class="block text-sm text-surface-700-300">
                    Пароль *
                    <input bind:value={userForm.password} type="password" class={inputCls + ' mt-1'} required />
                  </label>
                  <label class="block text-sm text-surface-700-300">
                    Роль *
                    <select bind:value={userForm.role_id} class={inputCls + ' mt-1'} required>
                      <option value="">Выберите роль</option>
                      {#each roles as role}
                        <option value={role._id}>{role.name}</option>
                      {/each}
                    </select>
                  </label>
                </div>
                {#if userError}<div class="text-sm text-error-600">{userError}</div>{/if}
                <div class="flex gap-2">
                  <button type="submit" class={btnPrimary}>Создать</button>
                  <button type="button" onclick={() => { showUserForm = false; }} class={btnSecondary}>Отмена</button>
                </div>
              </form>
            </div>
          {/if}

          <div class="overflow-x-auto rounded-xl border border-surface-300-700">
            <table class="w-full text-left text-sm">
              <thead class="border-b border-surface-300-700 bg-surface-100-900 text-xs font-medium uppercase text-surface-500-500">
                <tr>
                  <th class="px-4 py-3">Логин</th>
                  <th class="px-4 py-3">Имя</th>
                  <th class="px-4 py-3">Email</th>
                  <th class="px-4 py-3">Статус</th>
                  <th class="px-4 py-3 text-right">Действия</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-surface-300-700">
                {#each users as user (user._id)}
                  <tr class="hover:bg-surface-100-900/50">
                    <td class="px-4 py-3 font-mono text-xs text-surface-900-100">{user.username}</td>
                    <td class="px-4 py-3 text-surface-900-100">{user.display_name}</td>
                    <td class="px-4 py-3 text-surface-600-400">{user.email ?? '—'}</td>
                    <td class="px-4 py-3">
                      <span class="rounded-full px-2 py-0.5 text-xs font-medium {user.active ? 'bg-success-500/20 text-success-700' : 'bg-error-500/20 text-error-700'}">
                        {user.active ? 'Активен' : 'Неактивен'}
                      </span>
                    </td>
                    <td class="px-4 py-3 text-right">
                      <button onclick={() => deleteUser(user._id)} class={btnDanger}>Удалить</button>
                    </td>
                  </tr>
                {:else}
                  <tr><td colspan="5" class="px-4 py-8 text-center text-surface-500-500">Нет пользователей</td></tr>
                {/each}
              </tbody>
            </table>
          </div>
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
                  <label class="block text-sm text-surface-700-300">
                    Код *
                    <input bind:value={roleForm.code} class={inputCls + ' mt-1'} required />
                  </label>
                  <label class="block text-sm text-surface-700-300">
                    Название *
                    <input bind:value={roleForm.name} class={inputCls + ' mt-1'} required />
                  </label>
                  <label class="block text-sm text-surface-700-300">
                    Описание
                    <input bind:value={roleForm.description} class={inputCls + ' mt-1'} />
                  </label>
                </div>
                {#if roleError}<div class="text-sm text-error-600">{roleError}</div>{/if}
                <div class="flex gap-2">
                  <button type="submit" class={btnPrimary}>Создать</button>
                  <button type="button" onclick={() => { showRoleForm = false; }} class={btnSecondary}>Отмена</button>
                </div>
              </form>
            </div>
          {/if}

          <div class="overflow-x-auto rounded-xl border border-surface-300-700">
            <table class="w-full text-left text-sm">
              <thead class="border-b border-surface-300-700 bg-surface-100-900 text-xs font-medium uppercase text-surface-500-500">
                <tr>
                  <th class="px-4 py-3">Код</th>
                  <th class="px-4 py-3">Название</th>
                  <th class="px-4 py-3">Описание</th>
                  <th class="px-4 py-3 text-right">Действия</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-surface-300-700">
                {#each roles as role (role._id)}
                  <tr class="hover:bg-surface-100-900/50">
                    <td class="px-4 py-3 font-mono text-xs text-surface-900-100">{role.code}</td>
                    <td class="px-4 py-3 text-surface-900-100">{role.name}</td>
                    <td class="px-4 py-3 text-surface-600-400">{role.description ?? '—'}</td>
                    <td class="px-4 py-3 text-right">
                      <button onclick={() => deleteRole(role._id)} class={btnDanger}>Удалить</button>
                    </td>
                  </tr>
                {:else}
                  <tr><td colspan="4" class="px-4 py-8 text-center text-surface-500-500">Нет ролей</td></tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>

      {:else if currentNav === 'documents'}
        <div class="space-y-4">
          <h2 class="text-2xl font-bold text-surface-900-100">Документы</h2>
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">
            Документы будут доступны после настройки метаданных (этап 5)
          </div>
        </div>

      {:else if currentNav === 'catalogs'}
        <div class="space-y-4">
          <h2 class="text-2xl font-bold text-surface-900-100">Справочники</h2>
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">
            Справочники будут доступны после настройки метаданных (этап 4)
          </div>
        </div>

      {:else if currentNav === 'reports'}
        <div class="space-y-4">
          <h2 class="text-2xl font-bold text-surface-900-100">Отчёты</h2>
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">
            ОСВ, журнал проводок, карточка счёта, баланс — доступны в этапе 10
          </div>
        </div>

      {:else if currentNav === 'scripts'}
        <div class="space-y-4">
          <h2 class="text-2xl font-bold text-surface-900-100">Скрипты Rhai</h2>
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">
            Редактор скриптов будет доступен в этапе 9
          </div>
        </div>

      {:else if currentNav === 'settings'}
        <div class="space-y-4">
          <h2 class="text-2xl font-bold text-surface-900-100">Настройки</h2>
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">
            Настройки системы будут доступны после этапа 3
          </div>
        </div>
      {/if}
    </div>
  </main>
</div>
{/if}
