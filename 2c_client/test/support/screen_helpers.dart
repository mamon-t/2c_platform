import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:twoc_client/models/entity_schema.dart';
import 'package:twoc_client/models/object.dart';
import 'package:twoc_client/models/server_error.dart';
import 'package:twoc_client/providers/app_providers.dart';
import 'package:twoc_client/providers/sdui_providers.dart';
import 'package:twoc_client/services/auth_service.dart';
import 'package:twoc_client/services/object_service.dart';
import 'package:twoc_client/services/rpc_client.dart';

import 'fixtures.dart';

/// In-memory токен-хранилище для тестов (без libsecret на машине).
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
    // Сетевые провайдеры подписаны на authControllerProvider (через
    // _authRevision); восстанавливаю реальный контроллер, но с пустым
    // хранилищем — сессия не восстанавливается, не нужен ни libsecret,
    // ни appConfigProvider.
    authServiceProvider.overrideWith(
      (ref) => AuthService(
        rpcClient: RpcClient(baseUrl: 'http://127.0.0.1:8080'),
        storage: _MemoryStorage(),
      ),
    ),
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