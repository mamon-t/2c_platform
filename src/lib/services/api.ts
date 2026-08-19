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
  company_id: string;
  username: string;
  display_name: string;
  email: string | null;
  role_id: string;
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface Role {
  _id: string;
  company_id: string;
  code: string;
  name: string;
  description: string | null;
  created_at: string;
  updated_at: string;
}

export interface AuthResult {
  token: string;
  user: User;
}

export const api = {
  async getDiagnostics(): Promise<DiagnosticsReport> {
    return getAdapter().invoke<DiagnosticsReport>('get_diagnostics');
  },

  async connectDb(uri: string, dbName: string): Promise<DiagnosticsReport['mongodb']> {
    return getAdapter().invoke('connect_db', { input: { uri, db_name: dbName } });
  },

  async authenticate(username: string, password: string): Promise<AuthResult> {
    return getAdapter().invoke<AuthResult>('authenticate', { username, password });
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
  async listUsers(companyId: string): Promise<User[]> {
    return getAdapter().invoke<User[]>('list_users', { companyId });
  },
  async getUser(id: string): Promise<User> {
    return getAdapter().invoke<User>('get_user', { id });
  },
  async createUser(input: {
    company_id: string;
    username: string;
    display_name: string;
    email?: string;
    password: string;
    role_id: string;
  }): Promise<User> {
    return getAdapter().invoke<User>('create_user', { input });
  },
  async updateUser(id: string, input: { display_name?: string; email?: string; active?: boolean; role_id?: string }): Promise<User> {
    return getAdapter().invoke<User>('update_user', { id, input });
  },
  async deleteUser(id: string): Promise<void> {
    return getAdapter().invoke<void>('delete_user', { id });
  },

  // Roles
  async listRoles(companyId: string): Promise<Role[]> {
    return getAdapter().invoke<Role[]>('list_roles', { companyId });
  },
  async createRole(input: { company_id: string; code: string; name: string; description?: string }): Promise<Role> {
    return getAdapter().invoke<Role>('create_role', { input });
  },
  async deleteRole(id: string): Promise<void> {
    return getAdapter().invoke<void>('delete_role', { id });
  },

  async validateRhaiScript(source: string): Promise<void> {
    return getAdapter().invoke<void>('validate_rhai_script', { source });
  },

  async executeRhaiScript(source: string, context: string): Promise<unknown> {
    return getAdapter().invoke<unknown>('execute_rhai_script', { source, context });
  },
};
