import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/entity_schema.dart';
import '../models/server_error.dart';
import '../providers/sdui_providers.dart';
import '../services/metadata_service.dart';

/// Wire-значения `EntityKind` (snake_case) для дропдауна «Новый тип».
const _kinds = [
  'document',
  'catalog',
  'register',
  'task',
  'contract',
  'project',
  'setting',
  'custom',
];

/// Wire-значения `FieldType` (snake_case) для дропдауна типа поля.
const _fieldDataTypes = [
  'string',
  'text',
  'integer',
  'money',
  'date',
  'datetime',
  'boolean',
  'enum',
  'reference',
  'array',
  'table',
  'json',
  'file',
  'user',
  'company',
  'formula',
  'computed',
];

/// Редактор метаданных (фаза 15pre-3): декларативное управление схемой
/// типа сущности — поля, состояния, переходы, экспорт/импорт JSON.
///
/// Правки применяются только кнопкой «Сохранить»: схема целиком уходит в
/// `metadata.import` с увеличенной `metadata_version` (ensure-семантика
/// сервера: более новая версия применяется, равная — no-op). Пункт виден
/// в навигации только администраторам (гейт `metadata.manage`).
class MetadataEditorScreen extends ConsumerStatefulWidget {
  const MetadataEditorScreen({super.key, this.entityType});

  /// Код типа из URL (`/metadata/editor/:entityType?`); может быть null —
  /// тогда тип выбирается из выпадающего списка.
  final String? entityType;

  @override
  ConsumerState<MetadataEditorScreen> createState() =>
      _MetadataEditorScreenState();
}

