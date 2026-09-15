import 'dart:convert';

import 'package:flutter/material.dart';

import '../../models/entity_schema.dart';
import 'text_input_base.dart';

/// Поле свободного JSON (`json`); значение wire — любой JSON.
/// Разбор выполняется при каждом изменении; невалидный JSON подсвечивается.
class SduiJsonField extends StatelessWidget {
  const SduiJsonField(this.field, this.value, this.onChanged, this.readOnly,
      {super.key});

  final EntityField field;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final bool readOnly;

  @override
  Widget build(BuildContext context) {
    final raw = value;
    return TextInputField(
      field: field,
      value: raw,
      onChanged: onChanged,
      readOnly: readOnly,
      maxLines: 8,
      format: (v) {
        if (v == null) {
          return '';
        }
        if (v is String) {
          return v;
        }
        return jsonEncode(v);
      },
      parse: (text) {
        final trimmed = text.trim();
        if (trimmed.isEmpty) {
          return const ParseResult.valid(null);
        }
        try {
          return ParseResult.valid(jsonDecode(trimmed));
        } on FormatException {
          return const ParseResult.invalid(error: 'Некорректный JSON');
        }
      },
    );
  }
}