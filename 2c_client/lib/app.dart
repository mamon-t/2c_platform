import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'core/router.dart';
import 'core/theme.dart';

/// Корневой виджет приложения: тёмная Material 3 тема + диспетчер навигации.
class TwocApp extends ConsumerWidget {
  const TwocApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final router = ref.watch(routerProvider);
    return MaterialApp.router(
      title: '2C Platform',
      theme: buildAppTheme(),
      routerConfig: router,
    );
  }
}