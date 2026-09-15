import 'package:flutter/material.dart';

import '../../models/entity_schema.dart';
import 'readonly_field_widget.dart';

/// Поле ссылки (`reference`); значение wire — строка-идентификатор.
///
/// Для выбора нужен список кандидатов из `field.options['items']`
/// (список значений или пар `{label, value}`). Если список недоступен —
/// значение отображается только для чтения.
class SduiReferenceField extends StatelessWidget {
  const SduiReferenceField(this.field, this.value, this.onChanged, this.readOnly,
      {super.key});

  final EntityField field;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final bool readOnly;

  /// Кандидаты выбора: `Map<value,label>` (порядок мапы сохраняет перечисление).
  static Map<String, String> candidatesOf(EntityField field) {
    final o = field.options;
    final result = <String, String>{};
    if (o is Map) {
      final items = o['items'];
      if (items is List) {
        for (final item in items) {
          if (item is String) {
            result[item] = item;
          } else if (item is Map) {
            final value = item['value'];
            final label = item['label'];
            if (value is String) {
              result[value] = label is String ? label : value;
            }
          }
        }
      }
    } else if (o is List) {
      for (final item in o) {
        if (item is String) {
          result[item] = item;
        }
      }
    }
    return result;
  }

  @override
  Widget build(BuildContext context) {
    final candidates = candidatesOf(field);
    if (candidates.isEmpty) {
      return SduiReadOnlyField(field, value,
          label: '${field.label} (ссылка)');
    }
    final raw = value;
    final current = raw is String && candidates.containsKey(raw) ? raw : null;
    return DropdownButtonFormField<String>(
      initialValue: current,
      isExpanded: true,
      decoration: InputDecoration(labelText: field.label),
      items: [
        for (final entry in candidates.entries)
          DropdownMenuItem<String>(value: entry.key, child: Text(entry.value)),
      ],
      onChanged: readOnly ? null : (v) => v != null ? onChanged(v) : null,
    );
  }
}