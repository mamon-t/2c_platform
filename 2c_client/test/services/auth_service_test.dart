import 'dart:convert';

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:twoc_client/models/server_error.dart';
import 'package:twoc_client/services/auth_service.dart';
import 'package:twoc_client/services/rpc_client.dart';

class _MockRpcClient extends Mock implements RpcClient {}

class _MockStorage extends Mock implements FlutterSecureStorage {}

String _jwt({required int expSeconds}) {
  String b64(Map<String, dynamic> m) =>
      base64Url.encode(utf8.encode(jsonEncode(m))).replaceAll('=', '');
  final now = DateTime.now().toUtc().millisecondsSinceEpoch ~/ 1000;
  return '${b64({'alg': 'HS256'})}.${b64({
    'sub': 'u:1',
    'login': 'admin',
    'full_name': 'Админ',
    'position': 'Admin',
    'exp': expSeconds,
    'iat': now,
  })}.sig';
}

void main() {
  late _MockRpcClient rpc;
  late _MockStorage storage;
  late AuthService service;

  setUp(() {
    rpc = _MockRpcClient();
    storage = _MockStorage();
    when(() => storage.write(key: any(named: 'key'), value: any(named: 'value')))
        .thenAnswer((_) async {});
    when(() => storage.delete(key: any(named: 'key')))
        .thenAnswer((_) async {});
    service = AuthService(rpcClient: rpc, storage: storage);
  });

  group('login', () {
    test('persists token and adopts claims', () async {
      final raw = _jwt(expSeconds: DateTime.now().toUtc().add(const Duration(hours: 8)).millisecondsSinceEpoch ~/ 1000);
      when(
        () => rpc.command(
          module: 'core',
          action: 'user.login',
          payload: {'login': 'admin', 'password': 'secret'},
        ),
      ).thenAnswer((_) async => {
        'access_token': raw,
        'expires_at': '2026-09-11T12:00:00.000Z',
        'token_type': 'Bearer',
      });

      final token = await service.login('admin', 'secret');

      expect(token.accessToken, raw);
      expect(service.token, raw);
      expect(service.claims?.login, 'admin');
      expect(service.claims?.fullName, 'Админ');
      verify(
        () => storage.write(key: 'access_token', value: raw),
      ).called(1);
    });

    test('propagates ServerError on wrong credentials', () async {
      when(
        () => rpc.command(
          module: 'core',
          action: 'user.login',
          payload: any(named: 'payload'),
        ),
      ).thenThrow(const ServerError(ErrorCode.validation, 'неверный логин или пароль'));

      await expectLater(
        service.login('admin', 'nope'),
        throwsA(
          isA<ServerError>()
              .having((e) => e.code, 'code', ErrorCode.validation),
        ),
      );
    });
  });

  group('logout', () {
    test('clears local state even if server call fails', () async {
      when(
        () => rpc.command(module: 'core', action: 'user.logout'),
      ).thenThrow(const ServerError(ErrorCode.network, 'нет связи'));

      await service.logout();

      expect(service.token, isNull);
      verify(() => storage.delete(key: 'access_token')).called(1);
      verify(() => storage.delete(key: 'expires_at')).called(1);
    });
  });

  group('restoreSession', () {
    test('returns null when nothing saved', () async {
      when(() => storage.read(key: 'access_token'))
          .thenAnswer((_) async => null);

      expect(await service.restoreSession(), isNull);
    });

    test('adopts valid saved token', () async {
      final raw = _jwt(expSeconds: DateTime.now().toUtc().add(const Duration(hours: 8)).millisecondsSinceEpoch ~/ 1000);
      when(() => storage.read(key: 'access_token'))
          .thenAnswer((_) async => raw);

      final restored = await service.restoreSession();

      expect(restored, raw);
      expect(service.token, raw);
    });

    test('throws authRequired and clears storage when expired', () async {
      final expired =
          _jwt(expSeconds: DateTime.now().toUtc().subtract(const Duration(hours: 1)).millisecondsSinceEpoch ~/ 1000);
      when(() => storage.read(key: 'access_token'))
          .thenAnswer((_) async => expired);

      await expectLater(
        service.restoreSession(),
        throwsA(
          isA<ServerError>()
              .having((e) => e.isAuthRequired, 'isAuthRequired', isTrue),
        ),
      );
      expect(service.token, isNull);
      verify(() => storage.delete(key: 'access_token')).called(1);
      verify(() => storage.delete(key: 'expires_at')).called(1);
    });
  });

  group('refresh', () {
    test('is a hard seam until Phase 12', () {
      expect(
        () => service.refresh('stale-token'),
        throwsA(
          isA<ServerError>()
              .having((e) => e.code, 'code', ErrorCode.authRequired),
        ),
      );
    });
  });
}