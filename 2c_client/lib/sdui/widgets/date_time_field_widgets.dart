import 'package:flutter/material.dart';

import '../../models/entity_schema.dart';

String _two(int n) => n.toString().padLeft(2, '0');

/// Форматирует дату как 'dd.MM.yyyy' (просмотр в picker-поле).
String formatDate(DateTime d) => '${_two(d.day)}.${_two(d.month)}.${d.year}';

/// Форматирует datetime как 'dd.MM.yyyy HH:mm'.
String formatDateTime(DateTime d) =>
    '${formatDate(d)} ${_two(d.hour)}:${_two(d.minute)}';

/// Кнопка-поле выбора даты (`date`). Значение wire — 'YYYY-MM-DD'.
class SduiDateField extends StatefulWidget {
  const SduiDateField(this.field, this.value, this.onChanged, this.readOnly,
      {super.key});

  final EntityField field;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final bool readOnly;

  @override
  State<SduiDateField> createState() => _SduiDateFieldState();
}

class _SduiDateFieldState extends State<SduiDateField> {
  late String _text = _display(widget.value);

  static String _display(Object? value) {
    if (value is String) {
      final parsed = DateTime.tryParse(value);
      if (parsed != null) {
        return formatDate(parsed);
      }
    }
    return '';
  }

  Future<void> _pick() async {
    final initial = DateTime.tryParse(widget.value is String
        ? widget.value! as String
        : '');
    final picked = await showDatePicker(
      context: context,
      initialDate: initial ?? DateTime.now(),
      firstDate: DateTime(1900),
      lastDate: DateTime.now().add(const Duration(days: 365 * 100)),
    );
    if (picked == null) {
      return;
    }
    final wire = '${picked.year.toString().padLeft(4, '0')}-'
        '${_two(picked.month)}-${_two(picked.day)}';
    if (!mounted) {
      return;
    }
    setState(() => _text = formatDate(picked));
    widget.onChanged(wire);
  }

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      readOnly: true,
      enabled: !widget.readOnly,
      controller: TextEditingController(text: _text),
      decoration: InputDecoration(labelText: widget.field.label),
      onTap: widget.readOnly ? null : _pick,
    );
  }
}

/// Кнопка-поле выбора даты и времени (`datetime`). Значение wire — RFC3339.
class SduiDateTimeField extends StatefulWidget {
  const SduiDateTimeField(this.field, this.value, this.onChanged, this.readOnly,
      {super.key});

  final EntityField field;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final bool readOnly;

  @override
  State<SduiDateTimeField> createState() => _SduiDateTimeFieldState();
}

class _SduiDateTimeFieldState extends State<SduiDateTimeField> {
  late String _text = _display(widget.value);

  static String _display(Object? value) {
    if (value is String) {
      final parsed = DateTime.tryParse(value);
      if (parsed != null) {
        return formatDateTime(parsed.toLocal());
      }
    }
    return '';
  }

  Future<void> _pick() async {
    final initial = widget.value is String
        ? DateTime.tryParse(widget.value! as String)
        : null;
    final day = await showDatePicker(
      context: context,
      initialDate: initial ?? DateTime.now(),
      firstDate: DateTime(1900),
      lastDate: DateTime.now().add(const Duration(days: 365 * 100)),
    );
    if (day == null) {
      return;
    }
    if (!mounted) {
      return;
    }
    final time = await showTimePicker(
      context: context,
      initialTime: TimeOfDay.fromDateTime(initial ?? DateTime.now()),
    );
    final combined = DateTime(
      day.year,
      day.month,
      day.day,
      time?.hour ?? 12,
      time?.minute ?? 0,
    );
    if (!mounted) {
      return;
    }
    setState(() => _text = formatDateTime(combined));
    widget.onChanged(combined.toUtc().toIso8601String());
  }

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      readOnly: true,
      enabled: !widget.readOnly,
      controller: TextEditingController(text: _text),
      decoration: InputDecoration(labelText: widget.field.label),
      onTap: widget.readOnly ? null : _pick,
    );
  }
}