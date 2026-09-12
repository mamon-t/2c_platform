import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:twoc_client/app.dart';
import 'package:twoc_client/models/server_error.dart';
import 'package:twoc_client/providers/app_providers.dart';
import 'package:twoc_client/services/auth_service.dart';

class _MockAuthService extends Mock implements AuthService {}

void main() {
  testWidgets('login screen shows server error on wrong credentials',
      (WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();

    final auth = _MockAuthService();
    when(() => auth.restoreSession()).thenAnswer((_) async => null);
    when(
      () => auth.login(any(), any()),
    ).thenThrow(const ServerError(ErrorCode.validation, 'неверный логин или пароль'));

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          sharedPreferencesProvider.overrideWithValue(prefs),
          authServiceProvider.overrideWithValue(auth),
        ],
        child: const TwocApp(),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Войти'), findsOneWidget);

    await tester.enterText(find.byType(TextFormField).at(0), 'admin');
    await tester.enterText(find.byType(TextFormField).at(1), 'bad');
    await tester.tap(find.text('Войти'));
    await tester.pumpAndSettle();

    expect(find.text('неверный логин или пароль'), findsOneWidget);
    expect(find.text('Войти'), findsOneWidget);
  });
}