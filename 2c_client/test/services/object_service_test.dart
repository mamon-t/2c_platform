import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:twoc_client/services/object_service.dart';
import 'package:twoc_client/services/rpc_client.dart';

import '../support/fixtures.dart';

class _MockRpcClient extends Mock implements RpcClient {}

void main() {
  late _MockRpcClient rpc;
  late ObjectService service;

  setUp(() {
    rpc = _MockRpcClient();
    service = ObjectService(rpc);
  });

  test('list без company_id опускает поле', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'object.list',
        payload: const {'entity_type': 'account'},
      ),
    ).thenAnswer((_) async => [objectWire()]);

    final items = await service.list('account', null);

    expect(items, hasLength(1));
    expect(items.single.id, 'acc-1');
    verify(
      () => rpc.queryAny(
        module: 'core',
        action: 'object.list',
        payload: const {'entity_type': 'account'},
      ),
    ).called(1);
  });

  test('list передаёт company_id', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'object.list',
        payload: const {'entity_type': 'account', 'company_id': 'c1'},
      ),
    ).thenAnswer((_) async => [objectWire()]);

    await service.list('account', 'c1');

    verify(
      () => rpc.queryAny(
        module: 'core',
        action: 'object.list',
        payload: const {'entity_type': 'account', 'company_id': 'c1'},
      ),
    ).called(1);
  });

  test('get передаёт id', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'object.get',
        payload: const {'id': 'acc-1'},
      ),
    ).thenAnswer((_) async => objectWire(data: {'name': 'Касса'}));

    final obj = await service.get('acc-1');

    expect(obj.data, {'name': 'Касса'});
    verify(
      () => rpc.queryAny(
        module: 'core',
        action: 'object.get',
        payload: const {'id': 'acc-1'},
      ),
    ).called(1);
  });

  test('create передаёт все поля и kind', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'object.create',
        payload: const {
          'entity_type': 'account',
          'kind': 'catalog',
          'data': {'name': 'Касса'},
          'company_id': 'c1',
          'state': 'draft',
          'date': '2026-09-15',
          'parent_id': 'acc-0',
        },
      ),
    ).thenAnswer((_) async => objectWire());

    final obj = await service.create(
      'account',
      'catalog',
      {'name': 'Касса'},
      companyId: 'c1',
      state: 'draft',
      date: '2026-09-15',
      parentId: 'acc-0',
    );

    expect(obj.id, 'acc-1');
  });

  test('create без необязательных полей не включает их', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'object.create',
        payload: const {
          'entity_type': 'account',
          'kind': 'catalog',
          'data': {'name': 'Касса'},
        },
      ),
    ).thenAnswer((_) async => objectWire());

    await service.create('account', 'catalog', {'name': 'Касса'});

    verify(
      () => rpc.queryAny(
        module: 'core',
        action: 'object.create',
        payload: const {
          'entity_type': 'account',
          'kind': 'catalog',
          'data': {'name': 'Касса'},
        },
      ),
    ).called(1);
  });

  test('update передаёт expected_version и data', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'object.update',
        payload: const {
          'id': 'acc-1',
          'expected_version': 3,
          'data': {'name': 'Новое имя'},
        },
      ),
    ).thenAnswer((_) async => objectWire(version: 4));

    final obj = await service.update('acc-1', 3, data: {'name': 'Новое имя'});

    expect(obj.version, 4);
  });
}