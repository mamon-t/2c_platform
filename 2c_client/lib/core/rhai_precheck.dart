// Консервативный пре-чек «оператор не завершён ';'» (Фаза 15.3).
//
// Проверяет исходник Rhai до обращения к серверу: операторы должны
// завершаться `;`. Проверка консервативная — безопасные случаи (блоки,
// заголовки if/while/for/fn/loop, пустые строки, комментарии, последний
// оператор скрипта) не считаются ошибками, чтобы не мешать работе с
// незавершённым кодом.

/// Проблема, найденная в исходнике клиентским пре-чеком.
class RhaiIssue {
  const RhaiIssue({required this.line, required this.column, required this.message});

  final int line;
  final int column;
  final String message;

  @override
  bool operator ==(Object other) =>
      other is RhaiIssue &&
      other.line == line &&
      other.column == column &&
      other.message == message;

  @override
  int get hashCode => Object.hash(line, column, message);

  @override
  String toString() => 'RhaiIssue(строка $line:$column, $message)';
}

/// Ключевые слова, после которых следует блок или заголовок цикла/условия
/// (оператор `;` не требуется).
const Set<String> _blockHeaders = {
  'if', 'else', 'while', 'for', 'fn', 'loop', 'switch',
};

/// Символы, на которых строка может заканчиваться без `;`.
const Set<String> _continuationEnds = {
  '{', '}', '(', ',', '+', '-', '*', '/', '%', '=', '.', ':', '?',
  '&&', '||', '==', '!=', '<', '>', '<=', '>=', '+=', '-=', '*=', '/=',
  '=>',
};

/// Пре-чек исходника Rhai: возвращает список проблем с незавершёнными
/// операторами. Пустой список — претензий нет.
///
/// Правила:
/// - строка с `;` считается закрытой для предыдущего оператора;
/// - конец на `{`, `(`, `,` или бинарный оператор — продолжение выражения;
/// - заголовки if/else/while/for/fn/loop/switch завершаются блоком;
/// - пустые строки и комментарии пропускаются;
/// - последний непустой оператор без `;` допускается (значение скрипта).
List<RhaiIssue> rhaiPrecheck(String source) {
  if (source.trim().isEmpty) return const [];

  final lines = source.split('\n');
  final issues = <RhaiIssue>[];
  var inBlockComment = false;
  // Баланс открытых круглых скобок: строки внутри вызова/выражения в «(...)»
  // не требуют «;» до закрытия скобки.
  var parenBalance = 0;
  // Индекс последней непустой (не коммент) строки — её `;` не требуется.
  var lastRecordedLine = -1;
  for (var i = 0; i < lines.length; i++) {
    final raw = lines[i];
    final trimmed = raw.trim();
    if (trimmed.isEmpty) continue;
    if (inBlockComment) {
      if (trimmed.contains('*/')) inBlockComment = false;
      continue;
    }
    if (trimmed.startsWith('//')) continue;
    if (trimmed.startsWith('/*')) {
      if (!trimmed.contains('*/')) inBlockComment = true;
      continue;
    }
    lastRecordedLine = i;
  }
  // Однострочник — на него проверка не распространяется.
  if (lastRecordedLine < 0) return const [];

  for (var i = 0; i < lines.length; i++) {
    final raw = lines[i];
    final trimmed = raw.trim();
    if (trimmed.isEmpty) continue;

    // Комментарии.
    if (inBlockComment) {
      if (trimmed.contains('*/')) inBlockComment = false;
      continue;
    }
    if (trimmed.startsWith('//')) continue;
    if (trimmed.startsWith('/*')) {
      if (!trimmed.contains('*/')) inBlockComment = true;
      continue;
    }

    // Дельта по круглым скобкам учитывается для каждой значимой строки,
    // чтобы баланс держался синхронно со всеми ветками-пропусками.
    final delta = _parenDelta(trimmed);
    final insideParens = parenBalance > 0;

    // Заголовки блоков.
    if (_isBlockHeaderLine(trimmed)) {
      parenBalance += delta;
      continue;
    }

    // Строки во вложенных круглых скобках — аргументы/выражения, «;» не нужен.
    if (insideParens) {
      parenBalance += delta;
      continue;
    }

    // Последняя значимая строка — значение скрипта, `;` не обязателен.
    if (i == lastRecordedLine) {
      parenBalance += delta;
      continue;
    }

    // Строка уже завершена `;`.
    if (_lineEndsWithSemicolon(trimmed)) {
      parenBalance += delta;
      continue;
    }

    // Строка заканчивается на продолжение выражения.
    if (_endsWithContinuation(trimmed)) {
      parenBalance += delta;
      continue;
    }

    issues.add(RhaiIssue(
      line: i + 1,
      column: trimmed.length + 1,
      message: 'Оператор не завершён знаком «;»',
    ));
    parenBalance += delta;
  }
  return issues;
}

bool _isBlockHeaderLine(String trimmed) {
  final first = trimmed.split(RegExp(r'\s+')).first;
  return _blockHeaders.contains(first);
}

bool _lineEndsWithSemicolon(String trimmed) => trimmed.endsWith(';');

/// Разность числа открывающих и закрывающих круглых скобок в строке.
int _parenDelta(String trimmed) {
  var open = 0;
  var close = 0;
  for (final ch in trimmed.split('')) {
    if (ch == '(') {
      open++;
    } else if (ch == ')') {
      close++;
    }
  }
  return open - close;
}

bool _endsWithContinuation(String trimmed) {
  // Ищем последний «значимый» токен в конце строки. Упрощённо проверяем
  // суффиксы: если до комментария строка кончается на оператор — продолжение.
  final codePart = trimmed.split('//').first.trimRight();
  if (codePart.isEmpty) return true;
  for (final suffix in _continuationEnds) {
    if (codePart.endsWith(suffix)) return true;
  }
  return false;
}