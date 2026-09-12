import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/server_error.dart';

void main() {
  group('ErrorCode wire mapping', () {
    test('matches server error codes', () {
      expect(ErrorCode.fromWire('NOT_FOUND_ERROR'), ErrorCode.notFound);
      expect(ErrorCode.fromWire('CONFLICT_ERROR'), ErrorCode.conflict);
      expect(ErrorCode.fromWire('PERMISSION_ERROR'), ErrorCode.permission);
      expect(ErrorCode.fromWire('VALIDATION_ERROR'), ErrorCode.validation);
      expect(ErrorCode.fromWire('STORAGE_ERROR'), ErrorCode.storage);
    });

    test('unknown codes map to unknown', () {
      expect(ErrorCode.fromWire('SOMETHING_ELSE'), ErrorCode.unknown);
    });

    test('wireValue round-trip', () {
      for (final code in ErrorCode.values) {
        expect(ErrorCode.fromWire(code.wireValue), code);
      }
    });
  });

  group('ServerError', () {
    test('localizedMessage keeps server message when present', () {
      const e = ServerError(
        ErrorCode.validation,
        'неверный логин или пароль',
      );
      expect(e.localizedMessage, 'неверный логин или пароль');
    });

    test('localizedMessage falls back to RU text per code', () {
      expect(const ServerError(ErrorCode.notFound, '').localizedMessage, 'Объект не найден');
      expect(
        const ServerError(ErrorCode.permission, '').localizedMessage,
        'Недостаточно прав',
      );
      expect(
        const ServerError(ErrorCode.network, '').localizedMessage,
        'Нет соединения с сервером',
      );
    });

    test('isAuthRequired', () {
      expect(
        const ServerError(ErrorCode.authRequired, '').isAuthRequired,
        isTrue,
      );
      expect(
        const ServerError(ErrorCode.permission, '').isAuthRequired,
        isFalse,
      );
    });
  });
}