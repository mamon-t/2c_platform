import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'core/router.dart';
import 'core/theme.dart';
import 'core/app_theme.dart';
import 'providers/theme_providers.dart';

/// Корневой виджет приложения: тема из провайдера (встроенные + файловые)
/// и диспетчер навигации.
class TwocApp extends ConsumerWidget {
  const TwocApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final router = ref.watch(routerProvider);
    final themeAsync = ref.watch(appThemeProvider);
    return themeAsync.when(
      loading: () => MaterialApp(
        title: '2C Platform',
        theme: buildAppTheme(BuiltinThemes.dark),
        home: const Scaffold(
          body: Center(child: CircularProgressIndicator()),
        ),
      ),
      error: (e, _) => MaterialApp(
        title: '2C Platform',
        theme: buildAppTheme(BuiltinThemes.dark),
        home: Scaffold(
          body: Center(child: Text('Не удалось загрузить тему: $e')),
        ),
      ),
      data: (state) => MaterialApp.router(
        title: '2C Platform',
        theme: buildAppTheme(state.selected),
        routerConfig: router,
      ),
    );
  }
}