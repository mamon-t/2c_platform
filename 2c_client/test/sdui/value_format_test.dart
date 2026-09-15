import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/entity_schema.dart';
import 'package:twoc_client/sdui/value_format.dart';

EntityField _field(String type) => EntityField(
      code: 'f',
      label: 'F',
      dataType: type,
      required: false,
      order: 0,
    );

void main() {
  test('money форматируется в рубли с запятой', () {
    expect(displayValue(_field('money'), 1234), '12,34');
    expect(displayValue(_field('money'), '1234'), '12,34');
    expect(displayValue(_field('money'), 0), '0,00');
  });

  test('boolean → да/нет', () {
    expect(displayValue(_field('boolean'), true), 'да');
    expect(displayValue(_field('boolean'), false), 'нет');
  });

  test('List склеивается через запятую', () {
    expect(displayValue(_field('array'), ['a', 'b']), 'a, b');
    expect(displayValue(_field('array'), <String>[]), '—');
  });

  test('Map сериализуется в JSON', () {
    expect(displayValue(_field('json'), {'a': 1}), '{"a":1}');
  });

  test('null и пустая строка → длинное тире', () {
    expect(displayValue(_field('string'), null), '—');
    expect(displayValue(_field('string'), ''), '—');
  });

  test('строка и число показываются как есть', () {
    expect(displayValue(_field('string'), 'Касса'), 'Касса');
    expect(displayValue(_field('integer'), 42), '42');
  });
}