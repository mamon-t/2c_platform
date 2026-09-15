/// Навигация из манифестов модулей: ответ `module.navigation` (доработка
/// перед 11б).
///
/// Wire-контракт: `{modules: [{code, display_name, version, navigation:
/// [{code, label, entity_type}]}]}`. `entity_type` — целевой тип для каталога.
class ModuleNavigation {
  const ModuleNavigation({
    required this.code,
    required this.displayName,
    required this.version,
    required this.navigation,
  });

  final String code;
  final String displayName;
  final String version;
  final List<NavigationItem> navigation;

  factory ModuleNavigation.fromJson(Map<String, dynamic> json) {
    final nav = json['navigation'];
    return ModuleNavigation(
      code: _str(json['code'], 'module_navigation.code'),
      displayName: _str(json['display_name'], 'module_navigation.display_name'),
      version: _str(json['version'], 'module_navigation.version'),
      navigation: (nav is List)
          ? nav
              .map(
                (n) => NavigationItem.fromJson(
                  (n as Map).cast<String, dynamic>(),
                ),
              )
              .toList()
          : const [],
    );
  }
}

/// Пункт навигации (аналог `ManifestNavItem` + расширение `entity_type`).
class NavigationItem {
  const NavigationItem({
    required this.code,
    required this.label,
    required this.entityType,
  });

  final String code;
  final String label;
  final String? entityType;

  factory NavigationItem.fromJson(Map<String, dynamic> json) {
    return NavigationItem(
      code: _str(json['code'], 'navigation_item.code'),
      label: _str(json['label'], 'navigation_item.label'),
      entityType: json['entity_type'] as String?,
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