class _MetadataEditorScreenState extends ConsumerState<MetadataEditorScreen> {
  String? _selected;
  EntitySchema? _schema;
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    final initial = widget.entityType;
    _selected = initial;
    // Загружаем схему предвыбранного типа после первого кадра: в initState
    // вызывать setState напрямую нельзя, а без подгрузки редактор останется
    // в состоянии «Схема не загружена».
    if (initial != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          _loadSchema(initial);
        }
      });
    }
  }

  MetadataService get _metadata =>
      ref.read(metadataServiceProvider);

  Future<void> _loadSchema(String code) async {
    setState(() {
      _busy = true;
      _schema = null;
    });
    try {
      final companyId = ref.read(companyIdProvider);
      final schema = await _metadata.fetchSchema(code, companyId: companyId);
      if (!mounted) return;
      setState(() {
        _schema = schema;
        _busy = false;
      });
    } on ServerError catch (e) {
      if (!mounted) return;
      setState(() => _busy = false);
      _snack(e.message);
    } catch (e) {
      if (!mounted) return;
      setState(() => _busy = false);
      _snack('Не удалось загрузить схему: $e');
    }
  }

  void _snack(String message) {
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(message)));
  }

  Future<void> _save() async {
    final schema = _schema;
    if (schema == null) return;
    try {
      // Бампим версию: ensure-семантика применяет только более новую схему.
      await _metadata.importMetadata(
        schema.toJson(metadataVersion: schema.entityType.metadataVersion + 1),
      );
      await _loadSchema(schema.entityType.code);
      _snack('Схема сохранена');
    } on ServerError catch (e) {
      _snack(e.message);
    }
  }

  Future<void> _exportJson() async {
    final code = _schema?.entityType.code;
    if (code == null) return;
    try {
      final companyId = ref.read(companyIdProvider);
      final doc = await _metadata.exportMetadata(code, companyId: companyId);
      if (!mounted) return;
      await showDialog<void>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: const Text('Схема JSON'),
          content: SizedBox(
            width: 480,
            child: SingleChildScrollView(
              child: SelectableText(jsonEncode(doc)),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () async {
                await Clipboard.setData(ClipboardData(text: jsonEncode(doc)));
                if (dialogContext.mounted) {
                  Navigator.of(dialogContext).pop();
                }
                _snack('Схема скопирована в буфер');
              },
              child: const Text('Копировать'),
            ),
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Закрыть'),
            ),
          ],
        ),
      );
    } on ServerError catch (e) {
      _snack(e.message);
    }
  }

  Future<void> _importJson() async {
    final controller = TextEditingController();
    try {
      final doc = await showDialog<String>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: const Text('Импорт схемы JSON'),
          content: SizedBox(
            width: 480,
            child: TextField(
              controller: controller,
              maxLines: 12,
              decoration: const InputDecoration(
                border: OutlineInputBorder(),
                hintText: '{ "entity_type": { ... }, "fields": [...] }',
              ),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Отмена'),
            ),
            FilledButton(
              onPressed: () => Navigator.of(dialogContext).pop(controller.text),
              child: const Text('Импортировать'),
            ),
          ],
        ),
      );
      if (doc == null || doc.trim().isEmpty) return;
      final decoded = jsonDecode(doc);
      if (decoded is! Map) {
        throw const FormatException('ожидался JSON-объект');
      }
      await _metadata.importMetadata(
        decoded.cast<String, dynamic>(),
      );
      final code = (decoded['entity_type'] as Map)['code'] as String;
      await _loadSchema(code);
      if (mounted) {
        setState(() => _selected = code);
      }
      _snack('Схема импортирована');
    } on ServerError catch (e) {
      _snack(e.message);
    } catch (e) {
      _snack('Ошибка импорта: $e');
    }
  }

  Future<void> _createNewType() async {
    final codeController = TextEditingController();
    final nameController = TextEditingController();
    String kind = _kinds.first;
    try {
      final result = await showDialog<Map<String, String>>(
        context: context,
        builder: (dialogContext) => StatefulBuilder(
          builder: (dialogContext, setDialogState) => AlertDialog(
            title: const Text('Новый тип сущности'),
            content: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  controller: codeController,
                  decoration: const InputDecoration(
                    labelText: 'Код (code)',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: nameController,
                  decoration: const InputDecoration(
                    labelText: 'Наименование (name)',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 8),
                DropdownButtonFormField<String>(
                  initialValue: kind,
                  decoration: const InputDecoration(
                    labelText: 'Вид (kind)',
                    border: OutlineInputBorder(),
                  ),
                  items: [
                    for (final k in _kinds)
                      DropdownMenuItem(value: k, child: Text(k)),
                  ],
                  onChanged: (value) {
                    if (value != null) {
                      setDialogState(() => kind = value);
                    }
                  },
                ),
              ],
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(dialogContext).pop(),
                child: const Text('Отмена'),
              ),
              FilledButton(
                onPressed: () => Navigator.of(dialogContext).pop({
                  'code': codeController.text.trim(),
                  'name': nameController.text.trim(),
                  'kind': kind,
                }),
                child: const Text('Создать'),
              ),
            ],
          ),
        ),
      );
      if (result == null) return;
      final code = result['code']!;
      final name = result['name']!;
      if (code.isEmpty || name.isEmpty) {
        throw const FormatException('code и name обязательны');
      }
      final companyId = ref.read(companyIdProvider);
      await _metadata.createEntityType({
        'entity_type': {
          'code': code,
          'name': name,
          'kind': result['kind'],
          'company_id': companyId,
          'metadata_version': 1,
        },
        'fields': <Object>[],
        'states': <Object>[],
        'transitions': <Object>[],
        'forms': <Object>[],
        'actions': <Object>[],
        'relations': <Object>[],
      });
      ref.invalidate(schemasProvider);
      if (!mounted) return;
      setState(() => _selected = code);
      await _loadSchema(code);
      _snack('Тип создан');
    } on ServerError catch (e) {
      _snack(e.message);
    } catch (e) {
      _snack('Ошибка создания: $e');
    }
  }

  Future<void> _addOrEditField([EntityField? field]) async {
    final codeController = TextEditingController(text: field?.code);
    final labelController = TextEditingController(text: field?.label);
    final orderController = TextEditingController(
      text: (field?.order ?? 0).toString(),
    );
    String dataType = field?.dataType ?? _fieldDataTypes.first;
    bool required = field?.required ?? false;
    try {
      final result = await showDialog<_FieldDraft>(
        context: context,
        builder: (dialogContext) => StatefulBuilder(
          builder: (dialogContext, setDialogState) => AlertDialog(
            title: Text(field == null ? 'Новое поле' : 'Поле ${field.code}'),
            content: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  TextField(
                    controller: codeController,
                    decoration: const InputDecoration(
                      labelText: 'Код (code)',
                      border: OutlineInputBorder(),
                    ),
                  ),
                  const SizedBox(height: 8),
                  TextField(
                    controller: labelController,
                    decoration: const InputDecoration(
                      labelText: 'Метка (label)',
                      border: OutlineInputBorder(),
                    ),
                  ),
                  const SizedBox(height: 8),
                  DropdownButtonFormField<String>(
                    initialValue: dataType,
                    decoration: const InputDecoration(
                      labelText: 'Тип данных',
                      border: OutlineInputBorder(),
                    ),
                    items: [
                      for (final t in _fieldDataTypes)
                        DropdownMenuItem(value: t, child: Text(t)),
                    ],
                    onChanged: (value) {
                      if (value != null) {
                        setDialogState(() => dataType = value);
                      }
                    },
                  ),
                  const SizedBox(height: 8),
                  TextField(
                    controller: orderController,
                    keyboardType: TextInputType.number,
                    decoration: const InputDecoration(
                      labelText: 'Порядок',
                      border: OutlineInputBorder(),
                    ),
                  ),
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    title: const Text('Обязательное'),
                    value: required,
                    onChanged: (value) =>
                        setDialogState(() => required = value),
                  ),
                ],
              ),
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(dialogContext).pop(),
                child: const Text('Отмена'),
              ),
              FilledButton(
                onPressed: () => Navigator.of(dialogContext).pop(_FieldDraft(
                  code: codeController.text.trim(),
                  label: labelController.text.trim(),
                  dataType: dataType,
                  required: required,
                  order: int.tryParse(orderController.text.trim()) ?? 0,
                )),
                child: const Text('ОК'),
              ),
            ],
          ),
        ),
      );
      if (result == null) return;
      if (result.code.isEmpty || result.label.isEmpty) {
        throw const FormatException('code и label обязательны');
      }
      final schema = _schema!;
      final updated = EntityField(
        code: result.code,
        label: result.label,
        dataType: result.dataType,
        required: result.required,
        order: result.order,
      );
      final fields = [
        ...schema.fields.where((f) => f.code != result.code),
        updated,
      ]..sort((a, b) => a.order.compareTo(b.order));
      setState(() => _schema = schema.copyWith(fields: fields));
    } catch (e) {
      _snack('Ошибка поля: $e');
    }
  }

  Future<void> _deleteField(EntityField field) async {
    final ok = await _confirmDelete('Поле ${field.code}');
    if (ok != true) return;
    final schema = _schema!;
    setState(
      () => _schema = schema.copyWith(
        fields: schema.fields.where((f) => f.code != field.code).toList(),
      ),
    );
  }

  Future<void> _addOrEditState([EntityState? state]) async {
    final codeController = TextEditingController(text: state?.code);
    final labelController = TextEditingController(text: state?.label);
    bool isInitial = state?.isInitial ?? false;
    bool isFinal = state?.isFinal ?? false;
    try {
      final result = await showDialog<_StateDraft>(
        context: context,
        builder: (dialogContext) => StatefulBuilder(
          builder: (dialogContext, setDialogState) => AlertDialog(
            title: Text(state == null ? 'Новое состояние' : 'Состояние'),
            content: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  controller: codeController,
                  decoration: const InputDecoration(
                    labelText: 'Код (code)',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: labelController,
                  decoration: const InputDecoration(
                    labelText: 'Метка (label)',
                    border: OutlineInputBorder(),
                  ),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Начальное'),
                  value: isInitial,
                  onChanged: (value) =>
                      setDialogState(() => isInitial = value),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Конечное'),
                  value: isFinal,
                  onChanged: (value) => setDialogState(() => isFinal = value),
                ),
              ],
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(dialogContext).pop(),
                child: const Text('Отмена'),
              ),
              FilledButton(
                onPressed: () => Navigator.of(dialogContext).pop(_StateDraft(
                  code: codeController.text.trim(),
                  label: labelController.text.trim(),
                  isInitial: isInitial,
                  isFinal: isFinal,
                )),
                child: const Text('ОК'),
              ),
            ],
          ),
        ),
      );
      if (result == null) return;
      if (result.code.isEmpty || result.label.isEmpty) {
        throw const FormatException('code и label обязательны');
      }
      final schema = _schema!;
      final updated = EntityState(
        code: result.code,
        label: result.label,
        isInitial: result.isInitial,
        isFinal: result.isFinal,
      );
      final states = [
        ...schema.states.where((s) => s.code != result.code),
        updated,
      ]..sort((a, b) => a.code.compareTo(b.code));
      setState(() => _schema = schema.copyWith(states: states));
    } catch (e) {
      _snack('Ошибка состояния: $e');
    }
  }

  Future<void> _deleteState(EntityState state) async {
    final schema = _schema!;
    if (schema.states.length <= 1) {
      _snack('Нельзя удалить последнее состояние');
      return;
    }
    final ok = await _confirmDelete('Состояние ${state.code}');
    if (ok != true) return;
    setState(() {
      _schema = schema.copyWith(
        states: schema.states.where((s) => s.code != state.code).toList(),
        transitions: schema.transitions
            .where(
              (t) => t.fromState != state.code && t.toState != state.code,
            )
            .toList(),
      );
    });
  }

  Future<void> _addOrEditTransition([EntityTransition? transition]) async {
    final schema = _schema!;
    final stateCodes = schema.states.map((s) => s.code).toList();
    if (stateCodes.isEmpty) {
      _snack('Сначала создайте состояния');
      return;
    }
    final codeController = TextEditingController(text: transition?.code);
    final labelController = TextEditingController(text: transition?.label);
    String fromState = transition?.fromState ?? stateCodes.first;
    String toState = transition?.toState ??
        (stateCodes.length > 1 ? stateCodes[1] : stateCodes.first);
    try {
      final result = await showDialog<_TransitionDraft>(
        context: context,
        builder: (dialogContext) => StatefulBuilder(
          builder: (dialogContext, setDialogState) => AlertDialog(
            title: const Text('Переход'),
            content: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  controller: codeController,
                  decoration: const InputDecoration(
                    labelText: 'Код (code)',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: labelController,
                  decoration: const InputDecoration(
                    labelText: 'Метка (label)',
                    border: OutlineInputBorder(),
                  ),
                ),
                DropdownButtonFormField<String>(
                  initialValue: fromState,
                  items: [
                    for (final c in stateCodes)
                      DropdownMenuItem(value: c, child: Text(c)),
                  ],
                  onChanged: (value) {
                    if (value != null) {
                      setDialogState(() => fromState = value);
                    }
                  },
                ),
                DropdownButtonFormField<String>(
                  initialValue: toState,
                  items: [
                    for (final c in stateCodes)
                      DropdownMenuItem(value: c, child: Text(c)),
                  ],
                  onChanged: (value) {
                    if (value != null) {
                      setDialogState(() => toState = value);
                    }
                  },
                ),
              ],
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(dialogContext).pop(),
                child: const Text('Отмена'),
              ),
              FilledButton(
                onPressed: () => Navigator.of(dialogContext).pop(
                  _TransitionDraft(
                    code: codeController.text.trim(),
                    label: labelController.text.trim(),
                    fromState: fromState,
                    toState: toState,
                  ),
                ),
                child: const Text('ОК'),
              ),
            ],
          ),
        ),
      );
      if (result == null) return;
      if (result.code.isEmpty) {
        throw const FormatException('code обязателен');
      }
      final updated = EntityTransition(
        code: result.code,
        label: result.label,
        fromState: result.fromState,
        toState: result.toState,
      );
      final transitions = [
        ...schema.transitions.where((t) => t.code != result.code),
        updated,
      ]..sort((a, b) => a.code.compareTo(b.code));
      setState(() => _schema = schema.copyWith(transitions: transitions));
    } catch (e) {
      _snack('Ошибка перехода: $e');
    }
  }

  Future<void> _deleteTransition(EntityTransition transition) async {
    final ok = await _confirmDelete('Переход ${transition.code}');
    if (ok != true) return;
    final schema = _schema!;
    setState(
      () => _schema = schema.copyWith(
        transitions:
            schema.transitions.where((t) => t.code != transition.code).toList(),
      ),
    );
  }

  Future<bool?> _confirmDelete(String what) {
    return showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text('Удалить $what?'),
        content: const Text('Изменение применится после «Сохранить».'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('Отмена'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('Удалить'),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final types = ref.watch(schemasProvider);
    final selected = _selected;

    return Scaffold(
      appBar: AppBar(title: const Text('Редактор метаданных')),
      body: types.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Text('Не удалось загрузить типы: $e'),
          ),
        ),
        data: (typeList) {
          final codes = typeList.map((t) => t.code).toList();
          final current = selected != null && codes.contains(selected)
              ? selected
              : null;
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.all(16),
                child: Row(
                  children: [
                    Expanded(
                      child: DropdownButtonFormField<String?>(
                        initialValue: current,
                        decoration: const InputDecoration(
                          labelText: 'Тип сущности',
                          border: OutlineInputBorder(),
                        ),
                        items: [
                          for (final code in codes)
                            DropdownMenuItem(
                              value: code,
                              child: Text(
                                code,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                        ],
                        onChanged: (code) {
                          if (code == null) return;
                          setState(() => _selected = code);
                          _loadSchema(code);
                        },
                      ),
                    ),
                    const SizedBox(width: 8),
                    OutlinedButton.icon(
                      onPressed: _createNewType,
                      icon: const Icon(Icons.add),
                      label: const Text('Новый тип'),
                    ),
                  ],
                ),
              ),
              Expanded(
                child: _busy
                    ? const Center(child: CircularProgressIndicator())
                    : _schema == null
                        ? Center(
                            child: Text(
                              current == null
                                  ? 'Выберите тип для редактирования'
                                  : 'Схема не загружена',
                              style: Theme.of(context).textTheme.bodyMedium,
                            ),
                          )
                        : _editorBody(),
              ),
              if (_schema != null)
                Padding(
                  padding: const EdgeInsets.all(16),
                  child: Wrap(
                    spacing: 8,
                    children: [
                      FilledButton.icon(
                        onPressed: _save,
                        icon: const Icon(Icons.save),
                        label: const Text('Сохранить'),
                      ),
                      OutlinedButton.icon(
                        onPressed: _exportJson,
                        icon: const Icon(Icons.ios_share),
                        label: const Text('Схема JSON'),
                      ),
                      OutlinedButton.icon(
                        onPressed: _importJson,
                        icon: const Icon(Icons.file_download_outlined),
                        label: const Text('Импорт JSON'),
                      ),
                    ],
                  ),
                ),
            ],
          );
        },
      ),
    );
  }

  Widget _editorBody() {
    final schema = _schema!;
    return SingleChildScrollView(
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
        ListTile(
          title: Text(schema.entityType.name),
          subtitle: Text(
            '${schema.entityType.code} · ${schema.entityType.kind} · '
            'версия ${schema.entityType.metadataVersion}',
          ),
        ),
        _sectionHeader('Поля', onAdd: () => _addOrEditField()),
        if (schema.fields.isEmpty)
          const ListTile(title: Text('Поля не заданы')),
        for (final field in schema.fields)
          ListTile(
            leading: const Icon(Icons.label_outline),
            title: Text('${field.code} — ${field.label}'),
            subtitle: Text(
              '${field.dataType} · ${field.required ? 'обязательный' : 'необязательный'}'
              '${field.order != 0 ? ' · порядок ${field.order}' : ''}',
            ),
            trailing: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                IconButton(
                  icon: const Icon(Icons.edit_outlined),
                  onPressed: () => _addOrEditField(field),
                ),
                IconButton(
                  icon: const Icon(Icons.delete_outline),
                  onPressed: () => _deleteField(field),
                ),
              ],
            ),
          ),
        _sectionHeader('Состояния', onAdd: () => _addOrEditState()),
        if (schema.states.isEmpty)
          const ListTile(title: Text('Состояния не заданы')),
        for (final state in schema.states)
          ListTile(
            leading: const Icon(Icons.lens_outlined),
            title: Text('${state.code} — ${state.label}'),
            subtitle: Text(
              [
                if (state.isInitial) 'начальное',
                if (state.isFinal) 'конечное',
              ].join(', '),
            ),
            trailing: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                IconButton(
                  icon: const Icon(Icons.edit_outlined),
                  onPressed: () => _addOrEditState(state),
                ),
                IconButton(
                  icon: const Icon(Icons.delete_outline),
                  onPressed: () => _deleteState(state),
                ),
              ],
            ),
          ),
        _sectionHeader('Переходы', onAdd: () => _addOrEditTransition()),
        if (schema.transitions.isEmpty)
          const ListTile(title: Text('Переходы не заданы')),
        for (final transition in schema.transitions)
          ListTile(
            leading: const Icon(Icons.swap_vert),
            title: Text(
              '${transition.code} · ${transition.fromState} → '
              '${transition.toState}',
            ),
            subtitle: Text(transition.label),
            trailing: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                IconButton(
                  icon: const Icon(Icons.edit_outlined),
                  onPressed: () => _addOrEditTransition(transition),
                ),
                IconButton(
                  icon: const Icon(Icons.delete_outline),
                  onPressed: () => _deleteTransition(transition),
                ),
              ],
            ),
          ),
        const SizedBox(height: 24),
        ],
      ),
    );
  }

  Widget _sectionHeader(String title, {required VoidCallback onAdd}) {
    return Padding(
      padding: const EdgeInsets.only(top: 16, bottom: 4),
      child: Row(
        children: [
          Expanded(
            child: Text(
              title,
              style: Theme.of(context).textTheme.titleSmall,
            ),
          ),
          IconButton(
            icon: const Icon(Icons.add_circle_outline),
            tooltip: 'Добавить $title',
            onPressed: onAdd,
          ),
        ],
      ),
    );
  }
}

class _FieldDraft {
  const _FieldDraft({
    required this.code,
    required this.label,
    required this.dataType,
    required this.required,
    required this.order,
  });

  final String code;
  final String label;
  final String dataType;
  final bool required;
  final int order;
}

class _StateDraft {
  const _StateDraft({
    required this.code,
    required this.label,
    required this.isInitial,
    required this.isFinal,
  });

  final String code;
  final String label;
  final bool isInitial;
  final bool isFinal;
}

class _TransitionDraft {
  const _TransitionDraft({
    required this.code,
    required this.label,
    required this.fromState,
    required this.toState,
  });

  final String code;
  final String label;
  final String fromState;
  final String toState;
}