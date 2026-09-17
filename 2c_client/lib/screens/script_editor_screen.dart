import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../models/script.dart';
import '../models/server_error.dart';
import '../providers/sdui_providers.dart';
import '../screens/script_list_screen.dart';
import '../services/script_service.dart';

/// Редактор скрипта Rhai (ТЗ §15, Фаза 15.2).
///
/// Режим создания — роут `/scripts/new`; редактирование — `/scripts/:code/edit`.
/// Источник — многострочный `TextField`; кнопки «Проверить» (`script.validate`),
/// «Прогнать» (`script.test`, только для сохранённого скрипта) и «Сохранить».
/// Ошибки проверки показываются списком с координатами (строка:колонка).
class ScriptEditorScreen extends ConsumerStatefulWidget {
  const ScriptEditorScreen({super.key, required this.code});

  /// Код скрипта; `'new'` — создание.
  final String code;

  @override
  ConsumerState<ScriptEditorScreen> createState() =>
      _ScriptEditorScreenState();
}

class _ScriptEditorScreenState extends ConsumerState<ScriptEditorScreen> {
  late final _code = TextEditingController(text: widget.code == 'new' ? '' : widget.code);
  final _name = TextEditingController();
  final _source = TextEditingController();
  final _entityType = TextEditingController();
  String _scriptType = 'formula';
  bool _isActive = true;
  bool _loading = true;
  bool _saving = false;
  ScriptItem? _existing;
  ScriptValidation? _validation;
  String? _testResult;
  String? _error;

  bool get _creating => widget.code == 'new';

