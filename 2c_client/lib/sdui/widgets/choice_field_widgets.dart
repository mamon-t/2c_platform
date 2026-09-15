import 'package:flutter/material.dart';

import '../../models/entity_schema.dart';

/// Переключатель (`boolean`); значение wire — `bool`.
class SduiBooleanField extends StatelessWidget {
  const SduiBooleanField(this.field, this.value, this.onChanged, this.readOnly,
      {super.key});

  final EntityField field;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final bool readOnly;

  @override
  Widget build(BuildContext context) {
    final raw = value;
    return SwitchListTile(
      title: Text(field.label),
      value: raw is bool ? raw : false,
      onChanged: readOnly ? null : (v) => onChanged(v),
    );
  }
}

/// Выпадающий список (`enum`); варианты из `EntityField.enumValues`,
/// значение wire — строка.
class SduiEnumField extends StatelessWidget {
  const SduiEnumField(this.field, this.value, this.onChanged, this.readOnly,
      {super.key});

  final EntityField field;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final bool readOnly;

  @override
  Widget build(BuildContext context) {
    final variants = field.enumValues;
    final raw = value;
    final String? current = raw is String ? raw : null;
    return DropdownButtonFormField<String>(
      initialValue:
          (current != null && variants.contains(current)) ? current : null,
      isExpanded: true,
      decoration: InputDecoration(labelText: field.label),
      items: [
        for (final v in variants)
          DropdownMenuItem<String>(value: v, child: Text(v)),
      ],
      onChanged: readOnly ? null : (v) => v != null ? onChanged(v) : null,
    );
  }
}