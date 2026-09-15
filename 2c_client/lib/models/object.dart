/// Универсальный объект платформы (ТЗ §6): ответ `object.create/get/list/update`.
///
/// Wire-формат сервера (`core-domain::object::Object`): `data` — значения полей
/// (`money` — целые копейки, `date` — 'YYYY-MM-DD', `reference` — UUID-строка),
/// `computed` — вычисленные поля, `version` — OCC-счётчик.
class ObjectItem {
  const ObjectItem({
    required this.id,
    required this.entityType,
    required this.kind,
    required this.companyId,
    required this.state,
    required this.data,
    required this.computed,
    required this.version,
    this.number,
    this.date,
    this.parentId,
  });

  final String id;
  final String entityType;
  final String kind;
  final String companyId;
  final String state;
  final Map<String, dynamic> data;
  final Map<String, dynamic> computed;
  final int version;
  final String? number;
  final String? date;
  final String? parentId;

  ObjectItem copyWith({Map<String, dynamic>? data, String? state}) {
    return ObjectItem(
      id: id,
      entityType: entityType,
      kind: kind,
      companyId: companyId,
      state: state ?? this.state,
      data: data ?? this.data,
      computed: computed,
      version: version,
      number: number,
      date: date,
      parentId: parentId,
    );
  }

  factory ObjectItem.fromJson(Map<String, dynamic> json) {
    final data = json['data'];
    final computed = json['computed'];
    return ObjectItem(
      id: _str(json['id'], 'object.id'),
      entityType: _str(json['entity_type'], 'object.entity_type'),
      kind: _str(json['kind'], 'object.kind'),
      companyId: _str(json['company_id'], 'object.company_id'),
      state: _str(json['state'], 'object.state'),
      data: data is Map ? Map<String, dynamic>.from(data) : const {},
      computed: computed is Map
          ? Map<String, dynamic>.from(computed)
          : const {},
      version: json['version'] as int? ?? 1,
      number: json['number'] as String?,
      date: json['date'] as String?,
      parentId: json['parent_id'] as String?,
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