import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../models/entity_schema.dart';
import 'text_input_base.dart';

/// Целочисленное поле (`integer`); значение wire — `int`.
class SduiIntegerField extends StatelessWidget {
  const SduiIntegerField(this.field, this.value, this.onChanged, this.readOnly,
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
      keyboardType: const TextInputType.numberWithOptions(
        signed: true,
        decimal: false,
      ),
      inputFormatters: [
        FilteringTextInputFormatter.allow(RegExp(r'-?\d*')),
      ],
      format: (v) => v is int ? v.toString() : (v is String ? v : ''),
      parse: (text) {
        final parsed = int.tryParse(text);
        if (parsed == null) {
          return const ParseResult.invalid(error: 'Введите целое число');
        }
        return ParseResult.valid(parsed);
      },
    );
  }
}

/// Денежное поле (`money`); значение wire — целые копейки (`int`),
/// отображаются с двумя знаками после запятой.
class SduiMoneyField extends StatelessWidget {
  const SduiMoneyField(this.field, this.value, this.onChanged, this.readOnly,
      {super.key});

  final EntityField field;
  final Object? value;
  final ValueChanged<Object?> onChanged;
  final bool readOnly;

  static final RegExp _money = RegExp(r'^\s*-?\d+([.,]\d{0,2})?\s*$');

  static String format(Object? v) {
    if (v is int) {
      return (v / 100).toStringAsFixed(2).replaceAll('.', ',');
    }
    if (v is String) {
      final parsed = int.tryParse(v);
      if (parsed != null) {
        return (parsed / 100).toStringAsFixed(2).replaceAll('.', ',');
      }
    }
    return '';
  }

  /// Переводит введённый текст (рубли, разделитель `,` или `.`) в копейки.
  static Object? parseKopeks(String text) {
    final t = text.trim().replaceAll(',', '.');
    if (!_money.hasMatch(t)) {
      return null;
    }
    if (t.startsWith('-')) {
      final positive = parseKopeks(t.substring(1));
      return positive is int ? -positive : null;
    }
    if (!t.contains('.')) {
      return int.tryParse(t);
    }
    final parts = t.split('.');
    final whole = int.tryParse(parts[0]);
    if (whole == null) {
      return null;
    }
    final fraction = parts[1].padRight(2, '0');
    final frac = int.tryParse(fraction);
    if (frac == null) {
      return null;
    }
    return whole * 100 + frac;
  }

  @override
  Widget build(BuildContext context) {
    return TextInputField(
      field: field,
      value: value,
      onChanged: onChanged,
      readOnly: readOnly,
      keyboardType: const TextInputType.numberWithOptions(
        signed: true,
        decimal: true,
      ),
      format: format,
      parse: (text) {
        final kopeks = parseKopeks(text);
        if (kopeks == null) {
          return const ParseResult.invalid(
            error: 'Введите сумму с двумя знаками',
          );
        }
        return ParseResult.valid(kopeks);
      },
    );
  }
}