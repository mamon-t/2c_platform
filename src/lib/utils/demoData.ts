// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Демо-сценарий «ООО ЛесТорг»: мебельная торговля.
//! Заполняет платформу типовыми данными поверх публичных API.

import { api, type EntityType } from '$lib/services/api';

export interface DemoProgress {
  step: string;
  done: number;
  total: number;
}

type Progress = (p: DemoProgress) => void;

const DEMO_PASSWORD = 'demo123';

async function fetchWasm(name: string): Promise<number[]> {
  const res = await fetch(`/plugins/${name}`);
  if (!res.ok) throw new Error(`Не найден ${name} — соберите плагины (см. README)`);
  const buf = await res.arrayBuffer();
  return Array.from(new Uint8Array(buf));
}

async function ensureModule(
  name: string,
  file: string,
  installed: Set<string>,
  log: Progress,
): Promise<void> {
  if (!installed.has(name)) {
    const bytes = await fetchWasm(file);
    const mod = await api.modulesInstall(bytes);
    installed.add(mod.code);
    log({ step: `${mod.name}: установлен`, done: 0, total: 0 });
  }
  try {
    await api.modulesEnable(name);
  } catch { /* уже включён */ }
}

async function findTypeByCode(code: string): Promise<EntityType | undefined> {
  const types = await api.listEntityTypes();
  return types.find((t) => t.code === code);
}

async function createObject(typeId: string, data: Record<string, unknown>, date?: string): Promise<string> {
  const obj = await api.createObject({ entity_type_id: typeId, data, date });
  return obj._id;
}

/** Создать объект и провести его через плагин-оркестратор. */
async function postDocument(module: string, typeId: string, data: Record<string, unknown>): Promise<string> {
  const id = await createObject(typeId, data);
  await api.pluginCall(module, 'on_post', { id });
  return id;
}

// ── Основная функция ─────────────────────────────────────────

