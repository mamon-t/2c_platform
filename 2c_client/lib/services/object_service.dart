import '../models/object.dart';
import '../providers/app_providers.dart';
import '../services/rpc_client.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// CRUD универсальных объектов платформы (`object.create/get/list/update`).
///
/// Wire-контракт проверен по `apps/platform-server/src/commands.rs`;
/// `company_id` в запросах опускается, когда актор без компании (сервер
/// подставляет пустую строку).
class ObjectService {
  ObjectService(this._rpc);

  final RpcClient _rpc;

  /// Список объектов типа (`object.list`); без пагинации (сервер НЕ отдаёт
  /// offset, только limit с дефолтом 100).
  Future<List<ObjectItem>> list(String entityType, String? companyId) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'object.list',
      payload: {
        'entity_type': entityType,
        'company_id': ?companyId,
      },
    );
    final list = payload;
    if (list is! List) {
      throw const FormatException('object.list: ожидался массив');
    }
    return list
        .map((o) => ObjectItem.fromJson((o as Map).cast<String, dynamic>()))
        .toList();
  }

  /// Объект по идентификатору (`object.get`).
  Future<ObjectItem> get(String id) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'object.get',
      payload: {'id': id},
    );
    if (payload is! Map) {
      throw const FormatException('object.get: ожидался объект');
    }
    return ObjectItem.fromJson(payload.cast<String, dynamic>());
  }

  /// Создание объекта (`object.create`). `kind` обязателен сервером и
  /// берётся из метамодели типа.
  Future<ObjectItem> create(
    String entityType,
    String kind,
    Map<String, dynamic> data, {
    String? companyId,
    String? state,
    String? date,
    String? parentId,
  }) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'object.create',
      payload: {
        'entity_type': entityType,
        'kind': kind,
        'data': data,
        'company_id': ?companyId,
        'state': ?state,
        'date': ?date,
        'parent_id': ?parentId,
      },
    );
    if (payload is! Map) {
      throw const FormatException('object.create: ожидался объект');
    }
    return ObjectItem.fromJson(payload.cast<String, dynamic>());
  }

  /// Обновление объекта с OCC (`object.update`); `expectedVersion` обязателен.
  Future<ObjectItem> update(
    String id,
    int expectedVersion, {
    Map<String, dynamic>? data,
    String? state,
    String? date,
    String? parentId,
  }) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'object.update',
      payload: {
        'id': id,
        'expected_version': expectedVersion,
        'data': ?data,
        'state': ?state,
        'date': ?date,
        'parent_id': ?parentId,
      },
    );
    if (payload is! Map) {
      throw const FormatException('object.update: ожидался объект');
    }
    return ObjectItem.fromJson(payload.cast<String, dynamic>());
  }
}

/// Провайдер сервиса объектов.
final objectServiceProvider = Provider<ObjectService>((ref) {
  final rpc = ref.watch(rpcClientProvider);
  return ObjectService(rpc);
});