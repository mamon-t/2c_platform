import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/entity_schema.dart';
import 'package:twoc_client/models/object.dart';
import 'package:twoc_client/providers/sdui_providers.dart';
import 'package:twoc_client/screens/object_form_screen.dart';
import 'package:twoc_client/services/object_service.dart';

import '../support/fixtures.dart';
import '../support/screen_helpers.dart';
import '../support/test_router.dart';

void main() {
  final schema = EntitySchema.fromJson(schemaWire());
  final items = [
    ObjectItem.fromJson(
      objectWire(data: {'code': 'A001', 'name': 'Касса', 'balance': 1000}),
    ),
    ObjectItem.fromJson(
      objectWire(id: 'acc-2', data: {'code': 'A002', 'name': 'Расчёты', 'balance': -500}),
    ),
  ];

  Future<void> pumpCatalog(
    WidgetTester tester, {
    FakeObjectService? fake,
  }) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          schemaProvider.overrideWith((ref, entityType) async => schema),
          objectsProvider.overrideWith((ref, entityType) async => items),
          if (fake != null) objectServiceProvider.overrideWithValue(fake),
        ],
        child: MaterialApp.router(routerConfig: testRouter()),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets('показывает колонки схемы, значения и FAB', (tester) async {
    await pumpCatalog(tester);

    expect(find.text('Счет'), findsOneWidget); // AppBar title
    expect(find.text('Наименование'), findsOneWidget); // колонка
    expect(find.text('Касса'), findsOneWidget);
    expect(find.text('Расчёты'), findsOneWidget);
    expect(find.text('10,00'), findsOneWidget); // 1000 копеек
    expect(find.byType(FloatingActionButton), findsOneWidget);
  });

  testWidgets('тап по строке открывает форму просмотра', (tester) async {
    final fake = FakeObjectService(items: {for (final o in items) o.id: o});
    await pumpCatalog(tester, fake: fake);

    await tester.tap(find.text('Касса'));
    await tester.pumpAndSettle();

    expect(find.byType(ObjectFormScreen), findsOneWidget);
    expect(find.text('Сохранить'), findsNothing); // view-режим
  });

  testWidgets('FAB ведёт на форму создания', (tester) async {
    await pumpCatalog(tester);

    await tester.tap(find.byType(FloatingActionButton));
    await tester.pumpAndSettle();

    expect(find.byType(ObjectFormScreen), findsOneWidget);
    expect(find.text('Новый Счет'), findsOneWidget);
  });
}