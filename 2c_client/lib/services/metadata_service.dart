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
  Future<EntitySchema> fetchSchema(String entityType) async {
    final cached = _schemaCache[entityType];
    if (cached != null) {
      return cached;
    }
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'metadata.schema.get',
      payload: {'code': entityType},
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
}

/// Провайдер сервиса метаданных (живёт весь сеанс, кэш в памяти).
final metadataServiceProvider = Provider<MetadataService>((ref) {
  final rpc = ref.watch(rpcClientProvider);
  return MetadataService(rpc);
});