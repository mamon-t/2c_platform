import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/entity_schema.dart';
import 'package:twoc_client/sdui/widget_registry.dart';
import 'package:twoc_client/sdui/widgets/array_field_widget.dart';
import 'package:twoc_client/sdui/widgets/choice_field_widgets.dart';
import 'package:twoc_client/sdui/widgets/date_time_field_widgets.dart';
import 'package:twoc_client/sdui/widgets/json_field_widget.dart';
import 'package:twoc_client/sdui/widgets/number_field_widgets.dart';
import 'package:twoc_client/sdui/widgets/readonly_field_widget.dart';
import 'package:twoc_client/sdui/widgets/reference_field_widget.dart';
import 'package:twoc_client/sdui/widgets/text_field_widgets.dart';

EntityField _field(String type, {Object? options}) => EntityField(
      code: 'f',
      label: 'F',
      dataType: type,
      required: false,
      order: 0,
      options: options,
    );

Future<void> _pump(WidgetTester tester, EntityField field) async {
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Builder(
          builder: (context) => WidgetRegistry.instance
              .build(context, field, null, (_) {}, false),
        ),
      ),
    ),
  );
  await tester.pump();
}

void main() {
  final cases = <String, Type>{
    'string': SduiTextField,
    'text': SduiMultiTextField,
    'integer': SduiIntegerField,
    'money': SduiMoneyField,
    'date': SduiDateField,
    'datetime': SduiDateTimeField,
    'boolean': SduiBooleanField,
    'enum': SduiEnumField,
    'reference': SduiReferenceField,
    'array': SduiArrayField,
    'table': SduiArrayField,
    'json': SduiJsonField,
  };

  for (final entry in cases.entries) {
    testWidgets('тип ${entry.key} → ${entry.value}', (tester) async {
      final options = entry.key == 'enum'
          ? {'values': ['asset', 'liability']}
          : entry.key == 'reference'
              ? {'items': {'acc-1': 'Счет 1'}}
              : null;
      await _pump(tester, _field(entry.key, options: options));
      expect(find.byType(entry.value), findsOneWidget);
    });
  }

  testWidgets('неизвестный тип → SduiReadOnlyField', (tester) async {
    await _pump(tester, _field('file'));
    expect(find.byType(SduiReadOnlyField), findsOneWidget);
  });

  testWidgets('reference без вариантов → SduiReadOnlyField с меткой-ссылка',
      (tester) async {
    await _pump(tester, _field('reference'));
    expect(find.byType(SduiReadOnlyField), findsOneWidget);
    expect(find.text('F (ссылка)'), findsOneWidget);
  });

  testWidgets('передаёт значение в onChanged', (tester) async {
    Object? emitted;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Builder(
            builder: (context) => WidgetRegistry.instance.build(
              context,
              _field('string'),
              null,
              (v) => emitted = v,
              false,
            ),
          ),
        ),
      ),
    );
    await tester.enterText(find.byType(TextFormField), 'привет');
    expect(emitted, 'привет');
  });
}