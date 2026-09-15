import 'package:flutter/material.dart';

import '../../models/entity_schema.dart';
import 'text_input_base.dart';

/// Однострочное текстовое поле (`string`).
class SduiTextField extends StatelessWidget {
  const SduiTextField(this.field, this.value, this.onChanged, this.readOnly,
      {super.key});

  final EntityField field;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final bool readOnly;

  @override
  Widget build(BuildContext context) {
    return TextInputField(
      field: field,
      value: value,
      onChanged: onChanged,
      readOnly: readOnly,
      format: (v) => v is String ? v : '',
      parse: (text) => ParseResult.valid(text),
    );
  }
}

/// Многострочное текстовое поле (`text`).
class SduiMultiTextField extends StatelessWidget {
  const SduiMultiTextField(this.field, this.value, this.onChanged, this.readOnly,
      {super.key});

  final EntityField field;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final bool readOnly;

  @override
  Widget build(BuildContext context) {
    return TextInputField(
      field: field,
      value: value,
      onChanged: onChanged,
      readOnly: readOnly,
      maxLines: 4,
      format: (v) => v is String ? v : '',
      parse: (text) => ParseResult.valid(text),
    );
  }
}