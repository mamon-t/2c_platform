import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:twoc_client/screens/catalog_screen.dart';
import 'package:twoc_client/screens/home_screen.dart';
import 'package:twoc_client/screens/object_form_screen.dart';
import 'package:twoc_client/screens/script_editor_screen.dart';
import 'package:twoc_client/screens/script_list_screen.dart';

/// Минимальный роутер для экранных тестов (без auth-redirect).
GoRouter testRouter({String initialLocation = '/catalog/account'}) {
  return GoRouter(
    initialLocation: initialLocation,
    routes: [
      GoRoute(
        path: '/home',
        builder: (context, state) => const HomeScreen(),
      ),
      GoRoute(
        path: '/settings',
        builder: (context, state) =>
            const Scaffold(body: Center(child: Text('Настройки'))),
      ),
      GoRoute(
        path: '/catalog/:entityType',
        builder: (context, state) => CatalogScreen(
          entityType: state.pathParameters['entityType']!,
        ),
      ),
      GoRoute(
        path: '/scripts',
        builder: (context, state) => const ScriptListScreen(),
      ),
      GoRoute(
        path: '/scripts/new',
        builder: (context, state) => const ScriptEditorScreen(code: 'new'),
      ),
      GoRoute(
        path: '/scripts/:code/edit',
        builder: (context, state) => ScriptEditorScreen(
          code: state.pathParameters['code']!,
        ),
      ),
      GoRoute(
        path: '/object/:entityType/new',
        builder: (context, state) => ObjectFormScreen(
          entityType: state.pathParameters['entityType']!,
          id: 'new',
          mode: ObjectFormMode.create,
        ),
      ),
      GoRoute(
        path: '/object/:entityType/:id/edit',
        builder: (context, state) => ObjectFormScreen(
          entityType: state.pathParameters['entityType']!,
          id: state.pathParameters['id']!,
          mode: ObjectFormMode.edit,
        ),
      ),
      GoRoute(
        path: '/object/:entityType/:id',
        builder: (context, state) => ObjectFormScreen(
          entityType: state.pathParameters['entityType']!,
          id: state.pathParameters['id']!,
          mode: ObjectFormMode.view,
        ),
      ),
    ],
  );
}