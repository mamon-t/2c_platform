// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

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
  async createContact(input: { user_id: string; channel_type: string; value: string; is_primary?: boolean; purposes?: string[]; note?: string }): Promise<UserContact> {
    return getAdapter().invoke<UserContact>('create_contact', { input });
  },
  async updateContact(id: string, input: { value?: string; is_primary?: boolean; is_verified?: boolean; purposes?: string[]; note?: string }): Promise<UserContact> {
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

  // ── Универсальный мост к WASM-модулям ──
  /**
   * Вызвать функцию WASM-модуля. Возвращает РАЗОБРАННЫЙ вывод гостя.
   * Если гость вернул конверт {ok, data|error} (новый контракт SDK) —
   * он разворачивается; ошибка конверта бросается как исключение.
   * Иначе возвращается сырой JSON-вывод (совместимость с convert).
   */
  async pluginCall<T = unknown>(moduleId: string, fnName: string, args: Record<string, unknown> = {}): Promise<T> {
    const out = await getAdapter().invoke<string>('plugin_call', {
      moduleId,
      function: fnName,
      argsJson: JSON.stringify(args),
    });
    let v: unknown;
    try { v = JSON.parse(out); } catch { return out as T; }
    if (
      v && typeof v === 'object' && !Array.isArray(v) &&
      'ok' in v && typeof (v as PluginEnvelope<unknown>).ok === 'boolean'
    ) {
      const env = v as PluginEnvelope<T>;
      if (!env.ok) {
        const err = env.error;
        throw new Error(err ? `${err.code}: ${err.message}` : 'Ошибка модуля');
      }
      return env.data as T;
    }
    return v as T;
  },

  // ── Уведомления (in-app outbox) ──
  async notificationsList(limit?: number): Promise<NotificationItemTS[]> {
    return getAdapter().invoke<NotificationItemTS[]>('notifications_list', { limit: limit ?? null });
  },
  async notificationsCountUnread(): Promise<number> {
    return getAdapter().invoke<number>('notifications_count_unread');
  },
  async notificationSubscriptionsList(): Promise<unknown[]> {
    return getAdapter().invoke<unknown[]>('notification_subscriptions_list');
  },
  async notificationSubscriptionsUpsert(eventType: string, channels: string[], isMuted: boolean): Promise<void> {
    return getAdapter().invoke<void>('notification_subscriptions_upsert', { eventType, channels, isMuted });
  },
  async notificationsMarkRead(notificationId?: string): Promise<number> {
    return getAdapter().invoke<number>('notifications_mark_read', { notificationId: notificationId ?? null });
  },

  // ── Криптоподпись (host-side CryptoPro) ──
  async listCryptoCertificates(): Promise<CertificateInfo[]> {
    return getAdapter().invoke<CertificateInfo[]>('list_crypto_certificates');
  },
  async signDocument(dataBase64: string, certSha1: string, detached = true): Promise<SignatureResult> {
    return getAdapter().invoke<SignatureResult>('sign_document', {
      input: { data_base64: dataBase64, cert_sha1: certSha1, detached },
    });
  },
  /** Тестовый самоподписанный сертификат (settings.manage). Возвращает "контейнер|subject|sha1". */
  async createTestCertificate(name: string): Promise<string> {
    return getAdapter().invoke<string>('create_test_certificate', { name });
  },

  // ── Склад (stock) ──
  async stockBalances(locationId?: string, nomenclatureId?: string): Promise<StockBalancesTS> {
    return getAdapter().invoke<StockBalancesTS>('stock_balances', {
      locationId: locationId ?? null,
      nomenclatureId: nomenclatureId ?? null,
    });
  },
  async stockReportHandover(): Promise<StockHandoverReportTS> {
    return getAdapter().invoke<StockHandoverReportTS>('stock_report_handover');
  },
  async stockReportOverdue(): Promise<StockHandoverReportTS> {
    return getAdapter().invoke<StockHandoverReportTS>('stock_report_overdue');
  },
  async stockSeedMetadata(): Promise<string> {
    return getAdapter().invoke<string>('stock_seed_metadata');
  },

  // ── Учёт (ledger) ──
  async ledgerOsv(periodFrom?: string, periodTo?: string): Promise<LedgerOsvTS> {
    return getAdapter().invoke<LedgerOsvTS>('ledger_osv', {
      periodFrom: periodFrom ?? null,
      periodTo: periodTo ?? null,
    });
  },
  async ledgerJournal(opts: { dateFrom?: string; dateTo?: string; accountCode?: string; docId?: string; limit?: number } = {}): Promise<LedgerJournalEntryTS[]> {
    return getAdapter().invoke<LedgerJournalEntryTS[]>('ledger_journal', {
      dateFrom: opts.dateFrom ?? null,
      dateTo: opts.dateTo ?? null,
      accountCode: opts.accountCode ?? null,
      docId: opts.docId ?? null,
      limit: opts.limit ?? null,
    });
  },
  async ledgerCard(accountCode: string, dateFrom?: string, dateTo?: string): Promise<LedgerCardTS> {
    return getAdapter().invoke<LedgerCardTS>('ledger_card', {
      accountCode,
      dateFrom: dateFrom ?? null,
      dateTo: dateTo ?? null,
    });
  },
  async ledgerPeriodsList(): Promise<unknown[]> {
    return getAdapter().invoke<unknown[]>('ledger_periods_list');
  },
  async ledgerPeriodSetState(year: number, month: number, opened: boolean, closed: boolean, reopen?: boolean): Promise<void> {
    return getAdapter().invoke<void>('ledger_period_set_state', { year, month, opened, closed, reopen: reopen ?? false });
  },
  async ledgerAccountsList(): Promise<unknown[]> {
    return getAdapter().invoke<unknown[]>('ledger_accounts_list');
  },

  // ── Торговля (trade) ──
  async tradeSeedMetadata(): Promise<string> {
    return getAdapter().invoke<string>('trade_seed_metadata');
  },
  async tradeGetPrice(nomenclatureId: string, priceTypeId: string, onDate?: string): Promise<PriceOnDateTS | null> {
    return getAdapter().invoke<PriceOnDateTS | null>('trade_get_price', {
      nomenclatureId, priceTypeId, onDate: onDate ?? null,
    });
  },

  // ── Сообщения (messaging) ──
  async messagingRoomsList(roomType?: string): Promise<MessagingRoomPreviewTS[]> {
    return getAdapter().invoke<MessagingRoomPreviewTS[]>('messaging_rooms_list', { roomType: roomType ?? null });
  },
  async messagingRoomsCreate(title: string, memberIds: string[], entityRef?: Record<string, unknown>): Promise<Record<string, unknown>> {
    return getAdapter().invoke<Record<string, unknown>>('messaging_rooms_create', {
      title, memberIds, entityRef: entityRef ?? null,
    });
  },
  async messagingRoomsArchive(roomId: string): Promise<void> {
    return getAdapter().invoke<void>('messaging_rooms_archive', { roomId });
  },
  async messagingMessagesSend(roomId: string, content: string, replyTo?: string): Promise<MessagingMessageTS> {
    return getAdapter().invoke<MessagingMessageTS>('messaging_messages_send', {
      roomId, content, replyTo: replyTo ?? null,
    });
  },
  async messagingMessagesList(roomId: string, limit?: number): Promise<MessagingMessageTS[]> {
    return getAdapter().invoke<MessagingMessageTS[]>('messaging_messages_list', {
      roomId, limit: limit ?? null,
    });
  },
  async messagingMessagesEdit(messageId: string, content: string): Promise<void> {
    return getAdapter().invoke<void>('messaging_messages_edit', { messageId, content });
  },
  async messagingMessagesDelete(messageId: string): Promise<void> {
    return getAdapter().invoke<void>('messaging_messages_delete', { messageId });
  },
  async messagingReadsUpdate(roomId: string, lastMessageId: string): Promise<void> {
    return getAdapter().invoke<void>('messaging_reads_update', { roomId, lastMessageId });
  },

  // ── Оборудование (devices) ──
  async devicesList(): Promise<DeviceListItemTS[]> {
    return getAdapter().invoke<DeviceListItemTS[]>('devices_list');
  },
  async devicesGet(id: string): Promise<DeviceConfigTS> {
    return getAdapter().invoke<DeviceConfigTS>('devices_get', { id });
  },
  async devicesSave(id: string | null, input: DeviceConfigInputTS): Promise<DeviceConfigTS> {
    return getAdapter().invoke<DeviceConfigTS>('devices_save', { id, input });
  },
  async devicesDelete(id: string): Promise<void> {
    return getAdapter().invoke<void>('devices_delete', { id });
  },
  async devicesConnect(id: string): Promise<void> {
    return getAdapter().invoke<void>('devices_connect', { id });
  },
  async devicesDisconnect(id: string): Promise<void> {
    return getAdapter().invoke<void>('devices_disconnect', { id });
  },
  async devicesTest(id: string): Promise<string> {
    return getAdapter().invoke<string>('devices_test', { id });
  },
  async devicesListPorts(): Promise<PortDtoTS[]> {
    return getAdapter().invoke<PortDtoTS[]>('devices_list_ports');
  },
  async devicesWedgeScan(code: string): Promise<void> {
    return getAdapter().invoke<void>('devices_wedge_scan', { code });
  },
};

