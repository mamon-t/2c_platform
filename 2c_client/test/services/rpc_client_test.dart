import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:twoc_client/models/rpc_message.dart';
import 'package:twoc_client/models/server_error.dart';
import 'package:twoc_client/services/rpc_client.dart';

void main() {
  late MockClient httpMock;
  late RpcClient client;

  http.Response serverResponse(Map<String, dynamic> body) =>
      http.Response(jsonEncode(body), 200,
          headers: {'content-type': 'application/json'});

  setUp(() {
    httpMock = MockClient((request) async {
      if (request.url.path != '/rpc') {
        return http.Response('', 404);
      }
      return serverResponse({
        'type': 'error',
        'id': '0',
        'code': 'VALIDATION_ERROR',
        'message': 'unexpected',
      });
    });
    client = RpcClient(baseUrl: 'http://127.0.0.1:8080', httpClient: httpMock);
  });

  tearDown(() => client.dispose());

  test('command success returns payload and echoes request id', () async {
    httpMock = MockClient((request) async {
      final body = jsonDecode(request.body) as Map<String, dynamic>;
      expect(body['type'], 'command');
      expect(body['module'], 'core');
      expect(body['action'], 'company.create');
      expect(request.headers['content-type'], 'application/json');
      return serverResponse({
        'type': 'response',
        'id': body['id'],
        'payload': {'code': 'x', 'name': 'X'},
      });
    });
    client = RpcClient(baseUrl: 'http://127.0.0.1:8080', httpClient: httpMock);

    final payload = await client.command(
      module: 'core',
      action: 'company.create',
      payload: {'code': 'x', 'name': 'X'},
    );
    expect(payload['code'], 'x');
  });

  test('query success returns payload', () async {
    httpMock = MockClient((request) async {
      final body = jsonDecode(request.body) as Map<String, dynamic>;
      expect(body['type'], 'query');
      return serverResponse({
        'type': 'response',
        'id': body['id'],
        'payload': {'items': []},
      });
    });
    client = RpcClient(baseUrl: 'http://127.0.0.1:8080', httpClient: httpMock);

    final payload = await client.query(module: 'core', action: 'company.list');
    expect(payload['items'], isEmpty);
  });

  test('server error maps to ServerError', () async {
    httpMock = MockClient((request) async {
      final body = jsonDecode(request.body) as Map<String, dynamic>;
      return serverResponse({
        'type': 'error',
        'id': body['id'],
        'code': 'PERMISSION_ERROR',
        'message': 'недостаточно прав',
      });
    });
    client = RpcClient(baseUrl: 'http://127.0.0.1:8080', httpClient: httpMock);

    await expectLater(
      client.command(module: 'core', action: 'user.profile.get'),
      throwsA(
        isA<ServerError>()
            .having((e) => e.code, 'code', ErrorCode.permission),
      ),
    );
  });

  test('network timeout maps to ServerError.network', () async {
    httpMock = MockClient((request) async {
      await Future<void>.delayed(const Duration(milliseconds: 200));
      return serverResponse(const {});
    });
    client = RpcClient(
      baseUrl: 'http://127.0.0.1:8080',
      httpClient: httpMock,
      requestTimeout: const Duration(milliseconds: 20),
    );

    await expectLater(
      client.command(module: 'core', action: 'company.list'),
      throwsA(
        isA<ServerError>().having((e) => e.code, 'code', ErrorCode.network),
      ),
    );
  });

  test('non-JSON body maps to ServerError.network', () async {
    httpMock = MockClient((request) async => http.Response('<html>', 500));
    client = RpcClient(baseUrl: 'http://127.0.0.1:8080', httpClient: httpMock);

    await expectLater(
      client.command(module: 'core', action: 'company.list'),
      throwsA(isA<ServerError>()),
    );
  });

  test('adds Authorization header when token is set', () async {
    httpMock = MockClient((request) async {
      final body = jsonDecode(request.body) as Map<String, dynamic>;
      expect(request.headers['authorization'], 'Bearer abc.def.ghi');
      return serverResponse({
        'type': 'response',
        'id': body['id'],
        'payload': const {},
      });
    });
    client = RpcClient(baseUrl: 'http://127.0.0.1:8080', httpClient: httpMock);
    client.token = 'abc.def.ghi';

    await client.query(module: 'core', action: 'company.list');
  });

  test('server_push from server is rejected as invalid', () async {
    httpMock = MockClient((request) async {
      final body = jsonDecode(request.body) as Map<String, dynamic>;
      return serverResponse({
        'type': 'server_push',
        'id': body['id'],
        'module': 'core',
        'event_type': 'object.created',
        'payload': const {},
      });
    });
    client = RpcClient(baseUrl: 'http://127.0.0.1:8080', httpClient: httpMock);

    await expectLater(
      client.command(module: 'core', action: 'company.list'),
      throwsA(isA<ServerError>()),
    );
  });

  test('RpcMessage.fromJson works for response', () {
    final msg = RpcMessage.fromJson({
      'type': 'response',
      'id': '1',
      'payload': const {'ok': true},
    });
    expect(msg, isA<RpcMessageResponse>());
  });
}