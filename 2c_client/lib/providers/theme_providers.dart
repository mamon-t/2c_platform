import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/app_theme.dart';

/// Состояние тем: список доступных (встроенные + из каталога) и выбранная.
class AppThemeState {
  const AppThemeState({required this.themes, required this.selected});

  final List<AppTheme> themes;
  final AppTheme selected;

  AppThemeState select(AppTheme theme) => AppThemeState(
        themes: themes,
        selected: theme,
      );
}

/// Загружает темы и конфигурацию при старте; хранит выбранную тему.
class AppThemeController extends AsyncNotifier<AppThemeState> {
  ThemeStore get _store => ref.read(themeStoreProvider);

  @override
  Future<AppThemeState> build() async {
    final store = _store;
    final themes = await store.loadThemes();
    final configured = await store.readConfiguredTheme();
    final selected = _resolve(themes, configured);
    return AppThemeState(themes: themes, selected: selected);
  }

  /// Применяет тему и персистит в `config.toml`.
  Future<void> setTheme(String id) async {
    final current = state.valueOrNull;
    if (current == null) {
      return;
    }
    for (final t in current.themes) {
      if (t.id == id) {
        await _store.writeConfiguredTheme(id);
        state = AsyncData(current.select(t));
        return;
      }
    }
    throw StateError('Неизвестная тема: $id');
  }

  AppTheme _resolve(List<AppTheme> themes, String? configured) {
    for (final t in themes) {
      if (t.id == configured) {
        return t;
      }
    }
    return BuiltinThemes.dark;
  }
}

/// Провайдер хранилища тем (для тестов переопределяется на временный каталог).
final themeStoreProvider = Provider<ThemeStore>((ref) => ThemeStore());

/// Провайдер состояния тем приложения.
final appThemeProvider =
    AsyncNotifierProvider<AppThemeController, AppThemeState>(
  AppThemeController.new,
);