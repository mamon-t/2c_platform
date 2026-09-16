/// Мета-модель платформы (ТЗ §7): типы сущностей, поля, состояния, формы.
/// Ответы `metadata.schema.get` / `metadata.entity_type.list`.
///
/// Wire-контракт сервера (проверен по `apps/platform-server/src/commands.rs`):
/// `metadata.schema.get` возвращает объект `{entity_type, fields, states,
/// transitions, forms, actions, relations}`; `entity_type.list` — массив типов.
class EntitySchema {
  const EntitySchema({
    required this.entityType,
    required this.fields,
    required this.states,
    required this.transitions,
    required this.forms,
    required this.actions,
    required this.relations,
  });

  final EntityType entityType;
  final List<EntityField> fields;
  final List<EntityState> states;
  final List<EntityTransition> transitions;
  final List<EntityForm> forms;
  final List<EntityAction> actions;
  final List<EntityRelation> relations;

  /// Копия схемы с заменой выбранных секций (для редактора метаданных).
  EntitySchema copyWith({
    EntityType? entityType,
    List<EntityField>? fields,
    List<EntityState>? states,
    List<EntityTransition>? transitions,
    List<EntityForm>? forms,
    List<EntityAction>? actions,
    List<EntityRelation>? relations,
  }) {
    return EntitySchema(
      entityType: entityType ?? this.entityType,
      fields: fields ?? this.fields,
      states: states ?? this.states,
      transitions: transitions ?? this.transitions,
      forms: forms ?? this.forms,
      actions: actions ?? this.actions,
      relations: relations ?? this.relations,
    );
  }

  /// Wire-документ для `metadata.import`: `{entity_type, fields, states, ...}`.
  /// `metadataVersion` принудительно повышает версию типа (по умолчанию —
  /// текущая), чтобы ensure-семантика сервера применила изменения.
  Map<String, dynamic> toJson({int? metadataVersion}) {
    return {
      'entity_type': entityType.copyWith(
        metadataVersion: metadataVersion ?? entityType.metadataVersion,
      ).toJson(),
      'fields': fields.map((f) => f.toJson()).toList(),
      'states': states.map((s) => s.toJson()).toList(),
      'transitions': transitions.map((t) => t.toJson()).toList(),
      'forms': forms.map((f) => f.toJson()).toList(),
      'actions': actions.map((a) => a.toJson()).toList(),
      'relations': relations.map((r) => r.toJson()).toList(),
    };
  }

  /// Поля в порядке отображения (сервер хранит `order`).
  List<EntityField> get orderedFields {
    final sorted = [...fields]..sort((a, b) => a.order.compareTo(b.order));
    return sorted;
  }

  factory EntitySchema.fromJson(Map<String, dynamic> json) {
    return EntitySchema(
      entityType: EntityType.fromJson(
        _map(json['entity_type'], 'entity_schema.entity_type'),
      ),
      fields: _list(json['fields'])
          .map((f) => EntityField.fromJson(_map(f, 'entity_schema.fields[]')))
          .toList(),
      states: _list(json['states'])
          .map((s) => EntityState.fromJson(_map(s, 'entity_schema.states[]')))
          .toList(),
      transitions: _list(json['transitions'])
          .map((t) =>
              EntityTransition.fromJson(_map(t, 'entity_schema.transitions[]')))
          .toList(),
      forms: _list(json['forms'])
          .map((f) => EntityForm.fromJson(_map(f, 'entity_schema.forms[]')))
          .toList(),
      actions: _list(json['actions'])
          .map((a) => EntityAction.fromJson(_map(a, 'entity_schema.actions[]')))
          .toList(),
      relations: _list(json['relations'])
          .map(
            (r) => EntityRelation.fromJson(_map(r, 'entity_schema.relations[]')),
          )
          .toList(),
    );
  }
}

/// Декларация типа бизнес-сущности (`entity_types`).
class EntityType {
  const EntityType({
    required this.code,
    required this.name,
    required this.kind,
    this.companyId,
    this.isSystem = false,
    this.id,
    this.metadataVersion = 0,
  });

  final String? id;
  final String code;
  final String name;
  final String kind;
  final String? companyId;
  final bool isSystem;
  final int metadataVersion;

