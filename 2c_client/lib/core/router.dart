import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../providers/app_providers.dart';
import '../screens/catalog_screen.dart';
import '../screens/home_screen.dart';
import '../screens/login_screen.dart';
import '../screens/object_form_screen.dart';
import '../screens/settings_screen.dart';

/// Конфигурация навигации: `/login` → `/home` → каталоги/формы СДУИ.
final routerProvider = Provider<GoRouter>((ref) {
  return GoRouter(
    initialLocation: '/home',
    refreshListenable: ref.watch(routerRefreshProvider),
    redirect: (context, state) {
      final auth = ref.read(authControllerProvider);
      final path = state.matchedLocation;

      final atLogin = path == '/login';
      if (auth is AuthAuthenticated) {
        return atLogin ? '/home' : null;
      }
      // Восстановление сессии ещё не завершено. Не монтируем защищённые
      // экраны анонимно: иначе сетевые провайдеры успевают закэшировать
      // PERMISSION_ERROR до реального входа. Показываем /login (спиннер).
      if (auth is AuthRestoring) {
        return atLogin ? null : '/login';
      }
      return atLogin ? null : '/login';
    },
    routes: [
      GoRoute(path: '/login', builder: (context, state) => const LoginScreen()),
      GoRoute(path: '/home', builder: (context, state) => const HomeScreen()),
      GoRoute(
        path: '/settings',
        builder: (context, state) => const SettingsScreen(),
      ),
      GoRoute(
        path: '/catalog/:entityType',
        builder: (context, state) => CatalogScreen(
          entityType: state.pathParameters['entityType']!,
        ),
      ),
      GoRoute(
        path: '/object/:entityType/new',
        builder: (context, state) => ObjectFormScreen(
          entityType: state.pathParameters['entityType']!,
          id: 'new',
          mode: ObjectFormMode.create,
        ),
      ),
      GoRoute(
        path: '/object/:entityType/:id/edit',
        builder: (context, state) => ObjectFormScreen(
          entityType: state.pathParameters['entityType']!,
          id: state.pathParameters['id']!,
          mode: ObjectFormMode.edit,
        ),
      ),
      GoRoute(
        path: '/object/:entityType/:id',
        builder: (context, state) => ObjectFormScreen(
          entityType: state.pathParameters['entityType']!,
          id: state.pathParameters['id']!,
          mode: ObjectFormMode.view,
        ),
      ),
    ],
  );
});