import { writable, derived, get } from 'svelte/store';

export type ThemeMode = 'light' | 'dark' | 'system';

function createThemeStore() {
  const stored = typeof localStorage !== 'undefined'
    ? (localStorage.getItem('2c-theme') as ThemeMode | null)
    : null;

  const { subscribe, set, update } = writable<ThemeMode>(stored ?? 'system');

  function applyTheme(mode: ThemeMode) {
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    root.setAttribute('data-theme', 'nosh');

    if (mode === 'system') {
      const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      root.classList.add(prefersDark ? 'dark' : 'light');
    } else {
      root.classList.add(mode);
    }

    localStorage.setItem('2c-theme', mode);
  }

  return {
    subscribe,
    set(mode: ThemeMode) {
      set(mode);
      applyTheme(mode);
    },
    init() {
      applyTheme(get({ subscribe }));
    },
  };
}

export const theme = createThemeStore();

export const isDark = derived(theme, ($theme) => {
  if (typeof window === 'undefined') return false;
  if ($theme === 'system') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches;
  }
  return $theme === 'dark';
});
