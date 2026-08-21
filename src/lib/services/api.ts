import { getAdapter } from '$lib/adapters/transport';

export interface DiagnosticsReport {
  app_version: string;
  mongodb: {
    connected: boolean;
    host: string;
    version: string | null;
    replica_set: string | null;
    ok: boolean;
  };
  modules: Array<{
    code: string;
    name: string;
    version: string;
    active: boolean;
  }>;
}

export interface Company {
  _id: string;
  code: string;
  name: string;
  inn: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface User {
  _id: string;
  login: string;
  person_id: string | null;
  display_name: string;
  status: string;
  role_ids: string[];
  locale: string | null;
  timezone: string | null;
  last_login_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface Role {
  _id: string;
  company_id: string;
  code: string;
  name: string;
  description: string | null;
  permission_policy_ids: string[];
  created_at: string;
  updated_at: string;
}

export interface PermissionPolicy {
  _id: string;
  code: string;
  name: string;
  description: string | null;
  scope_type: string;
  subsystem_code: string;
  entity_type: string | null;
  actions: string[];
  record_scope: string;
  deny: boolean;
  priority: number;
  created_at: string;
  updated_at: string;
}

export interface Person {
  _id: string;
  last_name: string;
  first_name: string;
  middle_name: string | null;
  display_name: string;
  created_at: string;
  updated_at: string;
}

export interface UserContact {
  _id: string;
  user_id: string;
  channel_type: string;
  value: string;
  is_primary: boolean;
  is_verified: boolean;
  purposes: string[];
  note: string | null;
  created_at: string;
  updated_at: string;
}

export interface UserProfile {
  _id: string;
  company_id: string;
  company_name: string;
  company_code: string;
  role_id: string;
  role_name: string;
  position: string | null;
  department: string | null;
  employee_number: string | null;
  is_primary: boolean;
  is_active: boolean;
}

export interface UserCertificate {
  _id: string;
  user_id: string;
  provider_code: string;
  certificate_ref: string;
  subject: string;
  issuer: string;
  serial_number: string;
  fingerprint: string;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface AuthResult {
  token: string;
  user: User;
  companies: UserProfile[];
  role_code: string | null;
  role_name: string | null;
  role_id: string | null;
}

export interface MyPermissions {
  role_code: string;
  role_name: string;
  permissions: PermissionPolicy[];
}

export interface FieldChange {
  old: string | null;
  new: string | null;
}

export type AuditChanges = Record<string, FieldChange>;

export type AuditableAction =
  | 'login' | 'logout' | 'switch_company'
  | 'create_company' | 'update_company' | 'delete_company'
  | 'create_user' | 'update_user' | 'delete_user' | 'disable_user' | 'unlock_user'
  | 'create_role' | 'update_role' | 'delete_role'
  | 'add_user_profile' | 'update_user_profile' | 'remove_user_profile'
  | 'create_contact' | 'update_contact' | 'delete_contact'
  | 'update_person'
  | 'deactivate_certificate' | 'save_settings'
  | 'create_permission_policy' | 'delete_permission_policy'
  | 'create_document' | 'update_document' | 'delete_document'
  | 'post_document' | 'cancel_document' | 'archive_document'
  | 'create_catalog_entry' | 'update_catalog_entry' | 'delete_catalog_entry'
  | 'emit_event' | 'replay_event' | 'execute_script';

export interface AuditActionMeta {
  label: string;
  icon: string;
  target_type: string;
}

export const AUDIT_ACTION_META: Record<string, AuditActionMeta> = {
  login:                { label: 'Вход в систему',               icon: 'fa-solid fa-right-to-bracket text-success-500',     target_type: 'session' },
  logout:               { label: 'Выход из системы',             icon: 'fa-solid fa-right-from-bracket text-surface-500',   target_type: 'session' },
  switch_company:       { label: 'Смена компании',               icon: 'fa-solid fa-right-left text-warn-500',              target_type: 'company' },
  create_company:       { label: 'Создание компании',            icon: 'fa-solid fa-building text-primary-500',             target_type: 'company' },
  update_company:       { label: 'Обновление компании',          icon: 'fa-solid fa-building text-primary-500',             target_type: 'company' },
  delete_company:       { label: 'Удаление компании',            icon: 'fa-solid fa-building text-error-500',               target_type: 'company' },
  create_user:          { label: 'Создание пользователя',        icon: 'fa-solid fa-user-plus text-primary-500',            target_type: 'user' },
  update_user:          { label: 'Обновление пользователя',      icon: 'fa-solid fa-user-pen text-primary-500',             target_type: 'user' },
  delete_user:          { label: 'Удаление пользователя',        icon: 'fa-solid fa-user-minus text-error-500',             target_type: 'user' },
  disable_user:         { label: 'Блокировка пользователя',      icon: 'fa-solid fa-user-lock text-warn-500',              target_type: 'user' },
  unlock_user:          { label: 'Разблокировка пользователя',   icon: 'fa-solid fa-user-check text-success-500',           target_type: 'user' },
  create_role:          { label: 'Создание роли',                icon: 'fa-solid fa-shield-halved text-primary-500',        target_type: 'role' },
  update_role:          { label: 'Обновление роли',              icon: 'fa-solid fa-shield-halved text-primary-500',        target_type: 'role' },
  delete_role:          { label: 'Удаление роли',                icon: 'fa-solid fa-shield-halved text-error-500',          target_type: 'role' },
  add_user_profile:     { label: 'Добавление рабочего профиля',  icon: 'fa-solid fa-id-badge text-primary-500',             target_type: 'user_profile' },
  update_user_profile:  { label: 'Обновление рабочего профиля',  icon: 'fa-solid fa-id-badge text-primary-500',             target_type: 'user_profile' },
  remove_user_profile:  { label: 'Удаление рабочего профиля',    icon: 'fa-solid fa-id-badge text-error-500',               target_type: 'user_profile' },
  create_contact:       { label: 'Создание контакта',            icon: 'fa-solid fa-address-card text-primary-500',         target_type: 'user_contact' },
  update_contact:       { label: 'Обновление контакта',          icon: 'fa-solid fa-address-card text-primary-500',         target_type: 'user_contact' },
  delete_contact:       { label: 'Удаление контакта',            icon: 'fa-solid fa-address-card text-error-500',           target_type: 'user_contact' },
  update_person:        { label: 'Обновление персоны',           icon: 'fa-solid fa-user-pen text-primary-500',             target_type: 'person' },
  deactivate_certificate: { label: 'Деактивация сертификата',    icon: 'fa-solid fa-certificate text-warn-500',             target_type: 'user_certificate' },
  save_settings:        { label: 'Сохранение настроек',          icon: 'fa-solid fa-gear text-primary-500',                 target_type: 'setting' },
  create_permission_policy: { label: 'Создание политики доступа', icon: 'fa-solid fa-key text-primary-500',                 target_type: 'permission_policy' },
  delete_permission_policy: { label: 'Удаление политики доступа', icon: 'fa-solid fa-key text-error-500',                   target_type: 'permission_policy' },
  create_document:      { label: 'Создание документа',           icon: 'fa-solid fa-file-circle-plus text-primary-500',     target_type: 'document' },
  update_document:      { label: 'Обновление документа',         icon: 'fa-solid fa-file-pen text-primary-500',             target_type: 'document' },
  delete_document:      { label: 'Удаление документа',           icon: 'fa-solid fa-file-circle-xmark text-error-500',      target_type: 'document' },
  post_document:        { label: 'Проведение документа',         icon: 'fa-solid fa-file-circle-check text-success-500',    target_type: 'document' },
  cancel_document:      { label: 'Отмена документа',             icon: 'fa-solid fa-file-circle-minus text-warn-500',       target_type: 'document' },
  archive_document:     { label: 'Архивация документа',          icon: 'fa-solid fa-box-archive text-surface-500',          target_type: 'document' },
  create_catalog_entry: { label: 'Создание записи справочника',  icon: 'fa-solid fa-book text-primary-500',                 target_type: 'catalog_entry' },
  update_catalog_entry: { label: 'Обновление записи справочника', icon: 'fa-solid fa-book text-primary-500',                target_type: 'catalog_entry' },
  delete_catalog_entry: { label: 'Удаление записи справочника',  icon: 'fa-solid fa-book text-error-500',                   target_type: 'catalog_entry' },
  emit_event:           { label: 'Эмиссия события',              icon: 'fa-solid fa-bolt text-warn-500',                    target_type: 'event' },
  replay_event:         { label: 'Повторная обработка события',  icon: 'fa-solid fa-rotate text-primary-500',               target_type: 'event' },
  execute_script:       { label: 'Выполнение скрипта',           icon: 'fa-solid fa-code text-primary-500',                 target_type: 'rhai_script' },
};

export interface AuditEntry {
  _id: string;
  user_id: string;
  user_login?: string;
  company_id: string;
  action: string;
  target_type: string;
  target_id: string | null;
  target_login?: string;
  entity_type: string | null;
  object_id: string | null;
  changes: AuditChanges | null;
  event_id: string | null;
  signature_ref: string | null;
  ip_address: string | null;
  user_agent: string | null;
  occurred_at: string;
}

export interface AuditPage {
  entries: AuditEntry[];
  total_count: number;
  has_more: boolean;
  next_cursor: string | null;
  prev_cursor: string | null;
}

export interface AuditLogFilters {
  actions?: string[];
  target_type?: string;
  user_id?: string;
  date_from?: string;
  date_to?: string;
  limit?: number;
  before?: string;
  after?: string;
}

// ── Event Store types ──────────────────────────────────────

export interface ActorSnapshot {
  user_id: string;
  login: string;
  full_name: string | null;
  position: string | null;
  company_id: string;
}

export interface Event {
  _id: string;
  stream_type: string;
  stream_id: string;
  event_type: string;
  version: number;
  payload: Record<string, unknown>;
  metadata: ActorSnapshot;
  company_id: string;
  correlation_id: string | null;
  causation_id: string | null;
  signature_ref: string | null;
  occurred_at: string;
}

export interface EventPage {
  events: Event[];
  total_count: number;
  has_more: boolean;
  next_cursor: string | null;
}

export interface EventFilters {
  stream_type?: string;
  stream_id?: string;
  event_type?: string;
  correlation_id?: string;
  date_from?: string;
  date_to?: string;
  limit?: number;
  after?: string;
}

export const EVENT_TYPE_META: Record<string, { label: string; icon: string }> = {
  'object.created':   { label: 'Создан объект',     icon: 'fa-solid fa-plus text-success-500' },
  'object.updated':   { label: 'Обновлён объект',   icon: 'fa-solid fa-pen text-primary-500' },
  'object.posted':    { label: 'Проведён объект',   icon: 'fa-solid fa-check-double text-success-500' },
  'object.cancelled': { label: 'Отменён объект',    icon: 'fa-solid fa-xmark text-error-500' },
  'object.restored':  { label: 'Восстановлена версия', icon: 'fa-solid fa-rotate-left text-warning-500' },
  'user.created':     { label: 'Создан пользователь', icon: 'fa-solid fa-user-plus text-success-500' },
  'user.updated':     { label: 'Обновлён пользователь', icon: 'fa-solid fa-user-pen text-primary-500' },
};

// ── Metadata types ─────────────────────────────────────────

export type EntityKind = 'document' | 'catalog' | 'register' | 'task' | 'contract' | 'project' | 'setting' | 'custom';
export type FieldKind = 'string' | 'text' | 'integer' | 'money' | 'date' | 'datetime' | 'boolean' | 'enum' | 'reference' | 'array' | 'table' | 'json' | 'file' | 'user' | 'company' | 'formula' | 'computed';

export interface EntityType {
  _id: string;
  company_id: string | null;
  code: string;
  name: string;
  kind: EntityKind;
  description: string | null;
  icon: string | null;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface EntityField {
  _id: string;
  entity_type_id: string;
  code: string;
  name: string;
  field_kind: FieldKind;
  is_required: boolean;
  is_readonly: boolean;
  default_value: unknown;
  enum_values: string[] | null;
  reference_entity: string | null;
  order: number;
  group_name: string | null;
}

export interface EntityState {
  _id: string;
  entity_type_id: string;
  code: string;
  name: string;
  is_initial: boolean;
  is_final: boolean;
  color: string | null;
  order: number;
}

export interface EntityTransition {
  _id: string;
  entity_type_id: string;
  code: string;
  name: string;
  from_state: string;
  to_state: string;
  required_policy: string | null;
  require_signature: boolean;
}

export interface EntityForm {
  _id: string;
  entity_type_id: string;
  code: string;
  name: string;
  layout: unknown;
}

export interface EntityAction {
  _id: string;
  entity_type_id: string;
  code: string;
  name: string;
  description: string | null;
  action_type: string | null;
  is_dangerous: boolean;
}

export interface NumberSequence {
  _id: string;
  company_id: string;
  entity_type_id: string;
  entity_type_name: string;
  prefix: string;
  padding: number;
  suffix: string;
  current_value: number;
  updated_at: string;
}

export const ENTITY_KIND_META: Record<EntityKind, { label: string; icon: string }> = {
  document: { label: 'Документ', icon: 'fa-solid fa-file-lines' },
  catalog:  { label: 'Справочник', icon: 'fa-solid fa-book' },
  register: { label: 'Реестр', icon: 'fa-solid fa-list' },
  task:     { label: 'Задача', icon: 'fa-solid fa-list-check' },
  contract: { label: 'Договор', icon: 'fa-solid fa-file-contract' },
  project:  { label: 'Проект', icon: 'fa-solid fa-diagram-project' },
  setting:  { label: 'Настройка', icon: 'fa-solid fa-gear' },
  custom:   { label: 'Произвольный', icon: 'fa-solid fa-cube' },
};

export const FIELD_KIND_META: Record<FieldKind, { label: string; icon: string }> = {
  string:   { label: 'Строка', icon: 'fa-solid fa-font' },
  text:     { label: 'Текст', icon: 'fa-solid fa-align-left' },
  integer:  { label: 'Целое', icon: 'fa-solid fa-hashtag' },
  money:    { label: 'Деньги', icon: 'fa-solid fa-ruble-sign' },
  date:     { label: 'Дата', icon: 'fa-solid fa-calendar' },
  datetime: { label: 'Дата/время', icon: 'fa-solid fa-clock' },
  boolean:  { label: 'Да/нет', icon: 'fa-solid fa-toggle-on' },
  enum:     { label: 'Перечисление', icon: 'fa-solid fa-list-ul' },
  reference:{ label: 'Ссылка', icon: 'fa-solid fa-arrow-right' },
  array:    { label: 'Массив', icon: 'fa-solid fa-layer-group' },
  table:    { label: 'Таблица', icon: 'fa-solid fa-table' },
  json:     { label: 'JSON', icon: 'fa-solid fa-code' },
  file:     { label: 'Файл', icon: 'fa-solid fa-paperclip' },
  user:     { label: 'Пользователь', icon: 'fa-solid fa-user' },
  company:  { label: 'Компания', icon: 'fa-solid fa-building' },
  formula:  { label: 'Формула', icon: 'fa-solid fa-square-root-variable' },
  computed: { label: 'Вычисляемое', icon: 'fa-solid fa-calculator' },
};

// ── Object types ───────────────────────────────────────────

export type ObjectStateTS = 'draft' | 'active' | 'posted' | 'cancelled' | 'archived' | 'deleted';

export interface ObjectEntity {
  _id: string;
  entity_type_id: string;
  kind: string;
  company_id: string;
  state: ObjectStateTS;
  data: Record<string, unknown>;
  computed: Record<string, unknown> | null;
  number: string | null;
  date: string | null;
  parent_id: string | null;
  version: number;
  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;
}

export interface ObjectSnapshot {
  _id: string;
  object_id: string;
  version: number;
  data: Record<string, unknown>;
  state: ObjectStateTS;
  created_by: string;
  created_at: string;
  reason: string | null;
}

export interface ObjectPage {
  objects: ObjectEntity[];
  total_count: number;
  has_more: boolean;
}

export interface ObjectFilters {
  entity_type_id?: string;
  state?: string;
  parent_id?: string;
  search?: string;
  limit?: number;
  offset?: number;
}

export interface PluginFunction {
  name: string;
  label: string;
  description: string;
  input_schema: Record<string, unknown>;
}

export interface WasmModuleInfo {
  id: string;
  name: string;
  version: string;
  source: string;
  functions: PluginFunction[];
}

export type ModuleStatus = 'installed' | 'enabled' | 'disabled';

export interface InstalledModule {
  _id: string;
  code: string;
  name: string;
  description: string;
  version: string;
  author: string;
  api_version: string;
  capabilities: string[];
  functions: PluginFunction[];
  status: ModuleStatus;
  manifest: Record<string, unknown>;
  installed_at: string;
  updated_at: string;
}

export const OBJECT_STATE_META: Record<ObjectStateTS, { label: string; icon: string; color: string }> = {
  draft:     { label: 'Черновик', icon: 'fa-solid fa-pencil', color: 'bg-surface-400' },
  active:    { label: 'Активный', icon: 'fa-solid fa-check', color: 'bg-primary-500' },
  posted:    { label: 'Проведён', icon: 'fa-solid fa-check-double', color: 'bg-success-500' },
  cancelled: { label: 'Отменён', icon: 'fa-solid fa-xmark', color: 'bg-error-500' },
  archived:  { label: 'Архив', icon: 'fa-solid fa-box-archive', color: 'bg-warning-500' },
  deleted:   { label: 'Удалён', icon: 'fa-solid fa-trash', color: 'bg-error-700' },
};

// ── Print Forms types ─────────────────────────────────────

export type PaperFormat = 'a4' | 'a5' | 'letter';
export type Orientation = 'portrait' | 'landscape';

export interface PrintMargins {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface PrintTemplate {
  _id: string;
  code: string;
  name: string;
  entity_type: string;
  form_code: string;
  template_body: string;
  css_styles: string;
  paper_format: PaperFormat;
  orientation: Orientation;
  margins: PrintMargins;
  is_default: boolean;
  is_active: boolean;
  version: number;
  valid_from: string | null;
  valid_to: string | null;
  company_id: string | null;
  before_print_script: string | null;
  created_by: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreatePrintTemplateInput {
  code: string;
  name: string;
  entity_type: string;
  form_code: string;
  template_body: string;
  css_styles?: string;
  paper_format?: PaperFormat;
  orientation?: Orientation;
  margins?: PrintMargins;
  is_default?: boolean;
  before_print_script?: string;
}

export interface UpdatePrintTemplateInput {
  name?: string;
  template_body?: string;
  css_styles?: string;
  paper_format?: PaperFormat;
  orientation?: Orientation;
  margins?: PrintMargins;
  is_default?: boolean;
  is_active?: boolean;
  before_print_script?: string;
}

export const api = {
  async getDiagnostics(): Promise<DiagnosticsReport> {
    return getAdapter().invoke<DiagnosticsReport>('get_diagnostics');
  },

  async connectDb(uri: string, dbName: string): Promise<DiagnosticsReport['mongodb']> {
    return getAdapter().invoke('connect_db', { input: { uri, db_name: dbName } });
  },

  async authenticate(login: string, password: string): Promise<AuthResult> {
    return getAdapter().invoke<AuthResult>('authenticate', { login, password });
  },

  async getMe(): Promise<User | null> {
    return getAdapter().invoke<User | null>('get_me');
  },

  // Companies
  async listCompanies(): Promise<Company[]> {
    return getAdapter().invoke<Company[]>('list_companies');
  },
  async getCompany(id: string): Promise<Company> {
    return getAdapter().invoke<Company>('get_company', { id });
  },
  async createCompany(input: { code: string; name: string; inn?: string }): Promise<Company> {
    return getAdapter().invoke<Company>('create_company', { input });
  },
  async updateCompany(id: string, input: { name?: string; inn?: string; active?: boolean }): Promise<Company> {
    return getAdapter().invoke<Company>('update_company', { id, input });
  },
  async deleteCompany(id: string): Promise<void> {
    return getAdapter().invoke<void>('delete_company', { id });
  },

  // Users
  async listUsers(): Promise<User[]> {
    return getAdapter().invoke<User[]>('list_users');
  },
  async getUser(id: string): Promise<User> {
    return getAdapter().invoke<User>('get_user', { id });
  },
  async createUser(input: {
    login: string;
    password: string;
    display_name?: string;
    last_name?: string;
    first_name?: string;
    middle_name?: string;
    email?: string;
    company_id?: string;
    role_id?: string;
    position?: string;
    department?: string;
  }): Promise<User> {
    return getAdapter().invoke<User>('create_user', { input });
  },
  async updateUser(id: string, input: { status?: string; locale?: string; timezone?: string; new_password?: string; must_change_password?: boolean }): Promise<void> {
    return getAdapter().invoke<void>('update_user', { id, input });
  },
  async deleteUser(id: string): Promise<void> {
    return getAdapter().invoke<void>('delete_user', { id });
  },

  // Person
  async getPerson(id: string): Promise<Person> {
    return getAdapter().invoke<Person>('get_person', { id });
  },
  async updatePerson(id: string, input: { last_name?: string; first_name?: string; middle_name?: string; display_name?: string }): Promise<Person> {
    return getAdapter().invoke<Person>('update_person', { id, input });
  },

  // Contacts
  async listUserContacts(userId: string): Promise<UserContact[]> {
    return getAdapter().invoke<UserContact[]>('list_user_contacts', { userId });
  },
  async createContact(input: { user_id: string; channel_type: string; value: string; is_primary?: boolean; purposes?: string[] }): Promise<UserContact> {
    return getAdapter().invoke<UserContact>('create_contact', { input });
  },
  async updateContact(id: string, input: { value?: string; is_primary?: boolean; is_verified?: boolean; purposes?: string[] }): Promise<UserContact> {
    return getAdapter().invoke<UserContact>('update_contact', { id, input });
  },
  async deleteContact(id: string): Promise<void> {
    return getAdapter().invoke<void>('delete_contact', { id });
  },

  // Profiles
  async listUserProfiles(userId: string): Promise<UserProfile[]> {
    return getAdapter().invoke<UserProfile[]>('list_user_profiles', { userId });
  },
  async addUserProfile(input: { user_id: string; company_id: string; role_id: string; position?: string; department?: string }): Promise<UserProfile> {
    return getAdapter().invoke<UserProfile>('add_user_profile', { input });
  },
  async updateUserProfile(id: string, input: { role_id?: string; position?: string; department?: string; is_primary?: boolean; is_active?: boolean }): Promise<void> {
    return getAdapter().invoke<void>('update_user_profile', { id, input });
  },
  async removeUserProfile(id: string): Promise<void> {
    return getAdapter().invoke<void>('remove_user_profile', { id });
  },

  // Certificates
  async listUserCertificates(userId: string): Promise<UserCertificate[]> {
    return getAdapter().invoke<UserCertificate[]>('list_user_certificates', { userId });
  },
  async deactivateCertificate(id: string): Promise<void> {
    return getAdapter().invoke<void>('deactivate_certificate', { id });
  },

  // Roles
  async listRoles(companyId: string): Promise<Role[]> {
    return getAdapter().invoke<Role[]>('list_roles', { companyId });
  },
  async createRole(input: { company_id: string; code: string; name: string; description?: string; permission_policy_ids?: string[] }): Promise<Role> {
    return getAdapter().invoke<Role>('create_role', { input });
  },
  async updateRole(id: string, input: { name?: string; description?: string; permission_policy_ids?: string[] }): Promise<Role> {
    return getAdapter().invoke<Role>('update_role', { id, input });
  },
  async deleteRole(id: string): Promise<void> {
    return getAdapter().invoke<void>('delete_role', { id });
  },

  // Permission Policies
  async listPermissionPolicies(): Promise<PermissionPolicy[]> {
    return getAdapter().invoke<PermissionPolicy[]>('list_permission_policies');
  },
  async createPermissionPolicy(input: { code: string; name: string; scope_type: string; subsystem_code: string; entity_type?: string; actions: string[]; record_scope: string }): Promise<PermissionPolicy> {
    return getAdapter().invoke<PermissionPolicy>('create_permission_policy', { input });
  },
  async deletePermissionPolicy(id: string): Promise<void> {
    return getAdapter().invoke<void>('delete_permission_policy', { id });
  },

  // My permissions
  async getMyPermissions(): Promise<MyPermissions> {
    return getAdapter().invoke<MyPermissions>('get_my_permissions');
  },

  // Multi-company
  async switchCompany(companyId: string): Promise<AuthResult> {
    return getAdapter().invoke<AuthResult>('switch_company', { input: { company_id: companyId } });
  },

  // Settings
  async getContactTypes(): Promise<Array<{ code: string; name: string }>> {
    return getAdapter().invoke<Array<{ code: string; name: string }>>('get_contact_types');
  },
  async saveContactTypes(types: Array<{ code: string; name: string }>): Promise<void> {
    return getAdapter().invoke<void>('save_contact_types', { types });
  },

  // Audit
  async listAuditLogs(filters?: AuditLogFilters): Promise<AuditPage> {
    return getAdapter().invoke<AuditPage>('list_audit_logs', { filters: filters ?? null });
  },
  async getAuditEntry(id: string): Promise<AuditEntry | null> {
    return getAdapter().invoke<AuditEntry | null>('get_audit_entry', { id });
  },

  // Rhai
  async validateRhaiScript(source: string): Promise<void> {
    return getAdapter().invoke<void>('validate_rhai_script', { source });
  },
  async executeRhaiScript(source: string, context: string): Promise<unknown> {
    return getAdapter().invoke<unknown>('execute_rhai_script', { source, context });
  },

  // Event Store
  async listEvents(filters?: EventFilters): Promise<EventPage> {
    return getAdapter().invoke<EventPage>('list_events', { filters: filters ?? {} });
  },
  async getEvent(id: string): Promise<Event> {
    return getAdapter().invoke<Event>('get_event', { id });
  },
  async listStreamEvents(streamType: string, streamId: string): Promise<Event[]> {
    return getAdapter().invoke<Event[]>('list_stream_events', { stream_type: streamType, stream_id: streamId });
  },

  // Metadata
  async listEntityTypes(): Promise<EntityType[]> {
    return getAdapter().invoke<EntityType[]>('list_entity_types');
  },
  async getEntityType(id: string): Promise<EntityType> {
    return getAdapter().invoke<EntityType>('get_entity_type', { id });
  },
  async createEntityType(input: { code: string; name: string; kind: EntityKind; description?: string; icon?: string }): Promise<EntityType> {
    return getAdapter().invoke<EntityType>('create_entity_type', { input });
  },
  async updateEntityType(id: string, input: { name?: string; description?: string; icon?: string; is_active?: boolean }): Promise<EntityType> {
    return getAdapter().invoke<EntityType>('update_entity_type', { id, input });
  },
  async deleteEntityType(id: string): Promise<void> {
    return getAdapter().invoke<void>('delete_entity_type', { id });
  },

  async listEntityFields(entityTypeId: string): Promise<EntityField[]> {
    return getAdapter().invoke<EntityField[]>('list_entity_fields', { entity_type_id: entityTypeId });
  },
  async createEntityField(input: { entity_type_id: string; code: string; name: string; field_kind: FieldKind; is_required?: boolean; group_name?: string }): Promise<EntityField> {
    return getAdapter().invoke<EntityField>('create_entity_field', { input });
  },
  async updateEntityField(id: string, input: { name?: string; is_required?: boolean; order?: number }): Promise<EntityField> {
    return getAdapter().invoke<EntityField>('update_entity_field', { id, input });
  },
  async deleteEntityField(id: string): Promise<void> {
    return getAdapter().invoke<void>('delete_entity_field', { id });
  },

  async listEntityStates(entityTypeId: string): Promise<EntityState[]> {
    return getAdapter().invoke<EntityState[]>('list_entity_states', { entity_type_id: entityTypeId });
  },
  async createEntityState(input: { entity_type_id: string; code: string; name: string; is_initial?: boolean; is_final?: boolean; color?: string }): Promise<EntityState> {
    return getAdapter().invoke<EntityState>('create_entity_state', { input });
  },
  async updateEntityState(id: string, input: { name?: string; color?: string; is_final?: boolean }): Promise<void> {
    return getAdapter().invoke<void>('update_entity_state', { id, input });
  },
  async deleteEntityState(id: string): Promise<void> {
    return getAdapter().invoke<void>('delete_entity_state', { id });
  },

  async listEntityTransitions(entityTypeId: string): Promise<EntityTransition[]> {
    return getAdapter().invoke<EntityTransition[]>('list_entity_transitions', { entity_type_id: entityTypeId });
  },
  async createEntityTransition(input: { entity_type_id: string; code: string; name: string; from_state: string; to_state: string }): Promise<EntityTransition> {
    return getAdapter().invoke<EntityTransition>('create_entity_transition', { input });
  },
  async deleteEntityTransition(id: string): Promise<void> {
    return getAdapter().invoke<void>('delete_entity_transition', { id });
  },

  async listEntityForms(entityTypeId: string): Promise<EntityForm[]> {
    return getAdapter().invoke<EntityForm[]>('list_entity_forms', { entity_type_id: entityTypeId });
  },
  async listEntityActions(entityTypeId: string): Promise<EntityAction[]> {
    return getAdapter().invoke<EntityAction[]>('list_entity_actions', { entity_type_id: entityTypeId });
  },

  // Objects
  async listObjects(filters?: ObjectFilters): Promise<ObjectPage> {
    return getAdapter().invoke<ObjectPage>('list_objects', { filters: filters ?? {} });
  },
  async getObject(id: string): Promise<ObjectEntity> {
    return getAdapter().invoke<ObjectEntity>('get_object', { id });
  },
  async createObject(input: { entity_type_id: string; data: Record<string, unknown>; parent_id?: string; date?: string }): Promise<ObjectEntity> {
    return getAdapter().invoke<ObjectEntity>('create_object', { input });
  },
  async updateObject(id: string, input: { data: Record<string, unknown>; version: number; reason?: string }): Promise<ObjectEntity> {
    return getAdapter().invoke<ObjectEntity>('update_object', { id, input });
  },
  async postObject(id: string, version: number): Promise<ObjectEntity> {
    return getAdapter().invoke<ObjectEntity>('post_object', { id, version });
  },
  async cancelObject(id: string, version: number): Promise<ObjectEntity> {
    return getAdapter().invoke<ObjectEntity>('cancel_object', { id, version });
  },
  async restoreObjectVersion(id: string, targetVersion: number): Promise<ObjectEntity> {
    return getAdapter().invoke<ObjectEntity>('restore_object_version', { id, target_version: targetVersion });
  },
  async listObjectVersions(id: string): Promise<ObjectSnapshot[]> {
    return getAdapter().invoke<ObjectSnapshot[]>('list_object_versions', { id });
  },

  // ── WASM Runtime (плагины) ──
  async loadWasmModule(wasmBytes: number[], name: string): Promise<WasmModuleInfo> {
    return getAdapter().invoke<WasmModuleInfo>('wasm_load', { wasmBytes, name });
  },
  async unloadWasmModule(moduleId: string): Promise<void> {
    return getAdapter().invoke<void>('wasm_unload', { moduleId });
  },
  async listWasmModules(): Promise<WasmModuleInfo[]> {
    return getAdapter().invoke<WasmModuleInfo[]>('wasm_list');
  },
  async pluginCall(moduleId: string, function: string, argsJson: string): Promise<string> {
    return getAdapter().invoke<string>('plugin_call', { moduleId, function, argsJson });
  },

  // ── Печатные формы ──
  async printListTemplates(entityType: string, formCode?: string): Promise<PrintTemplate[]> {
    return getAdapter().invoke<PrintTemplate[]>('print_list_templates', { entityType, formCode: formCode ?? null });
  },
  async printGetTemplate(id: string): Promise<PrintTemplate> {
    return getAdapter().invoke<PrintTemplate>('print_get_template', { id });
  },
  async printCreateTemplate(input: CreatePrintTemplateInput): Promise<PrintTemplate> {
    return getAdapter().invoke<PrintTemplate>('print_create_template', { input });
  },
  async printUpdateTemplate(id: string, input: UpdatePrintTemplateInput): Promise<PrintTemplate> {
    return getAdapter().invoke<PrintTemplate>('print_update_template', { id, input });
  },
  async printDeleteTemplate(id: string): Promise<void> {
    return getAdapter().invoke<void>('print_delete_template', { id });
  },
  async printRender(templateId: string, objectId: string): Promise<string> {
    return getAdapter().invoke<string>('print_render', { templateId, objectId });
  },

  // ── Нумерация ──
  async numberingList(): Promise<NumberSequence[]> {
    return getAdapter().invoke<NumberSequence[]>('numbering_list');
  },
  async numberingGet(entityTypeId: string): Promise<NumberSequence | null> {
    return getAdapter().invoke<NumberSequence | null>('numbering_get', { entityTypeId });
  },
  async numberingUpdateFormat(entityTypeId: string, entityTypeName: string, input: { prefix?: string; padding?: number; suffix?: string }): Promise<NumberSequence> {
    return getAdapter().invoke<NumberSequence>('numbering_update_format', { entityTypeId, entityTypeName, input });
  },
  async numberingReset(entityTypeId: string, newValue?: number): Promise<void> {
    return getAdapter().invoke<void>('numbering_reset', { entityTypeId, new_value: newValue ?? null });
  },

  // ── Прикладные модули (WASM) ──
  async modulesList(): Promise<InstalledModule[]> {
    return getAdapter().invoke<InstalledModule[]>('modules_list');
  },
  async modulesGet(moduleId: string): Promise<InstalledModule> {
    return getAdapter().invoke<InstalledModule>('modules_get', { moduleId });
  },
  async modulesInstall(wasmBytes: number[]): Promise<InstalledModule> {
    return getAdapter().invoke<InstalledModule>('modules_install', { input: { wasm_bytes: wasmBytes } });
  },
  async modulesUninstall(moduleId: string): Promise<void> {
    return getAdapter().invoke<void>('modules_uninstall', { moduleId });
  },
  async modulesEnable(moduleId: string): Promise<void> {
    return getAdapter().invoke<void>('modules_enable', { moduleId });
  },
  async modulesDisable(moduleId: string): Promise<void> {
    return getAdapter().invoke<void>('modules_disable', { moduleId });
  },
  async modulesUpdateSettings(moduleId: string, settings: Record<string, unknown>): Promise<void> {
    return getAdapter().invoke<void>('modules_update_settings', { moduleId, settings });
  },
};