  EntityType copyWith({
    String? id,
    String? code,
    String? name,
    String? kind,
    String? companyId,
    bool? isSystem,
    int? metadataVersion,
  }) {
    return EntityType(
      id: id ?? this.id,
      code: code ?? this.code,
      name: name ?? this.name,
      kind: kind ?? this.kind,
      companyId: companyId ?? this.companyId,
      isSystem: isSystem ?? this.isSystem,
      metadataVersion: metadataVersion ?? this.metadataVersion,
    );
  }

  factory EntityType.fromJson(Map<String, dynamic> json) {
    return EntityType(
      id: json['id'] as String?,
      code: _str(json['code'], 'entity_type.code'),
      name: _str(json['name'], 'entity_type.name'),
      kind: _str(json['kind'], 'entity_type.kind'),
      companyId: json['company_id'] as String?,
      isSystem: json['is_system'] as bool? ?? false,
      metadataVersion: json['metadata_version'] as int? ?? 0,
    );
  }

  /// Wire-документ `metadata.import` (id опционален — сервер сгенерирует сам).
  Map<String, dynamic> toJson() {
    return {
      if (id != null) 'id': id,
      'code': code,
      'name': name,
      'kind': kind,
      if (companyId != null) 'company_id': companyId,
      if (isSystem) 'is_system': isSystem,
      'metadata_version': metadataVersion,
    };
  }
}

/// Декларация поля типа сущности (`entity_fields`).
class EntityField {
  const EntityField({
    required this.code,
    required this.label,
    required this.dataType,
    required this.required,
    required this.order,
    this.options,
  });

  final String code;
  final String label;

  /// Wire-значение `data_type` (snake_case, см. `FieldType` сервера).
  final String dataType;
  final bool required;
  final int order;

  /// Варианты enum / целевая ссылка (свободный JSON, `options` сервера).
  final Object? options;

  /// Варианты для `enum`-поля: строка или объект `{values: [...]}`.
  List<String> get enumValues {
    final o = options;
    if (o is List) {
      return o.whereType<String>().toList();
    }
    if (o is Map && o['values'] is List) {
      return (o['values'] as List).whereType<String>().toList();
    }
    return const [];
  }

  /// Целевой тип для `reference`-поля (из `options['entity_type']`).
  String? get referenceTarget {
    final o = options;
    if (o is Map && o['entity_type'] is String) {
      return o['entity_type'] as String;
    }
    return null;
  }

  EntityField copyWith({
    String? code,
    String? label,
    String? dataType,
    bool? required,
    int? order,
    Object? options,
  }) {
    return EntityField(
      code: code ?? this.code,
      label: label ?? this.label,
      dataType: dataType ?? this.dataType,
      required: required ?? this.required,
      order: order ?? this.order,
      options: options ?? this.options,
    );
  }

  factory EntityField.fromJson(Map<String, dynamic> json) {
    return EntityField(
      code: _str(json['code'], 'entity_field.code'),
      label: _str(json['label'], 'entity_field.label'),
      dataType: _str(json['data_type'], 'entity_field.data_type'),
      required: json['required'] as bool? ?? false,
      order: json['order'] as int? ?? 0,
      options: json['options'],
    );
  }

  /// Wire-документ `metadata.import`.
  Map<String, dynamic> toJson() {
    return {
      'code': code,
      'label': label,
      'data_type': dataType,
      'required': required,
      'order': order,
      if (options != null) 'options': options,
    };
  }
}

/// Состояние конечного автомата типа сущности (`entity_states`).
class EntityState {
  const EntityState({
    required this.code,
    required this.label,
    this.isInitial = false,
    this.isFinal = false,
  });

  final String code;
  final String label;
  final bool isInitial;
  final bool isFinal;

  EntityState copyWith({
    String? code,
    String? label,
    bool? isInitial,
    bool? isFinal,
  }) {
    return EntityState(
      code: code ?? this.code,
      label: label ?? this.label,
      isInitial: isInitial ?? this.isInitial,
      isFinal: isFinal ?? this.isFinal,
    );
  }

