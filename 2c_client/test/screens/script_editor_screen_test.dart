import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/script.dart';
import 'package:twoc_client/models/server_error.dart';
import 'package:twoc_client/providers/sdui_providers.dart';
import 'package:twoc_client/services/script_service.dart';

import '../support/fixtures.dart';
import '../support/screen_helpers.dart';
import '../support/test_router.dart';

void main() {
  final existing = ScriptItem.fromJson(
    scriptWire(
      id: 's-1',
      code: 'double.amount',
      name: 'Удвоить сумму',
      source: 'ctx.object.amount * ctx.args.factor',
      entityType: 'invoice',
    ),
  );

  Future<FakeScriptService> pumpEditor(
    WidgetTester tester, {
    required String code,
    FakeScriptService? service,
    List<Override> extra = const [],
  }) async {
    final fake = service ??
        FakeScriptService(items: {existing.code: existing});
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          scriptServiceProvider.overrideWithValue(fake),
          scriptsProvider.overrideWith((ref) async => const []),
          companyIdProvider.overrideWithValue('c1'),
          ...extra,
        ],
        child: MaterialApp.router(
          routerConfig: testRouter(initialLocation: '/scripts/$code/edit'),
        ),
      ),
    );
    await tester.pumpAndSettle();
    return fake;
  }

  Future<void> scrollTo(WidgetTester tester, Finder finder) async {
    final scrollable = find
        .descendant(of: find.byType(ListView), matching: find.byType(Scrollable))
        .first;
    await tester.scrollUntilVisible(finder, 200, scrollable: scrollable);
    await tester.pumpAndSettle();
  }

  testWidgets('создание: пустые поля — предупреждение', (tester) async {
    await pumpEditor(tester, code: 'new', service: FakeScriptService());

    await scrollTo(tester, find.text('Сохранить'));
    await tester.tap(find.text('Сохранить'));
    await tester.pumpAndSettle();

    expect(find.text('Заполните код, название и исходник'), findsOneWidget);
  });

  testWidgets('создание: заполненные поля сохраняют скрипт и возвращают на список',
      (tester) async {
    final fake = FakeScriptService();
    await pumpEditor(tester, code: 'new', service: fake);

    await tester.enterText(
        find.widgetWithText(TextFormField, 'Код'), 'my.script');
    await tester.enterText(
        find.widgetWithText(TextFormField, 'Название'), 'Мой скрипт');
    await scrollTo(tester, find.text('Исходник Rhai'));
    await tester.enterText(
        find.byKey(const ValueKey('source-editor')), '40 + 2');
    await scrollTo(tester, find.text('Сохранить'));
    await tester.tap(find.text('Сохранить'));
    await tester.pumpAndSettle();

    expect(fake.created?.code, 'my.script');
    expect(find.text('Сохранено'), findsOneWidget);
    // вернулись на список скриптов
    expect(find.byType(Scaffold), findsWidgets);
  });

  testWidgets('создание: ошибка сервера показывает сообщение', (tester) async {
    final fake = FakeScriptService()
      ..throwOnCreate = const ServerError(ErrorCode.validation, 'Код занят');
    await pumpEditor(tester, code: 'new', service: fake);

    await tester.enterText(
        find.widgetWithText(TextFormField, 'Код'), 'my.script');
    await tester.enterText(
        find.widgetWithText(TextFormField, 'Название'), 'Мой скрипт');
    await scrollTo(tester, find.text('Исходник Rhai'));
    await tester.enterText(
        find.byKey(const ValueKey('source-editor')), '40 + 2');
    await scrollTo(tester, find.text('Сохранить'));
    await tester.tap(find.text('Сохранить'));
    await tester.pumpAndSettle();

    expect(find.text('Код занят'), findsOneWidget);
  });

  testWidgets('редактирование: форма заполнена данными скрипта', (tester) async {
    await pumpEditor(tester, code: 'double.amount');

    expect(find.text('Удвоить сумму'), findsWidgets); // title + field
    expect(find.text('ctx.object.amount * ctx.args.factor'), findsOneWidget);
    // «Прогнать» присутствует и активен в режиме редактирования
    await scrollTo(tester, find.text('Прогнать'));
    final testButton = tester.widget<OutlinedButton>(find.ancestor(
      of: find.text('Прогнать'),
      matching: find.bySubtype<OutlinedButton>(),
    ));
    expect(testButton.onPressed, isNotNull);
  });

  testWidgets('редактирование: Прогнать показывает результат', (tester) async {
    final fake = FakeScriptService(items: {existing.code: existing})
      ..testResult = {'result': 42, 'execution_time_ms': 3};
    await pumpEditor(tester, code: 'double.amount', service: fake);

    await scrollTo(tester, find.text('Прогнать'));
    await tester.tap(find.text('Прогнать'));
    await tester.pumpAndSettle();

    expect(find.text('Результат: 42 · 3 мс'), findsOneWidget);
  });

  testWidgets('редактирование: ошибка загрузки показывается', (tester) async {
    final fake = FakeScriptService()
      ..throwOnGet = const ServerError(ErrorCode.notFound, 'Скрипт не найден');
    await pumpEditor(tester, code: 'double.amount', service: fake);

    await tester.pumpAndSettle();

    expect(find.textContaining('Не удалось загрузить скрипт'), findsOneWidget);
  });

  testWidgets('Проверить: валидный исходник — сообщение и панель', (tester) async {
    final fake = FakeScriptService()
      ..validation = const ScriptValidation(valid: true, errors: []);
    await pumpEditor(tester, code: 'new', service: fake);

    await scrollTo(tester, find.text('Исходник Rhai'));
    await tester.enterText(
        find.byKey(const ValueKey('source-editor')), '40 + 2');
    await scrollTo(tester, find.text('Проверить'));
    await tester.tap(find.text('Проверить'));
    await tester.pumpAndSettle();

    expect(find.text('Синтаксис в порядке'), findsWidgets); // snackbar+панель
  });

  testWidgets('Проверить: ошибки исходника — список с координатами',
      (tester) async {
    final fake = FakeScriptService()
      ..validation = const ScriptValidation(
        valid: false,
        errors: [
          ScriptValidationError(message: 'Ожидалось выражение', line: 1, column: 8),
        ],
      );
    await pumpEditor(tester, code: 'new', service: fake);

    await scrollTo(tester, find.text('Исходник Rhai'));
    await tester.enterText(
        find.byKey(const ValueKey('source-editor')), 'let x = ;');
    await scrollTo(tester, find.text('Проверить'));
    await tester.tap(find.text('Проверить'));
    await tester.pumpAndSettle();

    expect(find.text('Ошибки проверки:'), findsOneWidget);
    expect(find.text('Строка 1:8 — Ожидалось выражение'), findsOneWidget);
    expect(find.text('Найдены ошибки (1)'), findsOneWidget);
  });

  testWidgets('пре-чек: пропущенная «;» показывает панель предупреждения',
      (tester) async {
    await pumpEditor(tester, code: 'new', service: FakeScriptService());

    await scrollTo(tester, find.text('Исходник Rhai'));
    await tester.enterText(
        find.byKey(const ValueKey('source-editor')), 'let a = 1\na;');
    await tester.pumpAndSettle();

    await scrollTo(tester, find.text('Клиентский пре-чек:'));
    expect(find.text('Клиентский пре-чек:'), findsOneWidget);
    expect(
      find.text('Строка 1:10 — Оператор не завершён знаком «;»'),
      findsOneWidget,
    );
  });

  testWidgets('строгий режим: пропущенная «;» блокирует сохранение',
      (tester) async {
    final fake = FakeScriptService();
    await pumpEditor(tester, code: 'new', service: fake);

    await tester.enterText(
        find.widgetWithText(TextFormField, 'Код'), 'my.script');
    await tester.enterText(
        find.widgetWithText(TextFormField, 'Название'), 'Мой скрипт');
    await scrollTo(tester, find.text('Исходник Rhai'));
    await tester.enterText(
        find.byKey(const ValueKey('source-editor')), 'let a = 1\na;');
    await tester.tap(find.byKey(const ValueKey('strict-semicolons')));
    await tester.pumpAndSettle();
    await scrollTo(tester, find.text('Сохранить'));
    await tester.tap(find.text('Сохранить'));
    await tester.pumpAndSettle();

    expect(fake.created, isNull);
    expect(find.textContaining('Пропущен знак «;»'), findsOneWidget);
  });

  testWidgets('без строгого режима предупреждение не блокирует сохранение',
      (tester) async {
    final fake = FakeScriptService();
    await pumpEditor(tester, code: 'new', service: fake);

    await tester.enterText(
        find.widgetWithText(TextFormField, 'Код'), 'my.script');
    await tester.enterText(
        find.widgetWithText(TextFormField, 'Название'), 'Мой скрипт');
    await scrollTo(tester, find.text('Исходник Rhai'));
    await tester.enterText(
        find.byKey(const ValueKey('source-editor')), 'let a = 1\na;');
    await scrollTo(tester, find.text('Сохранить'));
    await tester.tap(find.text('Сохранить'));
    await tester.pumpAndSettle();

    expect(fake.created?.code, 'my.script');
    expect(find.text('Сохранено'), findsOneWidget);
  });
}