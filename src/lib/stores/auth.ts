import { writable, derived } from 'svelte/store';

export interface AuthUser {
  userId: string;
  companyId: string;
  roleId: string;
  username: string;
  displayName: string;
}

function createAuthStore() {
  const { subscribe, set, update } = writable<AuthUser | null>(null);

  return {
    subscribe,
    login(user: AuthUser) {
      set(user);
      localStorage.setItem('2c-user', JSON.stringify(user));
    },
    logout() {
      set(null);
      localStorage.removeItem('2c-user');
      localStorage.removeItem('2c-token');
    },
    restore() {
      if (typeof localStorage === 'undefined') return;
      const stored = localStorage.getItem('2c-user');
      if (stored) {
        try {
          set(JSON.parse(stored));
        } catch {
          localStorage.removeItem('2c-user');
        }
      }
    },
  };
}

export const auth = createAuthStore();
export const isAuthenticated = derived(auth, ($auth) => $auth !== null);
