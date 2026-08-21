export interface NavItem {
  code: string;
  label: string;
  icon: string;
  path: string;
  requiredPermission?: { subsystem: string; action: string };
}

export const allNavItems: NavItem[] = [
  { code: 'dashboard', label: 'Главная', icon: 'fa-solid fa-house', path: '/' },
  { code: 'companies', label: 'Компании', icon: 'fa-solid fa-building', path: '/companies', requiredPermission: { subsystem: 'companies', action: 'read' } },
  { code: 'users', label: 'Пользователи', icon: 'fa-solid fa-users', path: '/users', requiredPermission: { subsystem: 'users', action: 'read' } },
  { code: 'roles', label: 'Роли', icon: 'fa-solid fa-shield-halved', path: '/roles', requiredPermission: { subsystem: 'roles', action: 'read' } },
  { code: 'metadata', label: 'Метаданные', icon: 'fa-solid fa-database', path: '/metadata', requiredPermission: { subsystem: 'settings', action: 'manage' } },
  { code: 'documents', label: 'Документы', icon: 'fa-solid fa-file-lines', path: '/documents', requiredPermission: { subsystem: 'documents', action: 'read' } },
  { code: 'catalogs', label: 'Справочники', icon: 'fa-solid fa-book', path: '/catalogs', requiredPermission: { subsystem: 'catalogs', action: 'read' } },
  { code: 'objects', label: 'Все объекты', icon: 'fa-solid fa-cube', path: '/objects' },
  { code: 'events', label: 'События', icon: 'fa-solid fa-bolt', path: '/events', requiredPermission: { subsystem: 'audit', action: 'read' } },
  { code: 'reports', label: 'Отчёты', icon: 'fa-solid fa-chart-bar', path: '/reports', requiredPermission: { subsystem: 'reports', action: 'read' } },
  { code: 'scripts', label: 'Скрипты', icon: 'fa-solid fa-code', path: '/scripts', requiredPermission: { subsystem: 'scripts', action: 'read' } },
  { code: 'audit', label: 'Журнал', icon: 'fa-solid fa-clock-rotate-left', path: '/audit', requiredPermission: { subsystem: 'audit', action: 'read' } },
  { code: 'convert', label: 'Конвертация', icon: 'fa-solid fa-right-left', path: '/convert', requiredPermission: { subsystem: 'plugins', action: 'read' } },
  { code: 'print', label: 'Печатные формы', icon: 'fa-solid fa-print', path: '/print', requiredPermission: { subsystem: 'print', action: 'read' } },
  { code: 'numbering', label: 'Нумерация', icon: 'fa-solid fa-hashtag', path: '/numbering', requiredPermission: { subsystem: 'numbering', action: 'read' } },
  { code: 'settings', label: 'Настройки', icon: 'fa-solid fa-gear', path: '/settings', requiredPermission: { subsystem: 'settings', action: 'manage' } },
];
