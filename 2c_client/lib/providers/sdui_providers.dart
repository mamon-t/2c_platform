import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/entity_schema.dart';
import '../models/navigation_item.dart';
import '../models/object.dart';
import '../providers/app_providers.dart';
import '../services/metadata_service.dart';
import '../services/object_service.dart';

/// Компания аутентифицированного актора из JWT-claims (`null` — без профиля).
final companyIdProvider = Provider<String?>((ref) {
  final auth = ref.watch(authServiceProvider);
  return auth.claims?.companyId;
});

/// Список всех типов сущностей (один запрос на сессию).
final schemasProvider = FutureProvider<List<EntityType>>((ref) async {
  final metadata = ref.watch(metadataServiceProvider);
  return metadata.fetchEntityTypes();
});

/// Схема конкретного типа (кэшируется в `MetadataService`).
///
/// `company_id` берётся из claims актора и передаётся в `metadata.schema.get`:
/// без него сервер ищет схему в пустой компании (NOT_FOUND на реальном API).
final schemaProvider =
    FutureProvider.family<EntitySchema, String>((ref, entityType) async {
  final metadata = ref.watch(metadataServiceProvider);
  final companyId = ref.watch(companyIdProvider);
  return metadata.fetchSchema(entityType, companyId: companyId);
});

/// Список объектов типа `entityType` (перезапрашивается при invalidate).
final objectsProvider = FutureProvider.family<List<ObjectItem>, String>(
  (ref, entityType) async {
    final objects = ref.watch(objectServiceProvider);
    final companyId = ref.watch(companyIdProvider);
    return objects.list(entityType, companyId);
  },
);

/// Навигация из манифестов установленных модулей (`module.navigation`).
final navigationProvider = FutureProvider<List<ModuleNavigation>>((ref) async {
  final rpc = ref.watch(rpcClientProvider);
  final payload = await rpc.queryAny(
    module: 'core',
    action: 'module.navigation',
    payload: const {},
  );
  if (payload is! Map) {
    throw const FormatException('module.navigation: ожидался объект');
  }
  final modules = payload['modules'];
  if (modules is! List) {
    throw const FormatException('module.navigation: ожидался список modules');
  }
  return modules
      .map((m) => ModuleNavigation.fromJson((m as Map).cast<String, dynamic>()))
      .toList();
});