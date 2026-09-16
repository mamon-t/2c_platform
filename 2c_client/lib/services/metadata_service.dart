import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/entity_schema.dart';
import '../providers/app_providers.dart';
import '../services/rpc_client.dart';

/// Чтение метаданных платформы (ТЗ §7): каталог типов сущностей и схемы.
///
/// Данные кэшируются в памяти на время сессии; инвалидация не требуется,
/// пока метаданные не меняются из вне (Фаза 12+).
class MetadataService {
  MetadataService(this._rpc);

  final RpcClient _rpc;

  final Map<String, EntitySchema> _schemaCache = {};

  /// Запрашивает список всех типов сущностей (`metadata.entity_type.list`).
  Future<List<EntityType>> fetchEntityTypes() async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'metadata.entity_type.list',
      payload: const {},
    );
    final list = payload;
    if (list is! List) {
      throw const FormatException('metadata.entity_type.list: ожидался массив');
    }
    return list
        .map(
          (e) => EntityType.fromJson(
            (e as Map).cast<String, dynamic>(),
          ),
        )
        .toList();
  }

  /// Запрашивает полную схему типа (`metadata.schema.get`).
  ///
  /// Схема скоупится по компании актора: `company_id` берётся из JWT и
  /// передаётся в wire-запросе (сервер без него ищет по пустой компании).
  Future<EntitySchema> fetchSchema(String entityType, {String? companyId}) async {
    final cached = _schemaCache[entityType];
    if (cached != null) {
      return cached;
    }
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'metadata.schema.get',
      payload: {'code': entityType, 'company_id': ?companyId},
    );
    if (payload is! Map) {
      throw const FormatException('metadata.schema.get: ожидался объект');
    }
    final schema = EntitySchema.fromJson(
      payload.cast<String, dynamic>(),
    );
    _schemaCache[entityType] = schema;
    return schema;
  }

  /// По типу ищет метаданные в кэше (без сетевого запроса).
  EntitySchema? cachedSchema(String entityType) => _schemaCache[entityType];

  /// Полная очистка кэша схем.
  void clearCache() => _schemaCache.clear();

  /// Создаёт новый тип сущности (`metadata.entity_type.create`).
  ///
  /// Документ должен содержать `entity_type.code/name/kind/metadata_version`
  /// (что обеспечивает `EntitySchema.toJson`); `handle` событие
  /// `metadata.entity_type.created` пишет сам сервер.
  Future<void> createEntityType(Map<String, dynamic> schema) async {
    await _rpc.queryAny(
      module: 'core',
      action: 'metadata.entity_type.create',
      payload: schema,
    );
    final et = schema['entity_type'];
    if (et is Map && et['code'] is String) {
      _schemaCache.remove(et['code'] as String);
    }
  }

  /// Выгружает полную схему типа как JSON-документ (`metadata.export`).
  ///
  /// Возвращает wire-документ, пригодный для повторного `importMetadata`.
  Future<Map<String, dynamic>> exportMetadata(
    String entityType, {
    String? companyId,
  }) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'metadata.export',
      payload: {'code': entityType, 'company_id': ?companyId},
    );
    if (payload is! Map) {
      throw const FormatException('metadata.export: ожидался объект');
    }
    return payload.cast<String, dynamic>();
  }

  /// Импортирует схему из JSON-документа (`metadata.import`) с
  /// ensure-семантикой сервера: более новая `metadata_version` применяется,
  /// равная/более старая — игнорируется.
  Future<Map<String, dynamic>> importMetadata(
    Map<String, dynamic> schema,
  ) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'metadata.import',
      payload: schema,
    );
    if (payload is! Map) {
      throw const FormatException('metadata.import: ожидался объект');
    }
    final et = schema['entity_type'];
    if (et is Map && et['code'] is String) {
      _schemaCache.remove(et['code'] as String);
    }
    return payload.cast<String, dynamic>();
  }
}

/// Провайдер сервиса метаданных (живёт весь сеанс, кэш в памяти).
final metadataServiceProvider = Provider<MetadataService>((ref) {
  final rpc = ref.watch(rpcClientProvider);
  return MetadataService(rpc);
});