  @override
  void initState() {
    super.initState();
    if (!_creating) {
      _load();
    } else {
      _loading = false;
    }
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final script = await ref
          .read(scriptServiceProvider)
          .get(widget.code, companyId: ref.read(companyIdProvider));
      if (!mounted) {
        return;
      }
      setState(() {
        _existing = script;
        _name.text = script.name;
        _source.text = script.source;
        _entityType.text = script.entityType ?? '';
        _scriptType = script.scriptType;
        _isActive = script.isActive;
        _loading = false;
      });
    } on ServerError catch (e) {
      if (!mounted) {
        return;
      }
      setState(() {
        _error = e.localizedMessage;
        _loading = false;
      });
    }
  }

  @override
  void dispose() {
    _code.dispose();
    _name.dispose();
    _source.dispose();
    _entityType.dispose();
    super.dispose();
  }

  Future<void> _validate() async {
    final messenger = ScaffoldMessenger.of(context);
    setState(() => _validation = null);
    try {
      final v = await ref
          .read(scriptServiceProvider)
          .validate(_source.text.trim());
      if (!mounted) {
        return;
      }
      setState(() => _validation = v);
      messenger.showSnackBar(
        SnackBar(
          content: Text(v.valid ? 'Синтаксис в порядке' : 'Найдены ошибки (${v.errors.length})'),
        ),
      );
    } on ServerError catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(e.localizedMessage)));
    }
  }

  Future<void> _test() async {
    final code = _existing?.code ?? _code.text.trim();
    if (code.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Сначала сохраните скрипт')),
      );
      return;
    }
    final messenger = ScaffoldMessenger.of(context);
    setState(() => _testResult = null);
    try {
      final r = await ref.read(scriptServiceProvider).testRun(code);
      if (!mounted) {
        return;
      }
      final ms = r['execution_time_ms'];
      final suffix = ms is int ? ' · $ms мс' : '';
      setState(() => _testResult = 'Результат: ${r['result']}$suffix');
    } on ServerError catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(e.localizedMessage)));
    }
  }

  Future<void> _save() async {
    final messenger = ScaffoldMessenger.of(context);
    final code = _code.text.trim();
    final name = _name.text.trim();
    final source = _source.text.trim();
    if (code.isEmpty || name.isEmpty || source.isEmpty) {
      messenger.showSnackBar(
        const SnackBar(content: Text('Заполните код, название и исходник')),
      );
      return;
    }
    setState(() => _saving = true);
    try {
      final scripts = ref.read(scriptServiceProvider);
      final companyId = ref.read(companyIdProvider);
      final entityType =
          _entityType.text.trim().isEmpty ? null : _entityType.text.trim();
      if (_creating) {
        await scripts.create(
          code: code,
          name: name,
          source: source,
          scriptType: _scriptType,
          entityType: entityType,
          isActive: _isActive,
          companyId: companyId,
        );
      } else {
        await scripts.update(
          code,
          name: name,
          source: source,
          scriptType: _scriptType,
          entityType: entityType,
          isActive: _isActive,
          companyId: companyId,
        );
      }
      messenger.showSnackBar(const SnackBar(content: Text('Сохранено')));
      if (!mounted) {
        return;
      }
      ref.invalidate(scriptsProvider);
      if (context.canPop()) {
        context.pop();
      } else {
        context.go('/scripts');
      }
    } on ServerError catch (e) {
      if (!mounted) {
        return;
      }
      setState(() => _saving = false);
      messenger.showSnackBar(SnackBar(content: Text(e.localizedMessage)));
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return Scaffold(
        appBar: AppBar(title: const Text('Скрипт')),
        body: const Center(child: CircularProgressIndicator()),
      );
    }
    if (_error != null) {
      return Scaffold(
        appBar: AppBar(title: const Text('Скрипт')),
        body: Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Text('Не удалось загрузить скрипт: $_error'),
          ),
        ),
      );
    }

    return Scaffold(
      appBar: AppBar(
        title: Text(_creating ? 'Новый скрипт' : _existing?.name ?? widget.code),
      ),
      body: Form(
        child: ListView(
          padding: const EdgeInsets.all(16),
          children: [
            TextFormField(
              controller: _code,
              enabled: _creating,
              decoration: const InputDecoration(
                labelText: 'Код',
                helperText: 'Уникальный код скрипта',
              ),
            ),
            const SizedBox(height: 16),
            TextFormField(
              controller: _name,
              decoration: const InputDecoration(labelText: 'Название'),
            ),
            const SizedBox(height: 16),
            DropdownButtonFormField<String>(
              initialValue: _scriptType,
              decoration: const InputDecoration(labelText: 'Тип'),
              items: [
                for (final t in ScriptItem.scriptTypes)
                  DropdownMenuItem(
                    value: t,
                    child: Text(ScriptListScreen.typeLabels[t] ?? t),
                  ),
              ],
              onChanged: (v) => setState(() => _scriptType = v ?? 'formula'),
            ),
            const SizedBox(height: 16),
            TextFormField(
              controller: _entityType,
              decoration: const InputDecoration(
                labelText: 'Тип сущности',
                helperText: 'Например: invoice (пусто — универсальный скрипт)',
              ),
            ),
            const SizedBox(height: 16),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Активен'),
              value: _isActive,
              onChanged: (v) => setState(() => _isActive = v),
            ),
            const SizedBox(height: 8),
            TextFormField(
              controller: _source,
              minLines: 8,
              maxLines: null,
              keyboardType: TextInputType.multiline,
              style: const TextStyle(fontFamily: 'monospace'),
              decoration: const InputDecoration(
                labelText: 'Исходник Rhai',
                alignLabelWithHint: true,
                border: OutlineInputBorder(),
              ),
            ),
            if (_validation != null) _validationPanel(_validation!),
            if (_testResult != null)
              Padding(
                padding: const EdgeInsets.only(top: 12),
                child: Text(_testResult!),
              ),
            const SizedBox(height: 24),
            Row(
              children: [
                Expanded(
                  child: OutlinedButton.icon(
                    onPressed: _saving ? null : _validate,
                    icon: const Icon(Icons.rule),
                    label: const Text('Проверить'),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: OutlinedButton.icon(
                    onPressed: _saving || _creating ? null : _test,
                    icon: const Icon(Icons.play_arrow),
                    label: const Text('Прогнать'),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            ElevatedButton(
              onPressed: _saving ? null : _save,
              child: _saving
                  ? const SizedBox(
                      width: 20,
                      height: 20,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('Сохранить'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _validationPanel(ScriptValidation v) {
    return Container(
      margin: const EdgeInsets.only(top: 12),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: v.valid
            ? Theme.of(context).colorScheme.surfaceContainerHighest
            : Theme.of(context).colorScheme.errorContainer,
        borderRadius: BorderRadius.circular(8),
      ),
      child: v.valid
          ? const Text('Синтаксис в порядке')
          : Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text('Ошибки проверки:'),
                for (final e in v.errors)
                  Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Text(
                      e.line != null
                          ? 'Строка ${e.line}:${e.column ?? 0} — ${e.message}'
                          : '— ${e.message}',
                    ),
                  ),
              ],
            ),
    );
  }
}