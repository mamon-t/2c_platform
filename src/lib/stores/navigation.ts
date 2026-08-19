import { writable } from 'svelte/store';

export interface NavItem {
  code: string;
  label: string;
  icon: string;
  path: string;
}

export const navItems = writable<NavItem[]>([
  { code: 'dashboard', label: 'Главная', icon: 'grid', path: '/' },
  { code: 'companies', label: 'Компании', icon: 'building', path: '/companies' },
  { code: 'users', label: 'Пользователи', icon: 'users', path: '/users' },
  { code: 'roles', label: 'Роли', icon: 'shield', path: '/roles' },
  { code: 'documents', label: 'Документы', icon: 'file-text', path: '/documents' },
  { code: 'catalogs', label: 'Справочники', icon: 'book', path: '/catalogs' },
  { code: 'reports', label: 'Отчёты', icon: 'bar-chart', path: '/reports' },
  { code: 'scripts', label: 'Скрипты', icon: 'code', path: '/scripts' },
  { code: 'settings', label: 'Настройки', icon: 'settings', path: '/settings' },
]);

export const activeNav = writable<string>('dashboard');
