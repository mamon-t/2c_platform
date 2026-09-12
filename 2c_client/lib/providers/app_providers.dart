import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../core/config.dart';
import '../models/rpc_message.dart';
import '../services/auth_service.dart';
import '../services/rpc_client.dart';
import '../services/ws_client.dart';

/// Хранилище настроек. Передаётся в `main()` через override.
final sharedPreferencesProvider = Provider<SharedPreferences>(
  (ref) => throw UnimplementedError(
    'sharedPreferencesProvider: укажите override в main()',
  ),
);

/// Адрес сервера; правка на экране «Настройки» меняет состояние провайдера.
final appConfigProvider =
    StateNotifierProvider<AppConfigController, AppConfig>(
  (ref) => AppConfigController(ref.read(sharedPreferencesProvider)),
);

class AppConfigController extends StateNotifier<AppConfig> {
  AppConfigController(this._prefs) : super(AppConfig.defaults) {
    _load();
  }

  final SharedPreferences _prefs;

  Future<void> _load() async {
    final loaded = await AppConfig.load(_prefs);
    if (!mounted) return;
    state = loaded;
  }

  Future<void> setServerBaseUrl(String url) async {
    final next = state.copyWith(serverBaseUrl: url);
    state = next;
    await next.save(_prefs);
  }
}

// ── HTTP-клиент `/rpc` ────────────────────────────────────────────────────

final rpcClientProvider = Provider<RpcClient>((ref) {
  final config = ref.watch(appConfigProvider);
  final client = RpcClient(baseUrl: config.serverBaseUrl);
  ref.onDispose(client.dispose);
  return client;
});

/// Живой WebSocket-клиент `/ws` (пересоздаётся при смене адреса сервера).
final wsClientProvider = Provider<WsClient>((ref) {
  final config = ref.watch(appConfigProvider);
  final client = WsClient(wsBaseUrl: config.wsBaseUrl);
  ref.onDispose(client.dispose);
  return client;
});

/// Статус WS-соединения (индикатор в HomeScreen).
final wsConnectionProvider = StreamProvider<WsConnectionState>(
  (ref) => ref.watch(wsClientProvider).state,
);

/// Входящие ServerPush.
final pushProvider = StreamProvider<RpcMessage>(
  (ref) => ref.watch(wsClientProvider).messages,
);

// ── Авторизация ───────────────────────────────────────────────────────────

final authServiceProvider = Provider<AuthService>((ref) {
  final rpc = ref.watch(rpcClientProvider);
  return AuthService(rpcClient: rpc, storage: const FlutterSecureStorage());
});

final authControllerProvider =
    StateNotifierProvider<AuthController, AuthState>(
  (ref) => AuthController(ref),
);

/// Провайдер текущего access-токена (для виджетов, которым нужен Bearer).
final accessTokenProvider = Provider<String?>(
  (ref) {
    ref.watch(authControllerProvider);
    return ref.read(authControllerProvider.notifier).token;
  },
);

/// Слушатель для `GoRouter.refreshListenable`: AuthController инкрементирует
/// счётчик при каждом переходе AuthState, чтобы redirect пересчитался.
final routerRefreshProvider = Provider<ValueNotifier<int>>(
  (ref) {
    final notifier = ValueNotifier<int>(0);
    ref.onDispose(notifier.dispose);
    return notifier;
  },
);

sealed class AuthState {
  const AuthState();
}

class AuthRestoring extends AuthState {
  const AuthRestoring();
}

class AuthUnauthenticated extends AuthState {
  const AuthUnauthenticated();
}

class AuthAuthenticated extends AuthState {
  const AuthAuthenticated({
    required this.login,
    required this.fullName,
    this.position,
  });

  final String login;
  final String fullName;
  final String? position;
}

class AuthController extends StateNotifier<AuthState> {
  AuthController(this._ref) : super(const AuthRestoring()) {
    _restore();
  }

  final Ref _ref;

  AuthService get _service => _ref.read(authServiceProvider);
  RpcClient get _rpc => _ref.read(rpcClientProvider);
  WsClient get _ws => _ref.read(wsClientProvider);

  /// Токен текущей сессии (для UI-потоков).
  String? get token => _token;
  String? _token;

  Future<void> _restore() async {
    final restored = await _service.restoreSession();
    if (restored == null) {
      state = const AuthUnauthenticated();
      _notifyRoute();
      return;
    }
    _adopt(restored);
  }

  Future<void> login(String login, String password) async {
    final authToken = await _service.login(login, password);
    _adopt(authToken.accessToken);
  }

  Future<void> logout() async {
    await _service.logout();
    _clear();
  }

  void _adopt(String accessToken) {
    _token = accessToken;
    _rpc.token = accessToken;
    _ws.disconnect();
    _ws.connect(accessToken);
    final claims = _service.claims;
    state = AuthAuthenticated(
      login: claims?.login ?? '',
      fullName: claims?.fullName ?? '',
      position: claims?.position,
    );
    _notifyRoute();
  }

  void _clear() {
    _token = null;
    _rpc.token = null;
    _ws.disconnect();
    state = const AuthUnauthenticated();
    _notifyRoute();
  }

  void _notifyRoute() {
    _ref.read(routerRefreshProvider).value++;
  }
}