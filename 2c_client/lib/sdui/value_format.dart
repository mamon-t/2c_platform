import 'dart:convert';

import '../models/entity_schema.dart';
import '../sdui/widgets/number_field_widgets.dart';

/// Отображает значение поля в таблице каталога / read-only контекстах.
String displayValue(EntityField field, Object? value) {
  if (value == null) {
    return '—';
  }
  if (value is String && value.isEmpty) {
    return '—';
  }
  if (field.dataType == 'money') {
    final kopeks = value is int ? value : int.tryParse('$value');
    if (kopeks != null) {
      return SduiMoneyField.format(kopeks);
    }
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