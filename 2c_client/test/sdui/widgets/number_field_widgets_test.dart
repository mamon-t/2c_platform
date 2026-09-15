import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/sdui/widgets/number_field_widgets.dart';

void main() {
  group('SduiMoneyField.format', () {
    test('копейки → рубли с запятой', () {
      expect(SduiMoneyField.format(1234), '12,34');
      expect(SduiMoneyField.format(0), '0,00');
      expect(SduiMoneyField.format(-150), '-1,50');
    });

    test('строковое целое форматируется так же', () {
      expect(SduiMoneyField.format('1234'), '12,34');
    });

    test('не-число → пустая строка', () {
      expect(SduiMoneyField.format(null), '');
      expect(SduiMoneyField.format('abc'), '');
    });
  });

  group('SduiMoneyField.parseKopeks', () {
    test('целое без разделителя — копейки', () {
      expect(SduiMoneyField.parseKopeks('123'), 123);
      expect(SduiMoneyField.parseKopeks('12'), 12);
    });

    test('дробное с запятой и точкой', () {
      expect(SduiMoneyField.parseKopeks('123,45'), 12345);
      expect(SduiMoneyField.parseKopeks('123.45'), 12345);
      expect(SduiMoneyField.parseKopeks('1,5'), 150);
      expect(SduiMoneyField.parseKopeks('0,01'), 1);
    });

    test('отрицательное', () {
      expect(SduiMoneyField.parseKopeks('-1,5'), -150);
      expect(SduiMoneyField.parseKopeks('-100'), -100);
    });

    test('невалидное → null', () {
      expect(SduiMoneyField.parseKopeks('abc'), isNull);
      expect(SduiMoneyField.parseKopeks(''), isNull);
      expect(SduiMoneyField.parseKopeks('12,345'), isNull);
    });
  });
}