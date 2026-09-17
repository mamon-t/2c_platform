// Rhai-грамматика для подсветки синтаксиса (Фаза 15.3).
//
// Rhai по своей сути близок к Rust, поэтому за основу взята грамматика
// Rust из пакета `highlight` (languages/rust.dart) и доработана под
// особенности Rhai 1.26: убраны Rust-специфичные конструкции
// (mod/struct/enum/trait/impl/макросы/аренда/лда), добавлены операторы
// управления потоком и строки Rhai (в т.ч. raw-строки с решётками).
//
// Регистрация: `registerRhaiHighlight()` из main до построения редактора.

// Общие моды (C_LINE_COMMENT_MODE и пр.) живут в src/ пакета highlight и
// публично не экспортируются — подавляем lint для этого импорта.
// ignore_for_file: implementation_imports

import 'package:highlight/highlight.dart';
import 'package:highlight/src/common_modes.dart';

/// Встроенные функции и типы Rhai.
const String _rhaiBuiltIn =
    'print debug type_of len is_empty push pop shift append prepend insert '
    'remove clear contains get set delete keys values has has_key sort '
    'reverse min max abs floor ceil round sqrt to_int to_float to_string '
    'to_bool to_array to_blob to_decimal to_number parse_int parse_float '
    'parse_uint parse_int_str to_chars to_upper to_lower trim split splitn '
    'join replace find index_of starts_with ends_with is_shared '
    'generated e deep_shared shared bind is_def_var engine_call_fn '
    'encode_unicode decode_unicode capitalize count ev members '
    'extract_hashtbl extract_try f11';

/// Ключевые слова Rhai (включая литералы и модификаторы видимости).
const String _rhaiKeywords =
    'let const fn return if else while loop for in break continue throw '
    'try catch switch case default import export true false private shared '
    'do next';

/// Грамматика Rhai (Mode) по образцу rust.dart.
final Mode rhai = Mode(
  aliases: ['rhai'],
  keywords: {
    'keyword': _rhaiKeywords,
    'literal': 'true false',
    'built_in': _rhaiBuiltIn,
  },
  contains: [
    C_LINE_COMMENT_MODE,
    Mode(
      className: 'comment',
      begin: r'/\*',
      end: r'\*/',
      contains: [
        Mode(self: true),
        PHRASAL_WORDS_MODE,
      ],
    ),
    Mode(
      className: 'string',
      variants: [
        Mode(begin: '"(\\\\[\\s\\S]|[^"\\\\\n])*"'),
        Mode(begin: "'(\\\\[\\s\\S]|[^'\\\\\n])*'"),
      ],
    ),
    Mode(
      className: 'string',
      begin: 'r(#*)"(.|\\n)*?"\\1(?!#)',
    ),
    Mode(
      className: 'number',
      variants: [
        Mode(begin: '\\b0x[0-9a-fA-F_]+'),
        Mode(begin: '\\b0b[01_]+'),
        Mode(begin: '\\b0o[0-7_]+'),
        Mode(
          begin: r'\b\d[\d_]*(\.\d+)?([eE][+-]?\d+)?',
        ),
      ],
      relevance: 0,
    ),
    Mode(
      className: 'function',
      beginKeywords: 'fn',
      end: '(\\(|<)',
      excludeEnd: true,
      contains: [UNDERSCORE_TITLE_MODE],
    ),
    Mode(
      className: 'type',
      beginKeywords: 'const',
      end: '(=|;)',
      excludeEnd: true,
      contains: [
        Mode(
          className: 'title',
          begin: '[a-zA-Z_]\\w*',
          relevance: 0,
          endsParent: true,
        ),
      ],
      illegal: '\\S',
    ),
    Mode(begin: '->'),
  ],
);

/// Регистрирует Rhai-грамматику в глобальном реестре `highlight`.
/// Вызывать один раз при старте приложения, до построения `ScriptEditorScreen`.
///
/// Повторный вызов безопасен: `registerLanguage` перезаписывает грамматику
/// того же имени.
void registerRhaiHighlight() {
  highlight.registerLanguage('rhai', rhai);
}