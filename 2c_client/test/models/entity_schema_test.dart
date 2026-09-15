import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/entity_schema.dart';

import '../support/fixtures.dart';

void main() {
  group('EntitySchema.fromJson', () {
    test('разбирает entity_type и поля в порядке order', () {
      final schema = EntitySchema.fromJson(schemaWire());
      expect(schema.entityType.code, 'account');
      expect(schema.entityType.name, 'Счет');
      expect(schema.entityType.kind, 'catalog');
      expect(schema.fields, hasLength(6));
      expect(schema.orderedFields.first.code, 'code');
      expect(schema.orderedFields[1].code, 'name');
    });

    test('enum-поле извлекает варианты из options.values', () {
      final schema = EntitySchema.fromJson(schemaWire());
      final kind = schema.orderedFields.firstWhere((f) => f.code == 'kind');
      expect(kind.dataType, 'enum');
      expect(kind.enumValues, ['asset', 'liability']);
    });

    test('enum-поле принимает options-список строк', () {
      final wire = schemaWire();
      (wire['fields'] as List)
          .removeWhere((f) => (f as Map)['code'] == 'kind');
      (wire['fields'] as List).add(field('kind', 'Тип', 'enum',
          order: 3, options: ['asset', 'liability']));
      final schema = EntitySchema.fromJson(wire);
      final kind = schema.orderedFields.firstWhere((f) => f.code == 'kind');
      expect(kind.enumValues, ['asset', 'liability']);
    });

    test('required и порядок по умолчанию', () {
      final f = EntityField.fromJson(field('x', 'X', 'string'));
      expect(f.required, isFalse);
      expect(f.order, 0);
    });

    test('referenceTarget из options.entity_type', () {
      final f = EntityField.fromJson(
        field('parent', 'Родитель', 'reference',
            options: {'entity_type': 'account'}),
      );
      expect(f.referenceTarget, 'account');
    });

    test('пустые списки по умолчанию', () {
      final wire = schemaWire();
      wire['states'] = null;
      wire.remove('transitions');
      final schema = EntitySchema.fromJson(wire);
      expect(schema.states, isEmpty);
      expect(schema.transitions, isEmpty);
    });

    test('бросает FormatException при неверной структуре', () {
      expect(
        () => EntitySchema.fromJson(const {'entity_type': 'not-a-map'}),
        throwsFormatException,
      );
    });
  });
}