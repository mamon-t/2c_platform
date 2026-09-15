import 'dart:io';

import 'app_paths.dart';
import 'toml_subset.dart';

/// Тема оформления клиента: встроенная или из каталога `themes/*.toml`.
///
/// База фиксируется сегодня; `rawExtra` сохраняет незнакомые ключи файла
/// «как есть» для будущих полей (редакторы скриптов/метаданных/печатных
/// форм) — добавление новых полей в формат не сломает старые темы.
class AppTheme {
  const AppTheme({
    required this.id,
    required this.name,
    required this.kind,
    required this.seed,
    this.overrides = const {},
    this.rawExtra = const {},
  });

  /// Идентификатор: имя файла без расширения (или `light`/`dark` для
  /// встроенных). Пишется в `config.toml`.
  final String id;

  /// Отображаемое имя (`[theme] name`).
  final String name;

  /// `'light'` или `'dark'` (`[theme] kind`).
  final String kind;

  /// Цвет-семя в формате `0xFF` + RGB (`[theme] seed = "#1E88E5"`).
  final int seed;

  /// Переопределения ключевых цветов: `primary`/`secondary`/`surface` (int).
  final Map<String, Object?> overrides;

  /// Незнакомые ключи файла темы (форвард-совместимость).
  final Map<String, Object?> rawExtra;

  bool get isDark => kind == 'dark';

  AppTheme copyWith({int? seed, Map<String, Object?>? overrides}) => AppTheme(
        id: id,
        name: name,
        kind: kind,
        seed: seed ?? this.seed,
        overrides: overrides ?? this.overrides,
        rawExtra: rawExtra,
      );
}

/// Встроенные темы — всегда доступны, каталог может быть пуст.
class BuiltinThemes {
  const BuiltinThemes._();

  static const lightId = 'light';
  static const darkId = 'dark';
  static const defaultSeed = 0xFF1E88E5; // фирменный синий 2C

  static const AppTheme light = AppTheme(
    id: lightId,
    name: 'Светлая',
    kind: 'light',
    seed: defaultSeed,
  );

  static const AppTheme dark = AppTheme(
    id: darkId,
    name: 'Тёмная',
    kind: 'dark',
    seed: defaultSeed,
  );

  static const List<AppTheme> all = [light, dark];
}

/// Образцовая тема с полным описанием формата в комментариях; сидится в
/// каталог тем при первом запуске, чтобы новые темы создавались копированием.
const String exampleThemeToml = '''
# Формат темы клиента (2C Platform):
#
# Файл кладётся в ~/.config/2cplatform/themes/ как <id>.toml.
# id темы = имя файла без расширения (напр., "paper" → [theme].id = "paper").
#
#   [theme]
#   name       = "Отображаемое имя в настройках"
#   kind       = "light" | "dark"
#   seed       = "#RRGGBB"        # цвет-семя Material 3
#   primary    = "#RRGGBB"  # необязательно: переопределение базового цвета
#   secondary  = "#RRGGBB"  # необязательно
#   surface    = "#RRGGBB"  # необязательно: фон экранов
#
# Любые другие ключи секции [theme] сохраняются в темах "как есть"
# (зарезервировано под будущие поля: редактор скриптов/метаданных/печатных форм)
# и не влияют на рендеринг.
[theme]
name = "Paper (пример)"
kind = "light"
seed = "#607D8B"
# primary   = "#3F51B5"
# secondary = "#FFC107"
''';

/// Загрузчик тем и конфигурации из каталога `themes/` и `config.toml`.
class ThemeStore {
  ThemeStore([Directory? base]) : base = base ?? AppPaths.base();

  final Directory base;

  Directory get themesDir => Directory('${base.path}/themes');
  File get configFile => File('${base.path}/config.toml');

  /// Все доступные темы: встроенные + из каталога. При первом запуске
  /// создаёт каталог `themes/` и сидит образцовую тему с описанием формата.
  Future<List<AppTheme>> loadThemes() async {
    if (!themesDir.existsSync()) {
      themesDir.createSync(recursive: true);
      final sample = File('${themesDir.path}/example.toml');
      if (!sample.existsSync()) {
        sample.writeAsStringSync(exampleThemeToml);
      }
    }
    final custom = <AppTheme>[];
    for (final entry in themesDir.listSync()) {
      if (entry is! File || !entry.path.endsWith('.toml')) {
        continue;
      }
      final id = _basename(entry.path);
      final parsed = _loadFromFile(entry, id);
      custom.add(parsed);
    }
    custom.sort((a, b) => a.name.compareTo(b.name));
    return [...BuiltinThemes.all, ...custom];
  }

  /// Читает id темы из `config.toml` (`[app] theme = "id"`); null при
  /// отсутствии файла или ключа.
  Future<String?> readConfiguredTheme() async {
    final file = configFile;
    if (!file.existsSync()) {
      return null;
    }
    final doc = parseToml(file.readAsStringSync());
    return doc.get('app', 'theme') as String?;
  }

  /// Создаёт `config.toml` и пишет выбранную тему.
  Future<void> writeConfiguredTheme(String id) async {
    if (!base.existsSync()) {
      base.createSync(recursive: true);
    }
    final escaped = id.replaceAll(r'\', r'\\').replaceAll('"', r'\"');
    configFile.writeAsStringSync('[app]\ntheme = "$escaped"\n');
  }

  AppTheme _loadFromFile(File file, String id) {
    final doc = parseToml(file.readAsStringSync());
    final table = doc.tables['theme'] ?? const <String, Object?>{};
    final rawName = table['name'];
    final rawKind = table['kind'];
    final name =
        rawName is String && rawName.isNotEmpty ? rawName : id;
    final kind = rawKind is String && (rawKind == 'light' || rawKind == 'dark')
        ? rawKind
        : 'dark';
    final seed = _parseColor(table['seed']) ?? BuiltinThemes.defaultSeed;
    final overrides = <String, Object?>{};
    for (final key in const ['primary', 'secondary', 'surface']) {
      if (table.containsKey(key)) {
        overrides[key] = _parseColor(table[key]);
      }
    }
    final consumed = <String>{
      'name',
      'kind',
      'seed',
      ...overrides.keys,
    };
    final extra = <String, Object?>{
      for (final e in table.entries)
        if (!consumed.contains(e.key)) e.key: e.value,
    };
    return AppTheme(
      id: id,
      name: name,
      kind: kind,
      seed: seed,
      overrides: overrides,
      rawExtra: extra,
    );
  }

  /// Цвет из форматов `"#RRGGBB"`, `"0xFFRRGGBB"` или целого числа.
  int? _parseColor(Object? value) {
    if (value is int) {
      return 0xFF000000 | (value & 0xFFFFFF);
    }
    if (value is String) {
      final v = value.trim();
      if (RegExp(r'^#[0-9a-fA-F]{6}$').hasMatch(v)) {
        return 0xFF000000 | int.parse(v.substring(1), radix: 16);
      }
      if (RegExp(r'^0x[0-9a-fA-F]{8}$').hasMatch(v)) {
        return int.parse(v.substring(2), radix: 16);
      }
    }
    return null;
  }
}

String _basename(String path) {
  final idx = path.lastIndexOf(Platform.pathSeparator);
  final name = idx == -1 ? path : path.substring(idx + 1);
  return name.endsWith('.toml') ? name.substring(0, name.length - 5) : name;
}