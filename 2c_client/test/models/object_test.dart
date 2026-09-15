import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/object.dart';

import '../support/fixtures.dart';

void main() {
  group('ObjectItem', () {
    test('fromJson разбирает полную запись', () {
      final obj = ObjectItem.fromJson(
        objectWire(data: {'name': 'Касса', 'balance': 1000}, version: 3),
      );
      expect(obj.id, 'acc-1');
      expect(obj.entityType, 'account');
      expect(obj.kind, 'catalog');
      expect(obj.companyId, 'c1');
      expect(obj.state, 'draft');
      expect(obj.data, {'name': 'Касса', 'balance': 1000});
      expect(obj.version, 3);
      expect(obj.number, '00001');
    });

    test('data/computed отсутствуют → пустые карты', () {
      final obj = ObjectItem.fromJson(objectWire());
      expect(obj.data, isEmpty);
      expect(obj.computed, isEmpty);
    });

    test('copyWith меняет data и state, остальное сохраняет', () {
      final obj = ObjectItem.fromJson(objectWire(version: 5));
      final updated = obj.copyWith(data: {'name': 'Касса'}, state: 'active');
      expect(updated.data, {'name': 'Касса'});
      expect(updated.state, 'active');
      expect(updated.id, 'acc-1');
      expect(updated.version, 5);
    });
  });
}