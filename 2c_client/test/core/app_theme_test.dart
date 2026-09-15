import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/core/app_theme.dart';

void main() {
  late Directory base;

  setUp(() {
    base = Directory.systemTemp.createTempSync('app_theme_test');
  });

  tearDown(() {
    if (base.existsSync()) {
      base.deleteSync(recursive: true);
    }
  });

  ThemeStore store() => ThemeStore(base);

  group('ThemeStore.loadThemes', () {
    test('создаёт themes/ и сидит образец при первом запуске', () async {
      final store_ = store();
      final themes = await store_.loadThemes();
      expect(Directory('${base.path}/themes').existsSync(), isTrue);
      expect(
        File('${base.path}/themes/example.toml').existsSync(),
        isTrue,
      );
      // Встроенные + пример («Paper (пример)»).
      expect(themes.map((t) => t.id), containsAll(['light', 'dark', 'example']));
    });

    test('встроенные темы всегда первыми, файловые сортируются по имени',
        () async {
      Directory('${base.path}/themes').createSync(recursive: true);
      File('${base.path}/themes/beta.toml')
          .writeAsStringSync('[theme]\nname = "Бета"\nkind = "dark"\n');
      File('${base.path}/themes/alpha.toml')
          .writeAsStringSync('[theme]\nname = "Альфа"\nkind = "light"\n');
      final themes = await store().loadThemes();
      expect(themes.take(2).map((t) => t.id), ['light', 'dark']);
      expect(themes.skip(2).map((t) => t.id), ['alpha', 'beta']);
    });

    test('игнорирует не-.toml файлы', () async {
      Directory('${base.path}/themes').createSync(recursive: true);
      File('${base.path}/themes/readme.txt').writeAsStringSync('x');
      final themes = await store().loadThemes();
      expect(themes.map((t) => t.id), ['light', 'dark']);
      expect(themes.map((t) => t.id), isNot(contains('readme')));
    });
  });

  group('разбор файла темы', () {
    test('полные поля: name/kind/seed/overrides/rawExtra', () async {
      Directory('${base.path}/themes').createSync(recursive: true);
      File('${base.path}/themes/paper.toml').writeAsStringSync('''
[theme]
name = "Бумажная"
kind = "light"
seed = "#FFCC00"
primary = "#3F51B5"
surface = "#F5F5F5"
future_field = 42
''');
      final themes = await store().loadThemes();
      final paper = themes.firstWhere((t) => t.id == 'paper');
      expect(paper.name, 'Бумажная');
      expect(paper.kind, 'light');
      expect(paper.seed, 0xFFFFCC00);
      expect(paper.overrides['primary'], 0xFF3F51B5);
      expect(paper.overrides['surface'], 0xFFF5F5F5);
      expect(paper.overrides['secondary'], isNull);
      expect(paper.rawExtra, {'future_field': 42});
      expect(paper.isDark, isFalse);
    });

    test('по умолчанию: name = id, kind = dark, seed = фирменный', () async {
      Directory('${base.path}/themes').createSync(recursive: true);
      File('${base.path}/themes/mono.toml')
          .writeAsStringSync('[theme]\nmystery = true\n');
      final themes = await store().loadThemes();
      final mono = themes.firstWhere((t) => t.id == 'mono');
      expect(mono.name, 'mono');
      expect(mono.kind, 'dark');
      expect(mono.seed, BuiltinThemes.defaultSeed);
      expect(mono.isDark, isTrue);
    });

    test('цвет можно задать числом и 0x-строкой', () async {
      final file = File('${base.path}/themes/n.toml');
      file.parent.createSync(recursive: true);
      file.writeAsStringSync('[theme]\nseed = 255\nprimary = "0xFF112233"\n');
      final themes = await store().loadThemes();
      final n = themes.firstWhere((t) => t.id == 'n');
      expect(n.seed, 0xFF0000FF);
      expect(n.overrides['primary'], 0xFF112233);
    });
  });

  group('конфигурация выбранной темы', () {
    test('readConfiguredTheme вернёт null, пока конфига нет', () async {
      expect(await store().readConfiguredTheme(), isNull);
    });

    test('writeConfiguredTheme + readConfiguredTheme round-trip', () async {
      final store_ = store();
      await store_.writeConfiguredTheme('dark');
      expect(await store_.readConfiguredTheme(), 'dark');
      final content = File('${base.path}/config.toml').readAsStringSync();
      expect(content, '[app]\ntheme = "dark"\n');
    });

    test('writeConfiguredTheme экранирует кавычки и слэши', () async {
      await store().writeConfiguredTheme('a"\\b');
      expect(await store().readConfiguredTheme(), 'a"\\b');
    });
  });
}