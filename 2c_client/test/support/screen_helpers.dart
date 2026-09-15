import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:twoc_client/models/entity_schema.dart';
import 'package:twoc_client/models/object.dart';
import 'package:twoc_client/models/server_error.dart';
import 'package:twoc_client/providers/sdui_providers.dart';
import 'package:twoc_client/services/object_service.dart';

import 'fixtures.dart';

/// Фейковый сервис объектов для экранных тестов (внешняя зависимость RPC
/// не нужна — все методы возвращают предзаполненные данные).
class FakeObjectService implements ObjectService {
  FakeObjectService({Map<String, ObjectItem>? items}) : items = items ?? {};

  final Map<String, ObjectItem> items;
  ObjectItem? created;
  String? updatedId;
  int? updatedVersion;
  ServerError? throwOnUpdate;

  @override
  Future<List<ObjectItem>> list(String entityType, String? companyId) async =>
      items.values.toList();

  @override
  Future<ObjectItem> get(String id) async => items[id]!;

  @override
  Future<ObjectItem> create(
    String entityType,
    String kind,
    Map<String, dynamic> data, {
    String? companyId,
    String? state,
    String? date,
    String? parentId,
  }) async {
    created = ObjectItem(
      id: 'new-1',
      entityType: entityType,
      kind: kind,
      companyId: companyId ?? '',
      state: state ?? 'draft',
      data: data,
      computed: {},
      version: 1,
    );
    return created!;
  }

  @override
  Future<ObjectItem> update(
    String id,
    int expectedVersion, {
    Map<String, dynamic>? data,
    String? state,
    String? date,
    String? parentId,
  }) async {
    final error = throwOnUpdate;
    if (error != null) {
      throw error;
    }
    updatedId = id;
    updatedVersion = expectedVersion;
    final current = items[id]!;
    final updated = current.copyWith(data: data ?? current.data, state: state);
    items[id] = updated;
    return updated;
  }
}

/// Схема для экранных тестов (одно обязательное строковое поле).
EntitySchema testSchema({bool requiredName = true}) =>
    EntitySchema.fromJson(schemaWire(requiredName: requiredName));

/// Провайдеры для ObjectFormScreen-тестов.
List<Override> objectFormOverrides({
  required FakeObjectService objectService,
  EntitySchema? schema,
  String? companyId = 'c1',
}) {
  final s = schema ?? testSchema();
  return [
    schemaProvider.overrideWith((ref, entityType) async => s),
    objectServiceProvider.overrideWithValue(objectService),
    companyIdProvider.overrideWithValue(companyId),
  ];
}

/// Помощник перекачки экрана под ProviderScope.
Widget pumpSdui(Widget child, List<Override> overrides) => ProviderScope(
      overrides: overrides,
      child: MaterialApp(home: child),
    );