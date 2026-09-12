import 'dart:convert';

import 'package:freezed_annotation/freezed_annotation.dart';

part 'auth_token.freezed.dart';
part 'auth_token.g.dart';

/// Ответ `user.login` (порт `AuthToken`, сериализован в `Response.payload`).
@freezed
class AuthToken with _$AuthToken {
  const factory AuthToken({
    @JsonKey(name: 'access_token') required String accessToken,
    @JsonKey(name: 'expires_at') required DateTime expiresAt,
    @JsonKey(name: 'token_type') required String tokenType,
  }) = _AuthToken;

  factory AuthToken.fromJson(Map<String, dynamic> json) =>
      _$AuthTokenFromJson(json);
}

/// Claims JWT HS256 (`crates/core-api/src/token.rs`).
/// Токен клиент не выпускает и не проверяет (серверная зона); здесь — только
/// лёгкая распаковка для UI (имя пользователя) и срока жизни.
class JwtClaims {
  const JwtClaims({
    this.sub,
    required this.login,
    required this.fullName,
    this.position,
    this.companyId,
    required this.expiresAt,
    required this.issuedAt,
  });

  final String? sub;
  final String login;
  final String fullName;
  final String? position;
  final String? companyId;
  final DateTime expiresAt;
  final DateTime issuedAt;

  static JwtClaims? tryParse(String token) {
    final parts = token.split('.');
    if (parts.length != 3) {
      return null;
    }
    final payload = parts[1];
    try {
      final json = jsonDecode(
        utf8.decode(base64Url.decode(base64Url.normalize(payload))),
      );
      if (json is! Map<String, dynamic>) {
        return null;
      }
      final exp = json['exp'];
      final iat = json['iat'];
      if (exp is! int || iat is! int) {
        return null;
      }
      return JwtClaims(
        sub: json['sub'] as String?,
        login: json['login'] as String,
        fullName: json['full_name'] as String,
        position: json['position'] as String?,
        companyId: json['company_id'] as String?,
        expiresAt: DateTime.fromMillisecondsSinceEpoch(exp * 1000, isUtc: true),
        issuedAt: DateTime.fromMillisecondsSinceEpoch(iat * 1000, isUtc: true),
      );
    } on FormatException {
      return null;
    }
  }

  bool get isExpired => !expiresAt.isAfter(DateTime.now().toUtc());
}