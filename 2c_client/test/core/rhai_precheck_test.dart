import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/core/rhai_precheck.dart';

void main() {
  group('rhaiPrecheck', () {
    test('пустой исходник — без проблем', () {
      expect(rhaiPrecheck(''), isEmpty);
      expect(rhaiPrecheck('   \n\n'), isEmpty);
    });

    test('корректный код с точками с запятой — без проблем', () {
      const src = '''
let a = 1;
let b = a + 2;
a * b;
''';
      expect(rhaiPrecheck(src), isEmpty);
    });

    test('несколько операторов с пропущенной «;» — флагирует строку', () {
      const src = 'let a = 1\nlet b = 2;';
      final issues = rhaiPrecheck(src);
      expect(issues, hasLength(1));
      expect(issues.first.line, 1);
      expect(issues.first.message, contains('не завершён'));
    });

    test('продолжение выражения на следующей строке — без проблем', () {
      const src = '''
let total = value1 +
  value2;
total;
''';
      expect(rhaiPrecheck(src), isEmpty);
    });

    test('последний оператор без «;» (значение скрипта) — без проблем', () {
      const src = 'let a = 1;\na';
      expect(rhaiPrecheck(src), isEmpty);
    });

    test('заголовки if/else/while/for/fn без «;» — без проблем', () {
      const src = '''
if x > 0 {
  let y = 1;
}
while x < 10 {
  x = x + 1;
}
for item in items {
  let z = item;
}
fn calc(v) {
  v * 2;
}
''';
      expect(rhaiPrecheck(src), isEmpty);
    });

    test('пустые строки и комментарии игнорируются', () {
      const src = '''
// комментарий
let a = 1;
/* блоковый
   комментарий */
a;
''';
      expect(rhaiPrecheck(src), isEmpty);
    });

    test('строка с «;» внутри строкового литерала не считается завершённой', () {
      const src = '''
let s = "a;b";
s;
''';
      expect(rhaiPrecheck(src), isEmpty);
    });

    test('конец на открывающую скобку — продолжение', () {
      const src = '''
let f = fn(x) {
  x * 2;
};
let r = f(
  3
);
r;
''';
      expect(rhaiPrecheck(src), isEmpty);
    });

    test('конец на бинарный оператор в конце строки — без проблем', () {
      const src = 'let x = 1 &&\n  2;\nx;';
      expect(rhaiPrecheck(src), isEmpty);
    });
  });
}