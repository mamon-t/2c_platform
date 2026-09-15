import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../models/entity_schema.dart';
import '../models/object.dart';
import '../models/server_error.dart';
import '../providers/sdui_providers.dart';
import '../sdui/widget_registry.dart';
import '../services/object_service.dart';

/// Режим формы объекта.
enum ObjectFormMode { create, view, edit }

/// Экран объекта СДУИ: просмотр / создание / редактирование (ТЗ §11).
///
/// Поля строятся из метаданных `metadata.schema.get` в порядке `order`,
/// значения — из `object.data` (для `formula`/`computed` — из
/// `object.computed`). Валидация `required` локальная; сохранение — строгий
/// OCC (`expected_version`), при конфликте показывается диалог.
class ObjectFormScreen extends ConsumerStatefulWidget {
  const ObjectFormScreen({
    super.key,
    required this.entityType,
    required this.id,
    required this.mode,
  });

  final String entityType;

  /// Идентификатор объекта. Для создания — `'new'` (роут с этим сегментом).
  final String id;

  final ObjectFormMode mode;

  @override
  ConsumerState<ObjectFormScreen> createState() => _ObjectFormScreenState();
}

class _ObjectFormScreenState extends ConsumerState<ObjectFormScreen> {
  static const _readOnlyTypes = {
    'formula',
    'computed',
    'file',
    'user',
    'company',
  };

  final _values = <String, dynamic>{};
  final _scroll = ScrollController();
  late final Future<ObjectItem?> _objectFuture;
  bool _initialized = false;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _objectFuture = widget.mode == ObjectFormMode.create
        ? Future.value(null)
        : ref
            .read(objectServiceProvider)
            .get(widget.id)
            .then<ObjectItem?>((o) => o);
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  bool get _readOnly => widget.mode == ObjectFormMode.view;

  static bool _isEditable(EntityField f) => !_readOnlyTypes.contains(f.dataType);

  static bool _isMissing(Object? v) {
    if (v == null) {
      return true;
    }
    if (v is String && v.trim().isEmpty) {
      return true;
    }
    if (v is List && v.isEmpty) {
      return true;
    }
    if (v is Map && v.isEmpty) {
      return true;
    }
    return false;
  }

  void _initValues(EntitySchema schema, ObjectItem? object) {
    _initialized = true;
    for (final f in schema.orderedFields) {
      if (!_isEditable(f)) {
        continue;
      }
      final fromData = object?.data[f.code];
      _values[f.code] = fromData;
      if (fromData == null &&
          f.dataType == 'boolean' &&
          widget.mode == ObjectFormMode.create) {
        _values[f.code] = false;
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final schemaAsync = ref.watch(schemaProvider(widget.entityType));
    return schemaAsync.when(
      loading: () =>
          _scaffold('Загрузка…', const Center(child: CircularProgressIndicator())),
      error: (e, _) => _scaffold(
        'Ошибка',
        Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Text('Не удалось загрузить схему: $e'),
          ),
        ),
      ),
      data: (schema) => FutureBuilder<ObjectItem?>(
        future: _objectFuture,
        builder: (context, snapshot) {
          if (snapshot.connectionState == ConnectionState.waiting) {
            return _scaffold(
              'Загрузка…',
              const Center(child: CircularProgressIndicator()),
            );
          }
          if (snapshot.hasError) {
            return _scaffold(
              'Ошибка',
              Center(
                child: Padding(
                  padding: const EdgeInsets.all(24),
                  child: Text('Не удалось загрузить объект: ${snapshot.error}'),
                ),
              ),
            );
          }
          final object = snapshot.data;
          if (!_initialized) {
            _initValues(schema, object);
          }
          return _buildForm(schema, object);
        },
      ),
    );
  }

  Widget _scaffold(String? title, Widget body) => Scaffold(
        appBar: AppBar(title: Text(title ?? 'Объект')),
        body: body,
      );