// ── Plugin SDK: универсальный конверт host/гостевых вызовов ──

export interface PluginError {
  code: string;
  message: string;
}

export interface PluginEnvelope<T> {
  ok: boolean;
  data?: T;
  error?: PluginError;
}

/** Разворачивает конверт плагина; при ошибке бросает исключение с кодом. */
export function unwrapPlugin<T>(envelope: PluginEnvelope<T>): T {
  if (envelope.ok) return envelope.data as T;
  const err = envelope.error;
  throw new Error(err ? `${err.code}: ${err.message}` : 'Неизвестная ошибка модуля');
}

// ── Модуль «Заявки» (WASM requests-plugin) ──

export type ApproverTypeTS = 'user' | 'role';

export interface RouteStepTS {
  step_order: number;
  approver_type: ApproverTypeTS;
  approver_id: string;
  approver_name?: string | null;
  timeout_hours: number;
  is_required: boolean;
}

export interface RequestRouteTS {
  code: string;
  name: string;
  description?: string | null;
  steps: RouteStepTS[];
  /** Требовать ЭЦП на submit/approve/reject (задаёт маршрут). */
  requires_signature: boolean;
  is_active: boolean;
}

export type StepStatusTS = 'pending' | 'approved' | 'rejected' | 'skipped';
export type ApprovalStatusTS = 'in_progress' | 'approved' | 'rejected' | 'cancelled';

