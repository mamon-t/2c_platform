import 'package:go_router/go_router.dart';
import 'package:twoc_client/screens/catalog_screen.dart';
import 'package:twoc_client/screens/object_form_screen.dart';

/// Минимальный роутер для экранных тестов (без auth-redirect).
GoRouter testRouter({String initialLocation = '/catalog/account'}) {
  return GoRouter(
    initialLocation: initialLocation,
    routes: [
      GoRoute(
        path: '/catalog/:entityType',
        builder: (context, state) => CatalogScreen(
          entityType: state.pathParameters['entityType']!,
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