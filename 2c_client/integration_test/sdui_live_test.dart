import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:integration_test/integration_test.dart';
import 'package:logging/logging.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:twoc_client/app.dart';
import 'package:twoc_client/core/app_theme.dart';
import 'package:twoc_client/core/router.dart';
import 'package:twoc_client/providers/app_providers.dart';
import 'package:twoc_client/providers/sdui_providers.dart';
import 'package:twoc_client/providers/theme_providers.dart';
import 'package:twoc_client/services/auth_service.dart';
import 'package:twoc_client/services/object_service.dart';

/// In-memory хранилище токенов (не зависит от libsecret на тестовой машине).
class _MemoryStorage extends FlutterSecureStorage {
  final Map<String, String> _data = {};

  @override
  Future<String?> read({
    required String key,
    AppleOptions? iOptions,
    AndroidOptions? aOptions,
    LinuxOptions? lOptions,
    WebOptions? webOptions,
    AppleOptions? mOptions,
    WindowsOptions? wOptions,
  }) async =>
      _data[key];

  @override
  Future<void> write({
    required String key,
    required String? value,
    AppleOptions? iOptions,
    AndroidOptions? aOptions,
    LinuxOptions? lOptions,
    WebOptions? webOptions,
    AppleOptions? mOptions,
    WindowsOptions? wOptions,
  }) async {
    if (value == null) {
      _data.remove(key);
    } else {
      _data[key] = value;
    }
  }

  @override
  Future<void> delete({
    required String key,
    AppleOptions? iOptions,
    AndroidOptions? aOptions,
    LinuxOptions? lOptions,
    WebOptions? webOptions,
    AppleOptions? mOptions,
    WindowsOptions? wOptions,
  }) async {
    _data.remove(key);
  }
}

