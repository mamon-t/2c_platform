import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:logging/logging.dart';

import '../models/auth_token.dart';
import '../models/server_error.dart';
import 'rpc_client.dart';

/// Авторизация по JWT (ТЗ §10b): `user.login` / `user.logout`,
/// хранение access-токена в защищённом хранилище, восстановление сессии.
///
/// Срок жизни токена — 8 ч (сервер). Refresh-токенов на сервере нет:
/// `refresh()` — явный симптом «нужно перелогиниться» до Фазы 12 (оффлайн).
class AuthService {
  AuthService({
    required this.rpcClient,
    required this._storage,
    Logger? logger,
  }) : _log = logger ?? Logger('AuthService');

  final RpcClient rpcClient;
  final FlutterSecureStorage _storage;
  final Logger _log;

  static const _keyToken = 'access_token';
  static const _keyExpiresAt = 'expires_at';

  /// Признак того, что есть непустой сохранённый токен (без сетевых проверок).
  bool get hasSavedToken => _lastToken != null;

  String? _lastToken;
  JwtClaims? _claims;

  /// Текущий действующий access-токен (None до логина / после логаута).
  String? get token => _lastToken;

  /// Claims текущего токена (для UI: имя, должность).
  JwtClaims? get claims => _claims;

  /// Авторизация. Бросает `ServerError` (для всех сбоев сервера сообщение
  /// единое: «неверный логин или пароль»).
  Future<AuthToken> login(String login, String password) async {
    final payload = await rpcClient.command(
      module: 'core',
      action: 'user.login',
      payload: {'login': login, 'password': password},
    );
    final authToken = AuthToken.fromJson(payload);
    await _persist(authToken);
    _adopt(authToken.accessToken);
    _log.info('login ok: $login');
    return authToken;
  }

  /// Разлогин: best-effort `user.logout` (ошибки сервера игнорируются),
  /// токен гарантированно стирается.
  Future<void> logout() async {
    if (_lastToken != null) {
      try {
        await rpcClient.command(module: 'core', action: 'user.logout');
      } catch (_) {
        _log.warning('user.logout не выполнен — токен всё равно будет стёрт');
      }
    }
    await _clearLocal();
    _reset();
    _log.info('logout');
  }

  /// Восстанавливает сессию из защищённого хранилища.
  ///
  /// # Errors
  ///
  /// Бросает `ServerError(ErrorCode.authRequired)`, если токена нет, он битый
  /// или просрочен — вызывающий слой обязан отправить на `/login`.
  Future<String?> restoreSession() async {
    final rawToken = await _storage.read(key: _keyToken);
    if (rawToken == null || rawToken.isEmpty) {
      return null;
    }
    final claims = JwtClaims.tryParse(rawToken);
    if (claims == null || claims.isExpired) {
      _log.warning('сохранённый токен просрочен/битый');
      await _clearLocal();
      _reset();
      throw const ServerError(
        ErrorCode.authRequired,
        'Срок сессии истёк — войдите заново',
      );
    }
    _adopt(rawToken);
    return rawToken;
  }

  /// Точка расширения для Фазы 12 (refresh-токены). В 11a не реализована:
  /// истёкший токен означает повторный вход.
  Future<String> refresh(String expiredToken) {
    _log.warning('refresh запрошен без поддержки сервера');
    throw const ServerError(
      ErrorCode.authRequired,
      'Срок сессии истёк — войдите заново',
    );
  }

  Future<void> _persist(AuthToken authToken) async {
    await _storage.write(
      key: _keyToken,
      value: authToken.accessToken,
    );
    await _storage.write(
      key: _keyExpiresAt,
      value: authToken.expiresAt.toIso8601String(),
    );
  }

  Future<void> _clearLocal() async {
    await _storage.delete(key: _keyToken);
    await _storage.delete(key: _keyExpiresAt);
  }

  void _adopt(String rawToken) {
    _lastToken = rawToken;
    _claims = JwtClaims.tryParse(rawToken);
  }

  void _reset() {
    _lastToken = null;
    _claims = null;
  }
}