import 'package:flutter/widgets.dart';

import '../models/entity_schema.dart';
import 'widgets/array_field_widget.dart';
import 'widgets/choice_field_widgets.dart';
import 'widgets/date_time_field_widgets.dart';
import 'widgets/json_field_widget.dart';
import 'widgets/number_field_widgets.dart';
import 'widgets/readonly_field_widget.dart';
import 'widgets/reference_field_widget.dart';
import 'widgets/text_field_widgets.dart';

/// Билдер виджета поля по типу метаданных (`FieldType` → виджет).
///
/// `value` — текущее значение из `object.data`/`computed`; `onChanged`
/// вызывается только с типизированным значением (например, копейки `int` для
/// `money`, `'YYYY-MM-DD'` для `date`). В режиме `readOnly` виджет не вызывает
/// `onChanged`.
typedef FieldWidgetBuilder =
    Widget Function(
      BuildContext context,
      EntityField field,
      Object? value,
      ValueChanged<Object?> onChanged,
      bool readOnly,
    );

/// Реестр SDUI-виджетов: маппинг `data_type` → билдер.
class WidgetRegistry {
  WidgetRegistry() {
    _installDefault();
  }

  /// Общий экземпляр со стандартными виджетами (для экранов).
  static final WidgetRegistry instance = WidgetRegistry();

  final Map<String, FieldWidgetBuilder> _builders = {};

  void register(String fieldType, FieldWidgetBuilder builder) {
    _builders[fieldType] = builder;
  }

  bool has(String fieldType) => _builders.containsKey(fieldType);

  /// Собирает виджет для поля. Для неизвестного типа — read-only заглушка.
  Widget build(
    BuildContext context,
    EntityField field,
    Object? value,
    ValueChanged<Object?> onChanged,
    bool readOnly,
  ) {
    final builder =
        _builders[field.dataType] ?? _readOnlyFallbackBuilder;
    return builder(context, field, value, onChanged, readOnly);
  }

  void _installDefault() {
    register('string', (c, f, v, o, r) => SduiTextField(f, v, o, r));
    register('text', (c, f, v, o, r) => SduiMultiTextField(f, v, o, r));
    register('integer', (c, f, v, o, r) => SduiIntegerField(f, v, o, r));
    register('money', (c, f, v, o, r) => SduiMoneyField(f, v, o, r));
    register('date', (c, f, v, o, r) => SduiDateField(f, v, o, r));
    register('datetime', (c, f, v, o, r) => SduiDateTimeField(f, v, o, r));
    register('boolean', (c, f, v, o, r) => SduiBooleanField(f, v, o, r));
    register('enum', (c, f, v, o, r) => SduiEnumField(f, v, o, r));
    register(
      'reference',
      (c, f, v, o, r) => SduiReferenceField(f, v, o, r),
    );
    register('array', (c, f, v, o, r) => SduiArrayField(f, v, o, r));
    register('table', (c, f, v, o, r) => SduiArrayField(f, v, o, r));
    register('json', (c, f, v, o, r) => SduiJsonField(f, v, o, r));
  }
}

/// Fallback для нестандартных типов: только отображение значения.
Widget _readOnlyFallbackBuilder(
  BuildContext context,
  EntityField field,
  Object? value,
  ValueChanged<Object?> onChanged,
  bool readOnly,
) {
  return SduiReadOnlyField(field, value);
}