export interface StepStateTS {
  step_order: number;
  approver_type: ApproverTypeTS;
  approver_id: string;
  approver_name?: string | null;
  status: StepStatusTS;
  decided_at?: number | null;
  comment?: string | null;
  signature_der?: string | null;
}

export interface RequestApprovalTS {
  request_id: string;
  route_code: string;
  route_name: string;
  status: ApprovalStatusTS;
  current_step: number;
  steps: StepStateTS[];
  initiator_id: string;
  initiator_login: string;
  initiator_name?: string | null;
  submit_signature_der?: string | null;
  /** Снимок requires_signature маршрута на момент отправки. */
  requires_signature: boolean;
  submitted_at: number;
  completed_at?: number | null;
  last_comment?: string | null;
}

// ── Криптоподпись ──

export interface CertificateInfo {
  subject_name: string;
  issuer_name: string;
  sha1_hash: string;
  has_private_key: boolean;
  is_valid: boolean;
}

export interface SignatureResult {
  signature_der: number[];
  signer_subject: string;
  signer_issuer: string;
  signer_sha1: string;
  is_detached: boolean;
}

// ── Уведомления ──

export interface NotificationOutboxTS {
  _id: string;
  company_id: string;
  template_code: string;
  channel: 'in_app' | 'email';
  recipient_user_id: string;
  subject?: string | null;
  body: string;
  status: 'pending' | 'sent' | 'failed' | 'read';
  attempts: number;
  created_at: string;
}

