import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../providers/app_providers.dart';
import '../screens/home_screen.dart';
import '../screens/login_screen.dart';
import '../screens/settings_screen.dart';

/// Конфигурация навигации: `/login` → `/home` → `/settings`.
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
      // Восстановление сессии ещё не завершено — оставляем текущий маршрут.
      if (auth is AuthRestoring) {
        return atLogin ? null : null;
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
    ],
  );
});