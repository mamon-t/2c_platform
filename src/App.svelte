<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  import { theme } from '$lib/stores/theme';
  import { allNavItems } from '$lib/stores/navigation';
  import { auth, isAuthenticated, hasPermission, type AuthUser } from '$lib/stores/auth';
  import { get } from 'svelte/store';
  import { api, type DiagnosticsReport, type User, type SavedConnection } from '$lib/services/api';
  import { toastSuccess, toastError } from '$lib/components/ui/toast';
  import { onMount } from 'svelte';

  import AuditPage from '$lib/components/AuditPage.svelte';
  import SettingsPage from '$lib/components/SettingsPage.svelte';
  import EventsPage from '$lib/components/EventsPage.svelte';
  import MetadataPage from '$lib/components/MetadataPage.svelte';
  import ObjectsPage from '$lib/components/ObjectsPage.svelte';
  import RequestsPage from '$lib/components/RequestsPage.svelte';
  import DevicesPage from '$lib/components/DevicesPage.svelte';
  import StockPage from '$lib/components/StockPage.svelte';
  import TradePage from '$lib/components/TradePage.svelte';
  import MessagesPage from '$lib/components/MessagesPage.svelte';
  import ModulesPage from '$lib/components/ModulesPage.svelte';
  import PrintPage from '$lib/components/PrintPage.svelte';
  import NumberingPage from '$lib/components/NumberingPage.svelte';
  import ScriptsPage from '$lib/components/ScriptsPage.svelte';
  import ReportsPage from '$lib/components/ReportsPage.svelte';
  import ConvertPage from '$lib/components/ConvertPage.svelte';
  import OpeningBalancesScreen from '$lib/components/screens/OpeningBalancesScreen.svelte';

  import DbConnectScreen from '$lib/components/screens/DbConnectScreen.svelte';
  import LoginScreen from '$lib/components/screens/LoginScreen.svelte';
  import DashboardScreen from '$lib/components/screens/DashboardScreen.svelte';
  import CompaniesScreen from '$lib/components/screens/CompaniesScreen.svelte';
  import UsersScreen from '$lib/components/screens/UsersScreen.svelte';
  import RolesScreen from '$lib/components/screens/RolesScreen.svelte';
  import NotificationsBell from '$lib/components/NotificationsBell.svelte';
  import Toaster from '$lib/components/ui/Toaster.svelte';
  import DialogHost from '$lib/components/ui/DialogHost.svelte';
  import ConnectionsDialog from '$lib/components/ui/ConnectionsDialog.svelte';

  let loading = $state(true);
  let connected = $state(false);
  let diagnostics = $state<DiagnosticsReport | null>(null);
  let currentNav = $state('dashboard');
  let sidebarCollapsed = $state(false);
  let currentUser = $state<User | null>(null);
  let authUserData = $state<AuthUser | null>(null);
  let connections = $state<SavedConnection[]>([]);
  let currentDbUri = $state('');
  let showConnections = $state(false);

  // Состояние свёрнутых групп сайдбара (хранится в localStorage)
  const STORAGE_KEY = '2c-sidebar-collapsed-groups';
  function loadCollapsed(): Set<string> {
    try {
      const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]');
      if (Array.isArray(stored) && stored.length > 0) return new Set(stored);
    } catch { /* ignore */ }
    // Первый запуск: инициализировать из defaultCollapsed
    const defaults = new Set<string>();
    for (const item of allNavItems) {
      if (item.group && item.defaultCollapsed) defaults.add(item.group);
    }
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...defaults]));
    return defaults;
  }
  let collapsedGroups = $state(loadCollapsed());
  function toggleGroup(group: string) {
    collapsedGroups = new Set(collapsedGroups);
    if (collapsedGroups.has(group)) collapsedGroups.delete(group); else collapsedGroups.add(group);
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...collapsedGroups]));
  }

  let filteredNavItems = $derived(
    allNavItems.filter(item =>
      !item.requiredPermission ||
      !authUserData ||
      authUserData.permissions.length === 0 ||
      hasPermission(authUserData.permissions, item.requiredPermission.subsystem, item.requiredPermission.action)
    )
  );

  function toggleSidebar() { sidebarCollapsed = !sidebarCollapsed; }
  function setNav(code: string) { currentNav = code; }

  // ── Подключение к БД ──
  async function handleConnect(uri: string, dbName: string): Promise<string | null> {
    try {
      await api.connectDb(uri, dbName);
      connected = true;
      diagnostics = await api.getDiagnostics();
      return null;
    } catch (e) {
      return typeof e === 'string' ? e : (e as Error)?.message ?? 'Ошибка подключения';
    }
  }

  // ── Сохранённые подключения (список баз, как в 1С) ──
  async function reloadConnections() {
    try { connections = await api.listConnections(); } catch { connections = []; }
    try {
      const cfg = await api.getAppConfig();
      currentDbUri = cfg.mongodb_uri ?? '';
    } catch { /* тихо */ }
  }

  async function handleSelectConnection(conn: SavedConnection): Promise<string | null> {
    try {
      await api.connectDb(conn.uri, conn.db_name);
      diagnostics = await api.getDiagnostics();
      currentDbUri = conn.uri;
      toastSuccess(`Подключено: ${conn.name}`);
      return null;
    } catch (e) {
      return typeof e === 'string' ? e : (e as Error)?.message ?? 'Ошибка подключения';
    }
  }

  // ── Вход ──
  async function handleLogin(login: string, password: string): Promise<string | null> {
    try {
      const result = await api.authenticate(login, password);
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
      } catch { /* тихо */ }

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
      // Pre-load WASM-модулей для компании (бесшовно, ошибки — в toast)
      api.preloadModules().then((r) => {
        if (r.errors.length > 0) {
          const msg = r.errors.map((e) => `${e.code}: ${e.error}`).join('\n');
          toastError(`Ошибки загрузки модулей:\n${msg}`);
        }
      }).catch(() => { /* тихо при offline */ });
      return null;
    } catch (e) {
      return typeof e === 'string' ? e : (e as Error)?.message ?? 'Ошибка авторизации';
    }
  }

  function handleLogout() { auth.logout(); currentUser = null; authUserData = null; }

  async function handleExit() {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    getCurrentWindow().close();
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
      // Pre-load модулей для новой компании
      api.preloadModules().catch(() => { /* тихо */ });
    } catch (e) {
      console.error('Ошибка смены компании:', e);
    }
  }

  onMount(async () => {
    theme.init();
    reloadConnections();
    // Автоподключение: backend сам возьмёт MONGODB_URI из .env, если URI пустой
    try {
      diagnostics = await api.getDiagnostics();
      if (!diagnostics?.mongodb.ok) {
        diagnostics = { ...diagnostics!, mongodb: await api.connectDb('', '') };
      }
      connected = diagnostics?.mongodb.ok ?? false;
    } catch { /* покажем экран ручного подключения */ }
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
        } catch { /* тихо */ }
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
</script>

{#if !connected && !loading}
  <DbConnectScreen onSubmit={handleConnect} />
{:else if !$isAuthenticated && !loading}
  <LoginScreen
    onSubmit={handleLogin}
    {connections}
    currentUri={currentDbUri}
    onSelectConnection={handleSelectConnection}
    onOpenConnections={() => (showConnections = true)}
  />
{:else}
  <div class="flex h-screen overflow-hidden bg-surface-50-950">
    <main class="flex flex-1 flex-col overflow-hidden">
      <header class="flex shrink-0 items-center justify-between border-b border-surface-300-700 px-4 py-2">
        <h1 class="text-base font-semibold text-surface-900-100">{filteredNavItems.find((n) => n.code === currentNav)?.label ?? 'Главная'}</h1>
        <div class="flex items-center gap-3">
          <span class="rounded-full px-2 py-0.5 text-xs font-medium {diagnostics?.mongodb.ok ? 'bg-success-500/20 text-success-700' : 'bg-error-500/20 text-error-700'}">
            DB {diagnostics?.mongodb.ok ? 'OK' : 'ERR'}
          </span>
          <NotificationsBell />
          <button onclick={() => theme.set($theme === 'dark' ? 'light' : 'dark')} class="rounded-lg p-2 text-surface-500-500 hover:bg-surface-200-800" title="Сменить тему" aria-label="Сменить тему">
            <i class="fa-solid {$theme === 'dark' ? 'fa-sun' : 'fa-moon'}"></i>
          </button>
        </div>
      </header>

      <div class="flex-1 overflow-y-auto p-4">
        {#if currentNav === 'dashboard'}
          <DashboardScreen {diagnostics} {loading} />

        {:else if currentNav === 'companies'}
          <CompaniesScreen />
        {:else if currentNav === 'users'}
          <UsersScreen />
        {:else if currentNav === 'roles'}
          <RolesScreen />

        {:else if currentNav === 'convert'}
          <ConvertPage />
        {:else if currentNav === 'opening_balances'}
          <OpeningBalancesScreen />
        {:else if currentNav === 'objects' || currentNav === 'documents' || currentNav === 'catalogs'}
          <ObjectsPage />
        {:else if currentNav === 'requests'}
          <RequestsPage />
        {:else if currentNav === 'devices'}
          <DevicesPage />
        {:else if currentNav === 'stock'}
          <StockPage />
        {:else if currentNav === 'trade'}
          <TradePage />
        {:else if currentNav === 'messages'}
          <MessagesPage />
        {:else if currentNav === 'events'}
          <EventsPage />
        {:else if currentNav === 'metadata'}
          <MetadataPage />
        {:else if currentNav === 'audit'}
          <AuditPage />
        {:else if currentNav === 'modules'}
          <ModulesPage />
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
          <div class="rounded-xl border border-surface-300-700 bg-surface-50-950 p-6 text-center text-surface-500-500">Раздел в разработке</div>
        {/if}
      </div>
    </main>

    <aside class="flex flex-col border-l border-surface-300-700 bg-surface-100-900 transition-all duration-300" class:w-64={!sidebarCollapsed} class:w-16={sidebarCollapsed}>
      <div class="flex items-center gap-2 border-b border-surface-300-700 p-3">
        {#if !sidebarCollapsed}<span class="text-sm font-semibold text-surface-900-100">Платформа</span>{/if}
        <button onclick={toggleSidebar} class="ml-auto rounded p-1 text-surface-500-500 hover:bg-surface-200-800" aria-label={sidebarCollapsed ? 'Развернуть меню' : 'Свернуть меню'}>
          <i class="fa-solid {sidebarCollapsed ? 'fa-chevron-left' : 'fa-chevron-right'} text-sm"></i>
        </button>
      </div>
      <nav class="flex-1 overflow-y-auto p-2">
        {#each filteredNavItems as item, i (item.code)}
          {#if item.group && filteredNavItems[i - 1]?.group !== item.group}
            {@const isCollapsed = collapsedGroups.has(item.group)}
            <button
              onclick={() => toggleGroup(item.group!)}
              class="flex w-full items-center gap-1.5 px-3 pb-1 pt-3 text-[11px] font-medium uppercase tracking-wide text-surface-400 hover:text-surface-600-400"
            >
              <i class="fa-solid fa-chevron-right text-[9px] transition-transform {isCollapsed ? '' : 'rotate-90'}"></i>
              {item.group}
            </button>
          {/if}
          {#if !item.group || !collapsedGroups.has(item.group)}
            <button onclick={() => setNav(item.code)} aria-label={item.label}
              class="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors {currentNav === item.code ? 'bg-primary-500/10 font-medium text-primary-600' : 'text-surface-600-400 hover:bg-surface-200-800'}">
              <i class="{item.icon} w-5 text-center text-lg"></i>
              {#if !sidebarCollapsed}<span>{item.label}</span>{/if}
            </button>
          {/if}
        {/each}
      </nav>
      {#if currentUser && (get(auth)?.companies?.length ?? 0) > 1 && !sidebarCollapsed}
        <div class="border-t border-surface-300-700 p-3">
          <label class="text-xs text-surface-500-500" for="company-select">Компания</label>
          <select id="company-select"
            class="mt-1 w-full rounded-lg border border-surface-300-700 bg-surface-50-950 px-2 py-1.5 text-sm text-surface-900-100 focus:border-primary-500 focus:outline-none"
            onchange={(e) => handleSwitchCompany(e.currentTarget.value)}>
            {#each (get(auth)?.companies ?? []) as uc (uc.companyId)}
              <option value={uc.companyId} selected={uc.companyId === authUserData?.companyId}>{uc.companyName}</option>
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
                <span class="truncate text-sm font-medium text-surface-900-100">{currentUser.display_name}</span>
                <span class="truncate text-xs text-surface-500-500">{currentUser.login}</span>
              </div>
              <button onclick={handleLogout} class="rounded p-1 text-surface-500-500 hover:bg-surface-200-800" aria-label="Выйти"><i class="fa-solid fa-right-from-bracket text-sm"></i></button>
              <button onclick={handleExit} class="rounded p-1 text-surface-500-500 hover:bg-surface-200-800" aria-label="Закрыть приложение"><i class="fa-solid fa-power-off text-sm"></i></button>
            {/if}
          </div>
        </div>
      {/if}
    </aside>
  </div>

  <Toaster />
  <DialogHost />
  <ConnectionsDialog
    open={showConnections}
    onClose={() => (showConnections = false)}
    onChanged={reloadConnections}
  />
{/if}