  factory EntityState.fromJson(Map<String, dynamic> json) {
    return EntityState(
      code: _str(json['code'], 'entity_state.code'),
      label: _str(json['label'], 'entity_state.label'),
      isInitial: json['is_initial'] as bool? ?? false,
      isFinal: json['is_final'] as bool? ?? false,
    );
  }

  /// Wire-документ `metadata.import`.
  Map<String, dynamic> toJson() {
    return {
      'code': code,
      'label': label,
      'is_initial': isInitial,
      'is_final': isFinal,
    };
  }
}

/// Разрешённый переход между состояниями (`entity_transitions`).
class EntityTransition {
  const EntityTransition({
    required this.code,
    required this.label,
    required this.fromState,
    required this.toState,
  });

  final String code;
  final String label;
  final String fromState;
  final String toState;

  EntityTransition copyWith({
    String? code,
    String? label,
    String? fromState,
    String? toState,
  }) {
    return EntityTransition(
      code: code ?? this.code,
      label: label ?? this.label,
      fromState: fromState ?? this.fromState,
      toState: toState ?? this.toState,
    );
  }

  factory EntityTransition.fromJson(Map<String, dynamic> json) {
    return EntityTransition(
      code: _str(json['code'], 'entity_transition.code'),
      label: _str(json['label'], 'entity_transition.label'),
      fromState: _str(json['from_state'], 'entity_transition.from_state'),
      toState: _str(json['to_state'], 'entity_transition.to_state'),
    );
  }

  /// Wire-документ `metadata.import`.
  Map<String, dynamic> toJson() {
    return {
      'code': code,
      'label': label,
      'from_state': fromState,
      'to_state': toState,
    };
  }
}

/// Декларативная UI-форма (`entity_forms`). Поля формы на сервере явным
/// списком не хранятся — порядок берётся из `EntitySchema.orderedFields`;
/// `layout` — свободная JSON-разметка для SDUI-рендерера.
class EntityForm {
  const EntityForm({
    required this.code,
    required this.label,
    this.layout,
  });

  final String code;
  final String label;
  final Object? layout;

  factory EntityForm.fromJson(Map<String, dynamic> json) {
    return EntityForm(
      code: _str(json['code'], 'entity_form.code'),
      label: _str(json['label'], 'entity_form.label'),
      layout: json['layout'],
    );
  }

  /// Wire-документ `metadata.import`.
  Map<String, dynamic> toJson() {
    return {'code': code, 'label': label, if (layout != null) 'layout': layout};
  }
}

/// Действие типа сущности (`entity_actions`).
class EntityAction {
  const EntityAction({required this.code, required this.label});

  final String code;
  final String label;

  factory EntityAction.fromJson(Map<String, dynamic> json) {
    return EntityAction(
      code: _str(json['code'], 'entity_action.code'),
      label: _str(json['label'], 'entity_action.label'),
    );
  }

  /// Wire-документ `metadata.import`.
  Map<String, dynamic> toJson() {
    return {'code': code, 'label': label};
  }
}

/// Связь типа сущности с другим типом (`entity_relations`).
class EntityRelation {
  const EntityRelation({required this.code, required this.targetType});

  final String code;
  final String targetType;

  factory EntityRelation.fromJson(Map<String, dynamic> json) {
    return EntityRelation(
      code: _str(json['code'], 'entity_relation.code'),
      targetType: _str(json['target_type'], 'entity_relation.target_type'),
    );
  }

  /// Wire-документ `metadata.import`.
  Map<String, dynamic> toJson() {
    return {'code': code, 'target_type': targetType};
  }
}

Map<String, dynamic> _map(Object? value, String path) {
  final v = value;
  if (v is Map) {
    return Map<String, dynamic>.from(v);
  }
  throw FormatException('$path: ожидался объект, получено: $v');
}

List<Object?> _list(Object? value) {
  final v = value;
  if (v is List) {
    return v;
  }
  if (v == null) {
    return const [];
  }
  throw FormatException('ожидался список, получено: $v');
}

String _str(Object? value, String path) {
  final v = value;
  if (v is String) {
    return v;
  }
  throw FormatException('$path: ожидалась строка, получено: $v');
}