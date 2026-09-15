/// Пути конфигурации клиента.
///
/// Все пути вычисляются от базового каталога `~/.config/2cplatform` (Linux,
/// XDG); для тестов и изоляции окружение может переопределить базовый каталог
/// переменной `2CPLATFORM_CONFIG_DIR`.
library;

import 'dart:io';

class AppPaths {
  const AppPaths._();

  static const envOverride = '2CPLATFORM_CONFIG_DIR';

  /// Каталог конфигурации (`$XDG_CONFIG_HOME/2cplatform` → `~/.config/2cplatform`).
  static Directory base() {
    final override = Platform.environment[envOverride];
    if (override != null && override.isNotEmpty) {
      return Directory('$override/2cplatform');
    }
    final xdg = Platform.environment['XDG_CONFIG_HOME'];
    final home = Platform.environment['HOME'];
    final root = (xdg != null && xdg.isNotEmpty)
        ? xdg
        : (home != null && home.isNotEmpty)
            ? '$home/.config'
            : '.';
    return Directory('$root/2cplatform');
  }

  /// Каталог пользовательских тем (`themes/*.toml`).
  static Directory themesDir() => Directory('${base().path}/themes');

  /// Файл конфигурации приложения (`config.toml`).
  static File configFile() => File('${base().path}/config.toml');
}