// ── Оборудование (модуль devices) ──

export type DeviceKindTS = 'barcode_scanner' | 'scale' | 'fiscal_printer' | 'label_printer';

export type ConnectionKindTS =
  | { kind: 'keyboard_wedge' }
  | { kind: 'serial'; port: string; baud: number }
  | { kind: 'tcp'; host: string; port: number };

export interface DeviceConfigTS {
  id: string;
  company_id: string;
  kind: DeviceKindTS;
  name: string;
  connection: ConnectionKindTS;
  settings: Record<string, unknown>;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export type DeviceConfigInputTS = Omit<DeviceConfigTS, 'id' | 'company_id' | 'created_at' | 'updated_at'>;

export interface DeviceListItemTS extends DeviceConfigTS {
  connected: boolean;
}

export interface PortDtoTS {
  path: string;
  description: string;
}

/** Событие устройства (tauri event 'device-event', поле event). */
export interface DeviceEventTS {
  type: 'scanned' | 'weighed' | 'connected' | 'disconnected' | 'error';
  device_id?: string;
  code?: string;
  grams?: number;
  stable?: boolean;
  message?: string;
}


// ── Склад ──

export interface StockBalanceTS {
  location_id: string;
  nomenclature_id: string;
  quantity: number;
}

export interface StockBalancesTS {
  balances: StockBalanceTS[];
}

export interface StockHandoverItemTS {
  location_id: string;
  custodian_name?: string;
  responsible_user_id?: string;
  expected_return_date?: string;
  nomenclature_id: string;
  qty_on_hand: number;
  issued_at_ms?: number;
}

export interface StockHandoverReportTS {
  items: StockHandoverItemTS[];
}


// ── Учёт (ledger) ──

export interface LedgerOsvRowTS {
  code: string;
  name: string;
  type: string;
  debit_turnover: number;
  credit_turnover: number;
  balance: number;
}

export interface LedgerOsvTS {
  rows: LedgerOsvRowTS[];
}

export interface LedgerJournalEntryTS {
  id: string;
  date: string;
  posting_id: string;
  doc_kind?: string | null;
  doc_id?: string | null;
  debit_code: string;
  credit_code: string;
  amount: number;
  nomenclature_id?: string | null;
  description?: string | null;
  is_reversal: boolean;
}

export interface LedgerCardTS {
  account_code: string;
  sign: number;
  entries: Array<{
    date: string;
    doc_id?: string | null;
    description?: string | null;
    debit_code: string;
    credit_code: string;
    amount: number;
    is_debit: boolean;
    running_balance: number;
  }>;
  final_balance: number;
}

// ── Торговля ──

export interface PriceOnDateTS {
  object_id: string;
  nomenclature_id: string;
  price_type_id: string;
  value: number;
  valid_from: string;
}


// ── Уведомления (новый формат) ──

export interface NotificationItemTS {
  _id: string;
  user_id: string;
  company_id: string;
  notification_type: string;
  severity: 'info' | 'warning' | 'critical';
  title: string;
  body: string;
  entity_ref?: { entity_type: string; entity_id: string } | null;
  status: string;
  read_at?: string | null;
  created_at: string;
}


// ── Сообщения (messaging) ──

export interface MessagingRoomTS {
  _id: string;
  company_id: string;
  room_type: 'direct' | 'group' | 'document';
  title?: string | null;
  members: string[];
  entity_ref?: Record<string, unknown> | null;
  created_by: string;
  created_at: string;
  last_message_at?: string | null;
  is_archived: boolean;
}

export interface MessagingRoomPreviewTS {
  room: MessagingRoomTS;
  last_message?: { content: string; author_id: string; created_at: string } | null;
  unread_count: number;
}

export interface MessagingMessageTS {
  _id: string;
  company_id: string;
  room_id: string;
  author_id: string;
  content: string;
  reply_to?: string | null;
  is_deleted: boolean;
  edited_at?: string | null;
  created_at: string;
}