export async function seedDemoData(log: Progress): Promise<string> {
  const steps = [
    'модули', 'метаданные', 'пользователи', 'каталоги',
    'документы', 'заявки', 'сообщения',
  ];
  let done = 0;
  const tick = (step: string) => log({ step, done, total: steps.length });

  // ── 1. Модули ──
  tick('Установка WASM-модулей');
  const installedMods = await api.modulesList();
  const installedCodes = new Set(installedMods.map((m) => m.code));
  await ensureModule('requests', 'requests_plugin.wasm', installedCodes, (p) => log({ ...p, done }));
  await ensureModule('stock', 'stock_plugin.wasm', installedCodes, (p) => log({ ...p, done }));
  await ensureModule('trade', 'trade_plugin.wasm', installedCodes, (p) => log({ ...p, done }));
  done++;

  // ── 2. Метаданные склада/торговли + REQUEST ──
  tick('Метаданные: справочники и документы');
  await api.stockSeedMetadata();
  await api.tradeSeedMetadata();

  if (!(await findTypeByCode('REQUEST'))) {
    const req = await api.createEntityType({
      code: 'REQUEST', name: 'Заявка', kind: 'document',
      description: 'Заявки на закупку/расходы',
      icon: 'fa-solid fa-file-signature',
    });
    for (const [code, name, kind, required, enumValues] of [
      ['title', 'Тема', 'string', true, null],
      ['priority', 'Приоритет', 'enum', false, ['low', 'medium', 'high', 'critical']],
      ['amount', 'Сумма', 'money', false, null],
      ['deadline', 'Срок', 'date', false, null],
      ['description', 'Описание', 'text', false, null],
    ] as const) {
      await api.createEntityField({
        entity_type_id: req._id, code, name,
        field_kind: kind as never, is_required: required,
        enum_values: enumValues ? [...enumValues] : undefined,
      });
    }
    await api.createEntityState({ entity_type_id: req._id, code: 'draft', name: 'Черновик', is_initial: true });
    await api.createEntityState({ entity_type_id: req._id, code: 'posted', name: 'Согласована', is_final: true });
    await api.createEntityTransition({ entity_type_id: req._id, code: 'submit', name: 'На согласование', from_state: 'draft', to_state: 'posted' });
  }
  done++;

  // ── 3. Пользователи ──
  tick('Пользователи и роли');
  const existingUsers = await api.listUsers();
  const have = (login: string) => existingUsers.some((u) => u.login === login);
  const companies = await api.listCompanies();
  const companyId = companies[0]?._id;
  if (!companyId) throw new Error('Нет компании');
  const roles = await api.listRoles(companyId);
  const adminRole = roles.find((r) => r.code === 'ADMIN') ?? roles[0];
  const viewerRole = roles.find((r) => r.code === 'VIEWER');

  const mkUser = async (login: string, last: string, first: string, position: string, roleId: string | undefined) => {
    if (have(login)) {
      const found = existingUsers.find((u) => u.login === login)!;
      return found._id;
    }
    const u = await api.createUser({
      login, password: DEMO_PASSWORD,
      last_name: last, first_name: first,
      display_name: `${last} ${first}`,
      email: `${login}@lestorg.demo`,
      company_id: companyId,
      role_id: roleId,
      position,
    });
    return u._id;
  };

  const buhId = await mkUser('smirnova', 'Смирнова', 'Анна', 'Главный бухгалтер', adminRole?._id);
  const skladId = await mkUser('petrov', 'Петров', 'Пётр', 'Кладовщик', adminRole?._id);
  const managerId = await mkUser('maria', 'Козлова', 'Мария', 'Менеджер продаж', adminRole?._id);
  const ivanovId = await mkUser('ivanov', 'Иванов', 'Иван', 'Менеджер по закупкам', viewerRole?._id);
  done++;

  // ── 4. Каталоги ──
  tick('Каталоги: номенклатура, контрагенты, склады');
  const nomType = (await findTypeByCode('NOMENCLATURE'))!;
  const locType = (await findTypeByCode('STOCK_LOCATION'))!;
  const cpType = (await findTypeByCode('COUNTERPARTY'))!;
  const ptType = (await findTypeByCode('PRICE_TYPE'))!;
  const priceType = (await findTypeByCode('PRICE'))!;

  // Склады и подотчётники
  const warehouseId = await createObject(locType._id, { type: 'warehouse', is_active: true });
  const custodianId = await createObject(locType._id, { type: 'custodian', is_active: true });

  // Номенклатура
  const nomIds: Record<string, string> = {};
  for (const [code, name, category, unit] of [
    ['STOL-YA-120', 'Стол письменный «Ясень»', 'Мебель', 'шт'],
    ['STUL-OF-42', 'Стул офисный «Вега»', 'Мебель', 'шт'],
    ['SHKAF-3D', 'Шкаф 3-дверный «Дуб»', 'Мебель', 'шт'],
    ['POLKA-N5', 'Полка навесная Н-5', 'Мебель', 'шт'],
    ['DRIL-BOSCH', 'Дрель Bosch GSB 18V', 'Инструмент', 'шт'],
  ]) {
    nomIds[code] = await createObject(nomType._id, { code, type: 'item', category, unit, min_qty: null });
  }

  // Контрагенты
  const cpIds: Record<string, string> = {};
  for (const [name, legal, inn, t] of [
    ['ООО «СтройДвор»', 'ООО Строительный Двор', '7701234567', 'customer'],
    ['ИП Смирнов А.В.', 'Индивидуальный предприниматель Смирнов А.В.', '7702345678', 'customer'],
    ['ООО «ПлитПром»', 'ООО Плита Промышленная', '5013456789', 'supplier'],
    ['ООО «Фурнитура+»', 'ООО Фурнитура Плюс', '7814567890', 'supplier'],
  ] as const) {
    cpIds[name] = await createObject(cpType._id, {
      name, legal_name: legal, counterparty_type: t, inn,
      is_active: true, contacts: [], bank_accounts: [],
    });
  }

  // Тип цен и цены
  const retailTypeId = await createObject(ptType._id, { code: 'RETAIL', name: 'Розничная', purpose: 'retail', order: 1, is_active: true });
  const today = new Date().toISOString().slice(0, 10);
  for (const [nomCode, price] of [
    ['STOL-YA-120', 12400], ['STUL-OF-42', 4200], ['SHKAF-3D', 18900], ['POLKA-N5', 3100], ['DRIL-BOSCH', 9800],
  ] as const) {
    await createObject(priceType._id, {
      price_type_id: retailTypeId, nomenclature_id: nomIds[nomCode],
      value: price * 100, valid_from: today, is_active: true,
    });
  }
  done++;

  // ── 5. Документы ──
  tick('Проведение документов: закупка → продажа → выдача');
  const purchaseType = (await findTypeByCode('PURCHASE'))!;
  const salesType = (await findTypeByCode('SALES'))!;

  // Поступление от ООО «ПлитПром»
  await postDocument('trade', purchaseType._id, {
    warehouse_id: warehouseId,
    supplier_id: cpIds['ООО «ПлитПром»'],
    incoming_doc_number: 'УТ-000117',
    comment: 'Партия мебели по договору №41 от 12.08.2026',
    lines: [
      { nomenclature_id: nomIds['STOL-YA-120'], qty: 10, price: 800000 },
      { nomenclature_id: nomIds['STUL-OF-42'], qty: 30, price: 250000 },
      { nomenclature_id: nomIds['DRIL-BOSCH'], qty: 2, price: 720000 },
    ],
  });

  // Реализация ООО «СтройДвор»
  await postDocument('trade', salesType._id, {
    warehouse_id: warehouseId,
    customer_id: cpIds['ООО «СтройДвор»'],
    payment_method: 'cash',
    comment: 'Оснащение офиса, счёт №1042',
    lines: [
      { nomenclature_id: nomIds['STOL-YA-120'], qty: 4, price: 1240000 },
      { nomenclature_id: nomIds['STUL-OF-42'], qty: 10, price: 420000 },
    ],
  });

  // Выдача дрели под отчёт Иванову
  const handoverType = (await findTypeByCode('HANDOVER'))!;
  await postDocument('stock', handoverType._id, {
    from_location_id: warehouseId,
    to_location_id: custodianId,
    responsible_user_id: ivanovId,
    expected_return_date: '2026-09-30',
    lines: [{ nomenclature_id: nomIds['DRIL-BOSCH'], qty: 1 }],
  });
  done++;

  // ── 6. Заявка на закупку ──
  tick('Заявка на согласовании');
  const requestType = (await findTypeByCode('REQUEST'))!;
  // Маршрут OFFICE без ЭЦП: этап 1 — ivanov
  await api.pluginCall('requests', 'routes_save', {
    route: {
      code: 'OFFICE', name: 'Хозяйственные закупки', is_active: true,
      steps: [{ step_order: 1, approver_type: 'user', approver_id: ivanovId, timeout_hours: 24, is_required: true }],
    },
  });
  const requestId = await createObject(requestType._id, {
    title: 'Закупка полок Н-5 (50 шт)',
    priority: 'medium',
    amount: 15500000,
    deadline: '2026-09-15',
    description: 'Пополнение остатков ходовой позиции под осенний спрос.',
  });
  await api.pluginCall('requests', 'submit', {
    request_id: requestId, route_code: 'OFFICE', signature_der: '',
  });
  done++;

  // ── 7. Чат ──
  tick('Сообщения');
  const usersAll = await api.listUsers();
  const memberIds = [buhId, skladId, managerId].filter(Boolean);
  const room = (await api.messagingRoomsCreate(
    'Общий — ЛесТорг',
    memberIds.length >= 2 ? memberIds : usersAll.slice(0, 3).map((u) => u._id),
  )) as { room?: { _id?: string }; _id?: string };
  const roomId = room?.room?._id ?? room?._id;
  if (roomId) {
    await api.messagingMessagesSend(roomId, 'Добро пожаловать в демо-базу «ЛесТорг»! Здесь можно обсудить документы и склад.');
  }
  done++;

  return [
    `Компания: ${companies[0]?.name ?? '—'}`,
    `Пользователи: smirnova / petrov / maria / ivanov (пароль ${DEMO_PASSWORD})`,
    'Документы: поступление, реализация, выдача под отчёт — проведены',
    'Заявка «Полки Н-5» ждёт согласования у ivanov',
  ].join('\n');
}
