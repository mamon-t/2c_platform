import 'package:shared_preferences/shared_preferences.dart';

/// Значения/ключи настроек клиента.
class AppConfig {
  const AppConfig({required this.serverBaseUrl, required this.wsBaseUrl});

  /// Базовый адрес HTTP-API (например, `http://127.0.0.1:8080`).
  final String serverBaseUrl;

  /// Базовый адрес WebSocket-API (например, `ws://127.0.0.1:8080`).
  final String wsBaseUrl;

  /// Адрес сервера по умолчанию. Сознательно не «хардкод» в смысле «нельзя
  /// переопределить» — это дефолт для первого запуска, живёт рядом с сервером
  /// разработки и всегда редактируется на экране «Настройки».
  static const String defaultServerBaseUrl = 'http://127.0.0.1:8080';

  static const AppConfig defaults = AppConfig(
    serverBaseUrl: defaultServerBaseUrl,
    wsBaseUrl: 'ws://127.0.0.1:8080',
  );

  static String wsFromServer(String serverBaseUrl) {
    if (serverBaseUrl.startsWith('https://')) {
      return serverBaseUrl.replaceFirst('https://', 'wss://');
    }
    if (serverBaseUrl.startsWith('http://')) {
      return serverBaseUrl.replaceFirst('http://', 'ws://');
    }
    return serverBaseUrl;
  }

  static const _prefsKey = 'server_base_url';

  /// Читает настройки из хранилища, при отсутствии — дефолт.
  static Future<AppConfig> load(SharedPreferences prefs) async {
    final server = prefs.getString(_prefsKey) ?? defaultServerBaseUrl;
    return AppConfig(
      serverBaseUrl: server,
      wsBaseUrl: wsFromServer(server),
    );
  }

  /// Сохраняет новый адрес сервера и пересчитывает WS-адрес.
  Future<void> save(SharedPreferences prefs) async {
    await prefs.setString(_prefsKey, serverBaseUrl);
  }

  AppConfig copyWith({String? serverBaseUrl}) {
    final next = serverBaseUrl ?? this.serverBaseUrl;
    return AppConfig(
      serverBaseUrl: next,
      wsBaseUrl: wsFromServer(next),
    );
  }
}