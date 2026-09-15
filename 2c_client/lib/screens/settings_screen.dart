import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/config.dart';

import '../providers/app_providers.dart';
import '../providers/theme_providers.dart';

/// Настройки клиента: тема (выбор из встроенных + файловых) и адрес сервера.
class SettingsScreen extends ConsumerStatefulWidget {
  const SettingsScreen({super.key});

  @override
  ConsumerState<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends ConsumerState<SettingsScreen> {
  late final TextEditingController _serverController;
  String? _saved;
  String? _themeError;

  @override
  void initState() {
    super.initState();
    _serverController =
        TextEditingController(text: ref.read(appConfigProvider).serverBaseUrl);
  }

  @override
  void dispose() {
    _serverController.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final url = _serverController.text.trim();
    await ref
        .read(appConfigProvider.notifier)
        .setServerBaseUrl(url.isEmpty ? AppConfig.defaultServerBaseUrl : url);
    setState(() {
      _saved = 'Адрес сохранён';
    });
  }

  Future<void> _changeTheme(String? id) async {
    if (id == null) {
      return;
    }
    setState(() => _themeError = null);
    try {
      await ref.read(appThemeProvider.notifier).setTheme(id);
    } catch (e) {
      setState(() => _themeError = 'Не удалось сохранить тему: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    final current = ref.watch(appConfigProvider);
    final themeAsync = ref.watch(appThemeProvider);
    return Scaffold(
      appBar: AppBar(title: const Text('Настройки')),
      body: Padding(
        padding: const EdgeInsets.all(24),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 520),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text('Тема', style: Theme.of(context).textTheme.titleMedium),
              const SizedBox(height: 8),
              themeAsync.when(
                loading: () => const SizedBox(
                  height: 56,
                  child: Center(child: CircularProgressIndicator()),
                ),
                error: (e, _) => Text(
                  'Не удалось загрузить темы: $e',
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
                data: (state) => DropdownButtonFormField<String>(
                  initialValue: state.selected.id,
                  decoration: const InputDecoration(
                    prefixIcon: Icon(Icons.palette_outlined),
                  ),
                  items: [
                    for (final t in state.themes)
                      DropdownMenuItem<String>(
                        value: t.id,
                        child: Text(t.name),
                      ),
                  ],
                  onChanged: _changeTheme,
                ),
              ),
              if (_themeError != null) ...[
                const SizedBox(height: 8),
                Text(
                  _themeError!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ],
              const SizedBox(height: 20),
              Text(
                'Адрес сервера',
                style: Theme.of(context).textTheme.titleMedium,
              ),
              const SizedBox(height: 8),
              TextField(
                controller: _serverController,
                decoration: const InputDecoration(
                  hintText: 'http://127.0.0.1:8080',
                  prefixIcon: Icon(Icons.dns_outlined),
                ),
                keyboardType: TextInputType.url,
                onSubmitted: (_) => _save(),
              ),
              const SizedBox(height: 12),
              Text(
                'WebSocket: ${current.wsBaseUrl}',
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.outline,
                    ),
              ),
              if (_saved != null) ...[
                const SizedBox(height: 8),
                Text(_saved!, style: const TextStyle(color: Colors.green)),
              ],
              const SizedBox(height: 16),
              FilledButton.icon(
                onPressed: _save,
                icon: const Icon(Icons.save_outlined),
                label: const Text('Сохранить'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}