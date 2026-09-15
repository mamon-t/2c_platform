import 'dart:convert';

import 'package:flutter/material.dart';

import '../../models/entity_schema.dart';

/// Read-only отображение значения поля (для типов без типового виджета:
/// `file`, `user`, `company`, `formula`, `computed` и неизвестных типов).
class SduiReadOnlyField extends StatelessWidget {
  const SduiReadOnlyField(this.field, this.value, {this.label, super.key});

  final EntityField field;
  final Object? value;
  final String? label;

  static String format(Object? value) {
    if (value == null) {
      return '—';
    }
    if (value is String) {
      return value.isEmpty ? '—' : value;
    }
    if (value is bool) {
      return value ? 'да' : 'нет';
    }
    if (value is List) {
      return value.isEmpty ? '—' : value.join(', ');
    }
    if (value is Map) {
      return jsonEncode(value);
    }
    return value.toString();
  }

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      readOnly: true,
      initialValue: format(value),
      decoration: InputDecoration(labelText: label ?? field.label),
    );
  }
}

/// Отображает значение поля `computed`/`formula` из `object.computed`.
class SduiComputedField extends StatelessWidget {
  const SduiComputedField(this.field, this.value, {this.label, super.key});

  final EntityField field;
  final Object? value;
  final String? label;

  @override
  Widget build(BuildContext context) {
    return SduiReadOnlyField(field, value, label: label);
  }
}