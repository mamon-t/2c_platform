import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../models/entity_schema.dart';

/// Результат разбора текстового ввода в типизированное значение.
class ParseResult {
  const ParseResult.invalid({this.error})
      : valid = false,
        value = null;
  const ParseResult.valid(this.value)
      : valid = true,
        error = null;

  final bool valid;
  final Object? value;
  final String? error;
}

/// Состояние ввода для текстовых виджетов (`string`, `integer`, `money`,
/// `json`, строки массивов): контроллер + разбор + подсветка ошибки.
///
/// `format` переводит типизированное значение в начальный текст,
/// `parse` — текст в типизированное значение (невалидное → ошибка).
class TextInputField extends StatefulWidget {
  const TextInputField({
    super.key,
    required this.field,
    required this.value,
    required this.onChanged,
    required this.readOnly,
    required this.format,
    required this.parse,
    this.keyboardType,
    this.maxLines = 1,
    this.inputFormatters,
  });

  final EntityField field;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final bool readOnly;
  final String Function(Object? value) format;
  final ParseResult Function(String text) parse;
  final TextInputType? keyboardType;
  final int maxLines;
  final List<TextInputFormatter>? inputFormatters;

  @override
  State<TextInputField> createState() => _TextInputFieldState();
}

class _TextInputFieldState extends State<TextInputField> {
  late final TextEditingController _controller;
  String? _error;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(text: widget.format(widget.value));
  }

  @override
  void didUpdateWidget(TextInputField oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.value != widget.value) {
      final text = widget.format(widget.value);
      if (text != _controller.text) {
        _controller.text = text;
        _error = null;
      }
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _onTextChanged(String text) {
    setState(() {
      if (text.trim().isEmpty) {
        // Поле очищено — сбрасываем значение и ошибку.
        _error = null;
        widget.onChanged(null);
        return;
      }
      final result = widget.parse(text);
      if (result.valid) {
        _error = null;
        widget.onChanged(result.value);
      } else {
        _error = result.error;
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      controller: _controller,
      enabled: !widget.readOnly,
      keyboardType: widget.keyboardType,
      maxLines: widget.maxLines,
      inputFormatters: widget.inputFormatters,
      decoration: InputDecoration(
        labelText: widget.field.label,
        errorText: _error,
      ),
      onChanged: _onTextChanged,
    );
  }
}