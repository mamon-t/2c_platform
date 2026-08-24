<script lang="ts">
  import { onMount } from 'svelte';
  import { api, unwrapPlugin,
    type EntityType, type Object as PlatformObject, type ObjectPage,
    type RequestRouteTS, type RequestApprovalTS, type RouteStepTS,
    type CertificateInfo, type PluginEnvelope,
  } from '$lib/services/api';
  import { auth, hasPermission } from '$lib/stores/auth';

  const MODULE = 'requests';

  // ── State ──
  let loading = $state(true);
  let error = $state('');
  let notice = $state('');
  let tab = $state<'mine' | 'pending' | 'all' | 'routes'>('mine');

  let entityTypes = $state<EntityType[]>([]);
  let myObjects = $state<PlatformObject[]>([]);
  let pendingApprovals = $state<RequestApprovalTS[]>([]);
  let allApprovals = $state<RequestApprovalTS[]>([]);
  let routes = $state<RequestRouteTS[]>([]);
  let certificates = $state<CertificateInfo[]>([]);
  let users = $state<{ _id: string; display_name: string }[]>([]);
  let roles = $state<{ _id: string; code: string; name: string }[]>([]);
  let approvalsByRequest = $state<Record<string, RequestApprovalTS>>({});

  // Создание заявки
  let showCreate = $state(false);
  let creating = $state(false);
  let newReq = $state({ title: '', priority: 'medium', amount: '', deadline: '', description: '' });

  // Отправка на согласование
  let submitTarget = $state<PlatformObject | null>(null);
  let submitRouteCode = $state('');
  let submitCertSha1 = $state('');

  // Решение по этапу
  let decideTarget = $state<{ approval: RequestApprovalTS; approve: boolean } | null>(null);
  let decideComment = $state('');
  let decideCertSha1 = $state('');

  // Редактор маршрута
  let showRouteEditor = $state(false);
  let editingRoute = $state<RequestRouteTS | null>(null);
  let routeForm = $state<RequestRouteTS>(emptyRoute());
  let expanded = $state<string>('');

  function emptyRoute(): RequestRouteTS {
    return {
      code: '', name: '', description: null, is_active: true,
      requires_signature: false,
      steps: [{ step_order: 1, approver_type: 'user', approver_id: '', approver_name: null, timeout_hours: 0, is_required: true }],
    };
  }

  const canSubmit = () => $auth && hasPermission($auth.permissions, 'requests', 'submit');
  const canApprove = () => $auth && hasPermission($auth.permissions, 'requests', 'approve');
  const canManageRoutes = () => $auth && hasPermission($auth.permissions, 'requests', 'manage_routes');
  const canReadAll = () => $auth && hasPermission($auth.permissions, 'requests', 'read_all');

  function requestType(): EntityType | undefined {
    return entityTypes.find(t => t.code.toLowerCase() === 'request' || t.name.toLowerCase() === 'заявка');
  }

  // ── Загрузка ──
  async function load() {
    loading = true;
    error = '';
    try {
      entityTypes = await api.listEntityTypes();
      try {
        certificates = await api.listCryptoCertificates();
      } catch { certificates = []; }
      try {
        [users, roles] = await Promise.all([api.usersList(), api.rolesList()]);
      } catch { /* не критично */ }

      const rt = requestType();
      if (rt) {
        const page = await api.listObjects({ entity_type_id: rt._id, limit: 200 });
        myObjects = page.objects;
        const ids = myObjects.map(o => o._id);
        const pairs = await Promise.all(ids.map(async id => {
          try {
            const env = await api.pluginCall<RequestApprovalTS | null>(MODULE, 'approval_get', { request_id: id });
            return [id, unwrapPlugin(env)] as const;
          } catch { return [id, null] as const; }
        }));
        approvalsByRequest = Object.fromEntries(pairs.filter(([, v]) => v));
      }

      try {
        pendingApprovals = unwrapPlugin(await api.pluginCall<RequestApprovalTS[]>(MODULE, 'pending_approvals'));
      } catch (e) { pendingApprovals = []; if (!canSubmit()) throw e; }
      try {
        allApprovals = unwrapPlugin(await api.pluginCall<RequestApprovalTS[]>(MODULE, 'all_approvals'));
      } catch { allApprovals = []; }
      try {
        routes = unwrapPlugin(await api.pluginCall<RequestRouteTS[]>(MODULE, 'routes_list'));
      } catch (e) { routes = []; if (canManageRoutes()) throw e; }
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка загрузки';
    } finally {
      loading = false;
    }
  }

  onMount(load);

  // ── Хелперы ──
  function fmtTs(ms?: number | null): string {
    if (!ms) return '—';
    return new Date(ms).toLocaleString('ru-RU');
  }

  function statusMeta(s: string): { label: string; cls: string } {
    switch (s) {
      case 'in_progress': return { label: 'На согласовании', cls: 'bg-warning-500 text-white' };
      case 'approved': return { label: 'Согласована', cls: 'bg-success-500 text-white' };
      case 'rejected': return { label: 'Отклонена', cls: 'bg-error-500 text-white' };
      case 'cancelled': return { label: 'Отменена', cls: 'bg-surface-400 text-white' };
      default: return { label: s, cls: 'bg-surface-300' };
    }
  }

  function stepStatusMeta(s: string): { label: string; icon: string } {
    switch (s) {
      case 'approved': return { label: 'Согласован', icon: 'fa-solid fa-circle-check text-success-500' };
      case 'rejected': return { label: 'Отклонён', icon: 'fa-solid fa-circle-xmark text-error-500' };
      case 'skipped': return { label: 'Пропущен', icon: 'fa-solid fa-circle-minus text-surface-400' };
      default: return { label: 'Ожидает', icon: 'fa-regular fa-circle text-warning-500' };
    }
  }

  function bytesToBase64(bytes: number[]): string {
    let bin = '';
    for (const b of bytes) bin += String.fromCharCode(b);
    return btoa(bin);
  }

  function certOk(c: CertificateInfo): boolean {
    return c.has_private_key && c.is_valid;
  }

  // Тестовый сертификат (когда в MY пусто)
  let makingCert = $state(false);
  async function makeTestCert() {
    const name = prompt('Имя владельца (латиницей):', 'Test User');
    if (!name?.trim()) return;
    makingCert = true; error = ''; notice = '';
    try {
      const info = await api.createTestCertificate(name.trim());
      notice = `Тестовый сертификат создан: ${info.split('|')[1]}`;
      certificates = await api.listCryptoCertificates();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка создания сертификата';
    } finally {
      makingCert = false;
    }
  }

  // ── Создание ──
  async function createRequest() {
    const rt = requestType();
    if (!rt) { error = 'Тип сущности REQUEST не найден. Создайте его в разделе «Метаданные».'; return; }
    if (!newReq.title.trim()) { error = 'Укажите тему заявки'; return; }
    creating = true;
    error = '';
    try {
      await api.createObject({
        entity_type_id: rt._id,
        data: {
          title: newReq.title.trim(),
          priority: newReq.priority,
          amount: newReq.amount ? Number(newReq.amount) : null,
          deadline: newReq.deadline || null,
          description: newReq.description || null,
        },
        parent_id: null,
        date: new Date().toISOString().slice(0, 10),
      });
      showCreate = false;
      newReq = { title: '', priority: 'medium', amount: '', deadline: '', description: '' };
      notice = 'Заявка создана';
      await load();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка создания';
    } finally {
      creating = false;
    }
  }

  // ── Отправка на согласование ──
  let submitNeedsSig = $derived(
    routes.find(r => r.code === submitRouteCode)?.requires_signature ?? false
  );

  function openSubmit(o: PlatformObject) {
    submitTarget = o;
    const active = routes.filter(r => r.is_active);
    // По умолчанию — маршрут без ЭЦП, если есть
    submitRouteCode = (active.find(r => !r.requires_signature) ?? active[0])?.code ?? '';
    submitCertSha1 = '';
  }

  async function doSubmit() {
    if (!submitTarget) return;
    if (!submitRouteCode) { error = 'Выберите маршрут'; return; }
    let sigB64: string | null = null;
    if (submitNeedsSig) {
      if (!submitCertSha1) { error = 'Маршрут требует ЭЦП: выберите сертификат'; return; }
      const payload = canonicalSubmitPayload({
        id: submitTarget._id,
        version: submitTarget.version,
        state: submitTarget.state,
      });
      const sig = await api.signDocument(btoa(unescape(encodeURIComponent(payload))), submitCertSha1, true);
      sigB64 = bytesToBase64(sig.signature_der);
    }
    error = '';
    try {
      const env = await api.pluginCall(MODULE, 'submit', {
        request_id: submitTarget._id,
        route_code: submitRouteCode,
        signature_der: sigB64,
      });
      unwrapPlugin(env);
      submitTarget = null;
      notice = 'Отправлено на согласование';
      await load();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка отправки';
    }
  }

  // ── Решение ──
  function openDecide(a: RequestApprovalTS, approve: boolean) {
    decideTarget = { approval: a, approve };
    decideComment = '';
    decideCertSha1 = '';
  }

  async function doDecide() {
    if (!decideTarget) return;
    let sigB64: string | null = null;
    if (decideTarget.approval.requires_signature) {
      if (!decideCertSha1) { error = 'Маршрут требует ЭЦП: выберите сертификат'; return; }
      const sig = await api.signDocument(
        btoa(unescape(encodeURIComponent(canonicalDecisionPayload(
          decideTarget.approval.request_id,
          decideTarget.approve,
          decideComment || '',
        )))),
        decideCertSha1, true);
      sigB64 = bytesToBase64(sig.signature_der);
    }
    error = '';
    try {
      const fn = decideTarget.approve ? 'approve_step' : 'reject_step';
      const env = await api.pluginCall(MODULE, fn, {
        request_id: decideTarget.approval.request_id,
        comment: decideComment || null,
        signature_der: sigB64 ?? '',
      });
      unwrapPlugin(env);
      decideTarget = null;
      notice = 'Решение принято';
      await load();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка решения';
    }
  }

  // ── Маршруты ──
  function openRouteEditor(route?: RequestRouteTS) {
    editingRoute = route ?? null;
    routeForm = route ? structuredClone($state.snapshot(route)) : emptyRoute();
    showRouteEditor = true;
  }

  function addStep() {
    routeForm.steps.push({
      step_order: routeForm.steps.length + 1,
      approver_type: 'user',
      approver_id: '',
      approver_name: null,
      timeout_hours: 0,
      is_required: true,
    });
  }

  function removeStep(i: number) {
    routeForm.steps.splice(i, 1);
    routeForm.steps.forEach((s, idx) => (s.step_order = idx + 1));
  }

  function userName(id: string): string {
    if (!id) return '— не выбрано —';
    const u = users.find(u => u._id === id);
    return u?.display_name ?? id.slice(0, 8) + '…';
  }

  function roleName(id: string): string {
    if (!id) return '— не выбрано —';
    const r = roles.find(r => r._id === id);
    return r?.name ?? r?.code ?? id.slice(0, 8) + '…';
  }

  async function saveRoute() {
    error = '';
    try {
      const env = await api.pluginCall(MODULE, 'routes_save', { ...$state.snapshot(routeForm) });
      unwrapPlugin(env);
      showRouteEditor = false;
      await load();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка сохранения маршрута';
    }
  }

  async function deleteRoute(code: string) {
    if (!confirm(`Удалить маршрут «${code}»?`)) return;
    try {
      unwrapPlugin(await api.pluginCall(MODULE, 'routes_delete', { code }));
      await load();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка удаления';
    }
  }

  // Отмена процедуры инициатором
  async function cancelApproval(requestId: string) {
    if (!confirm('Отменить процедуру согласования? Заявка останется черновиком.')) return;
    try {
      unwrapPlugin(await api.pluginCall(MODULE, 'cancel_request', { request_id: requestId }));
      notice = 'Согласование отменено';
      await load();
    } catch (e: any) {
      error = typeof e === 'string' ? e : e?.message ?? 'Ошибка отмены';
    }
  }

  const meId = (): string => $auth?.user?._id ?? '';
