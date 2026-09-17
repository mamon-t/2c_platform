import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/script.dart';

import '../support/fixtures.dart';

void main() {
  group('ScriptItem.fromJson', () {
    test('полный wire-объект распарсен полностью', () {
      final item = ScriptItem.fromJson(scriptWire());

      expect(item.id, 's-1');
      expect(item.code, 'double.amount');
      expect(item.name, 'Удвоить сумму');
      expect(item.scriptType, 'formula');
      expect(item.source, 'ctx.object.amount * ctx.args.factor');
      expect(item.companyId, 'c1');
      expect(item.moduleCode, isNull);
      expect(item.entityType, 'invoice');
      expect(item.isActive, isTrue);
      expect(item.createdAt, '2026-09-17T10:00:00Z');
      expect(item.updatedAt, '2026-09-17T10:00:00Z');
    });

    test('опциональные поля без значений дают дефолты', () {
      final wire = scriptWire(
        companyId: null,
        entityType: null,
        isActive: false,
      )..remove('created_at');

      final item = ScriptItem.fromJson(wire);

      expect(item.companyId, isNull);
      expect(item.entityType, isNull);
      expect(item.isActive, isFalse);
      expect(item.createdAt, '');
    });

    test('известные типы скриптов покрывают wire', () {
      expect(ScriptItem.scriptTypes.length, 6);
      expect(ScriptItem.scriptTypes, contains('formula'));
      expect(ScriptItem.scriptTypes, contains('event_handler'));
    });

    test('отсутствие обязательного поля — FormatException', () {
      final wire = scriptWire()..remove('code');

      expect(() => ScriptItem.fromJson(wire), throwsFormatException);
    });
  });
}