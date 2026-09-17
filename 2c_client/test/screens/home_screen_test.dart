import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:twoc_client/models/entity_schema.dart';
import 'package:twoc_client/models/navigation_item.dart';
import 'package:twoc_client/models/object.dart';
import 'package:twoc_client/models/script.dart';
import 'package:twoc_client/providers/app_providers.dart';
import 'package:twoc_client/providers/sdui_providers.dart';
import 'package:twoc_client/screens/catalog_screen.dart';
import 'package:twoc_client/screens/script_list_screen.dart';
import 'package:twoc_client/services/auth_service.dart';
import 'package:twoc_client/services/rpc_client.dart';
import 'package:twoc_client/services/ws_client.dart';

import '../support/fixtures.dart';
import '../support/screen_helpers.dart';
import '../support/test_router.dart';

/// Навигация платформенного модуля (как отдаёт сервер в `module.navigation`).
List<ModuleNavigation> _platformModules() => const [
      ModuleNavigation(
        code: 'platform',
        displayName: 'Платформа',
        version: '1.0.0',
        navigation: [
          NavigationItem(
            code: 'companies',
            label: 'Компании',
            entityType: 'company',
          ),
          NavigationItem(
            code: 'users',
            label: 'Пользователи',
            entityType: 'user',
          ),
          NavigationItem(
            code: 'roles',
            label: 'Роли',
            entityType: 'role',
          ),
        ],
      ),
      ModuleNavigation(
        code: 'accounting',
        displayName: 'Учёт',
        version: '1.0.0',
        navigation: [
          NavigationItem(code: 'accounts', label: 'Счета', entityType: 'account'),
        ],
      ),
    ];

void main() {
  Future<void> pumpHome(
    WidgetTester tester, {
    List<ModuleNavigation>? modules,
    Object? navigationError,
    List<Override> extraOverrides = const [],
  }) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          sharedPreferencesProvider.overrideWithValue(prefs),
          authServiceProvider.overrideWith(
            (ref) => AuthService(
              rpcClient: RpcClient(baseUrl: 'http://127.0.0.1:8080'),
              storage: testSecureStorage(),
            ),
          ),
          wsConnectionProvider.overrideWith(
            (ref) => Stream.value(WsConnectionState.disconnected),
          ),
          navigationProvider.overrideWith((ref) async {
            if (navigationError != null) {
              throw navigationError;
            }
            return modules ?? _platformModules();
          }),
          ...extraOverrides,
        ],
        child: MaterialApp.router(routerConfig: testRouter(initialLocation: '/home')),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets('тело дома показывает разделы платформы и модулей', (tester) async {
    await pumpHome(tester);

    expect(find.text('Платформа'), findsOneWidget);
    expect(find.text('Компании'), findsOneWidget);
    expect(find.text('Пользователи'), findsOneWidget);
    expect(find.text('Роли'), findsOneWidget);
    expect(find.text('Учёт'), findsOneWidget);
    expect(find.text('Счета'), findsOneWidget);
  });

  testWidgets('drawer повторяет разделы навигации', (tester) async {
    await pumpHome(tester);

    await tester.tap(find.byType(DrawerButton));
    await tester.pumpAndSettle();

    expect(find.byType(Drawer), findsOneWidget);
    expect(find.text('Компании'), findsWidgets); // тело + drawer
    expect(find.text('Пользователи'), findsWidgets);
    expect(find.text('Роли'), findsWidgets);
  });

  testWidgets('тап по разделу «Компании» открывает каталог типов', (tester) async {
    final companySchema = EntitySchema.fromJson(schemaWire(
      code: 'company',
      name: 'Компания',
      kind: 'catalog',
      requiredName: false,
    ));
    final companies = [
      ObjectItem.fromJson(
        objectWire(
          id: 'c1',
          entityType: 'company',
          companyId: '',
          number: null,
          data: {'code': 'ACME', 'name': 'Acme Ltd'},
        ),
      ),
    ];
    await pumpHome(tester, modules: [
      _platformModules().first,
    ], extraOverrides: [
      schemaProvider.overrideWith((ref, entityType) async => companySchema),
      objectsProvider.overrideWith((ref, entityType) async => companies),
    ]);

    await tester.tap(find.text('Компании'));
    await tester.pumpAndSettle();

    expect(find.byType(CatalogScreen), findsOneWidget);
    expect(find.text('Компания'), findsOneWidget); // AppBar
    expect(find.text('Acme Ltd'), findsOneWidget);
  });

  testWidgets('пункт «Скрипты» рендерится и ведёт на каталог скриптов',
      (tester) async {
    const moduleWithScripts = ModuleNavigation(
      code: 'platform',
      displayName: 'Платформа',
      version: '1.0.0',
      navigation: [
        NavigationItem(
          code: 'scripts',
          label: 'Скрипты',
          entityType: null,
        ),
      ],
    );
    await pumpHome(tester, modules: [moduleWithScripts], extraOverrides: [
      scriptsProvider.overrideWith((ref) async => [
            ScriptItem.fromJson(scriptWire()),
          ]),
    ]);

    expect(find.text('Скрипты'), findsOneWidget);

    await tester.tap(find.text('Скрипты'));
    await tester.pumpAndSettle();

    expect(find.byType(ScriptListScreen), findsOneWidget);
    expect(find.text('double.amount'), findsOneWidget);
  });

  testWidgets('пустой список модулей — заглушка «Нет установленных модулей»',
      (tester) async {
    await pumpHome(tester, modules: const []);

    expect(find.text('Нет установленных модулей'), findsOneWidget);
  });

  testWidgets('сбой загрузки навигации — сообщение об ошибке', (tester) async {
    await pumpHome(tester, navigationError: 'boom');

    expect(find.textContaining('Не удалось загрузить разделы'), findsOneWidget);
  });
}