  Widget _buildForm(EntitySchema schema, ObjectItem? object) {
    final canEdit = widget.mode == ObjectFormMode.view && object != null;
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.mode == ObjectFormMode.create
            ? 'Новый ${schema.entityType.name}'
            : schema.entityType.name),
        actions: [
          if (canEdit)
            IconButton(
              icon: const Icon(Icons.edit),
              tooltip: 'Редактировать',
              onPressed: () => context
                  .push('/object/${widget.entityType}/${widget.id}/edit'),
            ),
        ],
      ),
      body: Form(
        child: ListView(
          controller: _scroll,
          padding: const EdgeInsets.all(16),
          children: [
            for (final f in schema.orderedFields)
              _fieldFor(schema, object, f),
            const SizedBox(height: 24),
            if (!_readOnly) _actions(schema, object),
          ],
        ),
      ),
    );
  }

  Widget _actions(EntitySchema schema, ObjectItem? object) {
    return Row(
      children: [
        Expanded(
          child: ElevatedButton(
            onPressed: _saving ? null : () => _save(schema, object),
            child: _saving
                ? const SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Text('Сохранить'),
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: OutlinedButton(
            onPressed: () => context.pop(),
            child: const Text('Отмена'),
          ),
        ),
      ],
    );
  }

  Widget _fieldFor(EntitySchema schema, ObjectItem? object, EntityField f) {
    final editable = _isEditable(f);
    final readOnly = _readOnly || !editable;
    final Object? value;
    if (f.dataType == 'formula' || f.dataType == 'computed') {
      value = object?.computed[f.code] ?? object?.data[f.code];
    } else if (editable) {
      value = _values[f.code];
    } else {
      value = object?.data[f.code] ?? _values[f.code];
    }
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: WidgetRegistry.instance.build(
        context,
        f,
        value,
        (v) => setState(() => _values[f.code] = v),
        readOnly,
      ),
    );
  }

  Map<String, dynamic> _collectData(EntitySchema schema) {
    final data = <String, dynamic>{};
    for (final f in schema.orderedFields) {
      if (!_isEditable(f)) {
        continue;
      }
      final v = _values[f.code];
      if (_isMissing(v)) {
        continue;
      }
      data[f.code] = v;
    }
    return data;
  }

  Future<void> _save(EntitySchema schema, ObjectItem? object) async {
    final missing = [
      for (final f in schema.orderedFields)
        if (f.required && _isMissing(_values[f.code])) f.label,
    ];
    if (missing.isNotEmpty) {
      final messenger = ScaffoldMessenger.of(context);
      messenger.showSnackBar(
        SnackBar(
            content: Text('Заполните обязательные поля: ${missing.join(', ')}')),
      );
      return;
    }

    final data = _collectData(schema);
    final messenger = ScaffoldMessenger.of(context);
    setState(() => _saving = true);
    try {
      if (widget.mode == ObjectFormMode.create) {
        await ref.read(objectServiceProvider).create(
              widget.entityType,
              schema.entityType.kind,
              data,
              companyId: ref.read(companyIdProvider),
            );
      } else {
        final version = object?.version;
        if (version == null) {
          messenger.showSnackBar(
            const SnackBar(content: Text('Не удалось сохранить: объект не загружен')),
          );
          return;
        }
        await ref.read(objectServiceProvider).update(
              widget.id,
              version,
              data: data,
            );
      }
      messenger.showSnackBar(const SnackBar(content: Text('Сохранено')));
      if (!mounted) {
        return;
      }
      if (context.canPop()) {
        context.pop();
      } else {
        context.go('/catalog/${widget.entityType}');
      }
    } on ServerError catch (e) {
      if (!mounted) {
        return;
      }
      setState(() => _saving = false);
      if (e.code == ErrorCode.conflict) {
        await showDialog<void>(
          context: context,
          builder: (context) => AlertDialog(
            title: const Text('Конфликт версий'),
            content: const Text(
              'Объект изменён другим пользователем. Откройте его заново и повторите сохранение.',
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(context).pop(),
                child: const Text('OK'),
              ),
            ],
          ),
        );
      } else {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(e.localizedMessage)),
        );
      }
    }
  }
}