import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/object.dart';
import 'package:twoc_client/models/server_error.dart';
import 'package:twoc_client/screens/object_form_screen.dart';

import '../support/fixtures.dart';
import '../support/screen_helpers.dart';
import '../support/test_router.dart';

void main() {
  final obj = ObjectItem.fromJson(
    objectWire(data: {'name': 'Касса', 'balance': 1234}),
  );

  Future<void> pumpForm(
    WidgetTester tester, {
    required List<Override> overrides,
    String initialLocation = '/object/account/new',
  }) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: overrides,
        child: MaterialApp.router(
          routerConfig: testRouter(initialLocation: initialLocation),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets('create: валидация required, затем создание с kind из схемы',
      (tester) async {
    final fake = FakeObjectService();
    await pumpForm(tester, overrides: objectFormOverrides(objectService: fake));

    expect(find.text('Новый Счет'), findsOneWidget);

    await tester.tap(find.widgetWithText(ElevatedButton, 'Сохранить'));
    await tester.pump();
    await tester.pump();
    expect(find.textContaining('Заполните обязательные поля'), findsOneWidget);
    expect(fake.created, isNull);

    await tester.enterText(find.byType(TextFormField).at(1), 'Касса');
    await tester.tap(find.widgetWithText(ElevatedButton, 'Сохранить'));
    await tester.pumpAndSettle();

    final created = fake.created;
    expect(created, isNotNull);
    expect(created!.entityType, 'account');
    expect(created.kind, 'catalog');
    expect(created.data, {'name': 'Касса', 'is_closed': false});
    expect(created.companyId, 'c1');
  });

  testWidgets('edit: предзаполнение и обновление с expected_version',
      (tester) async {
    final fake = FakeObjectService(items: {obj.id: obj});
    await pumpForm(
      tester,
      overrides: objectFormOverrides(objectService: fake),
      initialLocation: '/object/account/acc-1/edit',
    );

    expect(find.text('12,34'), findsOneWidget);
    expect(find.text('Касса'), findsOneWidget);

    await tester.enterText(find.byType(TextFormField).at(1), 'Касса-новая');
    await tester.tap(find.widgetWithText(ElevatedButton, 'Сохранить'));
    await tester.pumpAndSettle();

    expect(fake.updatedId, 'acc-1');
    expect(fake.updatedVersion, 1);
    expect(fake.items['acc-1']!.data['name'], 'Касса-новая');
  });

  testWidgets('view: read-only, без кнопки сохранения, с переходом в edit',
      (tester) async {
    final fake = FakeObjectService(items: {obj.id: obj});
    await pumpForm(
      tester,
      overrides: objectFormOverrides(objectService: fake),
      initialLocation: '/object/account/acc-1',
    );

    expect(find.text('Счет'), findsOneWidget);
    expect(find.byType(TextFormField), findsWidgets);
    expect(find.text('Сохранить'), findsNothing);

    await tester.tap(find.byIcon(Icons.edit));
    await tester.pumpAndSettle();
    expect(find.byType(ObjectFormScreen), findsOneWidget);
    expect(find.text('Сохранить'), findsOneWidget);
  });

  testWidgets('конфликт версий при сохранении → AlertDialog', (tester) async {
    final fake = FakeObjectService(items: {obj.id: obj});
    fake.throwOnUpdate = const ServerError(ErrorCode.conflict, 'Конфликт версий');
    await pumpForm(
      tester,
      overrides: objectFormOverrides(objectService: fake),
      initialLocation: '/object/account/acc-1/edit',
    );

    await tester.enterText(find.byType(TextFormField).at(1), 'X');
    await tester.tap(find.widgetWithText(ElevatedButton, 'Сохранить'));
    await tester.pumpAndSettle();

    expect(find.byType(AlertDialog), findsOneWidget);
    expect(find.text('Конфликт версий'), findsWidgets);
  });
}