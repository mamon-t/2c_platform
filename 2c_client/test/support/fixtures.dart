/// Общие фикстуры SDUI-тестов: wire-карты сервера в тестовых диапазонах.
library;

Map<String, dynamic> field(
  String code,
  String label,
  String type, {
  bool required = false,
  int order = 0,
  Object? options,
}) =>
    {
      'code': code,
      'label': label,
      'data_type': type,
      'required': required,
      'order': order,
      'options': options,
    };

Map<String, dynamic> schemaWire({
  String code = 'account',
  String name = 'Счет',
  String kind = 'catalog',
  bool requiredName = true,
}) =>
    {
      'entity_type': {'code': code, 'name': name, 'kind': kind},
      'fields': [
        field('name', 'Наименование', 'string', required: requiredName, order: 1),
        field('code', 'Код', 'string', order: 0),
        field('balance', 'Баланс', 'money', order: 2),
        if (code == 'account')
          field(
            'kind',
            'Тип',
            'enum',
            order: 3,
            options: {
              'values': ['asset', 'liability'],
            },
          ),
        field('is_closed', 'Закрыт', 'boolean', order: 4),
        field('tags', 'Теги', 'array', order: 5),
      ],
      'states': [
        {'code': 'draft', 'label': 'Черновик', 'is_initial': true},
        {'code': 'active', 'label': 'Активен', 'is_final': true},
      ],
      'transitions': [
        {'code': 'activate', 'label': 'Активировать', 'from_state': 'draft', 'to_state': 'active'},
      ],
      'forms': [
        {'code': 'main', 'label': 'Основная'},
      ],
      'actions': [
        {'code': 'print', 'label': 'Печать'},
      ],
      'relations': [
        {'code': 'parent', 'target_type': 'account'},
      ],
    };

Map<String, dynamic> objectWire({
  String id = 'acc-1',
  String entityType = 'account',
  int version = 1,
  Map<String, dynamic>? data,
  Map<String, dynamic>? computed,
  String state = 'draft',
  String companyId = 'c1',
  String? number = '00001',
}) =>
    {
      'id': id,
      'entity_type': entityType,
      'kind': 'catalog',
      'company_id': companyId,
      'state': state,
      'data': data,
      'computed': computed,
      'version': version,
      'number': number,
    };

Map<String, dynamic> navigationWire() => {
      'modules': [
        {
          'code': 'accounting',
          'display_name': 'Учёт',
          'version': '1.0.0',
          'navigation': [
            {'code': 'accounts', 'label': 'Счета', 'entity_type': 'account'},
            {'code': 'periods', 'label': 'Периоды', 'entity_type': 'period'},
          ],
        },
      ],
    };

List<Map<String, dynamic>> entityTypesWire() => [
      {'code': 'account', 'name': 'Счет', 'kind': 'catalog'},
      {'code': 'period', 'name': 'Период', 'kind': 'register'},
    ];