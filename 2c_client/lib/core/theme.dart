import 'package:flutter/material.dart';

import 'app_theme.dart';

/// Строит тему Material 3 из [AppTheme].
///
/// Базовый `InputDecorationTheme` даёт единый стиль полей во всех виджетах
/// SDUI-реестра; отдельные поля задают только `labelText`.
ThemeData buildAppTheme(AppTheme theme) {
  var scheme = ColorScheme.fromSeed(
    seedColor: Color(theme.seed),
    brightness: theme.isDark ? Brightness.dark : Brightness.light,
  );
  final primary = _color(theme.overrides, 'primary');
  final secondary = _color(theme.overrides, 'secondary');
  final surface = _color(theme.overrides, 'surface');
  if (primary != null || secondary != null || surface != null) {
    scheme = scheme.copyWith(
      primary: primary ?? scheme.primary,
      secondary: secondary ?? scheme.secondary,
      surface: surface ?? scheme.surface,
    );
  }
  final base = ThemeData(useMaterial3: true, colorScheme: scheme);
  return base.copyWith(
    scaffoldBackgroundColor: scheme.surface,
    inputDecorationTheme: InputDecorationTheme(
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
        borderSide: BorderSide(color: scheme.primary, width: 2),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
        borderSide: BorderSide(color: scheme.outlineVariant),
      ),
      errorBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
        borderSide: BorderSide(color: scheme.error),
      ),
    ),
  );
}

Color? _color(Map<String, Object?> overrides, String key) {
  final v = overrides[key];
  if (v is int) {
    return Color(v & 0xFFFFFFFF);
  }
  return null;
}