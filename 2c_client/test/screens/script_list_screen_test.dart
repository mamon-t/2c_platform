import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/script.dart';
import 'package:twoc_client/providers/sdui_providers.dart';
import 'package:twoc_client/screens/script_editor_screen.dart';
import 'package:twoc_client/services/script_service.dart';

import '../support/fixtures.dart';
import '../support/screen_helpers.dart';
import '../support/test_router.dart';

void main() {
  final items = [
    ScriptItem.fromJson(scriptWire()),
    ScriptItem.fromJson(
      scriptWire(
        id: 's-2',
        code: 'check.positive',
        name: 'Проверка баланса',
        scriptType: 'validator',
        source: 'ctx.object.balance >= 0',
        entityType: 'account',
      ),
    ),
  ];

  Future<void> pumpScriptList(
    WidgetTester tester, {
    List<ScriptItem>? scripts,
    Object? error,
    FakeScriptService? scriptsService,
  }) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          scriptsProvider.overrideWith((ref) async {
            if (error != null) {
              throw error;
            }
            return scripts ?? items;
          }),
          companyIdProvider.overrideWithValue('c1'),
          if (scriptsService != null)
            scriptServiceProvider.overrideWithValue(scriptsService),
        ],
        child: MaterialApp.router(
          routerConfig: testRouter(initialLocation: '/scripts'),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets('показывает код, название, тип и статус скриптов', (tester) async {
    await pumpScriptList(tester);

    expect(find.text('Скрипты'), findsOneWidget); // AppBar
    expect(find.text('double.amount'), findsOneWidget);
    expect(find.text('Удвоить сумму'), findsOneWidget);
    expect(find.text('Формула'), findsOneWidget);
    expect(find.text('Валидатор'), findsOneWidget);
    expect(find.text('Активен'), findsNWidgets(2));
    expect(find.byType(FloatingActionButton), findsOneWidget);
  });

  testWidgets('пустой список — заглушка', (tester) async {
    await pumpScriptList(tester, scripts: []);

    expect(find.text('Нет скриптов'), findsOneWidget);
  });

  testWidgets('ошибка загрузки — сообщение', (tester) async {
    await pumpScriptList(tester, error: Exception('boom'));

    expect(find.textContaining('Не удалось загрузить скрипты'), findsOneWidget);
  });

  testWidgets('тап по строке ведёт в редактор с загруженным скриптом',
      (tester) async {
    final service = FakeScriptService(
      items: {for (final s in items) s.code: s},
    );
    await pumpScriptList(tester, scriptsService: service);

    await tester.tap(find.text('double.amount'));
    await tester.pumpAndSettle();

    expect(find.byType(ScriptEditorScreen), findsOneWidget);
    expect(find.text('Удвоить сумму'), findsWidgets); // AppBar + поле названия
    // поле «Исходник Rhai» заполнено существующим исходником
    expect(find.text('ctx.object.amount * ctx.args.factor'), findsOneWidget);
  });

  testWidgets('FAB ведёт на форму создания', (tester) async {
    await pumpScriptList(tester);

    await tester.tap(find.byType(FloatingActionButton));
    await tester.pumpAndSettle();

    expect(find.byType(ScriptEditorScreen), findsOneWidget);
    expect(find.text('Новый скрипт'), findsOneWidget);
  });
}