import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/entity_schema.dart';
import 'package:twoc_client/providers/sdui_providers.dart';
import 'package:twoc_client/screens/metadata_editor_screen.dart';
import 'package:twoc_client/services/metadata_service.dart';

class FakeMetadataService implements MetadataService {
  FakeMetadataService(this.schemas);

  final Map<String, EntitySchema> schemas;

  final List<String> createdCodes = [];
  final List<Map<String, dynamic>> imported = [];
  final List<String> exported = [];

  @override
  Future<List<EntityType>> fetchEntityTypes() async =>
      schemas.values.map((s) => s.entityType).toList();

  @override
  Future<EntitySchema> fetchSchema(String entityType,
      {String? companyId}) async {
    final schema = schemas[entityType];
    if (schema == null) {
      throw StateError('нет схемы $entityType');
    }
    return schema;
  }

  @override
  EntitySchema? cachedSchema(String entityType) => schemas[entityType];

  @override
  void clearCache() {}

  @override
  Future<void> createEntityType(Map<String, dynamic> schema) async {
    createdCodes.add((schema['entity_type'] as Map)['code'] as String);
  }

  @override
  Future<Map<String, dynamic>> exportMetadata(
    String entityType, {
    String? companyId,
  }) async {
    exported.add(entityType);
    return schemas[entityType]!.toJson();
  }

  @override
  Future<Map<String, dynamic>> importMetadata(
    Map<String, dynamic> schema,
  ) async {
    imported.add(schema);
    return schema;
  }
}

EntitySchema _invoiceSchema() {
  const type = EntityType(
    id: 't1',
    code: 'invoice',
    name: 'Счёт',
    kind: 'document',
    companyId: '',
    isSystem: false,
    metadataVersion: 3,
  );
  return const EntitySchema(
    entityType: type,
    fields: [
      EntityField(
        code: 'sum',
        label: 'Сумма',
        dataType: 'money',
        required: true,
        order: 1,
      ),
      EntityField(
        code: 'comment',
        label: 'Комментарий',
        dataType: 'text',
        required: false,
        order: 2,
      ),
    ],
    states: [
      EntityState(code: 'draft', label: 'Черновик', isInitial: true),
      EntityState(code: 'posted', label: 'Проведён', isFinal: true),
    ],
    transitions: [
      EntityTransition(
        code: 'post',
        label: 'Провести',
        fromState: 'draft',
        toState: 'posted',
      ),
    ],
    forms: [],
    actions: [],
    relations: [],
  );
}

List<Override> _editorOverrides(FakeMetadataService metadata) {
  return [
    metadataServiceProvider.overrideWithValue(metadata),
    schemasProvider.overrideWith((ref) async => metadata.fetchEntityTypes()),
    companyIdProvider.overrideWithValue('c1'),
  ];
}

Widget _pump(Widget child, List<Override> overrides) => ProviderScope(
      overrides: overrides,
      child: MaterialApp(home: child),
    );

