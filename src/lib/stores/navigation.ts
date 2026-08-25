// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

export interface NavItem {
  code: string;
  label: string;
  icon: string;
  path: string;
  requiredPermission?: { subsystem: string; action: string };
  /** Заголовок секции в сайдбаре. */
  group?: string;
  /** Свёрнута ли группа по умолчанию. */
  defaultCollapsed?: boolean;
}

export const allNavItems: NavItem[] = [
  { code: 'dashboard', label: 'Главная', icon: 'fa-solid fa-house', path: '/' },
  { code: 'documents', label: 'Документы', icon: 'fa-solid fa-file-lines', path: '/documents', requiredPermission: { subsystem: 'documents', action: 'read' }, group: 'Торговля' },
  { code: 'requests', label: 'Заявки', icon: 'fa-solid fa-file-signature', path: '/requests', requiredPermission: { subsystem: 'requests', action: 'read' }, group: 'Торговля' },
  { code: 'trade', label: 'Торговля', icon: 'fa-solid fa-cart-shopping', path: '/trade', requiredPermission: { subsystem: 'trade', action: 'read' }, group: 'Торговля' },
  { code: 'stock', label: 'Склад', icon: 'fa-solid fa-boxes-stacked', path: '/stock', requiredPermission: { subsystem: 'stock', action: 'read' }, group: 'Торговля' },
  { code: 'catalogs', label: 'Справочники', icon: 'fa-solid fa-book', path: '/catalogs', requiredPermission: { subsystem: 'catalogs', action: 'read' }, group: 'Справочники' },
  { code: 'objects', label: 'Все объекты', icon: 'fa-solid fa-cube', path: '/objects', group: 'Справочники' },
  { code: 'reports', label: 'Отчёты', icon: 'fa-solid fa-chart-bar', path: '/reports', requiredPermission: { subsystem: 'reports', action: 'read' }, group: 'Отчёты' },
  { code: 'messages', label: 'Сообщения', icon: 'fa-solid fa-comments', path: '/messages', group: 'Обслуживание' },
  { code: 'convert', label: 'Конвертация', icon: 'fa-solid fa-right-left', path: '/convert', group: 'Обслуживание' },
  { code: 'opening_balances', label: 'Входящие сальдо', icon: 'fa-solid fa-scale-unbalanced', path: '/opening-balances', requiredPermission: { subsystem: 'accounting', action: 'read' }, group: 'Обслуживание' },
  // События — скрыты из основного меню (Event Store для разработчиков)
  // { code: 'events', label: 'События', icon: 'fa-solid fa-bolt', path: '/events', requiredPermission: { subsystem: 'audit', action: 'read' }, group: 'Обслуживание' },
  { code: 'audit', label: 'Журнал', icon: 'fa-solid fa-clock-rotate-left', path: '/audit', requiredPermission: { subsystem: 'audit', action: 'read' }, group: 'Обслуживание' },
  { code: 'companies', label: 'Компании', icon: 'fa-solid fa-building', path: '/companies', requiredPermission: { subsystem: 'companies', action: 'read' }, group: 'Администрирование', defaultCollapsed: true },
  { code: 'users', label: 'Пользователи', icon: 'fa-solid fa-users', path: '/users', requiredPermission: { subsystem: 'users', action: 'read' }, group: 'Администрирование' },
  { code: 'roles', label: 'Роли', icon: 'fa-solid fa-shield-halved', path: '/roles', requiredPermission: { subsystem: 'roles', action: 'read' }, group: 'Администрирование' },
  { code: 'metadata', label: 'Метаданные', icon: 'fa-solid fa-database', path: '/metadata', requiredPermission: { subsystem: 'settings', action: 'manage' }, group: 'Администрирование' },
  { code: 'scripts', label: 'Скрипты', icon: 'fa-solid fa-code', path: '/scripts', requiredPermission: { subsystem: 'scripts', action: 'read' }, group: 'Администрирование' },
  { code: 'modules', label: 'Прикладные модули', icon: 'fa-solid fa-puzzle-piece', path: '/modules', requiredPermission: { subsystem: 'plugins', action: 'read' }, group: 'Администрирование' },
  { code: 'print', label: 'Печатные формы', icon: 'fa-solid fa-print', path: '/print', requiredPermission: { subsystem: 'print', action: 'read' }, group: 'Администрирование' },
  { code: 'numbering', label: 'Нумерация', icon: 'fa-solid fa-hashtag', path: '/numbering', requiredPermission: { subsystem: 'numbering', action: 'read' }, group: 'Администрирование' },
  { code: 'devices', label: 'Оборудование', icon: 'fa-solid fa-plug', path: '/devices', requiredPermission: { subsystem: 'devices', action: 'read' }, group: 'Администрирование' },
  { code: 'settings', label: 'Настройки', icon: 'fa-solid fa-gear', path: '/settings', requiredPermission: { subsystem: 'settings', action: 'manage' }, group: 'Администрирование' },
];