/// Ожидает появление [finder] на экране. Живой сервер отвечает после кадров,
/// поэтому ждём с реальными промежуточными pump.
Future<void> waitFor(
  WidgetTester tester,
  Finder finder, {
  String? what,
  Duration timeout = const Duration(seconds: 30),
}) async {
  final deadline = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(deadline)) {
    if (finder.evaluate().isNotEmpty) {
      return;
    }
    await tester.pump(const Duration(milliseconds: 200));
  }
  final texts = find
      .byType(Text)
      .evaluate()
      .map((e) => (e.widget as Text).data)
      .whereType<String>()
      .toList();
  fail('Не дождались: ${what ?? finder}\nТексты на экране: $texts');
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  Logger.root.level = Level.ALL;
  Logger.root.onRecord
      .listen((r) => debugPrint('LOG ${r.loggerName} ${r.level.name}: ${r.message}'));

  const serverBase = 'http://127.0.0.1:8080';
  const login = 'admin11b';
  const password = '16aLythe-p9U-dromonika';

  testWidgets(
    '11b live: вход → разделы → каталог → правка → создание → OCC-конфликт → тема',
    (tester) async {
      // Пробуем сервер; при недоступности — печатаем SKIPPED (тест пропускается).
      try {
        final probe = await http
            .get(Uri.parse('$serverBase/health'))
            .timeout(const Duration(seconds: 3));
        if (probe.statusCode != 200) {
          debugPrint('SKIPPED: сервер недоступен (status=${probe.statusCode})');
          return;
        }
      } catch (e) {
        debugPrint('SKIPPED: сервер недоступен ($e)');
        return;
      }

      final prefs = await SharedPreferences.getInstance();
      await prefs.remove('server_base_url');
      final themeBase = await Directory.systemTemp.createTemp('theme_live');

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            sharedPreferencesProvider.overrideWithValue(prefs),
            authServiceProvider.overrideWith(
              (ref) => AuthService(
                rpcClient: ref.watch(rpcClientProvider),
                storage: _MemoryStorage(),
              ),
            ),
            themeStoreProvider.overrideWithValue(ThemeStore(themeBase)),
          ],
          child: const TwocApp(),
        ),
      );
      await tester.pumpAndSettle();

      // ── 1. Вход ──────────────────────────────────────────────────────
      await waitFor(tester, find.text('Войти'), what: 'экран входа');
      await tester.enterText(find.byType(TextFormField).at(0), login);
      await tester.enterText(find.byType(TextFormField).at(1), password);
      await tester.tap(find.text('Войти'));

      // ── 1b. Главный экран: разделы из module.navigation ─────────────
      await waitFor(tester, find.text('План счетов'),
          what: 'раздел "План счетов" после входа');
      expect(find.text('Учётные периоды'), findsWidgets);

      // ── 3. Каталог account ──────────────────────────────────────────
      await tester.tap(find.text('План счетов').first);
      // Имя демо-объекта меняется между прогонами (правка и OCC-шаг
      // переименовывают его), но код "001" стабилен — ищем строку по нему.
      await waitFor(tester, find.text('001'),
          what: 'демо-объект (код 001) в каталоге');
      // Название типа берётся из реальной схемы сервера ("Счёт плана счетов"),
      // поэтому проверяем по FAB и строке DataTable, а не по имени AppBar.
      expect(find.byTooltip('Создать'), findsOneWidget);

      // ── 4. Просмотр → правка имени → сохранение ─────────────────────
      await tester.tap(find.text('001').first);
      await waitFor(tester, find.byIcon(Icons.edit),
          what: 'форма просмотра (иконка правки)');
      final container =
          ProviderScope.containerOf(tester.element(find.byType(TextFormField).first));
      final objectsService = container.read(objectServiceProvider);
      final companyId = container.read(companyIdProvider);

      await tester.tap(find.byIcon(Icons.edit));
      await waitFor(tester, find.text('Сохранить'),
          what: 'форма правки (кнопка Сохранить)');
      await tester.enterText(
          find.byType(TextFormField).at(1), 'Касса основная (live)');
      await tester.tap(find.text('Сохранить'));
      // pop возвращает на экран просмотра (view), а не в каталог —
      // навигируем обратно через роутер.
      await tester.pumpAndSettle();
      container.read(routerProvider).go('/catalog/account');
      await waitFor(tester, find.byTooltip('Создать'),
          what: 'возврат в каталог после сохранения');

      final listed1 = await objectsService.list('account', companyId);
      final edited =
          listed1.firstWhere((o) => o.data['name'] == 'Касса основная (live)');
      expect(edited.data['name'], 'Касса основная (live)');

      // ── 5. Создание нового объекта через FAB ─────────────────────────
      await tester.tap(find.byTooltip('Создать'));
      // Название типа берётся из схемы: "Счёт плана счетов".
      await waitFor(tester, find.textContaining('Новый'),
          what: 'форма создания');
      await tester.enterText(find.byType(TextFormField).at(0), 'live-int');
      await tester.enterText(find.byType(TextFormField).at(1), 'Касса интеграции');
      await tester.tap(find.byType(DropdownButtonFormField<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('asset').last);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Сохранить'));
      await waitFor(tester, find.byTooltip('Создать'),
          what: 'возврат в каталог после создания');
      final listed2 = await objectsService.list('account', companyId);
      expect(listed2.any((o) => o.data['code'] == 'live-int'), isTrue,
          reason: 'новый объект должен уехать на сервер');

      // ── 6. OCC-конфликт ──────────────────────────────────────────────
      // Открываем тот же объект (код 001) на редактирование.
      await tester.tap(find.text('001').first);
      await waitFor(tester, find.byIcon(Icons.edit),
          what: 'форма просмотра перед конфликтом');
      await tester.tap(find.byIcon(Icons.edit));
      await waitFor(tester, find.text('Сохранить'),
          what: 'форма правки перед конфликтом');

      // Пока форма открыта (expected_version = V), на сервере дважды меняем объект.
      final fresh = await objectsService.get(edited.id);
      final bump1 = await objectsService.update(
        edited.id,
        fresh.version,
        data: {...fresh.data, 'name': 'Касса (служебный v1)'},
      );
      await objectsService.update(
        edited.id,
        bump1.version,
        data: {...fresh.data, 'name': 'Касса (служебный v2)'},
      );

      await tester.enterText(
          find.byType(TextFormField).at(1), 'Изменение в конфликте');
      await tester.tap(find.text('Сохранить'));
      await waitFor(tester, find.text('Конфликт версий'),
          what: 'диалог OCC-конфликта');
      await tester.tap(find.text('OK'));
      // После конфликта форма правки остаётся открытой (ручное разрешение).
      await waitFor(tester, find.text('Сохранить'),
          what: 'форма правки после закрытия диалога конфликта');

      // ── 7. Смена темы в настройках ───────────────────────────────────
      container.read(routerProvider).go('/settings');
      await waitFor(tester, find.text('Тема'), what: 'настройки: блок "Тема"');
      await tester.tap(find.byType(DropdownButtonFormField<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Светлая').last);
      await tester.pumpAndSettle();

      final settingsContext = tester.element(find.byType(Scaffold).first);
      expect(Theme.of(settingsContext).brightness, Brightness.light,
          reason: 'тема применилась в рантайме');
      final configFile =
          File('${themeBase.path}${Platform.pathSeparator}config.toml');
      expect(configFile.readAsStringSync(), contains('theme = "light"'),
          reason: 'выбор темы персистится в config.toml');
    },
  );
}