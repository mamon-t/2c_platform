/// Минимальный подмножество TOML для конфигурации клиента (темы, настройки).
///
/// Поддерживается только то, что нужно сегодня:
/// - секции-заголовки `[имя_секции]` (вложенные `[a.b]` не поддерживаются);
/// - `key = value`, где value: строка в двойных кавычках, целое число,
///   `true`/`false` или «сырое» слово (без кавычек, без пробелов);
/// - комментарии `#` в начале строки и после значения (вне кавычек);
/// - пустые строки.
///
/// Любое отклонение бросает [TomlParseException] с номером строки.
library;

class TomlParseException implements Exception {
  const TomlParseException(this.line, this.message);

  final int line;
  final String message;

  @override
  String toString() => 'toml: строка $line: $message';
}

/// Разобранный файл: `tables['']` — корневые ключи, `tables['name']` — секция.
class TomlDocument {
  TomlDocument(this.tables);

  final Map<String, Map<String, Object?>> tables;

  /// Значение из секции (пустая строка — корневая запись).
  Object? get(String table, String key) => tables[table]?[key];
}

/// Разбор текста в минимальном подмножестве TOML.
TomlDocument parseToml(String source) {
  final tables = <String, Map<String, Object?>>{'': {}};
  var current = '';
  final lines = source.split('\n');

  for (var i = 0; i < lines.length; i++) {
    var line = lines[i].trim();
    if (line.isEmpty || line.startsWith('#')) {
      continue;
    }
    // Незакомментированная часть строки (решетка в копиях игнорируется).
    line = _stripInlineComment(line).trim();
    if (line.isEmpty) {
      continue;
    }
    // Секция-заголовок.
    if (line.startsWith('[')) {
      if (!line.endsWith(']')) {
        throw TomlParseException(i + 1, 'некорректный заголовок секции');
      }
      final inside = line.substring(1, line.length - 1);
      if (inside.contains('.')) {
        throw TomlParseException(i + 1, 'вложенные секции не поддерживаются');
      }
      tables[inside] = {};
      current = inside;
      continue;
    }
    final eq = line.indexOf('=');
    if (eq == -1) {
      throw TomlParseException(i + 1, 'ожидалось key = value');
    }
    final key = line.substring(0, eq).trim();
    final valueRaw = line.substring(eq + 1).trim();
    if (key.isEmpty || valueRaw.isEmpty) {
      throw TomlParseException(i + 1, 'пустой ключ или значение');
    }
    (tables[current]!)[key] = parseTomlValue(valueRaw, i + 1);
  }
  return TomlDocument(tables);
}

/// Значение минимального подмножества: строка/число/bool/сырое слово.
Object? parseTomlValue(String raw, int line) {
  if (raw.startsWith('"')) {
    final lastQuote = raw.lastIndexOf('"');
    if (lastQuote <= 0) {
      throw TomlParseException(line, 'незакрытая строка-значение');
    }
    final body = raw.substring(1, lastQuote);
    final sb = StringBuffer();
    for (var i = 0; i < body.length; i++) {
      final c = body[i];
      if (c == r'\' && i + 1 < body.length) {
        final n = body[i + 1];
        if (n == 'n') {
          sb.write('\n');
        } else if (n == 't') {
          sb.write('\t');
        } else {
          sb.write(n);
        }
        i++;
      } else {
        sb.write(c);
      }
    }
    return sb.toString();
  }
  if (raw == 'true') {
    return true;
  }
  if (raw == 'false') {
    return false;
  }
  final asInt = int.tryParse(raw);
  if (asInt != null) {
    return asInt;
  }
  // «Сырое» слово: без пробелов и структурных символов.
  if (!raw.contains(RegExp(r'[\s\[\]"#]'))) {
    return raw;
  }
  throw TomlParseException(line, 'неподдерживаемое значение: $raw');
}

/// Обрезает строку по первому `#` ВНЕ двойных кавычек.
String _stripInlineComment(String line) {
  var inString = false;
  for (var i = 0; i < line.length; i++) {
    final c = line[i];
    if (inString && c == r'\') {
      i++;
    } else if (c == '"') {
      inString = !inString;
    } else if (c == '#' && !inString) {
      return line.substring(0, i);
    }
  }
  return line;
}