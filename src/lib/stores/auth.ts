import { writable, derived } from 'svelte/store';

export interface UserCompany {
  companyId: string;
  companyName: string;
  companyCode: string;
  roleId: string;
  roleName: string;
}

export interface AuthUser {
  userId: string;
  companyId: string;
  roleId: string;
  roleCode: string;
  roleName: string;
  login: string;
  displayName: string;
  companies: UserCompany[];
  permissions: Array<{ subsystemCode: string; actions: string[]; recordScope: string; deny: boolean }>;
}

function createAuthStore() {
  const { subscribe, set, update } = writable<AuthUser | null>(null);

  return {
    subscribe,
    login(user: AuthUser) {
      set(user);
      localStorage.setItem('2c-user', JSON.stringify(user));
      if (user.companies.length > 0) {
        localStorage.setItem('2c-company', user.companyId);
      }
    },
    logout() {
      set(null);
      localStorage.removeItem('2c-user');
      localStorage.removeItem('2c-token');
      localStorage.removeItem('2c-company');
    },
    switchCompany(companyId: string, roleCode: string, roleName: string, permissions: AuthUser['permissions']) {
      update((current) => {
        if (!current) return current;
        const company = current.companies.find((c) => c.companyId === companyId);
        if (!company) return current;
        const updated = {
          ...current,
          companyId: company.companyId,
          roleId: company.roleId,
          roleCode,
          roleName,
          permissions,
        };
        localStorage.setItem('2c-user', JSON.stringify(updated));
        localStorage.setItem('2c-company', companyId);
        return updated;
      });
    },
    restore() {
      if (typeof localStorage === 'undefined') return;
      const stored = localStorage.getItem('2c-user');
      if (stored) {
        try {
          const parsed = JSON.parse(stored);
          if (!parsed.roleCode) parsed.roleCode = 'SUPERADMIN';
          if (!parsed.roleName) parsed.roleName = '';
          if (!parsed.permissions) parsed.permissions = [];
          set(parsed);
        } catch {
          localStorage.removeItem('2c-user');
        }
      }
    },
    getLastCompanyId(): string | null {
      if (typeof localStorage === 'undefined') return null;
      return localStorage.getItem('2c-company');
    },
  };
}

export const auth = createAuthStore();
export const isAuthenticated = derived(auth, ($auth) => $auth !== null);

export function hasPermission(permissions: AuthUser['permissions'], subsystem: string, action: string): boolean {
  return permissions.some(p =>
    p.subsystemCode === subsystem &&
    p.actions.includes(action) &&
    !p.deny
  );
}
