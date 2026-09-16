import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../models/navigation_item.dart';
import '../providers/app_providers.dart';
import '../providers/sdui_providers.dart';
import '../services/ws_client.dart';

/// Главный экран: индикатор соединения, Drawer и корневой список разделов
/// из `module.navigation` (разделы установленных модулей СДУИ).
class HomeScreen extends ConsumerWidget {
  const HomeScreen({super.key});

  Future<void> _logout(BuildContext context, WidgetRef ref) async {
    await ref.read(authControllerProvider.notifier).logout();
    if (context.mounted) {
      context.go('/login');
    }
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final auth = ref.watch(authControllerProvider);
    final connection = ref.watch(wsConnectionProvider);
    final navigation = ref.watch(navigationProvider);

    final userLabel = auth is AuthAuthenticated
        ? (auth.fullName.isEmpty ? auth.login : auth.fullName)
        : '';

    return Scaffold(
      appBar: AppBar(
        title: const Text('2C Platform'),
        actions: [
          connection.when(
            data: (state) => _ConnectionIndicator(state: state),
            error: (_, _) => const _ConnectionIndicator(
              state: WsConnectionState.disconnected,
            ),
            loading: () => const _ConnectionIndicator(
              state: WsConnectionState.connecting,
            ),
          ),
          const SizedBox(width: 16),
        ],
      ),
      drawer: Drawer(
        child: ListView(
          padding: EdgeInsets.zero,
          children: [
            DrawerHeader(
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.primaryContainer,
              ),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.end,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Icon(Icons.account_circle, size: 40),
                  const SizedBox(height: 8),
                  Text(
                    userLabel,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ],
              ),
            ),
            ...navigation.when(
              data: (modules) => [
                for (final section in _NavigationSections.sectionsOf(modules))
                  _sectionTiles(context, section),
              ],
              error: (_, _) => <Widget>[],
              loading: () => <Widget>[],
            ),
            ListTile(
              leading: const Icon(Icons.home_outlined),
              title: const Text('Главная'),
              onTap: () => context.go('/home'),
            ),
            ListTile(
              leading: const Icon(Icons.settings_outlined),
              title: const Text('Настройки'),
              onTap: () => context.go('/settings'),
            ),
            const Divider(),
            ListTile(
              leading: const Icon(Icons.logout),
              title: const Text('Выход'),
              onTap: () => _logout(context, ref),
            ),
          ],
        ),
      ),
      body: navigation.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Text('Не удалось загрузить разделы: $e'),
          ),
        ),
        data: (modules) {
          final sections = _NavigationSections.sectionsOf(modules);
          if (sections.isEmpty) {
            return _EmptyModules();
          }
          return ListView(
            padding: const EdgeInsets.all(16),
            children: [
              for (final section in sections)
                _bodySection(context, section),
            ],
          );
        },
      ),
    );
  }

  /// Роут для пункта навигации: административный «Метаданные» открывает
  /// редактор метаданных, остальные пункты — универсальный каталог СДУИ.
  static String _routeFor(NavigationItem item) =>
      item.code == 'metadata' ? '/metadata/editor' : '/catalog/${item.entityType}';

  Widget _sectionTiles(BuildContext context, _NavigationSection section) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text(
              section.title,
              style: Theme.of(context).textTheme.labelLarge?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
            ),
          ),
        ),
        for (final item in section.items)
          ListTile(
            leading: const Icon(Icons.description_outlined),
            title: Text(item.label),
            onTap: () => context.push(_routeFor(item)),
          ),
      ],
    );
  }

  Widget _bodySection(BuildContext context, _NavigationSection section) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          section.title,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 8),
        Card(
          child: Column(
            children: [
              for (final item in section.items)
                ListTile(
                  leading: const Icon(Icons.description_outlined),
                  title: Text(item.label),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: () => context.push(_routeFor(item)),
                ),
            ],
          ),
        ),
        const SizedBox(height: 16),
      ],
    );
  }
}

/// Сгруппированная по модулям навигация: заголовок + пункты (`entity_type`).
class _NavigationSections {
  static List<_NavigationSection> sectionsOf(List<ModuleNavigation> modules) {
    return [
      for (final module in modules)
        _NavigationSection(
          title: module.displayName,
          items: [
            for (final item in module.navigation)
              // Административный пункт «Метаданные» не связан с сущностью,
              // но должен попадать в навигацию; остальные — каталоги СДУИ.
              if (item.entityType != null || item.code == 'metadata') item,
          ],
        ),
    ]..removeWhere((s) => s.items.isEmpty);
  }
}

class _NavigationSection {
  const _NavigationSection({required this.title, required this.items});

  final String title;
  final List<NavigationItem> items;
}

class _EmptyModules extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.widgets_outlined,
            size: 64,
            color: Theme.of(context).colorScheme.outline,
          ),
          const SizedBox(height: 16),
          Text(
            'Нет установленных модулей',
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 8),
          Text(
            'Здесь появятся рабочие разделы после подключения модулей',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: Theme.of(context).colorScheme.outline,
                ),
          ),
        ],
      ),
    );
  }
}

class _ConnectionIndicator extends StatelessWidget {
  const _ConnectionIndicator({required this.state});

  final WsConnectionState state;

  @override
  Widget build(BuildContext context) {
    final (color, label) = switch (state) {
      WsConnectionState.connected => (
          Colors.green,
          'Соединение активно',
        ),
      WsConnectionState.connecting => (
          Colors.orange,
          'Подключение…',
        ),
      WsConnectionState.disconnected => (
          Colors.red,
          'Нет соединения',
        ),
    };
    return Tooltip(
      message: label,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 10,
            height: 10,
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
          const SizedBox(width: 6),
          Text(label, style: Theme.of(context).textTheme.labelMedium),
        ],
      ),
    );
  }
}