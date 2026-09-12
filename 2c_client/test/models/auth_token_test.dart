import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/auth_token.dart';

String _b64Url(Map<String, dynamic> payload) {
  final json = utf8.encode(jsonEncode(payload));
  return base64Url.encode(json).replaceAll('=', '');
}

void main() {
  group('AuthToken', () {
    test('fromJson parses access_token/expires_at/token_type', () {
      final token = AuthToken.fromJson({
        'access_token': 'abc',
        'expires_at': '2026-09-11T12:00:00.000Z',
        'token_type': 'Bearer',
      });
      expect(token.accessToken, 'abc');
      expect(token.tokenType, 'Bearer');
      expect(token.expiresAt.isUtc, isTrue);
    });
  });

  group('JwtClaims', () {
    test('tryParse unpacks HS256 payload', () {
      final now = DateTime.now().toUtc();
      final exp = now.add(const Duration(hours: 8)).millisecondsSinceEpoch ~/ 1000;
      final payload = {
        'sub': 'u:1',
        'login': 'admin',
        'full_name': 'Админ',
        'position': 'Admin',
        'company_id': 'c:1',
        'exp': exp,
        'iat': now.millisecondsSinceEpoch ~/ 1000,
      };
      final token =
          '${_b64Url({'alg': 'HS256'})}.${_b64Url(payload)}.signature';
      final claims = JwtClaims.tryParse(token);
      expect(claims, isNotNull);
      expect(claims!.login, 'admin');
      expect(claims.fullName, 'Админ');
      expect(claims.position, 'Admin');
      expect(claims.expiresAt.isUtc, isTrue);
    });

    test('tryParse returns null for malformed token', () {
      expect(JwtClaims.tryParse('not-a-jwt'), isNull);
      expect(JwtClaims.tryParse('a.b'), isNull);
    });

    test('isExpired', () {
      final past = DateTime.now().toUtc().subtract(const Duration(hours: 1));
      final future = DateTime.now().toUtc().add(const Duration(hours: 1));
      final futureClaims = JwtClaims(
        login: 'u',
        fullName: 'U',
        expiresAt: future,
        issuedAt: past,
      );
      final pastClaims = JwtClaims(
        login: 'u',
        fullName: 'U',
        expiresAt: past,
        issuedAt: past,
      );
      expect(futureClaims.isExpired, isFalse);
      expect(pastClaims.isExpired, isTrue);
    });
  });
}