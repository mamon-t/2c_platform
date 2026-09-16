import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/entity_schema.dart';
import 'package:twoc_client/models/object.dart';
import 'package:twoc_client/providers/sdui_providers.dart';

import '../support/fixtures.dart';
import '../support/test_router.dart';

/// Схема ядрового типа `company` (как отдаёт сервер после 15pre-1).
Map<String, dynamic> _companySchemaWire() => {
      'entity_type': {'code': 'company', 'name': 'Компания', 'kind': 'catalog'},
      'fields': [
        field('code', 'Код', 'string', required: true, order: 1),
        field('name', 'Наименование', 'string', required: true, order: 2),
        field('is_active', 'Активна', 'boolean', order: 3),
      ],
      'states': [
        {'code': 'active', 'label': 'Активна', 'is_initial': true},
      ],
      'transitions': <Map<String, dynamic>>[],
      'forms': <Map<String, dynamic>>[],
      'actions': <Map<String, dynamic>>[],
      'relations': <Map<String, dynamic>>[],
    };

/// Схема ядрового типа `user` (поля из metadata_seed.rs).
Map<String, dynamic> _userSchemaWire() => {
      'entity_type': {'code': 'user', 'name': 'Пользователи', 'kind': 'catalog'},
      'fields': [
        field('login', 'Логин', 'string', required: true, order: 1),
        field('status', 'Статус', 'string', order: 2),
        field('role_ids', 'Роли', 'array', order: 3),
        field('locale', 'Локаль', 'string', order: 4),
        field('timezone', 'Часовой пояс', 'string', order: 5),
      ],
      'states': [
        {'code': 'active', 'label': 'Активен', 'is_initial': true},
        {'code': 'blocked', 'label': 'Заблокирован', 'is_final': true},
      ],
      'transitions': [
        {
          'code': 'block',
          'label': 'Заблокировать',
          'from_state': 'active',
          'to_state': 'blocked',
        },
      ],
      'forms': <Map<String, dynamic>>[],
      'actions': <Map<String, dynamic>>[],
      'relations': <Map<String, dynamic>>[],
    };

/// Схема ядрового типа `role` (индексы политик).
Map<String, dynamic> _roleSchemaWire() => {
      'entity_type': {'code': 'role', 'name': 'Роли', 'kind': 'catalog'},
      'fields': [
        field('code', 'Код', 'string', required: true, order: 1),
        field('name', 'Наименование', 'string', required: true, order: 2),
        field('permission_policy_codes', 'Политики', 'array', order: 3),
        field('is_system', 'Системная', 'boolean', order: 4),
      ],
      'states': [
        {'code': 'active', 'label': 'Активна', 'is_initial': true},
      ],
      'transitions': <Map<String, dynamic>>[],
      'forms': <Map<String, dynamic>>[],
      'actions': <Map<String, dynamic>>[],
      'relations': <Map<String, dynamic>>[],
    };

void main() {
  Future<void> pumpCatalog(
    WidgetTester tester, {
    required EntitySchema schema,
    required List<ObjectItem> items,
  }) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          schemaProvider.overrideWith((ref, entityType) async => schema),
          objectsProvider.overrideWith((ref, entityType) async => items),
        ],
        child: MaterialApp.router(routerConfig: testRouter()),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets('каталог компаний рендерит поля схемы и значения', (tester) async {
    final schema = EntitySchema.fromJson(_companySchemaWire());
    final items = [
      ObjectItem.fromJson(
        objectWire(
          id: 'c1',
          entityType: 'company',
          companyId: '',
          number: null,
          data: {'code': 'ACME', 'name': 'Acme Ltd', 'is_active': true},
        ),
      ),
      ObjectItem.fromJson(
        objectWire(
          id: 'c2',
          entityType: 'company',
          companyId: '',
          number: null,
          data: {'code': 'GLOBEX', 'name': 'Globex Corp', 'is_active': false},
        ),
      ),
    ];
    await pumpCatalog(tester, schema: schema, items: items);

    expect(find.text('Компания'), findsOneWidget); // AppBar
    expect(find.text('Код'), findsOneWidget);
    expect(find.text('Наименование'), findsOneWidget);
    expect(find.text('Acme Ltd'), findsOneWidget);
    expect(find.text('Globex Corp'), findsOneWidget);
    expect(find.text('да'), findsOneWidget); // is_active true
    expect(find.text('нет'), findsOneWidget);
  });

  testWidgets('каталог пользователей рендерит статус и контакты', (tester) async {
    final schema = EntitySchema.fromJson(_userSchemaWire());
    final items = [
      ObjectItem.fromJson(
        objectWire(
          id: 'u1',
          entityType: 'user',
          companyId: '',
          number: null,
          data: {
            'login': 'ivan',
            'status': 'active',
            'role_ids': ['r1'],
            'locale': 'ru-RU',
            'timezone': 'Europe/Moscow',
          },
        ),
      ),
    ];
    await pumpCatalog(tester, schema: schema, items: items);

    expect(find.text('Пользователи'), findsOneWidget); // AppBar
    expect(find.text('ivan'), findsOneWidget);
    expect(find.text('Europe/Moscow'), findsOneWidget);
    expect(find.text('r1'), findsOneWidget);
  });

  testWidgets('каталог ролей рендерит коды политик и системный флаг',
      (tester) async {
    final schema = EntitySchema.fromJson(_roleSchemaWire());
    final items = [
      ObjectItem.fromJson(
        objectWire(
          id: 'r1',
          entityType: 'role',
          companyId: 'c1',
          number: null,
          data: {
            'code': 'admin',
            'name': 'Администратор',
            'permission_policy_codes': ['platform.full'],
            'is_system': true,
          },
        ),
      ),
    ];
    await pumpCatalog(tester, schema: schema, items: items);

    expect(find.text('Роли'), findsOneWidget); // AppBar
    expect(find.text('Администратор'), findsOneWidget);
    expect(find.text('platform.full'), findsOneWidget);
    expect(find.text('да'), findsOneWidget); // is_system
  });

  testWidgets('пустой каталог ядрового типа — заглушка «Нет объектов»',
      (tester) async {
    final schema = EntitySchema.fromJson(_companySchemaWire());
    await pumpCatalog(tester, schema: schema, items: const []);

    expect(find.text('Компания'), findsOneWidget);
    expect(find.text('Нет объектов'), findsOneWidget);
  });
}