import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:twoc_client/core/app_theme.dart';
import 'package:twoc_client/providers/app_providers.dart';
import 'package:twoc_client/providers/theme_providers.dart';
import 'package:twoc_client/screens/settings_screen.dart';

void main() {
  late Directory base;

  setUp(() {
    base = Directory.systemTemp.createTempSync('settings_theme_test');
  });

  tearDown(() {
    if (base.existsSync()) {
      base.deleteSync(recursive: true);
    }
  });

  Future<void> pumpSettings(WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          sharedPreferencesProvider.overrideWithValue(prefs),
          themeStoreProvider.overrideWithValue(ThemeStore(base)),
        ],
        child: const MaterialApp(home: SettingsScreen()),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets('показывает текущую тему и список из провайдера', (tester) async {
    await pumpSettings(tester);
    // По умолчанию выбрана «Тёмная» (встроенная).
    expect(find.text('Тёмная'), findsWidgets);
    expect(find.text('Светлая'), findsNothing);
  });

  testWidgets('переключатель тем персистит выбор в config.toml', (tester) async {
    await pumpSettings(tester);
    await tester.tap(find.byType(DropdownButtonFormField<String>));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Светлая').last);
    await tester.pumpAndSettle();

    final content = File('${base.path}/config.toml').readAsStringSync();
    expect(content, '[app]\ntheme = "light"\n');
  });

  testWidgets('файловая тема из каталога доступна в списке', (tester) async {
    Directory('${base.path}/themes').createSync(recursive: true);
    File('${base.path}/themes/paper.toml')
        .writeAsStringSync('[theme]\nname = "Бумажная"\nkind = "light"\n');
    await pumpSettings(tester);

    await tester.tap(find.byType(DropdownButtonFormField<String>));
    await tester.pumpAndSettle();
    expect(find.text('Бумажная'), findsOneWidget);

    await tester.tap(find.text('Бумажная').last);
    await tester.pumpAndSettle();
    expect(
      File('${base.path}/config.toml').readAsStringSync(),
      '[app]\ntheme = "paper"\n',
    );
  });
}