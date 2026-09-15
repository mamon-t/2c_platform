import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/core/toml_subset.dart';

void main() {
  group('parseToml', () {
    test('разбирает секции и типы значений', () {
      final doc = parseToml('''
[app]
theme = "dark"
retries = 3
debug = true
mode = roda

[theme]
seed = "#1E88E5"
''');
      expect(doc.get('app', 'theme'), 'dark');
      expect(doc.get('app', 'retries'), 3);
      expect(doc.get('app', 'debug'), true);
      expect(doc.get('app', 'mode'), 'roda');
      expect(doc.get('theme', 'seed'), '#1E88E5');
    });

    test('гибкий разбор экранирования кавычек', () {
      final doc = parseToml(r'''[app]
note = "строка \"внутри\""
''');
      expect(doc.get('app', 'note'), 'строка "внутри"');
    });

    test('игнорирует комментарии и пустые строки', () {
      final doc = parseToml('''
# комментарий сверху

[app] # с хвостом
theme = "light" # после значения
''');
      expect(doc.get('app', 'theme'), 'light');
    });

    test('корневые ключи без секции', () {
      final doc = parseToml('title = "hello"');
      expect(doc.get('', 'title'), 'hello');
    });

    test('вложенные секции запрещены', () {
      expect(() => parseToml('[a.b]\nx = 1'), throwsA(isA<TomlParseException>()));
    });

    test('строка без "=" бросает исключение с номером строки', () {
      try {
        parseToml('ok = 1\nbroken line\n');
        fail('ожидалось исключение');
      } on TomlParseException catch (e) {
        expect(e.line, 2);
      }
    });

    test('пустой файл → пустые таблицы', () {
      final doc = parseToml('');
      expect(doc.get('app', 'theme'), isNull);
    });
  });
}