import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../models/script.dart';
import '../providers/sdui_providers.dart';

/// Каталог скриптов Rhai компании (ТЗ §15, Фаза 15.2).
///
/// Столбцы — код, название, тип, статус. Тап по строке → редактор скрипта,
/// FAB → создание нового. Список живёт в `scriptsProvider` и обновляется
/// через RefreshIndicator / invalidate после возврата из редактора.
class ScriptListScreen extends ConsumerWidget {
  const ScriptListScreen({super.key});

  /// Канонические русские подписи типов скрипта (порядок — как на сервере).
  static const Map<String, String> typeLabels = {
    'formula': 'Формула',
    'validator': 'Валидатор',
    'before_action': 'Перед действием',
    'after_action': 'После действия',
    'report': 'Отчёт',
    'event_handler': 'Обработчик события',
  };

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scriptsAsync = ref.watch(scriptsProvider);

    return Scaffold(
      appBar: AppBar(title: const Text('Скрипты')),
      floatingActionButton: FloatingActionButton(
        tooltip: 'Создать',
        onPressed: () => context.push('/scripts/new'),
        child: const Icon(Icons.add),
      ),
      body: scriptsAsync.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Text('Не удалось загрузить скрипты: $e'),
          ),
        ),
        data: (scripts) => RefreshIndicator(
          onRefresh: () async {
            ref.invalidate(scriptsProvider);
            await ref.read(scriptsProvider.future);
          },
          child: scripts.isEmpty
              ? ListView(
                  physics: const AlwaysScrollableScrollPhysics(),
                  children: const [
                    Padding(
                      padding: EdgeInsets.all(32),
                      child: Center(child: Text('Нет скриптов')),
                    ),
                  ],
                )
              : _scriptsTable(context, scripts),
        ),
      ),
    );
  }

  Widget _scriptsTable(BuildContext context, List<ScriptItem> scripts) {
    return SingleChildScrollView(
      child: DataTable(
        columns: const [
          DataColumn(label: Text('Код')),
          DataColumn(label: Text('Название')),
          DataColumn(label: Text('Тип')),
          DataColumn(label: Text('Статус')),
        ],
        rows: [
          for (final s in scripts)
            DataRow(
              onSelectChanged: (_) =>
                  context.push('/scripts/${s.code}/edit'),
              cells: [
                DataCell(Text(s.code)),
                DataCell(Text(s.name)),
                DataCell(Text(typeLabels[s.scriptType] ?? s.scriptType)),
                DataCell(Text(s.isActive ? 'Активен' : 'Отключён')),
              ],
            ),
        ],
      ),
    );
  }
}