void main() {
  testWidgets('редактор рендерит секции и строки схемы', (tester) async {
    final metadata = FakeMetadataService({'invoice': _invoiceSchema()});
    await tester.pumpWidget(
      _pump(
        const MetadataEditorScreen(entityType: 'invoice'),
        _editorOverrides(metadata),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Поля'), findsOneWidget);
    expect(find.text('Состояния'), findsOneWidget);
    expect(find.text('Переходы'), findsOneWidget);
    expect(find.text('sum — Сумма'), findsOneWidget);
    expect(find.text('comment — Комментарий'), findsOneWidget);
    expect(find.text('draft — Черновик'), findsOneWidget);
    expect(find.text('post · draft → posted'), findsOneWidget);
  });

  testWidgets('смена типа в дропдауне загружает другую схему', (tester) async {
    final metadata = FakeMetadataService({
      'invoice': _invoiceSchema(),
      'catalog': const EntitySchema(
        entityType: EntityType(
          code: 'catalog',
          name: 'Каталог',
          kind: 'catalog',
        ),
        fields: [
          EntityField(
            code: 'title',
            label: 'Заголовок',
            dataType: 'string',
            required: true,
            order: 1,
          ),
        ],
        states: [],
        transitions: [],
        forms: [],
        actions: [],
        relations: [],
      ),
    });
    await tester.pumpWidget(
      _pump(
        const MetadataEditorScreen(entityType: 'invoice'),
        _editorOverrides(metadata),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byType(DropdownButtonFormField<String?>));
    await tester.pumpAndSettle();
    await tester.tap(find.text('catalog').last);
    await tester.pumpAndSettle();

    expect(find.text('title — Заголовок'), findsOneWidget);
    expect(find.text('sum — Сумма'), findsNothing);
  });

  testWidgets('диалог добавления поля добавляет строку', (tester) async {
    final metadata = FakeMetadataService({'invoice': _invoiceSchema()});
    await tester.pumpWidget(
      _pump(
        const MetadataEditorScreen(entityType: 'invoice'),
        _editorOverrides(metadata),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.add_circle_outline).first);
    await tester.pumpAndSettle();
    await tester.enterText(find.widgetWithText(TextField, 'Код (code)'), 'vat');
    await tester.enterText(
      find.widgetWithText(TextField, 'Метка (label)'),
      'НДС',
    );
    await tester.tap(find.text('ОК'));
    await tester.pumpAndSettle();

    expect(find.text('vat — НДС'), findsOneWidget);
  });

  testWidgets('диалог удаления поля убирает строку', (tester) async {
    final metadata = FakeMetadataService({'invoice': _invoiceSchema()});
    await tester.pumpWidget(
      _pump(
        const MetadataEditorScreen(entityType: 'invoice'),
        _editorOverrides(metadata),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(
      find.descendant(
        of: find.widgetWithText(ListTile, 'sum — Сумма'),
        matching: find.byIcon(Icons.delete_outline),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Удалить'));
    await tester.pumpAndSettle();

    expect(find.text('sum — Сумма'), findsNothing);
    expect(find.text('comment — Комментарий'), findsOneWidget);
  });

  testWidgets('удаление состояния каскадно чистит переходы', (tester) async {
    final metadata = FakeMetadataService({'invoice': _invoiceSchema()});
    await tester.pumpWidget(
      _pump(
        const MetadataEditorScreen(entityType: 'invoice'),
        _editorOverrides(metadata),
      ),
    );
    await tester.pumpAndSettle();

    final removeButton = find.descendant(
      of: find.widgetWithText(ListTile, 'posted — Проведён'),
      matching: find.byIcon(Icons.delete_outline),
    );
    await tester.ensureVisible(removeButton);
    await tester.pumpAndSettle();
    await tester.tap(removeButton);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Удалить'));
    await tester.pumpAndSettle();

    expect(find.text('posted — Проведён'), findsNothing);
    expect(find.text('post · draft → posted'), findsNothing);
  });

  testWidgets('нельзя удалить последнее состояние', (tester) async {
    final metadata = FakeMetadataService({'invoice': _invoiceSchema()});
    final type = _invoiceSchema();
    final schema = type.copyWith(
      states: type.states.take(1).toList(),
      transitions: const [],
    );
    metadata.schemas['invoice'] = schema;
    await tester.pumpWidget(
      _pump(
        const MetadataEditorScreen(entityType: 'invoice'),
        _editorOverrides(metadata),
      ),
    );
    await tester.pumpAndSettle();

    final removeButton = find.descendant(
      of: find.widgetWithText(ListTile, 'draft — Черновик'),
      matching: find.byIcon(Icons.delete_outline),
    );
    await tester.ensureVisible(removeButton);
    await tester.pumpAndSettle();
    await tester.tap(removeButton);
    await tester.pumpAndSettle();

    expect(find.text('Нельзя удалить последнее состояние'), findsOneWidget);
  });

  testWidgets('сохранение повышает metadata_version и вызывает import',
      (tester) async {
    final metadata = FakeMetadataService({'invoice': _invoiceSchema()});
    await tester.pumpWidget(
      _pump(
        const MetadataEditorScreen(entityType: 'invoice'),
        _editorOverrides(metadata),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Сохранить'));
    await tester.pumpAndSettle();

    expect(metadata.imported, hasLength(1));
    final entityType = metadata.imported.single['entity_type'] as Map;
    expect(entityType['metadata_version'], 4);
    expect(metadata.imported.single['fields'], hasLength(2));
  });

  testWidgets('экспорт показывает JSON-диалог', (tester) async {
    final metadata = FakeMetadataService({'invoice': _invoiceSchema()});
    await tester.pumpWidget(
      _pump(
        const MetadataEditorScreen(entityType: 'invoice'),
        _editorOverrides(metadata),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Схема JSON'));
    await tester.pumpAndSettle();

    expect(metadata.exported, ['invoice']);
    expect(find.text('Схема JSON'), findsNWidgets(2));
    expect(find.byType(SelectableText), findsOneWidget);
    await tester.tap(find.text('Закрыть'));
    await tester.pumpAndSettle();
  });

  testWidgets('импорт JSON регистрирует схему через importMetadata',
      (tester) async {
    final metadata = FakeMetadataService({'invoice': _invoiceSchema()});
    await tester.pumpWidget(
      _pump(
        const MetadataEditorScreen(entityType: 'invoice'),
        _editorOverrides(metadata),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Импорт JSON'));
    await tester.pumpAndSettle();
    final doc = _invoiceSchema().toJson(metadataVersion: 9);
    await tester.enterText(
      find.byType(TextField).last,
      jsonEncode(doc),
    );
    await tester.tap(find.text('Импортировать'));
    await tester.pumpAndSettle();

    expect(metadata.imported, hasLength(1));
    expect(
      (metadata.imported.single['entity_type'] as Map)['code'],
      'invoice',
    );
  });

  testWidgets('диалог «Новый тип» создаёт тип', (tester) async {
    final metadata = FakeMetadataService({'invoice': _invoiceSchema()});
    await tester.pumpWidget(
      _pump(
        const MetadataEditorScreen(entityType: 'invoice'),
        _editorOverrides(metadata),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Новый тип'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.widgetWithText(TextField, 'Код (code)'),
      'contract',
    );
    await tester.enterText(
      find.widgetWithText(TextField, 'Наименование (name)'),
      'Договор',
    );
    await tester.tap(find.text('Создать'));
    await tester.pumpAndSettle();

    expect(metadata.createdCodes, ['contract']);
  });
}