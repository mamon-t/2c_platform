import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:twoc_client/services/metadata_service.dart';
import 'package:twoc_client/services/rpc_client.dart';

import '../support/fixtures.dart';

class _MockRpcClient extends Mock implements RpcClient {}

void main() {
  late _MockRpcClient rpc;
  late MetadataService service;

  setUp(() {
    rpc = _MockRpcClient();
    service = MetadataService(rpc);
  });

  test('fetchEntityTypes вызывает metadata.entity_type.list', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'metadata.entity_type.list',
        payload: const {},
      ),
    ).thenAnswer((_) async => entityTypesWire());

    final types = await service.fetchEntityTypes();

    expect(types, hasLength(2));
    expect(types.first.code, 'account');
    expect(types.first.kind, 'catalog');
    verify(
      () => rpc.queryAny(
        module: 'core',
        action: 'metadata.entity_type.list',
        payload: const {},
      ),
    ).called(1);
  });

  test('fetchSchema запрашивает metadata.schema.get и кэширует результат',
      () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'metadata.schema.get',
        payload: const {'code': 'account'},
      ),
    ).thenAnswer((_) async => schemaWire());

    final first = await service.fetchSchema('account');
    final second = await service.fetchSchema('account');

    expect(first.entityType.code, 'account');
    expect(identical(first, second), isTrue);
    verify(
      () => rpc.queryAny(
        module: 'core',
        action: 'metadata.schema.get',
        payload: const {'code': 'account'},
      ),
    ).called(1);
  });

  test('cachedSchema и clearCache', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'metadata.schema.get',
        payload: const {'code': 'account'},
      ),
    ).thenAnswer((_) async => schemaWire());

    expect(service.cachedSchema('account'), isNull);
    await service.fetchSchema('account');
    expect(service.cachedSchema('account'), isNotNull);

    service.clearCache();
    expect(service.cachedSchema('account'), isNull);
  });
}