import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:twoc_client/services/rpc_client.dart';
import 'package:twoc_client/services/script_service.dart';

import '../support/fixtures.dart';

class _MockRpcClient extends Mock implements RpcClient {}

void main() {
  late _MockRpcClient rpc;
  late ScriptService service;

  setUp(() {
    rpc = _MockRpcClient();
    service = ScriptService(rpc);
  });

  test('list вызывает script.list и парсит массив', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.list',
        payload: const {},
      ),
    ).thenAnswer((_) async => [scriptWire()]);

    final items = await service.list();

    expect(items, hasLength(1));
    expect(items.first.code, 'double.amount');
    verify(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.list',
        payload: const {},
      ),
    ).called(1);
  });

  test('list передаёт company_id', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.list',
        payload: const {'company_id': 'c1'},
      ),
    ).thenAnswer((_) async => [scriptWire()]);

    await service.list(companyId: 'c1');

    verify(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.list',
        payload: const {'company_id': 'c1'},
      ),
    ).called(1);
  });

  test('list с не-массивом бросает FormatException', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.list',
        payload: const {},
      ),
    ).thenAnswer((_) async => <String, dynamic>{});

    expect(service.list(), throwsFormatException);
  });

  test('get вызывает script.get по коду', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.get',
        payload: const {'code': 'double.amount', 'company_id': 'c1'},
      ),
    ).thenAnswer((_) async => scriptWire());

    final item = await service.get('double.amount', companyId: 'c1');

    expect(item.name, 'Удвоить сумму');
    verify(() => rpc.queryAny(
          module: 'core',
          action: 'script.get',
          payload: const {'code': 'double.amount', 'company_id': 'c1'},
        )).called(1);
  });

  test('create отправляет полный payload', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.create',
        payload: const {
          'code': 'sum',
          'name': 'Сумма',
          'source': 'a + b',
          'script_type': 'formula',
          'is_active': true,
          'company_id': 'c1',
          'entity_type': 'invoice',
        },
      ),
    ).thenAnswer((_) async => scriptWire(code: 'sum', name: 'Сумма'));

    final created = await service.create(
      code: 'sum',
      name: 'Сумма',
      source: 'a + b',
      companyId: 'c1',
      entityType: 'invoice',
    );

    expect(created.code, 'sum');
    verify(() => rpc.queryAny(
          module: 'core',
          action: 'script.create',
          payload: const {
            'code': 'sum',
            'name': 'Сумма',
            'source': 'a + b',
            'script_type': 'formula',
            'is_active': true,
            'company_id': 'c1',
            'entity_type': 'invoice',
          },
        )).called(1);
  });

  test('update шлёт только переданные поля', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.update',
        payload: const {'code': 'sum', 'name': 'Новое имя'},
      ),
    ).thenAnswer((_) async => scriptWire(code: 'sum', name: 'Новое имя'));

    await service.update('sum', name: 'Новое имя');

    verify(() => rpc.queryAny(
          module: 'core',
          action: 'script.update',
          payload: const {'code': 'sum', 'name': 'Новое имя'},
        )).called(1);
  });

  test('delete вызывает script.delete', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.delete',
        payload: const {'code': 'sum', 'company_id': 'c1'},
      ),
    ).thenAnswer((_) async => const <String, dynamic>{});

    await service.delete('sum', companyId: 'c1');

    verify(() => rpc.queryAny(
          module: 'core',
          action: 'script.delete',
          payload: const {'code': 'sum', 'company_id': 'c1'},
        )).called(1);
  });

  test('validate парсит структурный ответ', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.validate',
        payload: const {'source': 'let x = ;'},
      ),
    ).thenAnswer(
      (_) async => {
        'valid': false,
        'errors': [
          {'line': 1, 'column': 8, 'message': 'Ожидалось выражение'},
        ],
      },
    );

    final result = await service.validate('let x = ;');

    expect(result.valid, isFalse);
    expect(result.errors, hasLength(1));
    expect(result.errors.first.line, 1);
    expect(result.errors.first.column, 8);
    expect(result.errors.first.message, 'Ожидалось выражение');
  });

  test('testRun возвращает {result, execution_time_ms}', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.test',
        payload: const {
          'code': 'sum',
          'args': {'x': 20, 'y': 22},
        },
      ),
    ).thenAnswer(
      (_) async => {'result': 42, 'execution_time_ms': 3},
    );

    final result = await service.testRun('sum', args: {'x': 20, 'y': 22});

    expect(result['result'], 42);
    expect(result['execution_time_ms'], 3);
  });

  test('testRun без args опускает ключ args', () async {
    when(
      () => rpc.queryAny(
        module: 'core',
        action: 'script.test',
        payload: const {'code': 'sum'},
      ),
    ).thenAnswer((_) async => {'result': 42, 'execution_time_ms': 2});

    await service.testRun('sum');

    verify(() => rpc.queryAny(
          module: 'core',
          action: 'script.test',
          payload: const {'code': 'sum'},
        )).called(1);
  });
}