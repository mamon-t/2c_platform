import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:twoc_client/models/entity_schema.dart';
import 'package:twoc_client/models/object.dart';
import 'package:twoc_client/models/script.dart';
import 'package:twoc_client/models/server_error.dart';
import 'package:twoc_client/providers/app_providers.dart';
import 'package:twoc_client/providers/sdui_providers.dart';
import 'package:twoc_client/services/auth_service.dart';
import 'package:twoc_client/services/object_service.dart';
import 'package:twoc_client/services/rpc_client.dart';
import 'package:twoc_client/services/script_service.dart';

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

/// Новое in-memory token-хранилище для экранных тестов (без libsecret).
FlutterSecureStorage testSecureStorage() => _MemoryStorage();

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

/// Фейковый сервис скриптов для экранных тестов (без RPC).
class FakeScriptService implements ScriptService {
  FakeScriptService({Map<String, ScriptItem>? items}) : items = items ?? {};

  final Map<String, ScriptItem> items;
  ScriptItem? created;
  String? updatedCode;
  String? deletedCode;
  ScriptValidation? validation;
  Map<String, dynamic>? testResult;
  ServerError? throwOnValidate;
  ServerError? throwOnCreate;
  ServerError? throwOnGet;
  ServerError? throwOnTest;

  @override
  Future<List<ScriptItem>> list({String? companyId}) async => items.values
      .where((s) =>
          companyId == null || s.companyId == null || s.companyId == companyId)
      .toList();

  @override
  Future<ScriptItem> get(String code, {String? companyId}) async {
    final error = throwOnGet;
    if (error != null) {
      throw error;
    }
    final item = items[code];
    if (item == null) {
      throw const ServerError(ErrorCode.notFound, 'Объект не найден');
    }
    return item;
  }

  @override
  Future<ScriptItem> create({
    required String code,
    required String name,
    required String source,
    String scriptType = 'formula',
    String? companyId,
    String? entityType,
    bool isActive = true,
  }) async {
    final error = throwOnCreate;
    if (error != null) {
      throw error;
    }
    created = ScriptItem(
      id: 'new-$code',
      code: code,
      name: name,
      scriptType: scriptType,
      source: source,
      companyId: companyId,
      entityType: entityType,
      isActive: isActive,
      createdAt: '2026-09-17T10:00:00Z',
      updatedAt: '2026-09-17T10:00:00Z',
    );
    items[code] = created!;
    return created!;
  }

  @override
  Future<ScriptItem> update(
    String code, {
    String? companyId,
    String? name,
    String? source,
    String? scriptType,
    String? entityType,
    bool? isActive,
  }) async {
    updatedCode = code;
    final current = items[code]!;
    final updated = ScriptItem(
      id: current.id,
      code: current.code,
      name: name ?? current.name,
      scriptType: scriptType ?? current.scriptType,
      source: source ?? current.source,
      companyId: companyId ?? current.companyId,
      entityType: entityType ?? current.entityType,
      isActive: isActive ?? current.isActive,
      createdAt: current.createdAt,
      updatedAt: '2026-09-17T11:00:00Z',
    );
    items[code] = updated;
    return updated;
  }

  @override
  Future<void> delete(String code, {String? companyId}) async {
    deletedCode = code;
    items.remove(code);
  }

  @override
  Future<ScriptValidation> validate(String source) async {
    final error = throwOnValidate;
    if (error != null) {
      throw error;
    }
    return validation ?? const ScriptValidation(valid: true, errors: []);
  }

  @override
  Future<Map<String, dynamic>> testRun(
    String code, {
    Map<String, dynamic>? args,
  }) async {
    final error = throwOnTest;
    if (error != null) {
      throw error;
    }
    return testResult ?? {'result': 42, 'execution_time_ms': 3};
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