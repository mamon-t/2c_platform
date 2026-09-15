import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/navigation_item.dart';

import '../support/fixtures.dart';

void main() {
  group('ModuleNavigation', () {
    test('fromJson разбирает модули и пункты с entity_type', () {
      final payload = navigationWire();
      final modules =
          (payload['modules'] as List)
              .map((m) =>
                  ModuleNavigation.fromJson((m as Map).cast<String, dynamic>()))
              .toList();
      expect(modules, hasLength(1));
      final module = modules.single;
      expect(module.code, 'accounting');
      expect(module.displayName, 'Учёт');
      expect(module.version, '1.0.0');
      expect(module.navigation, hasLength(2));
      expect(module.navigation.first.label, 'Счета');
      expect(module.navigation.first.entityType, 'account');
    });

    test('navigation отсутствует → пусто', () {
      final module = ModuleNavigation.fromJson(const {
        'code': 'm',
        'display_name': 'M',
        'version': '1.0.0',
      });
      expect(module.navigation, isEmpty);
    });

    test('entity_type может отсутствовать', () {
      final item = NavigationItem.fromJson(const {'code': 'x', 'label': 'X'});
      expect(item.entityType, isNull);
    });
  });
}