/// Скрипт Rhai платформы (ТЗ §15): ответ `script.list` / `script.get`.
///
/// Wire-формат сервера (`core-domain::script::Script`): `script_type` —
/// snake_case (`formula`/`validator`/`before_action`/`after_action`/`report`/
/// `event_handler`), `company_id`/`module_code`/`entity_type` опциональны.
class ScriptItem {
  const ScriptItem({
    required this.id,
    required this.code,
    required this.name,
    required this.scriptType,
    required this.source,
    required this.isActive,
    required this.createdAt,
    required this.updatedAt,
    this.companyId,
    this.moduleCode,
    this.entityType,
  });

  final String id;
  final String code;
  final String name;
  final String scriptType;
  final String source;
  final String? companyId;
  final String? moduleCode;
  final String? entityType;
  final bool isActive;
  final String createdAt;
  final String updatedAt;

  /// Канонические wire-типы скрипта в порядке для UI.
  static const List<String> scriptTypes = [
    'formula',
    'validator',
    'before_action',
    'after_action',
    'report',
    'event_handler',
  ];

  factory ScriptItem.fromJson(Map<String, dynamic> json) {
    return ScriptItem(
      id: _str(json['id'], 'script.id'),
      code: _str(json['code'], 'script.code'),
      name: _str(json['name'], 'script.name'),
      scriptType: _str(json['script_type'], 'script.script_type'),
      source: _str(json['source'], 'script.source'),
      companyId: json['company_id'] as String?,
      moduleCode: json['module_code'] as String?,
      entityType: json['entity_type'] as String?,
      isActive: json['is_active'] as bool? ?? true,
      createdAt: json['created_at'] as String? ?? '',
      updatedAt: json['updated_at'] as String? ?? '',
    );
  }
}

String _str(Object? value, String path) {
  final v = value;
  if (v is String) {
    return v;
  }
  throw FormatException('$path: ожидалась строка, получено: $v');
}