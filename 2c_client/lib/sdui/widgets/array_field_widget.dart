import 'package:flutter/material.dart';

import '../../models/entity_schema.dart';

class _ArrayRow {
  _ArrayRow(this.id, this.text);
  final int id;
  String text;
}

/// Поле-массив (`array`/`table`); значение wire — `List`.
///
/// Строки редактируются как текст (v0 SDUI); массив передаётся только
/// непустым. Read-only режим скрывает кнопки добавления/удаления.
class SduiArrayField extends StatefulWidget {
  const SduiArrayField(this.field, this.value, this.onChanged, this.readOnly,
      {super.key});

  final EntityField field;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final bool readOnly;

  @override
  State<SduiArrayField> createState() => _SduiArrayFieldState();
}

class _SduiArrayFieldState extends State<SduiArrayField> {
  late final List<_ArrayRow> _rows = _init();
  int _nextId = 0;

  List<_ArrayRow> _init() {
    final result = <_ArrayRow>[];
    final raw = widget.value;
    if (raw is List) {
      for (final item in raw) {
        result.add(_ArrayRow(_nextId++, item.toString()));
      }
    }
    return result;
  }

  void _emit() {
    final values = _rows
        .map((r) => r.text.trim())
        .where((t) => t.isNotEmpty)
        .toList();
    widget.onChanged(values.isEmpty ? null : values);
  }

  void _change(int id, String text) {
    setState(() {
      for (final row in _rows) {
        if (row.id == id) {
          row.text = text;
        }
      }
    });
    _emit();
  }

  void _add() {
    setState(() => _rows.add(_ArrayRow(_nextId++, '')));
  }

  void _remove(int id) {
    setState(() => _rows.removeWhere((r) => r.id == id));
    _emit();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.only(top: 8, bottom: 4),
          child: Text(
            widget.field.label,
            style: Theme.of(context).textTheme.labelLarge,
          ),
        ),
        for (final row in _rows)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 4),
            child: Row(
              key: ValueKey(row.id),
              children: [
                Expanded(
                  child: TextField(
                    enabled: !widget.readOnly,
                    controller: TextEditingController(text: row.text),
                    onChanged: (t) => _change(row.id, t),
                    decoration: InputDecoration(
                      hintText: 'Элемент ${row.id + 1}',
                      isDense: true,
                    ),
                  ),
                ),
                if (!widget.readOnly)
                  IconButton(
                    icon: const Icon(Icons.remove_circle_outline),
                    tooltip: 'Удалить',
                    onPressed: () => _remove(row.id),
                  ),
              ],
            ),
          ),
        if (!widget.readOnly)
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              icon: const Icon(Icons.add),
              label: const Text('Добавить элемент'),
              onPressed: _add,
            ),
          ),
      ],
    );
  }
}