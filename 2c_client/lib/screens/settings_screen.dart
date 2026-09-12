import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/config.dart';

import '../providers/app_providers.dart';

/// Настройки клиента: адрес сервера (правка + сохранение).
class SettingsScreen extends ConsumerStatefulWidget {
  const SettingsScreen({super.key});

  @override
  ConsumerState<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends ConsumerState<SettingsScreen> {
  late final TextEditingController _serverController;
  String? _saved;

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

  @override
  Widget build(BuildContext context) {
    final current = ref.watch(appConfigProvider);
    return Scaffold(
      appBar: AppBar(title: const Text('Настройки')),
      body: Padding(
        padding: const EdgeInsets.all(24),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 520),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text('Адрес сервера', style: Theme.of(context).textTheme.titleMedium),
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