</script>

<div class="container mx-auto p-4 space-y-4">
  <header class="flex items-center justify-between">
    <h2 class="h4 flex items-center gap-2">
      <i class="fa-solid fa-file-signature"></i> Заявки
    </h2>
    <div class="flex gap-2">
      {#if canSubmit() && requestType()}
        <button class="btn btn-sm btn-primary" onclick={() => (showCreate = true)}>
          <i class="fa-solid fa-plus"></i> Создать заявку
        </button>
      {/if}
      <button class="btn btn-sm btn-outline" onclick={load}>
        <i class="fa-solid fa-rotate"></i>
      </button>
    </div>
  </header>

  {#if error}<div class="alert alert-error">{error}</div>{/if}
  {#if notice}<div class="alert alert-success">{notice}</div>{/if}

  <!-- Табы -->
  <div class="flex gap-1 border-b border-surface-200">
    {#each [['mine', 'Мои заявки'], ['pending', `На согласовании (${pendingApprovals.length})`], ['all', 'Все согласования'], ['routes', 'Маршруты']] as [key, label]}
      <button
        class="btn btn-sm {tab === key ? 'variant-filled-primary' : 'btn-transparent'} rounded-b-none"
        onclick={() => (tab = key as typeof tab)}
      >{label}</button>
    {/each}
  </div>

  {#if loading}
    <div class="p-8 text-center text-surface-500"><i class="fa-solid fa-spinner fa-spin"></i> Загрузка…</div>
  {:else if tab === 'mine'}
    <!-- Мои заявки -->
    <div class="space-y-2">
      {#each myObjects as o (o._id)}
        {@const apr = approvalsByRequest[o._id]}
        <div class="card p-3">
          <div class="flex items-center justify-between gap-3 cursor-pointer" onclick={() => (expanded = expanded === o._id ? '' : o._id)}>
            <div class="flex items-center gap-3 min-w-0">
              <i class="fa-solid fa-chevron-right transition-transform" class:rotate-90={expanded === o._id}></i>
              <span class="font-medium truncate">{o.data?.title ?? o.number ?? o._id.slice(0, 8)}</span>
              {#if apr}
                <span class="badge {statusMeta(apr.status).cls} text-xs">{statusMeta(apr.status).label}</span>
                <span class="text-xs text-surface-400">маршрут: {apr.route_name}</span>
              {:else}
                <span class="badge bg-surface-300 text-xs">Черновик</span>
              {/if}
            </div>
            <div class="flex gap-2 shrink-0">
              {#if !apr && canSubmit()}
                <button class="btn btn-sm variant-filled-secondary" onclick={(e) => { e.stopPropagation(); openSubmit(o); }}>
                  <i class="fa-solid fa-paper-plane"></i> Отправить
                </button>
              {:else if apr?.status === 'in_progress'}
                <button class="btn btn-sm btn-outline" onclick={(e) => { e.stopPropagation(); cancelApproval(o._id); }}>
                  <i class="fa-solid fa-ban"></i> Отозвать
                </button>
              {/if}
            </div>
          </div>

          {#if expanded === o._id && apr}
            <!-- Timeline этапов -->
            <div class="mt-3 ml-6 border-l-2 border-surface-300 pl-4 space-y-2">
              {#each apr.steps as step, i (step.step_order)}
                <div class="flex items-start gap-3">
                  <i class="{stepStatusMeta(step.status).icon} mt-1"></i>
                  <div class="min-w-0">
                    <div class="text-sm font-medium">
                      Этап {step.step_order}: {step.approver_name ?? (step.approver_type === 'role' ? roleName(step.approver_id) : userName(step.approver_id))}
                      {#if i === apr.current_step && apr.status === 'in_progress'}
                        <span class="badge bg-warning-500 text-white text-xs ml-1">текущий</span>
                      {/if}
                    </div>
                    {#if step.decided_at}
                      <div class="text-xs text-surface-500">{fmtTs(step.decided_at)}{step.comment ? ` · ${step.comment}` : ''}</div>
                    {/if}
                    {#if step.signature_der}
                      <div class="text-xs text-success-600"><i class="fa-solid fa-signature"></i> подписан ({step.signature_der.length} симв.)</div>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else}
        <div class="p-8 text-center text-surface-400">
          Заявок нет.
          {#if !requestType()}Сначала создайте тип сущности REQUEST в «Метаданных».{/if}
        </div>
      {/each}
    </div>

  {:else if tab === 'pending'}
    <!-- На согласовании у меня -->
    <div class="space-y-2">
      {#each pendingApprovals as a (a.request_id)}
        <div class="card p-3 flex items-center justify-between gap-3">
          <div class="min-w-0">
            <div class="font-medium truncate">{a.request_id.slice(0, 8)}… · маршрут «{a.route_name}»</div>
            <div class="text-xs text-surface-500">
              от {a.initiator_name ?? a.initiator_login}, {fmtTs(a.submitted_at)}
              · этап {a.current_step + 1} из {a.steps.length}
            </div>
          </div>
          {#if canApprove()}
            <div class="flex gap-2 shrink-0">
              <button class="btn btn-sm variant-filled-success" onclick={() => openDecide(a, true)}>
                <i class="fa-solid fa-check"></i> Согласовать
              </button>
              <button class="btn btn-sm variant-filled-error" onclick={() => openDecide(a, false)}>
                <i class="fa-solid fa-xmark"></i> Отклонить
              </button>
            </div>
          {/if}
        </div>
      {:else}
        <div class="p-8 text-center text-surface-400">Нет заявок, ожидающих вашего решения.</div>
      {/each}
    </div>

  {:else if tab === 'all'}
    <!-- Все согласования -->
    {#if canReadAll()}
      <div class="space-y-2">
        {#each allApprovals as a (a.request_id)}
          <div class="card p-3 flex items-center justify-between gap-3">
            <div class="min-w-0">
              <div class="font-medium truncate">{a.request_id.slice(0, 8)}… · «{a.route_name}»</div>
              <div class="text-xs text-surface-500">
                от {a.initiator_login} · {fmtTs(a.submitted_at)}
                {#if a.completed_at}→ {fmtTs(a.completed_at)}{/if}
              </div>
            </div>
            <span class="badge {statusMeta(a.status).cls} shrink-0">{statusMeta(a.status).label}</span>
          </div>
        {:else}
          <div class="p-8 text-center text-surface-400">Пока нет ни одной процедуры.</div>
        {/each}
      </div>
    {:else}
      <div class="p-8 text-center text-surface-400">Недостаточно прав (requests.read_all).</div>
    {/if}

  {:else if tab === 'routes'}
    <!-- Маршруты -->
    {#if canManageRoutes()}
      <button class="btn btn-sm btn-primary" onclick={() => openRouteEditor()}>
        <i class="fa-solid fa-plus"></i> Новый маршрут
      </button>
      <div class="space-y-2">
        {#each routes as r (r.code)}
          <div class="card p-3 flex items-center justify-between gap-3">
            <div class="min-w-0">
              <div class="font-medium">{r.name} <span class="text-xs text-surface-400">({r.code})</span>
                {#if !r.is_active}<span class="badge bg-surface-300 text-xs">отключён</span>{/if}
              </div>
              <div class="text-xs text-surface-500 truncate">
                {r.steps.map(s => `${s.step_order}. ${s.approver_name ?? (s.approver_type === 'role' ? roleName(s.approver_id) : userName(s.approver_id))}`).join(' → ')}
              </div>
            </div>
            <div class="flex gap-2 shrink-0">
              <button class="btn btn-sm btn-outline" onclick={() => openRouteEditor(r)}><i class="fa-solid fa-pen"></i></button>
              <button class="btn btn-sm btn-outline text-error-600" onclick={() => deleteRoute(r.code)}><i class="fa-solid fa-trash"></i></button>
            </div>
          </div>
        {:else}
          <div class="p-8 text-center text-surface-400">Маршрутов ещё нет.</div>
        {/each}
      </div>
    {:else}
      <div class="p-8 text-center text-surface-400">Недостаточно прав (requests.manage_routes).</div>
    {/if}
  {/if}
</div>

<!-- Модалка создания -->
{#if showCreate}
  <div class="fixed inset-0 bg-black/50 grid place-items-center z-50" role="presentation">
    <div class="card p-5 w-[480px] space-y-3">
      <h3 class="h5">Новая заявка</h3>
      <label class="label"><span class="text-error-500">*</span> Тема</label>
      <input class="input" bind:value={newReq.title} placeholder="Например: закупка оргтехники" />
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="label">Приоритет</label>
          <select class="select" bind:value={newReq.priority}>
            <option value="low">Низкий</option>
            <option value="medium">Средний</option>
            <option value="high">Высокий</option>
            <option value="critical">Критический</option>
          </select>
        </div>
        <div>
          <label class="label">Сумма</label>
          <input class="input" type="number" bind:value={newReq.amount} placeholder="0.00" />
        </div>
      </div>
      <label class="label">Срок исполнения</label>
      <input class="input" type="date" bind:value={newReq.deadline} />
      <label class="label">Описание</label>
      <textarea class="textarea" rows="3" bind:value={newReq.description}></textarea>
      <div class="flex justify-end gap-2 pt-2">
        <button class="btn btn-outline" onclick={() => (showCreate = false)}>Отмена</button>
        <button class="btn btn-primary" disabled={creating} onclick={createRequest}>
          {creating ? 'Создание…' : 'Создать'}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Модалка отправки -->
{#if submitTarget}
  <div class="fixed inset-0 bg-black/50 grid place-items-center z-50" role="presentation">
    <div class="card p-5 w-[440px] space-y-3">
      <h3 class="h5">Отправить на согласование</h3>
      <p class="text-sm text-surface-500 truncate">{submitTarget.data?.title ?? submitTarget._id}</p>
      <label class="label">Маршрут</label>
      <select class="select" bind:value={submitRouteCode}>
        {#each routes.filter(r => r.is_active) as r (r.code)}
          <option value={r.code}>{r.name} ({r.steps.length} эт.{r.requires_signature ? ' · ЭЦП' : ''})</option>
        {/each}
      </select>

      {#if submitNeedsSig}
        <label class="label"><i class="fa-solid fa-signature"></i> Сертификат подписи (маршрут требует ЭЦП)</label>
        <select class="select" bind:value={submitCertSha1}>
          <option value="" disabled selected>— выберите сертификат —</option>
          {#each certificates.filter(certOk) as c (c.sha1_hash)}
            <option value={c.sha1_hash}>{c.subject_name}</option>
          {/each}
        </select>
        {#if certificates.filter(certOk).length === 0}
          <div class="text-xs text-warn-600">
            Не найдено валидных сертификатов с приватным ключом (КриптоПро).
            <button class="underline hover:text-warn-700" onclick={makeTestCert} disabled={makingCert}>
              {makingCert ? 'Создание…' : 'Создать тестовый'}
            </button>
          </div>
        {/if}
      {:else}
        <div class="text-xs text-surface-400"><i class="fa-solid fa-circle-info"></i> Маршрут без электронной подписи</div>
      {/if}

      <div class="flex justify-end gap-2 pt-2">
        <button class="btn btn-outline" onclick={() => (submitTarget = null)}>Отмена</button>
        <button class="btn btn-primary" onclick={doSubmit}>
          <i class="fa-solid fa-paper-plane"></i> {submitNeedsSig ? 'Подписать и отправить' : 'Отправить'}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Модалка решения -->
{#if decideTarget}
  <div class="fixed inset-0 bg-black/50 grid place-items-center z-50" role="presentation">
    <div class="card p-5 w-[440px] space-y-3">
      <h3 class="h5">
        {decideTarget.approve ? '✓ Согласовать' : '✗ Отклонить'} заявку {decideTarget.approval.request_id.slice(0, 8)}…
      </h3>
      <label class="label">Комментарий</label>
      <textarea class="textarea" rows="3" bind:value={decideComment}
        placeholder={decideTarget.approve ? 'Согласовано' : 'Причина отклонения…'}></textarea>
      {#if decideTarget.approval.requires_signature}
        <label class="label"><i class="fa-solid fa-signature"></i> Сертификат подписи (маршрут требует ЭЦП)</label>
        <select class="select" bind:value={decideCertSha1}>
          <option value="" disabled selected>— выберите сертификат —</option>
          {#each certificates.filter(certOk) as c (c.sha1_hash)}
            <option value={c.sha1_hash}>{c.subject_name}</option>
          {/each}
        </select>
      {:else}
        <div class="text-xs text-surface-400"><i class="fa-solid fa-circle-info"></i> Маршрут без электронной подписи</div>
      {/if}

      <div class="flex justify-end gap-2 pt-2">
        <button class="btn btn-outline" onclick={() => (decideTarget = null)}>Отмена</button>
        <button class="btn {decideTarget.approve ? 'btn-success' : 'btn-error'}" onclick={doDecide}>
          <i class="fa-solid {decideTarget.approval.requires_signature ? 'fa-signature' : 'fa-check'}"></i>
          {decideTarget.approve ? 'Согласовать' : 'Отклонить'}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Редактор маршрута -->
{#if showRouteEditor}
  <div class="fixed inset-0 bg-black/50 grid place-items-center z-50 overflow-auto py-8" role="presentation">
    <div class="card p-5 w-[640px] space-y-3">
      <h3 class="h5">{editingRoute ? 'Изменить маршрут' : 'Новый маршрут'}</h3>
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="label">Код *</label>
          <input class="input" bind:value={routeForm.code} disabled={!!editingRoute} placeholder="STANDARD" />
        </div>
        <div>
          <label class="label">Название *</label>
          <input class="input" bind:value={routeForm.name} placeholder="Стандартное согласование" />
        </div>
      </div>
      <label class="flex items-center gap-2 text-sm">
        <input type="checkbox" class="checkbox" bind:checked={routeForm.is_active} /> Активен
      </label>
      <label class="flex items-center gap-2 text-sm" title="Submit/approve/reject потребуют квалифицированной ЭЦП">
        <input type="checkbox" class="checkbox" bind:checked={routeForm.requires_signature} />
        <i class="fa-solid fa-signature"></i> Требовать электронную подпись
      </label>

      <div class="divider">Этапы ({routeForm.steps.length})</div>
      {#each routeForm.steps as step, i}
        <div class="border border-surface-200 rounded p-3 space-y-2">
          <div class="flex items-center justify-between">
            <span class="text-sm font-medium">Этап {i + 1}</span>
            <button class="btn btn-sm btn-transparent text-error-600" onclick={() => removeStep(i)}>
              <i class="fa-solid fa-trash"></i>
            </button>
          </div>
          <div class="grid grid-cols-2 gap-2">
            <div>
              <label class="label text-xs">Тип</label>
              <select class="select select-sm" bind:value={step.approver_type}>
                <option value="user">Пользователь</option>
                <option value="role">Роль</option>
              </select>
            </div>
            <div>
              <label class="label text-xs">Утверждающий *</label>
              <select class="select select-sm" bind:value={step.approver_id}>
                <option value="" disabled>— выберите —</option>
                {#if step.approver_type === 'user'}
                  {#each users as u (u._id)}<option value={u._id}>{u.display_name}</option>{/each}
                {:else}
                  {#each roles as r (r._id)}<option value={r._id}>{r.name}</option>{/each}
                {/if}
              </select>
            </div>
          </div>
        </div>
      {/each}
      <button class="btn btn-sm btn-outline w-full" onclick={addStep}><i class="fa-solid fa-plus"></i> Добавить этап</button>

      <div class="flex justify-end gap-2 pt-2">
        <button class="btn btn-outline" onclick={() => (showRouteEditor = false)}>Отмена</button>
        <button class="btn btn-primary" onclick={saveRoute}>Сохранить</button>
      </div>
    </div>
  </div>
{/if}
